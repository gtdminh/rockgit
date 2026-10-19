use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Unmodified,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Unmerged,
    Untracked,
    Ignored,
}

impl ChangeKind {
    pub fn from_code(c: char) -> Self {
        match c {
            'M' => Self::Modified,
            'A' => Self::Added,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            'T' => Self::TypeChanged,
            'U' => Self::Unmerged,
            '?' => Self::Untracked,
            '!' => Self::Ignored,
            _ => Self::Unmodified,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Ordinary,
    Renamed,
    Unmerged,
    Untracked,
    Ignored,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: String,
    /// Source path for renames/copies.
    pub orig_path: Option<String>,
    pub kind: EntryKind,
    /// Change between HEAD and the index (staged).
    pub index: ChangeKind,
    /// Change between the index and the work tree (unstaged).
    pub worktree: ChangeKind,
    /// Raw two-letter `XY` code, e.g. `UU` for a both-modified conflict.
    pub code: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RepoStatus {
    /// `None` when HEAD is detached.
    pub branch: Option<String>,
    /// `None` on a branch without commits.
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<StatusEntry>,
}

impl RepoStatus {
    pub fn staged(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| {
            matches!(e.kind, EntryKind::Ordinary | EntryKind::Renamed)
                && e.index != ChangeKind::Unmodified
        })
    }

    pub fn unstaged(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| {
            matches!(e.kind, EntryKind::Ordinary | EntryKind::Renamed)
                && e.worktree != ChangeKind::Unmodified
        })
    }

    pub fn untracked(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Untracked)
    }

    pub fn conflicted(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Unmerged)
    }

    pub fn is_clean(&self) -> bool {
        !self
            .entries
            .iter()
            .any(|e| !matches!(e.kind, EntryKind::Ignored))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorationKind {
    /// A detached `HEAD`.
    Head,
    LocalBranch,
    RemoteBranch,
    Tag,
    Other,
}

/// A ref pointing at a commit, as shown next to it in `git log --decorate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefDecoration {
    /// Short display name, e.g. `main`, `origin/main`, `v1.0`.
    pub name: String,
    pub full_name: String,
    pub kind: DecorationKind,
    /// True for the checked-out branch (or detached HEAD).
    pub is_head: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    /// Unix seconds.
    pub author_time: i64,
    pub committer_name: String,
    pub committer_email: String,
    pub commit_time: i64,
    pub subject: String,
    pub body: String,
    pub refs: Vec<RefDecoration>,
}

impl Commit {
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: ChangeKind,
    /// `None` for binary files.
    pub added: Option<u32>,
    pub deleted: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitDetail {
    pub commit: Commit,
    pub files: Vec<FileChange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    /// Short name, e.g. `main` or `origin/main`.
    pub name: String,
    pub full_name: String,
    pub is_remote: bool,
    pub is_current: bool,
    pub id: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Upstream was configured but no longer exists on the remote.
    pub upstream_gone: bool,
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    /// Commit the tag ultimately points to.
    pub target: String,
    pub annotated: bool,
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StashEntry {
    pub index: usize,
    /// e.g. `stash@{0}`.
    pub refname: String,
    pub id: String,
    pub message: String,
    pub time: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflogEntry {
    pub id: String,
    /// e.g. `HEAD@{3}`.
    pub selector: String,
    pub message: String,
    pub time: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlameLine {
    pub line: u32,
    pub commit: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmoduleState {
    Uninitialized,
    UpToDate,
    /// Checked-out commit differs from the one recorded in the superproject.
    Modified,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submodule {
    pub path: String,
    pub id: String,
    pub state: SubmoduleState,
}

/// An in-progress multi-step operation that blocks normal work until finished or aborted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoState {
    Clean,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
}

/// Result of an operation that can stop on conflicts (merge, rebase, cherry-pick, pull, stash pop...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpOutcome {
    Success(String),
    /// Git stopped because of conflicts; resolve them, stage, then `*_continue`, or `*_abort`.
    Conflict {
        files: Vec<String>,
        output: String,
    },
}

impl OpOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, OpOutcome::Success(_))
    }
}
