use std::path::{Path, PathBuf};

use super::args::Args;
use super::error::{GitError, GitResult};
use super::runner::{GitOptions, Output, execute};
use super::types::OpOutcome;

/// A handle to one repository, driving the installed `git` executable.
///
/// Cheap to clone; holds no open resources, so it is `Send + Sync` and safe to use from
/// background threads.
#[derive(Clone, Debug)]
pub struct GitClient {
    pub(super) root: PathBuf,
    pub(super) git_dir: PathBuf,
    pub(super) bare: bool,
    pub(super) opts: GitOptions,
}

#[derive(Clone, Debug, Default)]
pub struct CloneOptions {
    pub branch: Option<String>,
    pub depth: Option<u32>,
    pub single_branch: bool,
    pub recurse_submodules: bool,
    pub bare: bool,
    pub mirror: bool,
    /// Partial clone filter, e.g. `blob:none`.
    pub filter: Option<String>,
    /// Remote name instead of `origin`.
    pub origin: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct InitOptions {
    pub bare: bool,
    pub initial_branch: Option<String>,
}

impl GitClient {
    /// Opens the repository containing `path` (walks up like `git` does).
    pub fn open(path: impl AsRef<Path>) -> GitResult<Self> {
        Self::open_with(path, GitOptions::default())
    }

    pub fn open_with(path: impl AsRef<Path>, opts: GitOptions) -> GitResult<Self> {
        let path = path.as_ref();
        let probe = Args::new("rev-parse").args(["--is-bare-repository", "--absolute-git-dir"]);
        let out = execute(&opts, Some(path), &probe, None, &[])?;
        if !out.success() {
            return Err(GitError::NotARepository(path.to_path_buf()));
        }
        let text = out.stdout_str();
        let mut lines = text.lines();
        let bare = lines.next() == Some("true");
        let git_dir = PathBuf::from(lines.next().unwrap_or_default());

        let root = if bare {
            git_dir.clone()
        } else {
            let top = execute(
                &opts,
                Some(path),
                &Args::new("rev-parse").arg("--show-toplevel"),
                None,
                &[],
            )?;
            if !top.success() {
                return Err(GitError::NotARepository(path.to_path_buf()));
            }
            PathBuf::from(top.stdout_str().trim_end())
        };
        Ok(Self {
            root,
            git_dir,
            bare,
            opts,
        })
    }

    /// Creates a repository at `path` (creating the directory if needed) and opens it.
    pub fn init(path: impl AsRef<Path>, init: &InitOptions, opts: GitOptions) -> GitResult<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)?;
        if let Some(b) = &init.initial_branch {
            safe_ref(b)?;
        }
        let args = Args::new("init")
            .flag(init.bare, "--bare")
            .kv("initial-branch", init.initial_branch.as_deref());
        check(&args, execute(&opts, Some(path), &args, None, &[])?)?;
        Self::open_with(path, opts)
    }

    /// Clones `url` (https, ssh, file or local path) into `dest` and opens it.
    pub fn clone_repo(
        url: &str,
        dest: impl AsRef<Path>,
        clone: &CloneOptions,
        opts: GitOptions,
    ) -> GitResult<Self> {
        safe_url(url)?;
        let dest = std::path::absolute(dest.as_ref())?;
        if let Some(b) = &clone.branch {
            safe_ref(b)?;
        }
        let args = Args::new("clone")
            .flag(clone.bare, "--bare")
            .flag(clone.mirror, "--mirror")
            .flag(clone.single_branch, "--single-branch")
            .flag(clone.recurse_submodules, "--recurse-submodules")
            .kv("branch", clone.branch.as_deref())
            .kv("depth", clone.depth)
            .kv("filter", clone.filter.as_deref())
            .kv("origin", clone.origin.as_deref())
            .arg("--")
            .arg(url)
            .arg(&dest);
        check(&args, execute(&opts, None, &args, None, &[])?)?;
        Self::open_with(dest, opts)
    }

    /// Version string of the installed git, e.g. `2.43.0`.
    pub fn git_version(opts: &GitOptions) -> GitResult<String> {
        let args = Args::new("version");
        let out = check(&args, execute(opts, None, &args, None, &[])?)?;
        Ok(out
            .stdout_str()
            .trim()
            .trim_start_matches("git version ")
            .to_string())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn is_bare(&self) -> bool {
        self.bare
    }

    pub fn options(&self) -> &GitOptions {
        &self.opts
    }

    pub fn with_options(mut self, opts: GitOptions) -> Self {
        self.opts = opts;
        self
    }

    // ---- internal command helpers -------------------------------------------------------

    /// Runs git and returns the output whatever the exit code.
    pub(super) fn run_raw(&self, args: &Args) -> GitResult<Output> {
        execute(&self.opts, Some(&self.root), args, None, &[])
    }

    pub(super) fn run(&self, args: Args) -> GitResult<Output> {
        check(&args, self.run_raw(&args)?)
    }

    /// Runs git and returns stdout with trailing whitespace trimmed.
    pub(super) fn run_str(&self, args: Args) -> GitResult<String> {
        Ok(self.run(args)?.stdout_str().trim_end().to_string())
    }

    pub(super) fn run_stdin(&self, args: Args, input: &[u8]) -> GitResult<Output> {
        let out = execute(
            &self.opts,
            Some(&self.root),
            &args,
            Some(input.to_vec()),
            &[],
        )?;
        check(&args, out)
    }

    pub(super) fn run_env(&self, args: Args, env: &[(&str, &str)]) -> GitResult<Output> {
        let out = execute(&self.opts, Some(&self.root), &args, None, env)?;
        check(&args, out)
    }

    /// Runs an operation that may legitimately stop on conflicts.
    pub(super) fn run_op(&self, args: Args) -> GitResult<OpOutcome> {
        self.run_op_env(args, &[])
    }

    pub(super) fn run_op_env(&self, args: Args, env: &[(&str, &str)]) -> GitResult<OpOutcome> {
        let out = execute(&self.opts, Some(&self.root), &args, None, env)?;
        let combined = join_output(&out);
        if out.success() {
            return Ok(OpOutcome::Success(combined));
        }
        let files = self.conflicted_files().unwrap_or_default();
        if files.is_empty() {
            return Err(command_error(&args, &out));
        }
        Ok(OpOutcome::Conflict {
            files,
            output: combined,
        })
    }

    pub fn has_commits(&self) -> GitResult<bool> {
        let args = Args::new("rev-parse").args(["--verify", "--quiet", "HEAD"]);
        Ok(self.run_raw(&args)?.success())
    }
}

pub(super) fn check(args: &Args, out: Output) -> GitResult<Output> {
    if out.success() {
        Ok(out)
    } else {
        Err(command_error(args, &out))
    }
}

pub(super) fn command_error(args: &Args, out: &Output) -> GitError {
    let mut stderr = super::error::redact(out.stderr_str().trim_end());
    if stderr.is_empty() {
        stderr = super::error::redact(out.stdout_str().trim_end());
    }
    GitError::Command {
        command: args.describe(),
        code: out.status,
        stderr,
    }
}

pub(super) fn join_output(out: &Output) -> String {
    let (o, e) = (out.stdout_str(), out.stderr_str());
    let (o, e) = (o.trim(), e.trim());
    match (o.is_empty(), e.is_empty()) {
        (true, _) => e.to_string(),
        (_, true) => o.to_string(),
        _ => format!("{o}\n{e}"),
    }
}

/// Rejects values that git would parse as options (`--upload-pack=...`) or that can't be refs.
pub(super) fn safe_ref(s: &str) -> GitResult<&str> {
    if s.is_empty() || s.starts_with('-') || s.contains(['\0', '\n']) {
        return Err(GitError::InvalidInput(format!(
            "invalid ref or name: {s:?}"
        )));
    }
    Ok(s)
}

pub(super) fn safe_url(s: &str) -> GitResult<&str> {
    if s.is_empty() || s.starts_with('-') || s.contains(['\0', '\n']) {
        return Err(GitError::InvalidInput("invalid repository URL".into()));
    }
    Ok(s)
}
