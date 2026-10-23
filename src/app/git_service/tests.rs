use super::*;

#[test]
fn load_repo_excludes_commits_reachable_only_from_agent_refs() {
    let root = std::env::temp_dir().join(format!(
        "rockgit-agent-ref-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let run_git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };

    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.name", "Test User"]);
    run_git(&["config", "user.email", "test@example.com"]);
    std::fs::write(root.join("base.txt"), "base").unwrap();
    run_git(&["add", "base.txt"]);
    run_git(&["commit", "-m", "branch commit"]);
    let branch_tip = run_git(&["rev-parse", "HEAD"]);
    run_git(&["branch", "feature/onboarding"]);

    std::fs::write(root.join("checkpoint.txt"), "checkpoint").unwrap();
    run_git(&["add", "checkpoint.txt"]);
    run_git(&["commit", "-m", "Agent host session checkpoint"]);
    let checkpoint = run_git(&["rev-parse", "HEAD"]);
    run_git(&["reset", "--hard", &branch_tip]);
    run_git(&[
        "update-ref",
        "refs/agents/test-session/checkpoints/turn/1",
        &checkpoint,
    ]);

    let (repo, _) = load_repo(&root).unwrap();
    let subjects: Vec<&str> = repo
        .commits
        .iter()
        .map(|commit| commit.summary.as_str())
        .collect();
    assert_eq!(subjects, ["branch commit"]);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn working_tree_changes_ignores_ignored_entries_and_formats_renames() {
    let status = git::RepoStatus {
        entries: vec![
            git::StatusEntry {
                path: "src/app/details.rs".to_owned(),
                orig_path: None,
                kind: git::EntryKind::Ordinary,
                index: git::ChangeKind::Modified,
                worktree: git::ChangeKind::Unmodified,
                code: "M ".to_owned(),
            },
            git::StatusEntry {
                path: "src/app/views/settings.rs".to_owned(),
                orig_path: None,
                kind: git::EntryKind::Untracked,
                index: git::ChangeKind::Unmodified,
                worktree: git::ChangeKind::Untracked,
                code: "??".to_owned(),
            },
            git::StatusEntry {
                path: "src/app/views/details.rs".to_owned(),
                orig_path: Some("src/app/views/inspector.rs".to_owned()),
                kind: git::EntryKind::Renamed,
                index: git::ChangeKind::Renamed,
                worktree: git::ChangeKind::Unmodified,
                code: "R ".to_owned(),
            },
            git::StatusEntry {
                path: "target/tmp".to_owned(),
                orig_path: None,
                kind: git::EntryKind::Ignored,
                index: git::ChangeKind::Unmodified,
                worktree: git::ChangeKind::Ignored,
                code: "!!".to_owned(),
            },
        ],
        ..Default::default()
    };

    let changes = working_tree_changes(&status);

    assert_eq!(changes.files.len(), 3);
    assert_eq!(changes.untracked_count, 1);
    assert_eq!(
        changes.files[2].display_path,
        "src/app/views/inspector.rs → src/app/views/details.rs"
    );
    assert_eq!(changes.files[0].status, "M ");
    assert!(changes.files[1].untracked);
}

#[test]
fn operation_output_message_trims_and_names_the_operation() {
    assert_eq!(operation_output_message(GitOperation::Fetch, "  ".to_owned()), "fetch completed");
    assert_eq!(operation_output_message(GitOperation::Push, "done".to_owned()), "push completed: done");
}

