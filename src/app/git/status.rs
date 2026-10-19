use super::args::Args;
use super::client::GitClient;
use super::error::GitResult;
use super::types::{ChangeKind, EntryKind, RepoState, RepoStatus, StatusEntry};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UntrackedMode {
    No,
    /// Untracked directories are reported as a single entry.
    Normal,
    /// Every untracked file is listed.
    #[default]
    All,
}

#[derive(Clone, Debug, Default)]
pub struct StatusOptions {
    pub untracked: UntrackedMode,
    pub include_ignored: bool,
}

impl GitClient {
    pub fn status(&self, o: &StatusOptions) -> GitResult<RepoStatus> {
        let args = Args::new("status")
            .args(["--porcelain=v2", "-z", "--branch"])
            .arg(match o.untracked {
                UntrackedMode::No => "--untracked-files=no",
                UntrackedMode::Normal => "--untracked-files=normal",
                UntrackedMode::All => "--untracked-files=all",
            })
            .flag(o.include_ignored, "--ignored");
        Ok(parse_status(&self.run(args)?.stdout_str()))
    }

    /// Paths with unresolved merge conflicts.
    pub fn conflicted_files(&self) -> GitResult<Vec<String>> {
        let args = Args::new("diff").args(["--name-only", "--diff-filter=U", "-z"]);
        Ok(split_nul(&self.run(args)?.stdout_str()))
    }

    /// Detects an unfinished merge/rebase/cherry-pick/revert/bisect.
    pub fn repo_state(&self) -> RepoState {
        let d = &self.git_dir;
        if d.join("rebase-merge").exists() || d.join("rebase-apply").exists() {
            RepoState::Rebasing
        } else if d.join("MERGE_HEAD").exists() {
            RepoState::Merging
        } else if d.join("CHERRY_PICK_HEAD").exists() {
            RepoState::CherryPicking
        } else if d.join("REVERT_HEAD").exists() {
            RepoState::Reverting
        } else if d.join("BISECT_LOG").exists() {
            RepoState::Bisecting
        } else {
            RepoState::Clean
        }
    }

    /// Checked-out branch name, `None` when HEAD is detached.
    pub fn current_branch(&self) -> GitResult<Option<String>> {
        let name = self.run_str(Args::new("branch").arg("--show-current"))?;
        Ok((!name.is_empty()).then_some(name))
    }

    /// Full id of HEAD, `None` on a branch without commits.
    pub fn head(&self) -> GitResult<Option<String>> {
        if !self.has_commits()? {
            return Ok(None);
        }
        Ok(Some(self.run_str(Args::new("rev-parse").arg("HEAD"))?))
    }

    /// Whether `path` is excluded by `.gitignore` rules.
    pub fn is_ignored(&self, path: &str) -> GitResult<bool> {
        let args = Args::new("check-ignore").arg("-q").paths([path]);
        Ok(self.run_raw(&args)?.success())
    }

    /// All tracked files, relative to the repository root.
    pub fn tracked_files(&self) -> GitResult<Vec<String>> {
        Ok(split_nul(
            &self.run(Args::new("ls-files").arg("-z"))?.stdout_str(),
        ))
    }
}

pub(super) fn split_nul(s: &str) -> Vec<String> {
    s.split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

fn change(c: Option<char>) -> ChangeKind {
    c.map_or(ChangeKind::Unmodified, ChangeKind::from_code)
}

/// Parses `git status --porcelain=v2 -z --branch`.
pub(super) fn parse_status(raw: &str) -> RepoStatus {
    let mut st = RepoStatus::default();
    let mut it = raw.split('\0');
    while let Some(tok) = it.next() {
        let Some(kind) = tok.bytes().next() else {
            continue;
        };
        if let Some(header) = tok.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.oid" if value != "(initial)" => st.head = Some(value.to_string()),
                "branch.head" if value != "(detached)" => st.branch = Some(value.to_string()),
                "branch.upstream" => st.upstream = Some(value.to_string()),
                "branch.ab" => {
                    for part in value.split(' ') {
                        if let Some(n) = part.strip_prefix('+') {
                            st.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = part.strip_prefix('-') {
                            st.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        match kind {
            b'1' | b'2' | b'u' => {
                let (fields, entry_kind) = match kind {
                    b'1' => (9, EntryKind::Ordinary),
                    b'2' => (10, EntryKind::Renamed),
                    _ => (11, EntryKind::Unmerged),
                };
                let f: Vec<&str> = tok.splitn(fields, ' ').collect();
                if f.len() != fields {
                    continue;
                }
                let code = f[1];
                let mut chars = code.chars();
                let (x, y) = (chars.next(), chars.next());
                let orig_path = (kind == b'2').then(|| it.next().unwrap_or_default().to_string());
                let unmerged = entry_kind == EntryKind::Unmerged;
                st.entries.push(StatusEntry {
                    path: f[fields - 1].to_string(),
                    orig_path,
                    kind: entry_kind,
                    index: if unmerged {
                        ChangeKind::Unmerged
                    } else {
                        change(x)
                    },
                    worktree: if unmerged {
                        ChangeKind::Unmerged
                    } else {
                        change(y)
                    },
                    code: code.to_string(),
                });
            }
            b'?' | b'!' => {
                let (entry_kind, change_kind, code) = if kind == b'?' {
                    (EntryKind::Untracked, ChangeKind::Untracked, "??")
                } else {
                    (EntryKind::Ignored, ChangeKind::Ignored, "!!")
                };
                st.entries.push(StatusEntry {
                    path: tok[2..].to_string(),
                    orig_path: None,
                    kind: entry_kind,
                    index: ChangeKind::Unmodified,
                    worktree: change_kind,
                    code: code.to_string(),
                });
            }
            _ => {}
        }
    }
    st
}
