//! A Git client for developer workflows, built on the installed `git` executable.
//!
//! Driving the real CLI (instead of linking libgit2) means SSH and HTTPS remotes work exactly
//! as they do in the user's terminal: `~/.ssh/config`, the SSH agent, credential helpers, proxies
//! and `url.*.insteadOf` rewrites are all honoured, and behaviour matches `git` itself.
//!
//! ```ignore
//! let repo = GitClient::open("/path/to/repo")?;
//! let status = repo.status(&StatusOptions::default())?;
//! repo.stage(&["src/main.rs".into()])?;
//! repo.commit(&CommitOptions { message: "Fix bug".into(), ..Default::default() })?;
//! repo.fetch(&FetchOptions { all: true, prune: true, ..Default::default() })?;
//! repo.push(&PushOptions::default())?;
//! let rows = repo.graph(&LogOptions { all: true, max_count: Some(500), ..Default::default() })?;
//! ```
//!
//! Every method is blocking: call them from a background thread, never the UI thread.
//! Operations that can stop on conflicts return [`OpOutcome`] rather than an error.
#![allow(dead_code, unused_imports)]

mod args;
mod branch;
mod client;
mod commit;
mod diff;
mod error;
mod graph;
mod history;
mod merge;
mod misc;
mod remote;
mod runner;
mod staging;
mod stash;
mod status;
mod tag;
mod types;

#[cfg(test)]
mod tests;

pub use client::{CloneOptions, GitClient, InitOptions};
pub use commit::CommitOptions;
pub use diff::{ApplyOptions, DiffLine, DiffLineKind, DiffOptions, DiffTarget, FileDiff, Hunk};
pub use error::{GitError, GitResult};
pub use graph::{GraphEdge, GraphRow, build_graph};
pub use history::{LogOptions, LogOrder};
pub use merge::{
    ConflictSide, MergeOptions, MergeStrategy, RebaseAction, RebaseOptions, RebaseStep, ResetMode,
};
pub use misc::ConfigScope;
pub use remote::{FetchOptions, PullOptions, PullStrategy, PushOptions};
pub use runner::GitOptions;
pub use staging::CleanOptions;
pub use stash::StashOptions;
pub use status::{StatusOptions, UntrackedMode};
pub use types::*;
