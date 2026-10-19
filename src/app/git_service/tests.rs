use super::*;

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
    assert_eq!(changes.staged_count, 2);
    assert_eq!(changes.untracked_count, 1);
    assert_eq!(
        changes.files[2].display_path,
        "src/app/views/inspector.rs → src/app/views/details.rs"
    );
    assert!(changes.files[0].staged);
    assert!(changes.files[1].untracked);
}

#[test]
fn operation_output_message_trims_and_names_the_operation() {
    assert_eq!(operation_output_message(GitOperation::Fetch, "  ".to_owned()), "fetch completed");
    assert_eq!(operation_output_message(GitOperation::Push, "done".to_owned()), "push completed: done");
}

