use super::args::Args;
use super::client::{GitClient, join_output, safe_ref, safe_url};
use super::error::{GitError, GitResult};
use super::types::{OpOutcome, Remote, RemoteRef};

#[derive(Clone, Debug, Default)]
pub struct FetchOptions {
    /// Remote to fetch; `None` uses the current branch's remote (or `origin`).
    pub remote: Option<String>,
    /// Fetch every remote (ignores `remote`).
    pub all: bool,
    /// Delete remote-tracking branches that no longer exist on the remote.
    pub prune: bool,
    /// Also fetch all tags.
    pub tags: bool,
    pub depth: Option<u32>,
    /// Turn a shallow clone into a full one.
    pub unshallow: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PullStrategy {
    /// Whatever the user's git configuration says.
    #[default]
    Default,
    Merge,
    Rebase,
    FastForwardOnly,
}

#[derive(Clone, Debug, Default)]
pub struct PullOptions {
    pub remote: Option<String>,
    /// Remote branch to pull; requires `remote`.
    pub branch: Option<String>,
    pub strategy: PullStrategy,
    /// Stash local changes around the pull.
    pub autostash: bool,
    pub prune: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PushOptions {
    /// Defaults to the branch's configured remote.
    pub remote: Option<String>,
    /// Branch or refspec such as `main` or `local:remote`; requires `remote`.
    pub refspec: Option<String>,
    /// Record the remote branch as upstream (`-u`).
    pub set_upstream: bool,
    /// Overwrite the remote only if it still has what we last fetched. Preferred over `force`.
    pub force_with_lease: bool,
    pub force: bool,
    /// Push all tags.
    pub tags: bool,
    /// Push annotated tags reachable from the pushed commits.
    pub follow_tags: bool,
    /// Delete `refspec` on the remote.
    pub delete: bool,
    pub dry_run: bool,
    pub no_verify: bool,
}

impl GitClient {
    pub fn remotes(&self) -> GitResult<Vec<Remote>> {
        let out = self.run(Args::new("remote").arg("-v"))?.stdout_str();
        let mut remotes: Vec<Remote> = Vec::new();
        for line in out.lines() {
            let Some((name, rest)) = line.split_once('\t') else {
                continue;
            };
            let Some((url, kind)) = rest.rsplit_once(' ') else {
                continue;
            };
            let idx = match remotes.iter().position(|r| r.name == name) {
                Some(i) => i,
                None => {
                    remotes.push(Remote {
                        name: name.to_string(),
                        fetch_url: String::new(),
                        push_url: String::new(),
                    });
                    remotes.len() - 1
                }
            };
            match kind {
                "(fetch)" => remotes[idx].fetch_url = url.to_string(),
                "(push)" => remotes[idx].push_url = url.to_string(),
                _ => {}
            }
        }
        Ok(remotes)
    }

    pub fn add_remote(&self, name: &str, url: &str) -> GitResult<()> {
        safe_ref(name)?;
        safe_url(url)?;
        self.run(Args::new("remote").args(["add", name, url]))
            .map(drop)
    }

    pub fn remove_remote(&self, name: &str) -> GitResult<()> {
        safe_ref(name)?;
        self.run(Args::new("remote").args(["remove", name]))
            .map(drop)
    }

    pub fn rename_remote(&self, old: &str, new: &str) -> GitResult<()> {
        safe_ref(old)?;
        safe_ref(new)?;
        self.run(Args::new("remote").args(["rename", old, new]))
            .map(drop)
    }

    /// Changes a remote's URL, e.g. to switch it between HTTPS and SSH. With `push_only`,
    /// only the push URL changes.
    pub fn set_remote_url(&self, name: &str, url: &str, push_only: bool) -> GitResult<()> {
        safe_ref(name)?;
        safe_url(url)?;
        let args = Args::new("remote")
            .arg("set-url")
            .flag(push_only, "--push")
            .args([name, url]);
        self.run(args).map(drop)
    }

    /// Downloads new objects and refs. Returns git's report.
    pub fn fetch(&self, o: &FetchOptions) -> GitResult<String> {
        if let Some(r) = &o.remote {
            safe_ref(r)?;
        }
        let args = Args::new("fetch")
            .flag(o.all, "--all")
            .flag(o.prune, "--prune")
            .flag(o.tags, "--tags")
            .flag(o.unshallow, "--unshallow")
            .kv("depth", o.depth)
            .opt(if o.all { None } else { o.remote.as_deref() });
        Ok(join_output(&self.run(args)?))
    }

    /// Fetches and integrates remote changes into the current branch. May stop on conflicts.
    pub fn pull(&self, o: &PullOptions) -> GitResult<OpOutcome> {
        if let Some(r) = &o.remote {
            safe_ref(r)?;
        }
        if let Some(b) = &o.branch {
            safe_ref(b)?;
            if o.remote.is_none() {
                return Err(GitError::InvalidInput("`branch` requires `remote`".into()));
            }
        }
        let args = Args::new("pull")
            .arg(match o.strategy {
                PullStrategy::Default => "--no-edit",
                PullStrategy::Merge => "--no-rebase",
                PullStrategy::Rebase => "--rebase",
                PullStrategy::FastForwardOnly => "--ff-only",
            })
            .flag(o.autostash, "--autostash")
            .flag(o.prune, "--prune")
            .opt(o.remote.as_deref())
            .opt(o.branch.as_deref());
        self.run_op(args)
    }

    /// Uploads commits/tags to a remote. Returns git's report.
    pub fn push(&self, o: &PushOptions) -> GitResult<String> {
        if let Some(r) = &o.remote {
            safe_ref(r)?;
        }
        if let Some(r) = &o.refspec {
            safe_ref(r)?;
            if o.remote.is_none() {
                return Err(GitError::InvalidInput("`refspec` requires `remote`".into()));
            }
        }
        if o.delete && o.refspec.is_none() {
            return Err(GitError::InvalidInput("`delete` requires `refspec`".into()));
        }
        let args = Args::new("push")
            .flag(o.set_upstream, "--set-upstream")
            .flag(o.force_with_lease, "--force-with-lease")
            .flag(o.force, "--force")
            .flag(o.tags, "--tags")
            .flag(o.follow_tags, "--follow-tags")
            .flag(o.delete, "--delete")
            .flag(o.dry_run, "--dry-run")
            .flag(o.no_verify, "--no-verify")
            .opt(o.remote.as_deref())
            .opt(o.refspec.as_deref());
        Ok(join_output(&self.run(args)?))
    }

    /// Deletes a branch on the remote.
    pub fn delete_remote_branch(&self, remote: &str, branch: &str) -> GitResult<String> {
        self.push(&PushOptions {
            remote: Some(remote.to_string()),
            refspec: Some(branch.to_string()),
            delete: true,
            ..Default::default()
        })
    }

    /// Lists refs of a remote name or URL without downloading anything. Also a cheap way to
    /// verify that credentials work.
    pub fn ls_remote(&self, remote_or_url: &str) -> GitResult<Vec<RemoteRef>> {
        safe_url(remote_or_url)?;
        let out = self
            .run(Args::new("ls-remote").arg("--").arg(remote_or_url))?
            .stdout_str();
        Ok(out
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(id, name)| RemoteRef {
                id: id.to_string(),
                name: name.to_string(),
            })
            .collect())
    }
}
