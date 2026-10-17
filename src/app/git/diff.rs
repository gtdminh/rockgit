use std::collections::HashMap;

use super::args::Args;
use super::client::{GitClient, safe_ref};
use super::error::{GitError, GitResult};
use super::types::{ChangeKind, FileChange};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffTarget {
    /// Work tree vs index (unstaged changes).
    Unstaged,
    /// Index vs HEAD (what the next commit will contain).
    Staged,
    /// A commit against its first parent (the empty tree for a root commit).
    Commit(String),
    /// Two arbitrary revisions.
    Between(String, String),
}

#[derive(Clone, Debug)]
pub struct DiffOptions {
    /// Lines of context around each hunk (git's default is 3).
    pub context: Option<u32>,
    pub ignore_whitespace: bool,
    pub detect_renames: bool,
    pub paths: Vec<String>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self { context: None, ignore_whitespace: false, detect_renames: true, paths: Vec::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    /// `\ No newline at end of file` marker.
    NoNewline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// Line text without the leading `+`/`-`/space marker.
    pub content: String,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// The full `@@ -a,b +c,d @@ section` line.
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDiff {
    /// `None` for added files.
    pub old_path: Option<String>,
    /// `None` for deleted files.
    pub new_path: Option<String>,
    pub binary: bool,
    /// Raw header lines (`diff --git`, `index`, `---`, `+++`...), needed to rebuild patches.
    pub header: Vec<String>,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    /// The path to show for this file.
    pub fn path(&self) -> &str {
        self.new_path.as_deref().or(self.old_path.as_deref()).unwrap_or("")
    }

    /// A standalone patch containing only hunk `index`; feed it to `apply_patch` to stage,
    /// unstage or discard just that hunk.
    pub fn hunk_patch(&self, index: usize) -> Option<String> {
        let hunk = self.hunks.get(index)?;
        let mut patch = self.header.join("\n");
        patch.push('\n');
        patch.push_str(&hunk.header);
        patch.push('\n');
        for l in &hunk.lines {
            patch.push(match l.kind {
                DiffLineKind::Context => ' ',
                DiffLineKind::Added => '+',
                DiffLineKind::Removed => '-',
                DiffLineKind::NoNewline => '\\',
            });
            patch.push_str(&l.content);
            patch.push('\n');
        }
        Some(patch)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ApplyOptions {
    /// Apply to the index instead of the work tree.
    pub cached: bool,
    /// Apply the patch in reverse (to unstage or discard).
    pub reverse: bool,
    /// Only verify that the patch applies.
    pub check_only: bool,
}

impl GitClient {
    pub fn diff(&self, target: &DiffTarget, o: &DiffOptions) -> GitResult<Vec<FileDiff>> {
        Ok(parse_diff(&self.diff_text(target, o)?))
    }

    /// The raw unified diff.
    pub fn diff_text(&self, target: &DiffTarget, o: &DiffOptions) -> GitResult<String> {
        let args = self.diff_args(target, o, &[])?;
        Ok(self.run(args)?.stdout_str())
    }

    /// Files changed with added/deleted line counts.
    pub fn changed_files(&self, target: &DiffTarget, o: &DiffOptions) -> GitResult<Vec<FileChange>> {
        let statuses = self.run(self.diff_args(target, o, &["--name-status", "-z"])?)?.stdout_str();
        let numstat = self.run(self.diff_args(target, o, &["--numstat", "-z"])?)?.stdout_str();
        let counts = parse_numstat(&numstat);

        let mut files = Vec::new();
        let mut it = statuses.split('\0').filter(|s| !s.is_empty());
        while let Some(code) = it.next() {
            let status = code.chars().next().map_or(ChangeKind::Unmodified, ChangeKind::from_code);
            let (path, old_path) = if matches!(status, ChangeKind::Renamed | ChangeKind::Copied) {
                let old = it.next().unwrap_or_default().to_string();
                (it.next().unwrap_or_default().to_string(), Some(old))
            } else {
                (it.next().unwrap_or_default().to_string(), None)
            };
            let (added, deleted) = counts.get(&path).copied().unwrap_or((None, None));
            files.push(FileChange { path, old_path, status, added, deleted });
        }
        Ok(files)
    }

    /// Applies a patch (e.g. from `FileDiff::hunk_patch`) to the index and/or work tree.
    pub fn apply_patch(&self, patch: &str, o: ApplyOptions) -> GitResult<()> {
        let args = Args::new("apply")
            .args(["--recount", "--whitespace=nowarn"])
            .flag(o.cached, "--cached")
            .flag(o.reverse, "--reverse")
            .flag(o.check_only, "--check");
        self.run_stdin(args, patch.as_bytes()).map(drop)
    }

    /// Contents of `path` at `rev` (`:` for the index version).
    pub fn file_at(&self, rev: &str, path: &str) -> GitResult<Vec<u8>> {
        if rev != ":" {
            safe_ref(rev)?;
        }
        let spec = if rev == ":" { format!(":{path}") } else { format!("{rev}:{path}") };
        Ok(self.run(Args::new("show").arg(spec))?.stdout)
    }

    /// Contents of a conflicted file's stage: 1 = common ancestor, 2 = ours, 3 = theirs.
    pub fn conflict_stage(&self, path: &str, stage: u8) -> GitResult<Option<Vec<u8>>> {
        if !(1..=3).contains(&stage) {
            return Err(GitError::InvalidInput("stage must be 1, 2 or 3".into()));
        }
        let out = self.run_raw(&Args::new("show").arg(format!(":{stage}:{path}")))?;
        Ok(out.success().then_some(out.stdout))
    }

    fn diff_args(&self, target: &DiffTarget, o: &DiffOptions, extra: &[&str]) -> GitResult<Args> {
        let mut args = Args::new("diff")
            .args(["--no-color", "--no-ext-diff"])
            .arg(if o.detect_renames { "-M" } else { "--no-renames" })
            .flag(o.ignore_whitespace, "-w")
            .kv("unified", o.context)
            .args(extra);
        args = match target {
            DiffTarget::Unstaged => args,
            DiffTarget::Staged => args.arg("--cached"),
            DiffTarget::Commit(id) => {
                safe_ref(id)?;
                let base = match self.parents_of(id)?.into_iter().next() {
                    Some(parent) => parent,
                    None => self.empty_tree()?,
                };
                args.arg(base).arg(id)
            },
            DiffTarget::Between(a, b) => {
                safe_ref(a)?;
                safe_ref(b)?;
                args.arg(a).arg(b)
            },
        };
        Ok(args.paths(&o.paths))
    }

    fn empty_tree(&self) -> GitResult<String> {
        let args = Args::new("hash-object").args(["-t", "tree", "--stdin"]);
        Ok(String::from_utf8_lossy(&self.run_stdin(args, b"")?.stdout).trim().to_string())
    }
}

fn parse_numstat(raw: &str) -> HashMap<String, (Option<u32>, Option<u32>)> {
    let mut map = HashMap::new();
    let mut it = raw.split('\0').filter(|s| !s.is_empty()).peekable();
    while let Some(entry) = it.next() {
        let mut f = entry.splitn(3, '\t');
        let (Some(a), Some(d), Some(path)) = (f.next(), f.next(), f.next()) else { continue };
        // Renames are emitted as `a\td\t\0old\0new`.
        let path = if path.is_empty() {
            it.next();
            it.next().unwrap_or_default().to_string()
        } else {
            path.to_string()
        };
        map.insert(path, (a.parse().ok(), d.parse().ok()));
    }
    map
}

fn strip_marker(s: &str) -> Option<String> {
    let s = s.split('\t').next().unwrap_or(s);
    if s == "/dev/null" {
        return None;
    }
    Some(s.strip_prefix("a/").or_else(|| s.strip_prefix("b/")).unwrap_or(s).to_string())
}

/// Parses a unified diff produced by `git diff` / `git show`.
pub(super) fn parse_diff(raw: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let (mut old_no, mut new_no) = (0u32, 0u32);

    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let (old, new) = split_diff_git(rest);
            files.push(FileDiff {
                old_path: old,
                new_path: new,
                binary: false,
                header: vec![line.to_string()],
                hunks: Vec::new(),
            });
            continue;
        }
        let Some(file) = files.last_mut() else { continue };

        if file.hunks.is_empty() && !line.starts_with("@@") {
            file.header.push(line.to_string());
            if let Some(p) = line.strip_prefix("--- ") {
                file.old_path = strip_marker(p);
            } else if let Some(p) = line.strip_prefix("+++ ") {
                file.new_path = strip_marker(p);
            } else if let Some(p) = line.strip_prefix("rename from ") {
                file.old_path = Some(p.to_string());
            } else if let Some(p) = line.strip_prefix("rename to ") {
                file.new_path = Some(p.to_string());
            } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
                file.binary = true;
            }
            continue;
        }

        if line.starts_with("@@") {
            if let Some(h) = parse_hunk_header(line) {
                old_no = h.old_start;
                new_no = h.new_start;
                file.hunks.push(h);
            }
            continue;
        }

        let Some(hunk) = file.hunks.last_mut() else { continue };
        let mut chars = line.chars();
        let marker = chars.next();
        let content = chars.as_str().to_string();
        let diff_line = match marker {
            Some('+') => {
                let l = DiffLine { kind: DiffLineKind::Added, content, old_no: None, new_no: Some(new_no) };
                new_no += 1;
                l
            },
            Some('-') => {
                let l = DiffLine { kind: DiffLineKind::Removed, content, old_no: Some(old_no), new_no: None };
                old_no += 1;
                l
            },
            Some('\\') => DiffLine { kind: DiffLineKind::NoNewline, content, old_no: None, new_no: None },
            _ => {
                let l = DiffLine {
                    kind: DiffLineKind::Context,
                    content,
                    old_no: Some(old_no),
                    new_no: Some(new_no),
                };
                old_no += 1;
                new_no += 1;
                l
            },
        };
        hunk.lines.push(diff_line);
    }
    files
}

/// Splits `a/<old> b/<new>`; paths may contain spaces, so try the identical-name case first.
fn split_diff_git(rest: &str) -> (Option<String>, Option<String>) {
    if let Some(len) = rest.len().checked_sub(5).filter(|n| n % 2 == 0).map(|n| n / 2) {
        let (a, b) = (&rest[..2 + len], &rest[3 + len..]);
        if a.starts_with("a/") && b.starts_with("b/") && a[2..] == b[2..] {
            return (Some(a[2..].to_string()), Some(b[2..].to_string()));
        }
    }
    match rest.split_once(" b/") {
        Some((a, b)) => (Some(a.strip_prefix("a/").unwrap_or(a).to_string()), Some(b.to_string())),
        None => (None, None),
    }
}

fn parse_hunk_header(line: &str) -> Option<Hunk> {
    let inner = line.strip_prefix("@@ -")?;
    let (ranges, _) = inner.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let range = |s: &str| -> Option<(u32, u32)> {
        match s.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((s.parse().ok()?, 1)),
        }
    };
    let (old_start, old_lines) = range(old)?;
    let (new_start, new_lines) = range(new)?;
    Some(Hunk {
        header: line.to_string(),
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines: Vec::new(),
    })
}
