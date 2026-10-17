//! Git service: listens on the event bus and uses `GitClient` to perform git work on a
//! background thread, reporting results back to the UI as actions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::{
    actions::{AppAction, GitOperation},
    bus,
    git::{self, ChangeKind, DecorationKind, GitClient, LogOptions},
    model::{Branch, CommitNode, FileChange, LoadedRepo, RefBadge, RefKind},
    send_ui,
};

const MAX_COMMITS: usize = 1000;

/// Starts the service on its own thread.
pub fn start() {
    let rx = bus::subscribe();
    thread::spawn(move || GitService::default().run(rx));
}

#[derive(Default)]
struct GitService {
    /// Root of the repository currently shown, if it was loaded by this service.
    path: Option<PathBuf>,
    /// Commit ids in the same order as the model's commits.
    ids: Vec<String>,
}

impl GitService {
    fn run(mut self, rx: Receiver<AppAction>) {
        while let Ok(action) = rx.recv() {
            match action {
                AppAction::OpenRepository(path) => self.open(path),
                AppAction::RunGitOperation(GitOperation::Refresh) => {
                    if let Some(path) = self.path.clone() {
                        self.open(path);
                    }
                }
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
}

fn load_repo(path: &Path) -> Result<(LoadedRepo, Vec<String>), String> {
    let client = GitClient::open(path).map_err(|e| format!("Not a git repository: {e}"))?;

    let git_branches = client.branches().map_err(|e| e.to_string())?;
    let rows = client
        .graph(&LogOptions {
            all: true,
            max_count: Some(MAX_COMMITS),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;
    let head = client.head().map_err(|e| e.to_string())?;

    let ids: Vec<String> = rows.iter().map(|row| row.commit.id.clone()).collect();
    let index_of: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();

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
            }
        })
        .collect();

    let current = git_branches.iter().position(|b| b.is_current && !b.is_remote);
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
            commits,
            selected_commit: tip,
        },
        ids,
    ))
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

