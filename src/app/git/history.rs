use super::args::Args;
use super::client::{GitClient, safe_ref};
use super::error::{GitError, GitResult};
use super::types::{Commit, CommitDetail, DecorationKind, RefDecoration};

const LOG_FORMAT: &str = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%D%x1f%s%x1f%b%x1e";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogOrder {
    /// Children before parents, branches kept together. Required for a correct graph.
    #[default]
    Topo,
    /// Newest first, parents never before children.
    Date,
    /// Git's default ordering.
    Plain,
}

#[derive(Clone, Debug, Default)]
pub struct LogOptions {
    /// Starting points (branches, tags, ids, ranges like `a..b`). Empty means `HEAD`.
    pub revs: Vec<String>,
    /// Start from every ref instead (`--all`).
    pub all: bool,
    pub max_count: Option<usize>,
    pub skip: usize,
    pub author: Option<String>,
    /// Match commit messages against this pattern.
    pub grep: Option<String>,
    pub ignore_case: bool,
    /// Match `grep` literally instead of as a regex.
    pub fixed_strings: bool,
    pub since: Option<String>,
    pub until: Option<String>,
    /// Only commits touching these paths.
    pub paths: Vec<String>,
    pub first_parent: bool,
    pub no_merges: bool,
    /// Follow renames; requires exactly one path.
    pub follow: bool,
    pub order: LogOrder,
}

impl GitClient {
    pub fn log(&self, o: &LogOptions) -> GitResult<Vec<Commit>> {
        if o.revs.is_empty() && !o.all && !self.has_commits()? {
            return Ok(Vec::new());
        }
        for r in &o.revs {
            safe_ref(r)?;
        }
        let args = Args::new("log")
            .arg("--decorate=full")
            .arg(format!("--format={LOG_FORMAT}"))
            .flag(o.order == LogOrder::Topo, "--topo-order")
            .flag(o.order == LogOrder::Date, "--date-order")
            .flag(o.all, "--all")
            .flag(o.first_parent, "--first-parent")
            .flag(o.no_merges, "--no-merges")
            .flag(o.follow, "--follow")
            .flag(o.ignore_case, "--regexp-ignore-case")
            .flag(o.fixed_strings, "--fixed-strings")
            .kv("max-count", o.max_count)
            .kv("skip", (o.skip > 0).then_some(o.skip))
            .kv("author", o.author.as_deref())
            .kv("grep", o.grep.as_deref())
            .kv("since", o.since.as_deref())
            .kv("until", o.until.as_deref())
            .args(&o.revs)
            .paths(&o.paths);
        parse_log(&self.run(args)?.stdout_str())
    }

    /// One commit by id/ref, with the files it changed (against its first parent).
    pub fn commit_detail(&self, rev: &str) -> GitResult<CommitDetail> {
        safe_ref(rev)?;
        let commit = self
            .log(&LogOptions {
                revs: vec![rev.to_string()],
                max_count: Some(1),
                ..Default::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| GitError::InvalidInput(format!("unknown commit {rev}")))?;
        let files = self.changed_files(&super::diff::DiffTarget::Commit(commit.id.clone()), &Default::default())?;
        Ok(CommitDetail { commit, files })
    }

    /// History of one file, following renames.
    pub fn file_history(&self, path: &str, max_count: Option<usize>) -> GitResult<Vec<Commit>> {
        self.log(&LogOptions {
            paths: vec![path.to_string()],
            follow: true,
            max_count,
            order: LogOrder::Plain,
            ..Default::default()
        })
    }

    /// Resolves any revision expression to a full commit id.
    pub fn rev_parse(&self, rev: &str) -> GitResult<String> {
        safe_ref(rev)?;
        self.run_str(Args::new("rev-parse").args(["--verify", "--quiet"]).arg(format!("{rev}^{{commit}}")))
    }

    pub fn merge_base(&self, a: &str, b: &str) -> GitResult<Option<String>> {
        safe_ref(a)?;
        safe_ref(b)?;
        let out = self.run_raw(&Args::new("merge-base").args([a, b]))?;
        Ok(out.success().then(|| out.stdout_str().trim().to_string()))
    }

    /// Commits only in `a` and only in `b`, i.e. `(ahead, behind)` when `a` is local and `b` upstream.
    pub fn ahead_behind(&self, a: &str, b: &str) -> GitResult<(u32, u32)> {
        safe_ref(a)?;
        safe_ref(b)?;
        let out = self.run_str(
            Args::new("rev-list")
                .args(["--left-right", "--count"])
                .arg(format!("{a}...{b}")),
        )?;
        let mut parts = out.split_whitespace().map(|n| n.parse::<u32>().unwrap_or(0));
        Ok((parts.next().unwrap_or(0), parts.next().unwrap_or(0)))
    }

    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> GitResult<bool> {
        safe_ref(ancestor)?;
        safe_ref(descendant)?;
        let args = Args::new("merge-base").args(["--is-ancestor", ancestor, descendant]);
        Ok(self.run_raw(&args)?.success())
    }

    pub fn describe(&self, rev: &str) -> GitResult<String> {
        safe_ref(rev)?;
        self.run_str(Args::new("describe").args(["--tags", "--always", rev]))
    }

    pub(super) fn parents_of(&self, rev: &str) -> GitResult<Vec<String>> {
        safe_ref(rev)?;
        let out = self.run_str(Args::new("rev-list").args(["--parents", "-n", "1", rev]))?;
        Ok(out.split_whitespace().skip(1).map(str::to_string).collect())
    }
}

pub(super) fn parse_log(raw: &str) -> GitResult<Vec<Commit>> {
    let mut commits = Vec::new();
    for rec in raw.split('\x1e') {
        let rec = rec.trim_start_matches(['\n', '\r']);
        if rec.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = rec.splitn(11, '\x1f').collect();
        if f.len() != 11 {
            return Err(GitError::Parse(format!("unexpected log record: {rec:?}")));
        }
        let id = f[0].to_string();
        commits.push(Commit {
            short_id: id.chars().take(7).collect(),
            id,
            parents: f[1].split_whitespace().map(str::to_string).collect(),
            author_name: f[2].to_string(),
            author_email: f[3].to_string(),
            author_time: f[4].parse().unwrap_or(0),
            committer_name: f[5].to_string(),
            committer_email: f[6].to_string(),
            commit_time: f[7].parse().unwrap_or(0),
            refs: parse_decorations(f[8]),
            subject: f[9].to_string(),
            body: f[10].trim().to_string(),
        });
    }
    Ok(commits)
}

/// Parses `%D` produced with `--decorate=full`, e.g. `HEAD -> refs/heads/main, tag: refs/tags/v1`.
fn parse_decorations(s: &str) -> Vec<RefDecoration> {
    let mut refs = Vec::new();
    for item in s.split(", ").map(str::trim).filter(|i| !i.is_empty()) {
        let (full, is_head) = match item.strip_prefix("HEAD -> ") {
            Some(rest) => (rest, true),
            None => (item.strip_prefix("tag: ").unwrap_or(item), item == "HEAD"),
        };
        let (name, kind) = if let Some(n) = full.strip_prefix("refs/heads/") {
            (n, DecorationKind::LocalBranch)
        } else if let Some(n) = full.strip_prefix("refs/remotes/") {
            if n.ends_with("/HEAD") {
                continue;
            }
            (n, DecorationKind::RemoteBranch)
        } else if let Some(n) = full.strip_prefix("refs/tags/") {
            (n, DecorationKind::Tag)
        } else if full == "HEAD" {
            ("HEAD", DecorationKind::Head)
        } else {
            (full, DecorationKind::Other)
        };
        refs.push(RefDecoration {
            name: name.to_string(),
            full_name: full.to_string(),
            kind,
            is_head,
        });
    }
    refs
}
