use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::app::git::error::redact;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Temp directory removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("rockgit-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(std::fs::canonicalize(dir).unwrap())
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Ignores the developer's own git configuration so tests behave the same everywhere.
fn isolated() -> GitOptions {
    GitOptions {
        env: vec![
            ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ],
        ..Default::default()
    }
}

fn init_repo(tmp: &Tmp, name: &str) -> GitClient {
    let repo = GitClient::init(
        tmp.path(name),
        &InitOptions { initial_branch: Some("main".into()), ..Default::default() },
        isolated(),
    )
    .unwrap();
    repo.set_local_identity("Test User", "test@example.com").unwrap();
    repo.config_set(ConfigScope::Local, "commit.gpgsign", "false").unwrap();
    repo
}

fn write(repo: &GitClient, path: &str, content: &str) {
    let full = repo.root().join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, content).unwrap();
}

fn commit_file(repo: &GitClient, path: &str, content: &str, msg: &str) -> String {
    write(repo, path, content);
    repo.stage(&[path.to_string()]).unwrap();
    repo.commit(&CommitOptions { message: msg.into(), ..Default::default() }).unwrap()
}

fn paths(v: Vec<&StatusEntry>) -> Vec<&str> {
    v.into_iter().map(|e| e.path.as_str()).collect()
}

fn fake_commit(id: &str, parents: &[&str]) -> Commit {
    Commit {
        id: id.into(),
        short_id: id.into(),
        parents: parents.iter().map(|p| p.to_string()).collect(),
        author_name: String::new(),
        author_email: String::new(),
        author_time: 0,
        committer_name: String::new(),
        committer_email: String::new(),
        commit_time: 0,
        subject: String::new(),
        body: String::new(),
        refs: vec![],
    }
}

fn fake_commit_with_refs(id: &str, parents: &[&str], refs: Vec<RefDecoration>) -> Commit {
    let mut commit = fake_commit(id, parents);
    commit.refs = refs;
    commit
}

// ---- repository lifecycle ---------------------------------------------------------------

#[test]
fn open_rejects_non_repository_and_finds_root_from_subdir() {
    let tmp = Tmp::new();
    assert!(matches!(GitClient::open_with(&tmp.0, isolated()), Err(GitError::NotARepository(_))));

    let repo = init_repo(&tmp, "r");
    std::fs::create_dir_all(repo.root().join("a/b")).unwrap();
    let sub = GitClient::open_with(repo.root().join("a/b"), isolated()).unwrap();
    assert_eq!(sub.root(), repo.root());
    assert!(!sub.is_bare());
}

#[test]
fn git_version_is_reported() {
    assert!(GitClient::git_version(&isolated()).unwrap().chars().next().unwrap().is_ascii_digit());
}

#[test]
fn missing_git_binary_is_reported() {
    let opts = GitOptions { git_binary: "definitely-not-git".into(), ..Default::default() };
    assert!(matches!(GitClient::git_version(&opts), Err(GitError::GitNotFound(_))));
}

// ---- status / staging / commit ----------------------------------------------------------

#[test]
fn unborn_repo_status_stage_unstage_and_first_commit() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    assert!(!repo.has_commits().unwrap());
    assert!(repo.log(&LogOptions::default()).unwrap().is_empty());
    assert!(repo.head().unwrap().is_none());

    write(&repo, "a.txt", "one\n");
    let st = repo.status(&StatusOptions::default()).unwrap();
    assert_eq!(st.branch.as_deref(), Some("main"));
    assert_eq!(paths(st.untracked().collect()), ["a.txt"]);
    assert!(!st.is_clean());

    repo.stage(&["a.txt".into()]).unwrap();
    assert_eq!(paths(repo.status(&Default::default()).unwrap().staged().collect()), ["a.txt"]);

    repo.unstage(&["a.txt".into()]).unwrap();
    let st = repo.status(&Default::default()).unwrap();
    assert_eq!(st.staged().count(), 0);
    assert_eq!(st.untracked().count(), 1);

    repo.stage_all().unwrap();
    let id = repo.commit(&CommitOptions { message: "first\n\nbody text".into(), ..Default::default() }).unwrap();
    assert_eq!(repo.head().unwrap().as_deref(), Some(id.as_str()));
    assert!(repo.status(&Default::default()).unwrap().is_clean());
}

#[test]
fn status_reports_staged_unstaged_rename_and_conflict_free_state() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "keep.txt", "k\n", "keep");
    commit_file(&repo, "old.txt", "line1\nline2\nline3\nline4\n", "old");

    repo.move_path("old.txt", "new.txt").unwrap();
    write(&repo, "keep.txt", "changed\n");
    let st = repo.status(&Default::default()).unwrap();

    let renamed: Vec<_> = st.staged().filter(|e| e.kind == EntryKind::Renamed).collect();
    assert_eq!(renamed.len(), 1);
    assert_eq!(renamed[0].path, "new.txt");
    assert_eq!(renamed[0].orig_path.as_deref(), Some("old.txt"));
    assert_eq!(paths(st.unstaged().collect()), ["keep.txt"]);
    assert_eq!(repo.repo_state(), RepoState::Clean);
}

#[test]
fn commit_amend_reword_and_validation() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let first = commit_file(&repo, "a", "1\n", "original");

    let err = repo.commit(&CommitOptions::default()).unwrap_err();
    assert!(matches!(err, GitError::InvalidInput(_)));

    let reworded = repo.reword_last_commit("better message\n\nwith body").unwrap();
    assert_ne!(first, reworded);
    let c = &repo.log(&Default::default()).unwrap()[0];
    assert_eq!(c.subject, "better message");
    assert_eq!(c.body, "with body");

    write(&repo, "b", "2\n");
    repo.stage(&["b".into()]).unwrap();
    repo.commit(&CommitOptions { amend: true, ..Default::default() }).unwrap();
    let log = repo.log(&Default::default()).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].subject, "better message");
    assert_eq!(repo.commit_detail("HEAD").unwrap().files.len(), 2);
}

#[test]
fn discard_clean_remove_and_restore() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a.txt", "orig\n", "a");
    commit_file(&repo, "sub/b.txt", "b\n", "b");

    write(&repo, "a.txt", "edited\n");
    repo.discard_changes(&["a.txt".into()]).unwrap();
    assert_eq!(std::fs::read_to_string(repo.root().join("a.txt")).unwrap(), "orig\n");

    write(&repo, "a.txt", "edited again\n");
    write(&repo, "sub/b.txt", "edited\n");
    repo.discard_all_changes().unwrap();
    assert!(repo.status(&Default::default()).unwrap().is_clean());

    write(&repo, "junk.txt", "x");
    write(&repo, "dir/junk2.txt", "x");
    let preview = repo
        .clean(&CleanOptions { directories: true, dry_run: true, ..Default::default() })
        .unwrap();
    assert_eq!(preview.len(), 2);
    assert!(repo.root().join("junk.txt").exists());
    repo.clean(&CleanOptions { directories: true, ..Default::default() }).unwrap();
    assert!(!repo.root().join("junk.txt").exists());
    assert!(!repo.root().join("dir").exists());

    write(&repo, "a.txt", "scratch\n");
    repo.restore_from("HEAD", &["a.txt".into()]).unwrap();
    assert_eq!(std::fs::read_to_string(repo.root().join("a.txt")).unwrap(), "orig\n");

    repo.remove(&["a.txt".into()], false, false).unwrap();
    assert!(!repo.root().join("a.txt").exists());
    assert_eq!(repo.status(&Default::default()).unwrap().staged().count(), 1);
}

#[test]
fn ignored_files_are_detected() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    write(&repo, ".gitignore", "*.log\n");
    write(&repo, "x.log", "x");
    assert!(repo.is_ignored("x.log").unwrap());
    assert!(!repo.is_ignored("x.txt").unwrap());
    let st = repo.status(&StatusOptions { include_ignored: true, ..Default::default() }).unwrap();
    assert!(st.entries.iter().any(|e| e.kind == EntryKind::Ignored && e.path == "x.log"));
}

// ---- diff -------------------------------------------------------------------------------

fn numbered(n: u32, changes: &[(u32, &str)]) -> String {
    (1..=n)
        .map(|i| {
            changes
                .iter()
                .find(|(l, _)| *l == i)
                .map_or_else(|| format!("line {i}\n"), |(_, t)| format!("{t}\n"))
        })
        .collect()
}

#[test]
fn diff_parses_hunks_and_stages_single_hunk() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "f.txt", &numbered(30, &[]), "init");
    write(&repo, "f.txt", &numbered(30, &[(2, "EDIT TWO"), (28, "EDIT TWENTYEIGHT")]));

    let diffs = repo.diff(&DiffTarget::Unstaged, &Default::default()).unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].path(), "f.txt");
    assert_eq!(diffs[0].hunks.len(), 2);
    let removed: Vec<_> = diffs[0].hunks[0]
        .lines
        .iter()
        .filter(|l| l.kind == DiffLineKind::Removed)
        .collect();
    assert_eq!(removed[0].content, "line 2");
    assert_eq!(removed[0].old_no, Some(2));

    let patch = diffs[0].hunk_patch(0).unwrap();
    repo.apply_patch(&patch, ApplyOptions { cached: true, ..Default::default() }).unwrap();
    let staged = repo.diff(&DiffTarget::Staged, &Default::default()).unwrap();
    let unstaged = repo.diff(&DiffTarget::Unstaged, &Default::default()).unwrap();
    assert_eq!(staged[0].hunks.len(), 1);
    assert_eq!(unstaged[0].hunks.len(), 1);
    assert!(staged[0].hunks[0].lines.iter().any(|l| l.content == "EDIT TWO"));

    repo.apply_patch(&patch, ApplyOptions { cached: true, reverse: true, ..Default::default() }).unwrap();
    assert!(repo.diff(&DiffTarget::Staged, &Default::default()).unwrap().is_empty());

    let discard = repo.diff(&DiffTarget::Unstaged, &Default::default()).unwrap()[0].hunk_patch(0).unwrap();
    repo.apply_patch(&discard, ApplyOptions { reverse: true, ..Default::default() }).unwrap();
    let content = std::fs::read_to_string(repo.root().join("f.txt")).unwrap();
    assert!(content.contains("line 2\n") && content.contains("EDIT TWENTYEIGHT"));
}

#[test]
fn diff_handles_paths_with_spaces_new_deleted_and_binary_files() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "gone.txt", "bye\n", "base");
    std::fs::remove_file(repo.root().join("gone.txt")).unwrap();
    write(&repo, "my file.txt", "hello\n");
    std::fs::write(repo.root().join("bin.dat"), [0u8, 159, 146, 150, 0, 1]).unwrap();
    repo.stage_all().unwrap();

    let diffs = repo.diff(&DiffTarget::Staged, &Default::default()).unwrap();
    let by = |p: &str| diffs.iter().find(|d| d.path() == p).unwrap_or_else(|| panic!("missing {p}"));
    let spaced = by("my file.txt");
    assert_eq!(spaced.old_path, None);
    assert_eq!(spaced.hunks[0].lines[0].content, "hello");
    let gone = by("gone.txt");
    assert_eq!(gone.new_path, None);
    assert!(by("bin.dat").binary);

    let stats = repo.changed_files(&DiffTarget::Staged, &Default::default()).unwrap();
    let spaced = stats.iter().find(|f| f.path == "my file.txt").unwrap();
    assert_eq!((spaced.status, spaced.added, spaced.deleted), (ChangeKind::Added, Some(1), Some(0)));
    assert_eq!(stats.iter().find(|f| f.path == "bin.dat").unwrap().added, None);
}

#[test]
fn commit_detail_shows_renames_and_root_commit_files() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let root = commit_file(&repo, "a.txt", "1\n2\n3\n4\n5\n", "root");
    let detail = repo.commit_detail(&root).unwrap();
    assert_eq!(detail.files.len(), 1);
    assert_eq!(detail.files[0].status, ChangeKind::Added);

    repo.move_path("a.txt", "b.txt").unwrap();
    let id = repo.commit(&CommitOptions { message: "rename".into(), ..Default::default() }).unwrap();
    let f = &repo.commit_detail(&id).unwrap().files[0];
    assert_eq!((f.status, f.path.as_str(), f.old_path.as_deref()), (ChangeKind::Renamed, "b.txt", Some("a.txt")));

    assert_eq!(repo.file_at(&root, "a.txt").unwrap(), b"1\n2\n3\n4\n5\n");
    assert_eq!(repo.file_at(":", "b.txt").unwrap(), b"1\n2\n3\n4\n5\n");
}

// ---- history & graph --------------------------------------------------------------------

#[test]
fn log_filters_search_and_decorations() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let a = commit_file(&repo, "a", "1", "alpha feature");
    commit_file(&repo, "b", "2", "beta fix");
    commit_file(&repo, "a", "3", "gamma feature");
    repo.create_tag("v1", Some(&a), None, false).unwrap();

    let all = repo.log(&Default::default()).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].subject, "gamma feature");
    assert!(all[0].refs.iter().any(|r| r.name == "main" && r.kind == DecorationKind::LocalBranch && r.is_head));
    assert!(all[2].refs.iter().any(|r| r.name == "v1" && r.kind == DecorationKind::Tag));

    let feat = repo.log(&LogOptions { grep: Some("feature".into()), ..Default::default() }).unwrap();
    assert_eq!(feat.len(), 2);
    let limited = repo.log(&LogOptions { max_count: Some(1), skip: 1, ..Default::default() }).unwrap();
    assert_eq!(limited[0].subject, "beta fix");
    let by_path = repo.log(&LogOptions { paths: vec!["b".into()], ..Default::default() }).unwrap();
    assert_eq!(by_path.len(), 1);
    assert_eq!(repo.file_history("a", None).unwrap().len(), 2);
    assert_eq!(repo.rev_parse("v1").unwrap(), a);
    assert_eq!(repo.ahead_behind("main", "v1").unwrap(), (2, 0));
    assert!(repo.is_ancestor("v1", "main").unwrap());
    assert_eq!(repo.merge_base("v1", "main").unwrap().as_deref(), Some(a.as_str()));
}

#[test]
fn graph_for_real_merge_history() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "base", "0", "base");
    repo.create_branch("feature", None, true).unwrap();
    commit_file(&repo, "f", "1", "feature work");
    repo.switch_branch("main", false).unwrap();
    commit_file(&repo, "m", "1", "main work");
    repo.merge("feature", &MergeOptions { strategy: MergeStrategy::NoFastForward, ..Default::default() })
        .unwrap();

    let rows = repo.graph(&LogOptions { all: true, ..Default::default() }).unwrap();
    assert_eq!(rows.len(), 4);
    let merge = &rows[0];
    assert!(merge.commit.is_merge());
    assert_eq!(merge.outgoing.len(), 2);
    assert!(rows.iter().map(|r| r.width).max().unwrap() >= 2);
    let base = rows.last().unwrap();
    assert_eq!(base.commit.subject, "base");
    assert!(base.outgoing.is_empty());
}

#[test]
fn graph_layout_linear_history_uses_one_lane() {
    let rows = build_graph(vec![fake_commit("c", &["b"]), fake_commit("b", &["a"]), fake_commit("a", &[])]);
    assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1));
    assert!(rows[0].incoming.is_empty());
    assert_eq!(rows[1].incoming.len(), 1);
}

#[test]
fn graph_layout_branch_and_merge() {
    // M merges B (first parent) and F; both descend from A.
    let rows = build_graph(vec![
        fake_commit("M", &["B", "F"]),
        fake_commit("B", &["A"]),
        fake_commit("F", &["A"]),
        fake_commit("A", &[]),
    ]);
    let shape = |r: &GraphRow| -> Vec<(usize, usize)> { r.outgoing.iter().map(|e| (e.from, e.to)).collect() };

    assert_eq!(rows[0].lane, 0);
    assert_eq!(shape(&rows[0]), [(0, 0), (0, 1)]);
    assert_eq!(rows[1].lane, 0);
    assert_eq!(rows[1].passthrough.iter().map(|e| (e.from, e.to)).collect::<Vec<_>>(), [(1, 1)]);
    assert_eq!(rows[2].lane, 1);
    assert_eq!(shape(&rows[2]), [(1, 0)]);
    assert_eq!(rows[3].lane, 0);
    assert_eq!(rows[2].width, 2);
    assert_eq!(rows[3].width, 1);
    // The side branch keeps a different color than the main line.
    assert_ne!(rows[0].outgoing[0].color, rows[0].outgoing[1].color);
}

#[test]
fn graph_layout_keeps_main_branch_on_leftmost_lane() {
    let rows = build_graph(vec![
        fake_commit_with_refs(
            "F2",
            &["F1"],
            vec![RefDecoration {
                name: "feature/demo".into(),
                full_name: "refs/heads/feature/demo".into(),
                kind: DecorationKind::LocalBranch,
                is_head: true,
            }],
        ),
        fake_commit_with_refs(
            "M",
            &["B", "F1"],
            vec![RefDecoration {
                name: "main".into(),
                full_name: "refs/heads/main".into(),
                kind: DecorationKind::LocalBranch,
                is_head: false,
            }],
        ),
        fake_commit("B", &["A"]),
        fake_commit("F1", &["A"]),
        fake_commit("A", &[]),
    ]);

    let by_id = |id: &str| rows.iter().find(|row| row.commit.id == id).unwrap();

    assert_eq!(by_id("M").lane, 0);
    assert!(by_id("M").is_mainline);
    assert_eq!(by_id("B").lane, 0);
    assert!(by_id("B").is_mainline);
    assert_eq!(by_id("A").lane, 0);
    assert!(by_id("A").is_mainline);
    assert_ne!(by_id("F1").lane, 0);
    assert!(!by_id("F1").is_mainline);
}

#[test]
fn graph_layout_tolerates_truncated_history() {
    let rows = build_graph(vec![fake_commit("c", &["b"]), fake_commit("d", &["x"])]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].lane, 1);
}

// ---- branches / tags --------------------------------------------------------------------

#[test]
fn branch_lifecycle() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1", "one");

    repo.create_branch("dev", None, false).unwrap();
    repo.create_branch("topic", None, true).unwrap();
    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("topic"));

    let names: Vec<_> = repo.branches().unwrap().into_iter().map(|b| (b.name, b.is_current)).collect();
    assert!(names.contains(&("topic".to_string(), true)));
    assert!(names.contains(&("dev".to_string(), false)));

    repo.rename_branch("dev", "develop").unwrap();
    repo.switch_branch("main", false).unwrap();
    commit_file(&repo, "b", "2", "two");
    repo.switch_branch("topic", false).unwrap();
    repo.checkout_detached("main").unwrap();
    assert_eq!(repo.current_branch().unwrap(), None);
    assert!(repo.status(&Default::default()).unwrap().branch.is_none());
    repo.switch_branch("main", false).unwrap();

    commit_file(&repo, "c", "3", "three");
    repo.create_branch("unmerged", None, true).unwrap();
    commit_file(&repo, "d", "4", "four");
    repo.switch_branch("main", false).unwrap();
    assert!(repo.delete_branch("unmerged", false).is_err());
    repo.delete_branch("unmerged", true).unwrap();
    repo.delete_branch("develop", false).unwrap();
    assert!(repo.branches_containing("main").unwrap().contains(&"main".to_string()));
}

#[test]
fn switching_with_dirty_tree_fails_unless_discarding() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1\n", "one");
    repo.create_branch("other", None, true).unwrap();
    commit_file(&repo, "a", "2\n", "two");
    repo.switch_branch("main", false).unwrap();

    write(&repo, "a", "dirty\n");
    assert!(repo.switch_branch("other", false).is_err());
    repo.switch_branch("other", true).unwrap();
    assert_eq!(std::fs::read_to_string(repo.root().join("a")).unwrap(), "2\n");
}

#[test]
fn tags_lightweight_annotated_and_delete() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let id = commit_file(&repo, "a", "1", "one");
    repo.create_tag("light", None, None, false).unwrap();
    repo.create_tag("rel-1.0", None, Some("Release 1.0\n\nnotes"), false).unwrap();
    assert!(repo.create_tag("light", None, None, false).is_err());
    repo.create_tag("light", None, None, true).unwrap();

    let tags = repo.tags().unwrap();
    let light = tags.iter().find(|t| t.name == "light").unwrap();
    let ann = tags.iter().find(|t| t.name == "rel-1.0").unwrap();
    assert!(!light.annotated && ann.annotated);
    assert_eq!(light.target, id);
    assert_eq!(ann.target, id);
    assert_eq!(ann.subject, "Release 1.0");

    repo.delete_tag("light").unwrap();
    assert_eq!(repo.tags().unwrap().len(), 1);
}

// ---- merge / rebase / cherry-pick / revert / reset --------------------------------------

fn diverge_with_conflict(tmp: &Tmp) -> GitClient {
    let repo = init_repo(tmp, "r");
    commit_file(&repo, "f.txt", "base\n", "base");
    repo.create_branch("other", None, true).unwrap();
    commit_file(&repo, "f.txt", "theirs\n", "other change");
    repo.switch_branch("main", false).unwrap();
    commit_file(&repo, "f.txt", "ours\n", "main change");
    repo
}

#[test]
fn merge_conflict_resolution_and_continue() {
    let tmp = Tmp::new();
    let repo = diverge_with_conflict(&tmp);

    let outcome = repo.merge("other", &Default::default()).unwrap();
    let OpOutcome::Conflict { files, .. } = outcome else { panic!("expected conflict") };
    assert_eq!(files, ["f.txt"]);
    assert_eq!(repo.repo_state(), RepoState::Merging);
    assert_eq!(repo.status(&Default::default()).unwrap().conflicted().count(), 1);
    assert_eq!(repo.conflict_stage("f.txt", 2).unwrap().unwrap(), b"ours\n");
    assert_eq!(repo.conflict_stage("f.txt", 3).unwrap().unwrap(), b"theirs\n");

    repo.resolve_conflict("f.txt", ConflictSide::Theirs).unwrap();
    assert!(repo.merge_continue().unwrap().is_success());
    assert_eq!(repo.repo_state(), RepoState::Clean);
    assert_eq!(std::fs::read_to_string(repo.root().join("f.txt")).unwrap(), "theirs\n");
    assert!(repo.log(&Default::default()).unwrap()[0].is_merge());
}

#[test]
fn merge_abort_restores_state() {
    let tmp = Tmp::new();
    let repo = diverge_with_conflict(&tmp);
    assert!(!repo.merge("other", &Default::default()).unwrap().is_success());
    repo.merge_abort().unwrap();
    assert_eq!(repo.repo_state(), RepoState::Clean);
    assert_eq!(std::fs::read_to_string(repo.root().join("f.txt")).unwrap(), "ours\n");
}

#[test]
fn fast_forward_only_and_squash_merge() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1", "base");
    repo.create_branch("feat", None, true).unwrap();
    commit_file(&repo, "b", "2", "feat 1");
    commit_file(&repo, "c", "3", "feat 2");
    repo.switch_branch("main", false).unwrap();

    repo.merge("feat", &MergeOptions { strategy: MergeStrategy::Squash, ..Default::default() }).unwrap();
    assert_eq!(repo.status(&Default::default()).unwrap().staged().count(), 2);
    repo.reset("HEAD", ResetMode::Hard).unwrap();

    repo.merge("feat", &MergeOptions { strategy: MergeStrategy::FastForwardOnly, ..Default::default() }).unwrap();
    assert_eq!(repo.log(&Default::default()).unwrap().len(), 3);
}

#[test]
fn rebase_with_conflict_then_abort_and_continue() {
    let tmp = Tmp::new();
    let repo = diverge_with_conflict(&tmp);
    repo.switch_branch("other", false).unwrap();

    let outcome = repo.rebase(&RebaseOptions { upstream: "main".into(), ..Default::default() }).unwrap();
    assert!(matches!(outcome, OpOutcome::Conflict { .. }));
    assert_eq!(repo.repo_state(), RepoState::Rebasing);
    repo.rebase_abort().unwrap();
    assert_eq!(repo.repo_state(), RepoState::Clean);

    repo.rebase(&RebaseOptions { upstream: "main".into(), ..Default::default() }).unwrap();
    repo.resolve_conflict("f.txt", ConflictSide::Theirs).unwrap();
    assert!(repo.rebase_continue().unwrap().is_success());
    assert_eq!(repo.repo_state(), RepoState::Clean);
    assert_eq!(repo.log(&Default::default()).unwrap().len(), 3);
}

#[test]
fn interactive_rebase_squashes_reorders_and_drops() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "base", "0", "base");
    commit_file(&repo, "a", "1", "add a");
    commit_file(&repo, "b", "2", "add b");
    commit_file(&repo, "c", "3", "fixup a");
    commit_file(&repo, "d", "4", "drop me");

    let mut todo = repo.rebase_todo("HEAD~4").unwrap();
    assert_eq!(todo.len(), 4);
    let (fix, drop_me) = (todo[2].clone(), todo[3].clone());
    todo[2] = RebaseStep { action: RebaseAction::Fixup, commit: fix.commit.clone() };
    todo[3] = RebaseStep { action: RebaseAction::Drop, commit: drop_me.commit };
    // Move the fixup right after `add a` so it melds into it.
    let moved = todo.remove(2);
    todo.insert(1, moved);

    assert!(repo.rebase_interactive("HEAD~4", &todo).unwrap().is_success());
    let subjects: Vec<_> = repo.log(&Default::default()).unwrap().into_iter().map(|c| c.subject).collect();
    assert_eq!(subjects, ["add b", "add a", "base"]);
    assert!(!repo.root().join("d").exists());
    assert!(repo.root().join("c").exists());
}

#[test]
fn cherry_pick_and_revert() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1", "base");
    repo.create_branch("side", None, true).unwrap();
    let picked = commit_file(&repo, "s", "side", "side commit");
    repo.switch_branch("main", false).unwrap();

    assert!(repo.cherry_pick(&[picked], false, None).unwrap().is_success());
    assert!(repo.root().join("s").exists());
    assert!(repo.cherry_pick(&[], false, None).is_err());

    assert!(repo.revert(&["HEAD".into()], false, None).unwrap().is_success());
    assert!(!repo.root().join("s").exists());
    let log = repo.log(&Default::default()).unwrap();
    assert!(log[0].subject.starts_with("Revert"));
}

#[test]
fn cherry_pick_conflict_can_be_aborted() {
    let tmp = Tmp::new();
    let repo = diverge_with_conflict(&tmp);
    let other = repo.rev_parse("other").unwrap();
    let outcome = repo.cherry_pick(&[other], false, None).unwrap();
    assert!(matches!(outcome, OpOutcome::Conflict { .. }));
    assert_eq!(repo.repo_state(), RepoState::CherryPicking);
    repo.cherry_pick_abort().unwrap();
    assert_eq!(repo.repo_state(), RepoState::Clean);
}

#[test]
fn reset_modes_and_undo_last_commit() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let first = commit_file(&repo, "a", "1", "one");
    commit_file(&repo, "b", "2", "two");

    repo.undo_last_commit().unwrap();
    assert_eq!(repo.head().unwrap().as_deref(), Some(first.as_str()));
    assert_eq!(paths(repo.status(&Default::default()).unwrap().staged().collect()), ["b"]);

    repo.reset("HEAD", ResetMode::Mixed).unwrap();
    assert_eq!(repo.status(&Default::default()).unwrap().staged().count(), 0);
    assert_eq!(repo.status(&Default::default()).unwrap().untracked().count(), 1);

    repo.stage_all().unwrap();
    repo.reset("HEAD", ResetMode::Hard).unwrap();
    assert!(!repo.root().join("b").exists());
}

// ---- stash ------------------------------------------------------------------------------

#[test]
fn stash_push_list_apply_pop_drop() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1\n", "one");

    assert!(!repo.stash_push(&Default::default()).unwrap());

    write(&repo, "a", "changed\n");
    write(&repo, "new.txt", "untracked\n");
    assert!(repo
        .stash_push(&StashOptions { message: Some("wip".into()), include_untracked: true, ..Default::default() })
        .unwrap());
    assert!(repo.status(&Default::default()).unwrap().is_clean());

    let stashes = repo.stash_list().unwrap();
    assert_eq!(stashes.len(), 1);
    assert!(stashes[0].message.contains("wip"));
    let files = repo
        .changed_files(&DiffTarget::Commit(GitClient::stash_rev(0)), &Default::default())
        .unwrap();
    assert_eq!(files.len(), 1);

    assert!(repo.stash_apply(0, false).unwrap().is_success());
    assert_eq!(repo.stash_list().unwrap().len(), 1);
    repo.reset("HEAD", ResetMode::Hard).unwrap();
    repo.clean(&CleanOptions { directories: true, ..Default::default() }).unwrap();

    assert!(repo.stash_pop(0, false).unwrap().is_success());
    assert!(repo.stash_list().unwrap().is_empty());
    assert_eq!(std::fs::read_to_string(repo.root().join("a")).unwrap(), "changed\n");

    repo.stash_push(&Default::default()).unwrap();
    repo.stash_drop(0).unwrap();
    assert!(repo.stash_list().unwrap().is_empty());
}

// ---- remotes ----------------------------------------------------------------------------

fn bare_remote(tmp: &Tmp, name: &str) -> PathBuf {
    let path = tmp.path(name);
    GitClient::init(
        &path,
        &InitOptions { bare: true, initial_branch: Some("main".into()) },
        isolated(),
    )
    .unwrap();
    path
}

fn clone_of(remote: &Path, dest: PathBuf) -> GitClient {
    let repo = GitClient::clone_repo(remote.to_str().unwrap(), dest, &CloneOptions::default(), isolated()).unwrap();
    repo.set_local_identity("Other Dev", "other@example.com").unwrap();
    repo.config_set(ConfigScope::Local, "commit.gpgsign", "false").unwrap();
    repo
}

#[test]
fn push_fetch_pull_between_clones() {
    let tmp = Tmp::new();
    let remote = bare_remote(&tmp, "origin.git");
    let alice = init_repo(&tmp, "alice");
    alice.add_remote("origin", remote.to_str().unwrap()).unwrap();
    commit_file(&alice, "a", "1", "first");
    alice
        .push(&PushOptions {
            remote: Some("origin".into()),
            refspec: Some("main".into()),
            set_upstream: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(alice.status(&Default::default()).unwrap().upstream.as_deref(), Some("origin/main"));

    let bob = clone_of(&remote, tmp.path("bob"));
    assert_eq!(bob.log(&Default::default()).unwrap().len(), 1);

    commit_file(&alice, "a", "2", "second");
    alice.push(&PushOptions::default()).unwrap();

    bob.fetch(&FetchOptions { all: true, prune: true, ..Default::default() }).unwrap();
    let st = bob.status(&Default::default()).unwrap();
    assert_eq!((st.ahead, st.behind), (0, 1));
    let b = bob.branches().unwrap().into_iter().find(|b| b.name == "main").unwrap();
    assert_eq!((b.ahead, b.behind), (0, 1));

    assert!(bob.pull(&PullOptions { strategy: PullStrategy::FastForwardOnly, ..Default::default() }).unwrap().is_success());
    assert_eq!(bob.log(&Default::default()).unwrap().len(), 2);

    let remotes = bob.remotes().unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].name, "origin");
    assert_eq!(remotes[0].push_url, remote.to_str().unwrap());
    let refs = bob.ls_remote("origin").unwrap();
    assert!(refs.iter().any(|r| r.name == "refs/heads/main"));
}

#[test]
fn rejected_push_pull_rebase_and_force_with_lease() {
    let tmp = Tmp::new();
    let remote = bare_remote(&tmp, "origin.git");
    let alice = init_repo(&tmp, "alice");
    alice.add_remote("origin", remote.to_str().unwrap()).unwrap();
    commit_file(&alice, "a", "1\n", "first");
    let push = PushOptions {
        remote: Some("origin".into()),
        refspec: Some("main".into()),
        set_upstream: true,
        ..Default::default()
    };
    alice.push(&push).unwrap();

    let bob = clone_of(&remote, tmp.path("bob"));
    commit_file(&alice, "alice.txt", "A\n", "alice work");
    alice.push(&PushOptions::default()).unwrap();
    commit_file(&bob, "bob.txt", "B\n", "bob work");

    assert!(bob.push(&PushOptions::default()).is_err());
    assert!(bob.pull(&PullOptions { strategy: PullStrategy::Rebase, ..Default::default() }).unwrap().is_success());
    bob.push(&PushOptions::default()).unwrap();

    // Alice's tracking info is stale, so a lease-protected force push must be refused.
    commit_file(&alice, "alice2.txt", "A2\n", "alice rewrite");
    alice.reset("HEAD~1", ResetMode::Hard).unwrap();
    commit_file(&alice, "alice3.txt", "A3\n", "diverged");
    assert!(alice.push(&PushOptions { force_with_lease: true, ..Default::default() }).is_err());
}

#[test]
fn pull_with_divergence_reports_merge_conflict() {
    let tmp = Tmp::new();
    let remote = bare_remote(&tmp, "origin.git");
    let alice = init_repo(&tmp, "alice");
    alice.add_remote("origin", remote.to_str().unwrap()).unwrap();
    commit_file(&alice, "f", "base\n", "base");
    alice
        .push(&PushOptions { remote: Some("origin".into()), refspec: Some("main".into()), set_upstream: true, ..Default::default() })
        .unwrap();
    let bob = clone_of(&remote, tmp.path("bob"));

    commit_file(&alice, "f", "alice\n", "alice");
    alice.push(&PushOptions::default()).unwrap();
    commit_file(&bob, "f", "bob\n", "bob");

    let outcome = bob.pull(&PullOptions { strategy: PullStrategy::Merge, ..Default::default() }).unwrap();
    assert!(matches!(outcome, OpOutcome::Conflict { ref files, .. } if files == &["f"]));
    bob.merge_abort().unwrap();
}

#[test]
fn remote_management_and_branch_tracking() {
    let tmp = Tmp::new();
    let remote = bare_remote(&tmp, "origin.git");
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1", "one");
    repo.add_remote("origin", remote.to_str().unwrap()).unwrap();
    repo.rename_remote("origin", "up").unwrap();
    repo.set_remote_url("up", "https://example.com/team/repo.git", true).unwrap();
    let r = &repo.remotes().unwrap()[0];
    assert_eq!(r.name, "up");
    assert_eq!(r.push_url, "https://example.com/team/repo.git");
    assert_eq!(r.fetch_url, remote.to_str().unwrap());
    repo.set_remote_url("up", remote.to_str().unwrap(), true).unwrap();

    repo.create_branch("topic", None, true).unwrap();
    repo.push(&PushOptions {
        remote: Some("up".into()),
        refspec: Some("topic".into()),
        set_upstream: true,
        ..Default::default()
    })
    .unwrap();
    assert!(repo.branches().unwrap().iter().any(|b| b.name == "topic" && b.upstream.as_deref() == Some("up/topic")));
    assert!(repo.branches().unwrap().iter().any(|b| b.name == "up/topic" && b.is_remote));

    repo.unset_upstream("topic").unwrap();
    repo.set_upstream("topic", "up/topic").unwrap();
    repo.switch_branch("main", false).unwrap();
    repo.delete_branch("topic", true).unwrap();
    repo.checkout_remote_branch("up/topic", None).unwrap();
    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("topic"));

    repo.switch_branch("main", false).unwrap();
    repo.delete_remote_branch("up", "topic").unwrap();
    repo.fetch(&FetchOptions { remote: Some("up".into()), prune: true, ..Default::default() }).unwrap();
    assert!(!repo.branches().unwrap().iter().any(|b| b.name == "up/topic"));

    repo.create_tag("rel", None, Some("release"), false).unwrap();
    repo.push_tag("up", "rel").unwrap();
    assert!(repo.ls_remote("up").unwrap().iter().any(|r| r.name == "refs/tags/rel"));
    repo.delete_remote_tag("up", "rel").unwrap();
    assert!(!repo.ls_remote("up").unwrap().iter().any(|r| r.name == "refs/tags/rel"));
    repo.remove_remote("up").unwrap();
    assert!(repo.remotes().unwrap().is_empty());
}

#[test]
fn clone_options_branch_depth_and_bare() {
    let tmp = Tmp::new();
    let src = init_repo(&tmp, "src");
    commit_file(&src, "a", "1", "one");
    commit_file(&src, "a", "2", "two");
    src.create_branch("dev", None, true).unwrap();
    commit_file(&src, "d", "x", "dev commit");
    let url = format!("file://{}", src.root().display());

    let shallow = GitClient::clone_repo(
        &url,
        tmp.path("shallow"),
        &CloneOptions { branch: Some("dev".into()), depth: Some(1), ..Default::default() },
        isolated(),
    )
    .unwrap();
    assert_eq!(shallow.current_branch().unwrap().as_deref(), Some("dev"));
    assert_eq!(shallow.log(&Default::default()).unwrap().len(), 1);

    let bare = GitClient::clone_repo(&url, tmp.path("bare.git"), &CloneOptions { bare: true, ..Default::default() }, isolated()).unwrap();
    assert!(bare.is_bare());
    assert!(bare.log(&LogOptions { all: true, ..Default::default() }).unwrap().len() >= 3);
}

// ---- misc -------------------------------------------------------------------------------

#[test]
fn blame_reports_author_per_line() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "f", "one\ntwo\n", "first");
    let second = commit_file(&repo, "f", "one\ntwo\nthree\n", "second");

    let lines = repo.blame("f", None).unwrap();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].summary, "first");
    assert_eq!(lines[1].summary, "first");
    assert_eq!(lines[2].commit, second);
    assert_eq!((lines[2].line, lines[2].content.as_str(), lines[2].author.as_str()), (3, "three", "Test User"));
}

#[test]
fn reflog_records_head_movements() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    assert!(repo.reflog(10, None).unwrap().is_empty());
    let first = commit_file(&repo, "a", "1", "one");
    commit_file(&repo, "b", "2", "two");
    repo.reset("HEAD~1", ResetMode::Hard).unwrap();

    let log = repo.reflog(10, None).unwrap();
    assert_eq!(log.len(), 3);
    assert!(log[0].message.contains("reset"));
    assert_eq!(log[0].id, first);
    assert_eq!(log[0].selector, "HEAD@{0}");
}

#[test]
fn config_identity_roundtrip() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    assert_eq!(repo.config_get("custom.missing").unwrap(), None);
    repo.config_set(ConfigScope::Local, "custom.key", "value with spaces").unwrap();
    assert_eq!(repo.config_get("custom.key").unwrap().as_deref(), Some("value with spaces"));
    repo.config_unset(ConfigScope::Local, "custom.key").unwrap();
    assert_eq!(repo.config_get("custom.key").unwrap(), None);
    assert_eq!(repo.user_identity().unwrap(), (Some("Test User".into()), Some("test@example.com".into())));
    assert!(repo.set_local_identity("", "x").is_err());
}

#[test]
fn worktrees_add_list_remove() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1", "one");
    let wt = tmp.path("wt");
    repo.add_worktree(&wt, None, Some("wt-branch")).unwrap();

    let list = repo.worktrees().unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().any(|w| w.branch.as_deref() == Some("wt-branch")));
    let linked = GitClient::open_with(&wt, isolated()).unwrap();
    assert_eq!(linked.current_branch().unwrap().as_deref(), Some("wt-branch"));

    repo.remove_worktree(&wt, false).unwrap();
    assert_eq!(repo.worktrees().unwrap().len(), 1);
}

#[test]
fn submodules_list_and_update() {
    let tmp = Tmp::new();
    let lib = init_repo(&tmp, "lib");
    commit_file(&lib, "lib.txt", "lib\n", "lib");

    let app = init_repo(&tmp, "app");
    app.config_set(ConfigScope::Local, "protocol.file.allow", "always").unwrap();
    let url = lib.root().to_str().unwrap().to_string();
    app.run(super::args::Args::new("submodule").args(["add", "--", &url, "vendor/lib"])).unwrap();
    app.commit(&CommitOptions { message: "add submodule".into(), ..Default::default() }).unwrap();

    let subs = app.submodules().unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].path, "vendor/lib");
    assert_eq!(subs[0].state, SubmoduleState::UpToDate);
    app.update_submodules(true).unwrap();
}

// ---- safety & robustness ----------------------------------------------------------------

#[test]
fn option_like_input_is_rejected_before_reaching_git() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1", "one");

    for bad in ["--upload-pack=touch /tmp/pwned", "-D", ""] {
        assert!(matches!(repo.create_branch(bad, None, false), Err(GitError::InvalidInput(_))), "{bad}");
        assert!(matches!(repo.switch_branch(bad, false), Err(GitError::InvalidInput(_))), "{bad}");
        assert!(matches!(repo.add_remote("x", bad), Err(GitError::InvalidInput(_))), "{bad}");
        assert!(matches!(repo.ls_remote(bad), Err(GitError::InvalidInput(_))), "{bad}");
        assert!(matches!(repo.rev_parse(bad), Err(GitError::InvalidInput(_))), "{bad}");
        assert!(matches!(
            repo.log(&LogOptions { revs: vec![bad.into()], ..Default::default() }),
            Err(GitError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        GitClient::clone_repo("--upload-pack=x", tmp.path("c"), &Default::default(), isolated()),
        Err(GitError::InvalidInput(_))
    ));
    assert!(matches!(repo.stage(&[]), Err(GitError::InvalidInput(_))));
}

#[test]
fn paths_starting_with_dash_are_treated_as_paths() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    write(&repo, "-rf", "x");
    repo.stage(&["-rf".into()]).unwrap();
    assert_eq!(paths(repo.status(&Default::default()).unwrap().staged().collect()), ["-rf"]);
}

#[test]
fn unicode_and_special_filenames_round_trip() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    for name in ["héllo wörld.txt", "日本語.md", "with\ttab.txt", "quote\"d.txt"] {
        write(&repo, name, "x");
    }
    let st = repo.status(&Default::default()).unwrap();
    let mut got: Vec<_> = st.untracked().map(|e| e.path.clone()).collect();
    got.sort();
    assert_eq!(got.len(), 4);
    assert!(got.contains(&"héllo wörld.txt".to_string()));
    repo.stage_all().unwrap();
    assert_eq!(repo.status(&Default::default()).unwrap().staged().count(), 4);
}

#[test]
fn commit_messages_with_special_characters_are_preserved() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    write(&repo, "a", "1");
    repo.stage_all().unwrap();
    let msg = "subject: \"quoted\" $(rm -rf) `x` ünï\n\nbody line 1\n  indented\n\n- bullet";
    repo.commit(&CommitOptions { message: msg.into(), ..Default::default() }).unwrap();
    let c = &repo.log(&Default::default()).unwrap()[0];
    assert_eq!(c.subject, "subject: \"quoted\" $(rm -rf) `x` ünï");
    assert_eq!(c.body, "body line 1\n  indented\n\n- bullet");
}

#[test]
fn credentials_are_redacted_from_errors() {
    assert_eq!(redact("git clone https://user:s3cret@host.com/r.git x"), "git clone https://***@host.com/r.git x");
    assert_eq!(redact("ssh://git@host/r and https://a:b@h2/x"), "ssh://***@host/r and https://***@h2/x");
    assert_eq!(redact("no urls here"), "no urls here");

    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let err = repo.ls_remote("https://user:s3cret@127.0.0.1:9/nope.git").unwrap_err();
    let text = err.to_string();
    assert!(!text.contains("s3cret"), "leaked secret: {text}");
}

#[test]
fn auth_failures_are_classified() {
    let e = |s: &str| GitError::Command { command: String::new(), code: Some(128), stderr: s.into() };
    assert!(e("git@host: Permission denied (publickey).").is_auth_failure());
    assert!(e("fatal: could not read Username for 'https://x': terminal prompts disabled").is_auth_failure());
    assert!(e("remote: HTTP Basic: Access denied").is_auth_failure());
    assert!(!e("fatal: repository not found").is_auth_failure());
    assert!(!GitError::Parse(String::new()).is_auth_failure());
}

#[test]
fn https_without_credentials_fails_fast_instead_of_prompting() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let start = Instant::now();
    // Nothing listens on port 9, so this must fail promptly and never block on a prompt.
    assert!(repo.ls_remote("https://127.0.0.1:9/private.git").is_err());
    assert!(start.elapsed() < Duration::from_secs(20));
}

#[test]
fn hung_commands_time_out() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    let opts = GitOptions {
        timeout: Some(Duration::from_millis(400)),
        ssh_command: Some("sh -c 'sleep 5' --".into()),
        ..isolated()
    };
    let repo = repo.with_options(opts);
    let start = Instant::now();
    let err = repo.ls_remote("ssh://localhost/repo.git").unwrap_err();
    assert!(matches!(err, GitError::Timeout { .. }), "{err}");
    assert!(start.elapsed() < Duration::from_secs(4));
}

#[test]
fn concurrent_use_from_threads() {
    let tmp = Tmp::new();
    let repo = init_repo(&tmp, "r");
    commit_file(&repo, "a", "1", "one");
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let r = repo.clone();
            std::thread::spawn(move || r.status(&Default::default()).unwrap().is_clean())
        })
        .collect();
    assert!(handles.into_iter().all(|h| h.join().unwrap()));
}
