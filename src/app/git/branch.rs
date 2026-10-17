use super::args::Args;
use super::client::{GitClient, safe_ref};
use super::error::{GitError, GitResult};
use super::types::Branch;

const BRANCH_FORMAT: &str = "%(HEAD)%1f%(refname)%1f%(refname:short)%1f%(objectname)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(contents:subject)";

impl GitClient {
    /// Local and remote-tracking branches (symbolic `origin/HEAD` entries are skipped).
    pub fn branches(&self) -> GitResult<Vec<Branch>> {
        let args = Args::new("for-each-ref")
            .arg(format!("--format={BRANCH_FORMAT}"))
            .args(["refs/heads", "refs/remotes"]);
        let out = self.run(args)?.stdout_str();
        let mut branches = Vec::new();
        for line in out.lines() {
            let f: Vec<&str> = line.splitn(7, '\x1f').collect();
            if f.len() < 7 || f[1].ends_with("/HEAD") {
                continue;
            }
            let track = f[5];
            let (mut ahead, mut behind) = (0, 0);
            for part in track.split(", ") {
                if let Some(n) = part.strip_prefix("ahead ") {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix("behind ") {
                    behind = n.parse().unwrap_or(0);
                }
            }
            branches.push(Branch {
                name: f[2].to_string(),
                full_name: f[1].to_string(),
                is_remote: f[1].starts_with("refs/remotes/"),
                is_current: f[0] == "*",
                id: f[3].to_string(),
                upstream: (!f[4].is_empty()).then(|| f[4].to_string()),
                ahead,
                behind,
                upstream_gone: track == "gone",
                subject: f[6].to_string(),
            });
        }
        Ok(branches)
    }

    /// Creates a branch at `start` (default HEAD), optionally switching to it.
    pub fn create_branch(&self, name: &str, start: Option<&str>, checkout: bool) -> GitResult<()> {
        safe_ref(name)?;
        if let Some(s) = start {
            safe_ref(s)?;
        }
        let args = if checkout {
            Args::new("checkout").arg("-b").arg(name).opt(start)
        } else {
            Args::new("branch").arg(name).opt(start)
        };
        self.run(args).map(drop)
    }

    /// Switches to a local branch. A remote-only name creates a tracking branch automatically.
    /// With `discard_local_changes`, uncommitted changes are thrown away.
    pub fn switch_branch(&self, name: &str, discard_local_changes: bool) -> GitResult<()> {
        safe_ref(name)?;
        let args = Args::new("switch")
            .flag(discard_local_changes, "--discard-changes")
            .arg(name);
        self.run(args).map(drop)
    }

    /// Checks out a new local branch tracking `remote_branch` (e.g. `origin/feature`).
    pub fn checkout_remote_branch(&self, remote_branch: &str, local_name: Option<&str>) -> GitResult<()> {
        safe_ref(remote_branch)?;
        let local = match local_name {
            Some(n) => safe_ref(n)?,
            None => remote_branch
                .split_once('/')
                .map(|(_, rest)| rest)
                .ok_or_else(|| GitError::InvalidInput("expected <remote>/<branch>".into()))?,
        };
        self.run(Args::new("switch").args(["-c", local, "--track", remote_branch]))
            .map(drop)
    }

    /// Detaches HEAD at a commit, tag or branch tip.
    pub fn checkout_detached(&self, rev: &str) -> GitResult<()> {
        safe_ref(rev)?;
        self.run(Args::new("switch").args(["--detach", rev])).map(drop)
    }

    /// Restores files from a revision into the work tree (not the index).
    pub fn restore_from(&self, rev: &str, paths: &[String]) -> GitResult<()> {
        safe_ref(rev)?;
        if paths.is_empty() {
            return Err(GitError::InvalidInput("no paths given".into()));
        }
        self.run(Args::new("restore").arg(format!("--source={rev}")).paths(paths))
            .map(drop)
    }

    /// Deletes a local branch. Without `force`, refuses unmerged branches.
    pub fn delete_branch(&self, name: &str, force: bool) -> GitResult<()> {
        safe_ref(name)?;
        self.run(Args::new("branch").arg(if force { "-D" } else { "-d" }).arg(name))
            .map(drop)
    }

    pub fn rename_branch(&self, old: &str, new: &str) -> GitResult<()> {
        safe_ref(old)?;
        safe_ref(new)?;
        self.run(Args::new("branch").args(["-m", old, new])).map(drop)
    }

    /// Sets the upstream of `branch` (e.g. `origin/main`).
    pub fn set_upstream(&self, branch: &str, upstream: &str) -> GitResult<()> {
        safe_ref(branch)?;
        safe_ref(upstream)?;
        self.run(Args::new("branch").arg(format!("--set-upstream-to={upstream}")).arg(branch))
            .map(drop)
    }

    pub fn unset_upstream(&self, branch: &str) -> GitResult<()> {
        safe_ref(branch)?;
        self.run(Args::new("branch").args(["--unset-upstream", branch])).map(drop)
    }

    /// Local and remote branches that contain `commit`.
    pub fn branches_containing(&self, commit: &str) -> GitResult<Vec<String>> {
        safe_ref(commit)?;
        let args = Args::new("branch")
            .args(["--all", "--format=%(refname:short)", "--contains", commit]);
        Ok(self
            .run(args)?
            .stdout_str()
            .lines()
            .map(str::to_string)
            .collect())
    }
}
