use std::path::PathBuf;

use super::args::Args;
use super::client::{GitClient, safe_ref};
use super::error::{GitError, GitResult};
use super::types::OpOutcome;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Fast-forward when possible, otherwise create a merge commit.
    #[default]
    Default,
    /// Always create a merge commit.
    NoFastForward,
    FastForwardOnly,
    /// Combine into the index without committing or recording the merge.
    Squash,
}

#[derive(Clone, Debug, Default)]
pub struct MergeOptions {
    pub strategy: MergeStrategy,
    /// Merge commit message; git generates one when empty.
    pub message: Option<String>,
    /// Stop before committing so the result can be reviewed.
    pub no_commit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    /// Move HEAD only; keep index and work tree (undo commit, keep changes staged).
    Soft,
    /// Move HEAD and reset the index; keep the work tree.
    Mixed,
    /// Move HEAD and discard all changes. Destructive.
    Hard,
    /// Like hard, but refuses to overwrite local changes.
    Keep,
}

#[derive(Clone, Debug, Default)]
pub struct RebaseOptions {
    /// Commits after this point are replayed.
    pub upstream: String,
    /// Replay onto this base instead of `upstream`.
    pub onto: Option<String>,
    /// Branch to rebase; defaults to the current one.
    pub branch: Option<String>,
    pub autostash: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebaseAction {
    Pick,
    /// Meld into the previous commit, combining messages.
    Squash,
    /// Meld into the previous commit, discarding this message.
    Fixup,
    Drop,
    /// Stop here so the commit can be amended; then call `rebase_continue`.
    Edit,
}

impl RebaseAction {
    fn keyword(self) -> &'static str {
        match self {
            RebaseAction::Pick => "pick",
            RebaseAction::Squash => "squash",
            RebaseAction::Fixup => "fixup",
            RebaseAction::Drop => "drop",
            RebaseAction::Edit => "edit",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebaseStep {
    pub action: RebaseAction,
    pub commit: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictSide {
    /// The branch being merged into (during a rebase: the branch being rebased onto).
    Ours,
    /// The incoming changes (during a rebase: the commit being replayed).
    Theirs,
}

impl GitClient {
    // ---- merge --------------------------------------------------------------------------

    pub fn merge(&self, rev: &str, o: &MergeOptions) -> GitResult<OpOutcome> {
        safe_ref(rev)?;
        let args = Args::new("merge")
            .arg(match o.strategy {
                MergeStrategy::Default => "--no-edit",
                MergeStrategy::NoFastForward => "--no-ff",
                MergeStrategy::FastForwardOnly => "--ff-only",
                MergeStrategy::Squash => "--squash",
            })
            .flag(o.no_commit, "--no-commit")
            .opt(o.message.as_deref().map(|m| format!("--message={m}")))
            .arg(rev);
        self.run_op(args)
    }

    pub fn merge_abort(&self) -> GitResult<()> {
        self.run(Args::new("merge").arg("--abort")).map(drop)
    }

    /// Finishes a merge after conflicts were resolved and staged.
    pub fn merge_continue(&self) -> GitResult<OpOutcome> {
        self.run_op(Args::new("merge").arg("--continue"))
    }

    // ---- rebase -------------------------------------------------------------------------

    pub fn rebase(&self, o: &RebaseOptions) -> GitResult<OpOutcome> {
        safe_ref(&o.upstream)?;
        for r in [&o.onto, &o.branch].into_iter().flatten() {
            safe_ref(r)?;
        }
        let args = Args::new("rebase")
            .flag(o.autostash, "--autostash")
            .opt(o.onto.as_deref().map(|b| format!("--onto={b}")))
            .arg(&o.upstream)
            .opt(o.branch.as_deref());
        self.run_op(args)
    }

    /// The commits a rebase onto `upstream` would replay, oldest first, all as `pick`.
    pub fn rebase_todo(&self, upstream: &str) -> GitResult<Vec<RebaseStep>> {
        safe_ref(upstream)?;
        let out = self.run_str(
            Args::new("rev-list")
                .args(["--reverse", "--topo-order", "--no-merges"])
                .arg(format!("{upstream}..HEAD")),
        )?;
        Ok(out
            .lines()
            .map(|c| RebaseStep { action: RebaseAction::Pick, commit: c.to_string() })
            .collect())
    }

    /// Rebases onto `upstream` using an explicit plan: reorder, squash, fixup, drop or edit
    /// commits. Start from `rebase_todo` and modify it.
    pub fn rebase_interactive(&self, upstream: &str, todo: &[RebaseStep]) -> GitResult<OpOutcome> {
        safe_ref(upstream)?;
        let mut text = String::new();
        for step in todo {
            safe_ref(&step.commit)?;
            text.push_str(&format!("{} {}\n", step.action.keyword(), step.commit));
        }
        let path = temp_todo_path();
        std::fs::write(&path, text)?;
        // Git runs this through the shell with the todo file appended, so `cp` swaps in our plan.
        let quoted = path.to_string_lossy().replace('\'', "'\\''");
        let editor = format!("cp '{quoted}'");
        let result = self.run_op_env(
            Args::new("rebase").arg("--interactive").arg(upstream),
            &[("GIT_SEQUENCE_EDITOR", editor.as_str())],
        );
        let _ = std::fs::remove_file(&path);
        result
    }

    pub fn rebase_continue(&self) -> GitResult<OpOutcome> {
        self.run_op(Args::new("rebase").arg("--continue"))
    }

    pub fn rebase_skip(&self) -> GitResult<OpOutcome> {
        self.run_op(Args::new("rebase").arg("--skip"))
    }

    pub fn rebase_abort(&self) -> GitResult<()> {
        self.run(Args::new("rebase").arg("--abort")).map(drop)
    }

    // ---- cherry-pick / revert -----------------------------------------------------------

    /// Applies commits onto the current branch. `mainline` selects the parent for merge commits.
    pub fn cherry_pick(&self, commits: &[String], no_commit: bool, mainline: Option<u32>) -> GitResult<OpOutcome> {
        if commits.is_empty() {
            return Err(GitError::InvalidInput("no commits given".into()));
        }
        for c in commits {
            safe_ref(c)?;
        }
        let args = Args::new("cherry-pick")
            .flag(no_commit, "--no-commit")
            .kv("mainline", mainline)
            .args(commits);
        self.run_op(args)
    }

    pub fn cherry_pick_continue(&self) -> GitResult<OpOutcome> {
        self.run_op(Args::new("cherry-pick").arg("--continue"))
    }

    pub fn cherry_pick_skip(&self) -> GitResult<OpOutcome> {
        self.run_op(Args::new("cherry-pick").arg("--skip"))
    }

    pub fn cherry_pick_abort(&self) -> GitResult<()> {
        self.run(Args::new("cherry-pick").arg("--abort")).map(drop)
    }

    /// Creates new commits that undo the given commits.
    pub fn revert(&self, commits: &[String], no_commit: bool, mainline: Option<u32>) -> GitResult<OpOutcome> {
        if commits.is_empty() {
            return Err(GitError::InvalidInput("no commits given".into()));
        }
        for c in commits {
            safe_ref(c)?;
        }
        let args = Args::new("revert")
            .arg("--no-edit")
            .flag(no_commit, "--no-commit")
            .kv("mainline", mainline)
            .args(commits);
        self.run_op(args)
    }

    pub fn revert_continue(&self) -> GitResult<OpOutcome> {
        self.run_op(Args::new("revert").arg("--continue"))
    }

    pub fn revert_abort(&self) -> GitResult<()> {
        self.run(Args::new("revert").arg("--abort")).map(drop)
    }

    // ---- reset --------------------------------------------------------------------------

    pub fn reset(&self, target: &str, mode: ResetMode) -> GitResult<()> {
        safe_ref(target)?;
        let flag = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
            ResetMode::Keep => "--keep",
        };
        self.run(Args::new("reset").arg(flag).arg(target)).map(drop)
    }

    /// Removes the last commit, keeping its changes staged.
    pub fn undo_last_commit(&self) -> GitResult<()> {
        self.reset("HEAD~1", ResetMode::Soft)
    }

    // ---- conflict resolution ------------------------------------------------------------

    /// Resolves a conflicted path by taking one side wholesale, and stages it.
    pub fn resolve_conflict(&self, path: &str, side: ConflictSide) -> GitResult<()> {
        let flag = match side {
            ConflictSide::Ours => "--ours",
            ConflictSide::Theirs => "--theirs",
        };
        self.run(Args::new("checkout").arg(flag).paths([path]))?;
        self.run(Args::new("add").paths([path])).map(drop)
    }

    /// Stages paths after their conflict markers were edited away by hand.
    pub fn mark_resolved(&self, paths: &[String]) -> GitResult<()> {
        self.stage(paths)
    }
}

fn temp_todo_path() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    std::env::temp_dir().join(format!("rockgit-todo-{}-{nanos}", std::process::id()))
}
