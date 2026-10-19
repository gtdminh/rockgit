use super::args::Args;
use super::client::{GitClient, safe_ref};
use super::error::GitResult;
use super::types::{OpOutcome, StashEntry};

#[derive(Clone, Debug, Default)]
pub struct StashOptions {
    pub message: Option<String>,
    /// Also stash untracked files.
    pub include_untracked: bool,
    /// Leave staged changes in the index.
    pub keep_index: bool,
    /// Stash only these paths.
    pub paths: Vec<String>,
}

fn stash_ref(index: usize) -> String {
    format!("stash@{{{index}}}")
}

impl GitClient {
    /// Saves local changes. Returns `false` when there was nothing to stash.
    pub fn stash_push(&self, o: &StashOptions) -> GitResult<bool> {
        let args = Args::new("stash")
            .arg("push")
            .flag(o.include_untracked, "--include-untracked")
            .flag(o.keep_index, "--keep-index")
            .opt(o.message.as_deref().map(|m| format!("--message={m}")))
            .paths(&o.paths);
        let out = self.run(args)?;
        Ok(!out.stdout_str().contains("No local changes to save"))
    }

    pub fn stash_list(&self) -> GitResult<Vec<StashEntry>> {
        let args = Args::new("stash").args(["list", "--format=%gd%x1f%H%x1f%gs%x1f%ct"]);
        let out = self.run(args)?.stdout_str();
        Ok(out
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let f: Vec<&str> = line.splitn(4, '\x1f').collect();
                (f.len() == 4).then(|| StashEntry {
                    index,
                    refname: f[0].to_string(),
                    id: f[1].to_string(),
                    message: f[2].to_string(),
                    time: f[3].parse().unwrap_or(0),
                })
            })
            .collect())
    }

    /// Re-applies a stash and keeps it. `restore_index` also restores what was staged.
    pub fn stash_apply(&self, index: usize, restore_index: bool) -> GitResult<OpOutcome> {
        let args = Args::new("stash")
            .arg("apply")
            .flag(restore_index, "--index")
            .arg(stash_ref(index));
        self.run_op(args)
    }

    /// Re-applies a stash and drops it on success.
    pub fn stash_pop(&self, index: usize, restore_index: bool) -> GitResult<OpOutcome> {
        let args = Args::new("stash")
            .arg("pop")
            .flag(restore_index, "--index")
            .arg(stash_ref(index));
        self.run_op(args)
    }

    pub fn stash_drop(&self, index: usize) -> GitResult<()> {
        self.run(Args::new("stash").arg("drop").arg(stash_ref(index)))
            .map(drop)
    }

    /// Deletes every stash. Destructive.
    pub fn stash_clear(&self) -> GitResult<()> {
        self.run(Args::new("stash").arg("clear")).map(drop)
    }

    /// Creates a branch from the commit the stash was made on, applies the stash, then drops it.
    pub fn stash_to_branch(&self, branch: &str, index: usize) -> GitResult<()> {
        safe_ref(branch)?;
        self.run(
            Args::new("stash")
                .arg("branch")
                .arg(branch)
                .arg(stash_ref(index)),
        )
        .map(drop)
    }

    /// Diff target for viewing a stash: `diff(&DiffTarget::Commit(stash_rev(0)), ..)`.
    pub fn stash_rev(index: usize) -> String {
        stash_ref(index)
    }
}
