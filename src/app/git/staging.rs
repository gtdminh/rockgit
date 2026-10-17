use super::args::Args;
use super::client::GitClient;
use super::error::{GitError, GitResult};

#[derive(Clone, Debug, Default)]
pub struct CleanOptions {
    /// Also remove untracked directories.
    pub directories: bool,
    /// Also remove files ignored by `.gitignore`.
    pub ignored: bool,
    /// Only report what would be removed.
    pub dry_run: bool,
    /// Limit to these paths; empty means the whole work tree.
    pub paths: Vec<String>,
}

fn require_paths(paths: &[String]) -> GitResult<()> {
    if paths.is_empty() {
        return Err(GitError::InvalidInput("no paths given".into()));
    }
    Ok(())
}

impl GitClient {
    /// Stages the given paths (new, modified and deleted files).
    pub fn stage(&self, paths: &[String]) -> GitResult<()> {
        require_paths(paths)?;
        self.run(Args::new("add").paths(paths)).map(drop)
    }

    pub fn stage_all(&self) -> GitResult<()> {
        self.run(Args::new("add").arg("-A")).map(drop)
    }

    /// Removes paths from the index, keeping work-tree contents. Works before the first commit too.
    pub fn unstage(&self, paths: &[String]) -> GitResult<()> {
        require_paths(paths)?;
        if self.has_commits()? {
            self.run(Args::new("restore").arg("--staged").paths(paths)).map(drop)
        } else {
            self.run(Args::new("rm").args(["-r", "--cached", "-f", "-q"]).paths(paths))
                .map(drop)
        }
    }

    pub fn unstage_all(&self) -> GitResult<()> {
        self.run(Args::new("reset").arg("--quiet")).map(drop)
    }

    /// Throws away unstaged changes to tracked files. Destructive.
    pub fn discard_changes(&self, paths: &[String]) -> GitResult<()> {
        require_paths(paths)?;
        self.run(Args::new("restore").paths(paths)).map(drop)
    }

    /// Throws away every unstaged change to tracked files. Destructive.
    pub fn discard_all_changes(&self) -> GitResult<()> {
        self.run(Args::new("restore").paths([":/"])).map(drop)
    }

    /// Deletes untracked files. Returns the affected paths (those that would be, with `dry_run`).
    /// Destructive unless `dry_run`.
    pub fn clean(&self, o: &CleanOptions) -> GitResult<Vec<String>> {
        let args = Args::new("clean")
            .arg(if o.dry_run { "-n" } else { "-f" })
            .flag(o.directories, "-d")
            .flag(o.ignored, "-x")
            .paths(&o.paths);
        let out = self.run(args)?.stdout_str();
        Ok(out
            .lines()
            .filter_map(|l| {
                l.strip_prefix("Would remove ")
                    .or_else(|| l.strip_prefix("Removing "))
                    .map(str::to_string)
            })
            .collect())
    }

    /// `git rm`: deletes from the index and (unless `cached`) the work tree.
    pub fn remove(&self, paths: &[String], cached: bool, recursive: bool) -> GitResult<()> {
        require_paths(paths)?;
        let args = Args::new("rm")
            .flag(cached, "--cached")
            .flag(recursive, "-r")
            .paths(paths);
        self.run(args).map(drop)
    }

    /// `git mv`: moves/renames a tracked path.
    pub fn move_path(&self, from: &str, to: &str) -> GitResult<()> {
        self.run(Args::new("mv").paths([from, to])).map(drop)
    }
}
