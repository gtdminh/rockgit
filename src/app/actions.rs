use std::path::PathBuf;

use crate::app::model::{AppConfig, FileChange, LoadedRepo};

#[derive(Clone, Debug)]
pub enum AppAction {
    OpenSettingsDialog,
    SaveSettingsDialog,
    CloseSettingsDialog,
    UpdateConfig(AppConfig),
    SelectRepository(usize),
    SelectBranch(usize),
    ToggleBranchListNode(String),
    ToggleWorkingTreeListNode(String),
    SelectUncommittedChanges,
    RevertWorkingTreeFile(String),
    RevertAllWorkingTreeChanges,
    CommitWorkingTree {
        message: String,
        push_after: bool,
    },
    SelectCommit(usize),
    SelectOlderCommit,
    SelectNewerCommit,
    SetSearchQuery(String),
    /// Raised by the branch popup; the app delegate resolves it to `SetBranchFilter`.
    BranchFilterChanged,
    SetBranchFilter(Option<usize>),
    ToggleShowRemote,
    RunGitOperation(GitOperation),
    /// Request (handled by the git service) to open a repository folder.
    OpenRepository(PathBuf),
    /// Result from the git service: a repository with its branches and graph.
    RepositoryLoaded(Box<LoadedRepo>),
    /// Result from the git service: loading failed.
    RepositoryLoadFailed(String),
    /// Result from the git service: an operation succeeded and wants to update the status bar.
    SetLastAction(String),
    /// Result from the git service: changed files of a commit.
    CommitFilesLoaded {
        index: usize,
        id: String,
        files: Vec<FileChange>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitOperation {
    Fetch,
    Pull,
    Push,
    Refresh,
}

impl GitOperation {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fetch => "Fetch",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::Refresh => "Refresh",
        }
    }
}
