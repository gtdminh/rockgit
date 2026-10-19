use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::args::Args;
use super::client::{GitClient, safe_ref};
use super::error::{GitError, GitResult};
use super::types::{BlameLine, ReflogEntry, Submodule, SubmoduleState, Worktree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigScope {
    /// This repository's `.git/config`.
    Local,
    /// The user's `~/.gitconfig`.
    Global,
}

impl GitClient {
    // ---- blame / reflog -----------------------------------------------------------------

    /// Line-by-line authorship of a file at `rev` (default: the work tree version).
    pub fn blame(&self, path: &str, rev: Option<&str>) -> GitResult<Vec<BlameLine>> {
        if let Some(r) = rev {
            safe_ref(r)?;
        }
        let args = Args::new("blame").arg("--porcelain").opt(rev).paths([path]);
        Ok(parse_blame(&self.run(args)?.stdout_str()))
    }

    /// Recent positions of HEAD (or another ref); the safety net for recovering lost commits.
    pub fn reflog(&self, max_count: usize, reference: Option<&str>) -> GitResult<Vec<ReflogEntry>> {
        if let Some(r) = reference {
            safe_ref(r)?;
        }
        if !self.has_commits()? {
            return Ok(Vec::new());
        }
        let args = Args::new("reflog")
            .arg("--format=%H%x1f%gd%x1f%gs%x1f%ct")
            .kv("max-count", Some(max_count))
            .opt(reference);
        let out = self.run(args)?.stdout_str();
        Ok(out
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.splitn(4, '\x1f').collect();
                (f.len() == 4).then(|| ReflogEntry {
                    id: f[0].to_string(),
                    selector: f[1].to_string(),
                    message: f[2].to_string(),
                    time: f[3].parse().unwrap_or(0),
                })
            })
            .collect())
    }

    // ---- worktrees ----------------------------------------------------------------------

    pub fn worktrees(&self) -> GitResult<Vec<Worktree>> {
        let out = self
            .run(Args::new("worktree").args(["list", "--porcelain"]))?
            .stdout_str();
        let mut list = Vec::new();
        for block in out.split("\n\n").filter(|b| !b.trim().is_empty()) {
            let mut wt = Worktree {
                path: PathBuf::new(),
                head: None,
                branch: None,
                bare: false,
                locked: false,
            };
            for line in block.lines() {
                let (key, value) = line.split_once(' ').unwrap_or((line, ""));
                match key {
                    "worktree" => wt.path = PathBuf::from(value),
                    "HEAD" => wt.head = Some(value.to_string()),
                    "branch" => {
                        wt.branch = Some(
                            value
                                .strip_prefix("refs/heads/")
                                .unwrap_or(value)
                                .to_string(),
                        )
                    }
                    "bare" => wt.bare = true,
                    "locked" => wt.locked = true,
                    _ => {}
                }
            }
            list.push(wt);
        }
        Ok(list)
    }

    /// Checks out `rev_or_branch` into a new directory. With `new_branch`, creates that branch.
    pub fn add_worktree(
        &self,
        path: &Path,
        rev_or_branch: Option<&str>,
        new_branch: Option<&str>,
    ) -> GitResult<()> {
        for r in [rev_or_branch, new_branch].into_iter().flatten() {
            safe_ref(r)?;
        }
        let args = Args::new("worktree")
            .arg("add")
            .opt(new_branch.map(|b| format!("-b{b}")))
            .arg("--")
            .arg(path)
            .opt(rev_or_branch);
        self.run(args).map(drop)
    }

    pub fn remove_worktree(&self, path: &Path, force: bool) -> GitResult<()> {
        let args = Args::new("worktree")
            .arg("remove")
            .flag(force, "--force")
            .arg("--")
            .arg(path);
        self.run(args).map(drop)
    }

    // ---- submodules ---------------------------------------------------------------------

    pub fn submodules(&self) -> GitResult<Vec<Submodule>> {
        let out = self.run(Args::new("submodule").arg("status"))?.stdout_str();
        let mut list = Vec::new();
        for line in out.lines() {
            let Some(marker) = line.chars().next() else {
                continue;
            };
            let rest = &line[marker.len_utf8()..];
            let mut parts = rest.splitn(2, ' ');
            let (Some(id), Some(tail)) = (parts.next(), parts.next()) else {
                continue;
            };
            let path = tail.rsplit_once(" (").map_or(tail, |(p, _)| p);
            list.push(Submodule {
                path: path.to_string(),
                id: id.to_string(),
                state: match marker {
                    '-' => SubmoduleState::Uninitialized,
                    '+' => SubmoduleState::Modified,
                    'U' => SubmoduleState::Conflict,
                    _ => SubmoduleState::UpToDate,
                },
            });
        }
        Ok(list)
    }

    /// Initializes and checks out submodules at the commits recorded in the superproject.
    pub fn update_submodules(&self, recursive: bool) -> GitResult<()> {
        let args = Args::new("submodule")
            .args(["update", "--init"])
            .flag(recursive, "--recursive");
        self.run(args).map(drop)
    }

    // ---- configuration ------------------------------------------------------------------

    /// Effective value of a setting (local overrides global), `None` when unset.
    pub fn config_get(&self, key: &str) -> GitResult<Option<String>> {
        safe_ref(key)?;
        let out = self.run_raw(&Args::new("config").args(["--get", key]))?;
        Ok(match out.status {
            Some(0) => Some(out.stdout_str().trim_end().to_string()),
            Some(1) => None,
            _ => return Err(super::client::command_error(&Args::new("config"), &out)),
        })
    }

    pub fn config_set(&self, scope: ConfigScope, key: &str, value: &str) -> GitResult<()> {
        safe_ref(key)?;
        self.run(
            Args::new("config")
                .arg(scope_flag(scope))
                .args([key, value]),
        )
        .map(drop)
    }

    pub fn config_unset(&self, scope: ConfigScope, key: &str) -> GitResult<()> {
        safe_ref(key)?;
        self.run(
            Args::new("config")
                .arg(scope_flag(scope))
                .args(["--unset", key]),
        )
        .map(drop)
    }

    /// `(user.name, user.email)` as git would use them for commits.
    pub fn user_identity(&self) -> GitResult<(Option<String>, Option<String>)> {
        Ok((
            self.config_get("user.name")?,
            self.config_get("user.email")?,
        ))
    }

    /// Sets the commit identity for this repository only.
    pub fn set_local_identity(&self, name: &str, email: &str) -> GitResult<()> {
        if name.trim().is_empty() || email.trim().is_empty() {
            return Err(GitError::InvalidInput("name and email are required".into()));
        }
        self.config_set(ConfigScope::Local, "user.name", name)?;
        self.config_set(ConfigScope::Local, "user.email", email)
    }
}

fn scope_flag(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::Local => "--local",
        ConfigScope::Global => "--global",
    }
}

fn is_blame_header(line: &str) -> bool {
    let mut parts = line.split(' ');
    let first = parts.next().unwrap_or_default();
    first.len() >= 40 && first.bytes().all(|b| b.is_ascii_hexdigit()) && parts.count() >= 2
}

/// Parses `git blame --porcelain`, where commit metadata appears only on first occurrence.
fn parse_blame(raw: &str) -> Vec<BlameLine> {
    let mut commits: HashMap<String, (String, i64, String)> = HashMap::new();
    let mut lines = Vec::new();
    let mut current: Option<(String, u32)> = None;

    for line in raw.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            if let Some((sha, number)) = current.take() {
                let (author, time, summary) = commits.get(&sha).cloned().unwrap_or_default();
                lines.push(BlameLine {
                    line: number,
                    commit: sha,
                    author,
                    time,
                    summary,
                    content: content.to_string(),
                });
            }
        } else if is_blame_header(line) {
            let mut p = line.split(' ');
            let sha = p.next().unwrap_or_default().to_string();
            let number = p.nth(1).and_then(|n| n.parse().ok()).unwrap_or(0);
            commits.entry(sha.clone()).or_default();
            current = Some((sha, number));
        } else if let Some((sha, _)) = &current {
            let entry = commits.entry(sha.clone()).or_default();
            if let Some(v) = line.strip_prefix("author ") {
                entry.0 = v.to_string();
            } else if let Some(v) = line.strip_prefix("author-time ") {
                entry.1 = v.parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("summary ") {
                entry.2 = v.to_string();
            }
        }
    }
    lines
}
