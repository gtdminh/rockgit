//! Git service: listens on the event bus and uses `GitClient` to perform git work on a
//! background thread, reporting results back to the UI as actions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::app::{
    actions::{AppAction, GitOperation},
    bus,
    git::{
        self, ChangeKind, CleanOptions, CommitOptions, DecorationKind, GitClient, LogOptions,
        PushOptions, StatusOptions,
    },
    model::{
        AppConfig, Branch, CommitNode, FileChange, LoadedRepo, RefBadge, RefKind,
        WorkingTreeChanges, WorkingTreeFile,
    },
    send_ui,
};

const MAX_COMMITS: usize = 1000;

/// Starts the service on its own thread.
pub fn start(config: AppConfig) {
    let rx = bus::subscribe();
    thread::spawn(move || GitService::new(config).run(rx));
}

struct GitService {
    autorefresh_time: u64,
    /// Root of the repository currently shown, if it was loaded by this service.
    path: Option<PathBuf>,
    /// Commit ids in the same order as the model's commits.
    ids: Vec<String>,
}

impl GitService {
    fn new(config: AppConfig) -> Self {
        Self {
            autorefresh_time: config.autorefresh_time,
            path: None,
            ids: Vec::new(),
        }
    }

    fn run(mut self, rx: Receiver<AppAction>) {
        loop {
            let action = match self.autorefresh_interval() {
                Some(interval) => match rx.recv_timeout(interval) {
                    Ok(action) => action,
                    Err(RecvTimeoutError::Timeout) => {
                        self.refresh_current_repo();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match rx.recv() {
                    Ok(action) => action,
                    Err(_) => break,
                },
            };

            match action {
                AppAction::UpdateConfig(config) => self.update_config(config),
                AppAction::OpenRepository(path) => self.open(path),
                AppAction::RunGitOperation(GitOperation::Refresh) => self.refresh_current_repo(),
                AppAction::RevertWorkingTreeFile(path) => self.revert_working_tree_file(&path),
                AppAction::RevertAllWorkingTreeChanges => self.revert_all_working_tree_changes(),
                AppAction::CommitWorkingTree {
                    message,
                    push_after,
                } => self.commit_working_tree(&message, push_after),
                // A sample (non-loaded) repository was selected.
                AppAction::SelectRepository(_) => {
                    self.path = None;
                    self.ids.clear();
                }
                AppAction::SelectCommit(index) => self.load_commit_files(index),
                _ => {}
            }
        }
    }

    fn autorefresh_interval(&self) -> Option<Duration> {
        (self.autorefresh_time > 0).then(|| Duration::from_secs(self.autorefresh_time))
    }

    fn refresh_current_repo(&mut self) {
        if let Some(path) = self.path.clone() {
            self.open(path);
        }
    }

    fn update_config(&mut self, config: AppConfig) {
        self.autorefresh_time = config.autorefresh_time;
        self.refresh_current_repo();
    }

    /// Loads branches and the graph, then selects the latest commit of the current branch.
    fn open(&mut self, path: PathBuf) {
        match load_repo(&path) {
            Ok((loaded, ids)) => {
                self.path = Some(PathBuf::from(&loaded.path));
                self.ids = ids;
                send_ui(AppAction::RepositoryLoaded(Box::new(loaded)));
            }
            Err(message) => send_ui(AppAction::RepositoryLoadFailed(message)),
        }
    }

    fn load_commit_files(&self, index: usize) {
        let (Some(path), Some(id)) = (&self.path, self.ids.get(index)) else {
            return;
        };
        if let Ok(files) = commit_files(path, id) {
            send_ui(AppAction::CommitFilesLoaded {
                index,
                id: id.clone(),
                files,
            });
        }
    }

    fn revert_working_tree_file(&mut self, path: &str) {
        let Some(repo_path) = self.path.clone() else {
            return;
        };

        let result = (|| -> Result<String, String> {
            let client = GitClient::open(&repo_path).map_err(|e| e.to_string())?;
            let status = client
                .status(&StatusOptions::default())
                .map_err(|e| e.to_string())?;
            let entry = status
                .entries
                .iter()
                .find(|entry| entry.path == path)
                .cloned()
                .ok_or_else(|| format!("No working tree change found for {path}"))?;

            revert_entry(&client, &entry)?;
            Ok(format!("Reverted {path}"))
        })();

        self.finish_working_tree_operation(repo_path, result);
    }

    fn revert_all_working_tree_changes(&mut self) {
        let Some(repo_path) = self.path.clone() else {
            return;
        };

        let result = (|| -> Result<String, String> {
            let client = GitClient::open(&repo_path).map_err(|e| e.to_string())?;
            let status = client
                .status(&StatusOptions::default())
                .map_err(|e| e.to_string())?;

            revert_all_entries(&client, &status)?;
            Ok("Reverted all working tree changes".to_owned())
        })();

        self.finish_working_tree_operation(repo_path, result);
    }

    fn commit_working_tree(&mut self, message: &str, push_after: bool) {
        let Some(repo_path) = self.path.clone() else {
            return;
        };

        let result = (|| -> Result<String, String> {
            let trimmed = message.trim();
            if trimmed.is_empty() {
                return Err("Commit message is required".to_owned());
            }

            let client = GitClient::open(&repo_path).map_err(|e| e.to_string())?;
            client.stage_all().map_err(|e| e.to_string())?;
            let commit_id = client
                .commit(&CommitOptions {
                    message: trimmed.to_owned(),
                    ..Default::default()
                })
                .map_err(|e| e.to_string())?;

            if push_after {
                client.push(&PushOptions::default()).map_err(|e| {
                    format!(
                        "Committed {} but push failed: {e}",
                        &commit_id[..commit_id.len().min(7)]
                    )
                })?;
            }

            Ok(if push_after {
                format!(
                    "Committed {} and pushed changes",
                    &commit_id[..commit_id.len().min(7)]
                )
            } else {
                format!("Committed {}", &commit_id[..commit_id.len().min(7)])
            })
        })();

        self.finish_working_tree_operation(repo_path, result);
    }

    fn finish_working_tree_operation(
        &mut self,
        repo_path: PathBuf,
        result: Result<String, String>,
    ) {
        self.open(repo_path);
        match result {
            Ok(message) | Err(message) => send_ui(AppAction::SetLastAction(message)),
        }
    }
}

fn load_repo(path: &Path) -> Result<(LoadedRepo, Vec<String>), String> {
    let client = GitClient::open(path).map_err(|e| format!("Not a git repository: {e}"))?;

    let git_branches = client.branches().map_err(|e| e.to_string())?;
    let status = client
        .status(&StatusOptions::default())
        .map_err(|e| e.to_string())?;
    let rows = client
        .graph(&LogOptions {
            all: true,
            max_count: Some(MAX_COMMITS),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;
    let head = client.head().map_err(|e| e.to_string())?;

    let ids: Vec<String> = rows.iter().map(|row| row.commit.id.clone()).collect();
    let index_of: HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    let mut commits: Vec<CommitNode> = rows
        .iter()
        .map(|row| {
            let commit = &row.commit;
            CommitNode {
                id: commit.id.clone(),
                short_id: commit.short_id.clone(),
                summary: commit.subject.clone(),
                author: commit.author_name.clone(),
                relative_time: relative_time(commit.author_time),
                lane: row.lane,
                parents: commit
                    .parents
                    .iter()
                    .filter_map(|p| index_of.get(p.as_str()).copied())
                    .collect(),
                refs: ref_badges(&commit.refs),
                files: Vec::new(),
                is_head: commit.refs.iter().any(|r| r.is_head),
                is_mainline: row.is_mainline,
            }
        })
        .collect();

    let current = git_branches
        .iter()
        .position(|b| b.is_current && !b.is_remote);
    let active_branch = current.unwrap_or(0);

    // Latest commit of the current branch (falls back to HEAD, then the newest commit).
    let tip = current
        .map(|i| git_branches[i].id.clone())
        .or(head)
        .and_then(|id| index_of.get(id.as_str()).copied())
        .unwrap_or(0);

    if let (Some(node), Some(id)) = (commits.get_mut(tip), ids.get(tip)) {
        node.files = commit_files(client.root(), id).unwrap_or_default();
    }

    let root = client.root().to_path_buf();
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());

    let branches: Vec<Branch> = git_branches
        .iter()
        .map(|b| Branch {
            name: b.name.clone(),
            ahead: b.ahead.min(u8::MAX as u32) as u8,
            behind: b.behind.min(u8::MAX as u32) as u8,
            is_remote: b.is_remote,
            is_current: b.is_current,
        })
        .collect();

    Ok((
        LoadedRepo {
            name,
            path: root.display().to_string(),
            branches,
            tags: client
                .tags()
                .map(|tags| tags.into_iter().map(|t| t.name).collect())
                .unwrap_or_default(),
            active_branch,
            working_tree: working_tree_changes(&status),
            commits,
            selected_commit: tip,
        },
        ids,
    ))
}

fn working_tree_changes(status: &git::RepoStatus) -> WorkingTreeChanges {
    WorkingTreeChanges {
        files: status
            .entries
            .iter()
            .filter(|entry| entry.kind != git::EntryKind::Ignored)
            .map(|entry| WorkingTreeFile {
                repo_path: entry.path.clone(),
                display_path: entry
                    .orig_path
                    .as_ref()
                    .map(|original| format!("{original} → {}", entry.path))
                    .unwrap_or_else(|| entry.path.clone()),
                status: entry.code.clone(),
                staged: entry_staged(entry),
                unstaged: entry_unstaged(entry),
                untracked: entry.kind == git::EntryKind::Untracked,
            })
            .collect(),
        staged_count: status.staged().count(),
        unstaged_count: status.unstaged().count(),
        untracked_count: status.untracked().count(),
    }
}

fn revert_entry(client: &GitClient, entry: &git::StatusEntry) -> Result<(), String> {
    let path = vec![entry.path.clone()];
    let has_commits = client.has_commits().map_err(|e| e.to_string())?;

    if entry_staged(entry) {
        client.unstage(&path).map_err(|e| e.to_string())?;
    }

    if has_commits && entry.kind != git::EntryKind::Untracked {
        client.discard_changes(&path).map_err(|e| e.to_string())?;
    }

    if entry.kind == git::EntryKind::Untracked || entry.index == git::ChangeKind::Added {
        client
            .clean(&CleanOptions {
                directories: true,
                paths: path,
                ..Default::default()
            })
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn revert_all_entries(client: &GitClient, status: &git::RepoStatus) -> Result<(), String> {
    let staged_paths: Vec<String> = status
        .entries
        .iter()
        .filter(|entry| entry.kind != git::EntryKind::Ignored && entry_staged(entry))
        .map(|entry| entry.path.clone())
        .collect();

    if !staged_paths.is_empty() {
        client.unstage(&staged_paths).map_err(|e| e.to_string())?;
    }

    if client.has_commits().map_err(|e| e.to_string())?
        && status.entries.iter().any(|entry| {
            entry.kind != git::EntryKind::Ignored && entry.kind != git::EntryKind::Untracked
        })
    {
        client.discard_all_changes().map_err(|e| e.to_string())?;
    }

    if status.entries.iter().any(|entry| {
        entry.kind == git::EntryKind::Untracked || entry.index == git::ChangeKind::Added
    }) {
        client
            .clean(&CleanOptions {
                directories: true,
                ..Default::default()
            })
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn entry_staged(entry: &git::StatusEntry) -> bool {
    matches!(
        entry.kind,
        git::EntryKind::Ordinary | git::EntryKind::Renamed | git::EntryKind::Unmerged
    ) && entry.index != git::ChangeKind::Unmodified
}

fn entry_unstaged(entry: &git::StatusEntry) -> bool {
    matches!(
        entry.kind,
        git::EntryKind::Ordinary | git::EntryKind::Renamed | git::EntryKind::Unmerged
    ) && entry.worktree != git::ChangeKind::Unmodified
}

fn commit_files(path: &Path, id: &str) -> Result<Vec<FileChange>, String> {
    let client = GitClient::open(path).map_err(|e| e.to_string())?;
    let detail = client.commit_detail(id).map_err(|e| e.to_string())?;
    Ok(detail
        .files
        .into_iter()
        .map(|f| FileChange {
            path: f.path,
            status: status_letter(f.status).to_owned(),
        })
        .collect())
}

fn status_letter(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added | ChangeKind::Untracked => "A",
        ChangeKind::Deleted => "D",
        ChangeKind::Renamed => "R",
        ChangeKind::Copied => "C",
        ChangeKind::Unmerged => "U",
        _ => "M",
    }
}

fn ref_badges(refs: &[git::RefDecoration]) -> Vec<RefBadge> {
    let mut badges = Vec::new();
    for r in refs {
        let kind = match r.kind {
            DecorationKind::Head => RefKind::Head,
            DecorationKind::LocalBranch => RefKind::LocalBranch,
            DecorationKind::RemoteBranch => RefKind::RemoteBranch,
            DecorationKind::Tag => RefKind::Tag,
            DecorationKind::Other => continue,
        };
        if r.is_head && kind == RefKind::LocalBranch {
            badges.push(RefBadge {
                label: "HEAD".to_owned(),
                kind: RefKind::Head,
            });
        }
        badges.push(RefBadge {
            label: r.name.clone(),
            kind,
        });
    }
    badges
}

fn relative_time(unix_seconds: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let delta = (now - unix_seconds).max(0);
    match delta {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{}m ago", delta / 60),
        3600..=86399 => format!("{}h ago", delta / 3600),
        86400..=2_591_999 => format!("{}d ago", delta / 86400),
        2_592_000..=31_535_999 => format!("{}mo ago", delta / 2_592_000),
        _ => format!("{}y ago", delta / 31_536_000),
    }
}

#[cfg(test)]
mod tests;
