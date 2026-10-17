use crate::app::model::{Branch, CommitNode, FileChange, RefBadge, RefKind, Repository};

pub fn sample_repositories() -> Vec<Repository> {
    vec![
        Repository {
            name: "RockGit".to_owned(),
            path: "~/GTM_Projects/RockGit".to_owned(),
            branch_count: 8,
            has_uncommitted_changes: true,
        },
        Repository {
            name: "workspace-tools".to_owned(),
            path: "~/src/workspace-tools".to_owned(),
            branch_count: 12,
            has_uncommitted_changes: false,
        },
        Repository {
            name: "infra-config".to_owned(),
            path: "~/src/infra-config".to_owned(),
            branch_count: 8,
            has_uncommitted_changes: true,
        },
        Repository {
            name: "design-system".to_owned(),
            path: "~/src/design-system".to_owned(),
            branch_count: 18,
            has_uncommitted_changes: false,
        },
    ]
}

fn branch(name: &str, ahead: u8, behind: u8, is_remote: bool) -> Branch {
    Branch {
        name: name.to_owned(),
        ahead,
        behind,
        is_remote,
    }
}

pub fn sample_branches() -> Vec<Branch> {
    vec![
        branch("main", 0, 0, false),
        branch("feature/graph-ui", 3, 1, false),
        branch("feature/search", 2, 0, false),
        branch("hotfix/empty-repo", 1, 0, false),
        branch("release/v0.2", 1, 0, false),
        branch("origin/main", 0, 0, true),
        branch("origin/feature/search", 0, 1, true),
        branch("origin/release/v0.2", 0, 2, true),
    ]
}

fn head() -> RefBadge {
    RefBadge {
        label: "HEAD".to_owned(),
        kind: RefKind::Head,
    }
}

fn local(label: &str) -> RefBadge {
    RefBadge {
        label: label.to_owned(),
        kind: RefKind::LocalBranch,
    }
}

fn remote(label: &str) -> RefBadge {
    RefBadge {
        label: label.to_owned(),
        kind: RefKind::RemoteBranch,
    }
}

fn tag(label: &str) -> RefBadge {
    RefBadge {
        label: label.to_owned(),
        kind: RefKind::Tag,
    }
}

fn file(status: &str, path: &str) -> FileChange {
    FileChange {
        path: path.to_owned(),
        status: status.to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
fn commit(
    short_id: &str,
    summary: &str,
    author: &str,
    relative_time: &str,
    lane: usize,
    parents: Vec<usize>,
    refs: Vec<RefBadge>,
    files: Vec<FileChange>,
) -> CommitNode {
    let is_head = refs.iter().any(|badge| badge.kind == RefKind::Head);
    CommitNode {
        id: String::new(),
        short_id: short_id.to_owned(),
        summary: summary.to_owned(),
        author: author.to_owned(),
        relative_time: relative_time.to_owned(),
        lane,
        parents,
        refs,
        files,
        is_head,
    }
}

/// Newest-first history. `parents` are indexes into this list.
///
/// Lanes: 0 = main, 1 = feature/search (and later release/v0.2),
/// 2 = hotfix/empty-repo and feature/graph-ui.
pub fn sample_commits() -> Vec<CommitNode> {
    vec![
        // 0
        commit(
            "f1a9c02",
            "Merge branch 'feature/search' into main",
            "Minh Tran",
            "2m ago",
            0,
            vec![1, 2],
            vec![head(), local("main"), remote("origin/main")],
            vec![file("M", "src/main.rs"), file("A", "src/search.rs")],
        ),
        // 1
        commit(
            "e84b7d3",
            "Bump version to 0.3.0",
            "Sara Chen",
            "10m ago",
            0,
            vec![3],
            vec![tag("v0.3.0")],
            vec![file("M", "Cargo.toml"), file("M", "CHANGELOG.md")],
        ),
        // 2
        commit(
            "9c3d1aa",
            "Highlight matching commits in search results",
            "Minh Tran",
            "25m ago",
            1,
            vec![4],
            vec![local("feature/search"), remote("origin/feature/search")],
            vec![file("M", "src/app/views/graph.rs")],
        ),
        // 3
        commit(
            "7d02be8",
            "Merge branch 'hotfix/empty-repo'",
            "Sara Chen",
            "40m ago",
            0,
            vec![5, 6],
            vec![],
            vec![file("M", "src/app/model.rs")],
        ),
        // 4
        commit(
            "2b6e5f1",
            "Add search field to toolbar",
            "Minh Tran",
            "1h ago",
            1,
            vec![7],
            vec![],
            vec![file("M", "src/app/views/toolbar.rs")],
        ),
        // 5
        commit(
            "a13c9e7",
            "Update dependencies",
            "Priya Raman",
            "1h ago",
            0,
            vec![8],
            vec![],
            vec![file("M", "Cargo.lock"), file("M", "Cargo.toml")],
        ),
        // 6
        commit(
            "6f48d20",
            "Fix crash on empty repository",
            "Daria Gomez",
            "2h ago",
            2,
            vec![8],
            vec![local("hotfix/empty-repo")],
            vec![file("M", "src/app/controller.rs"), file("A", "tests/empty_repo.rs")],
        ),
        // 7
        commit(
            "d5e7a41",
            "Add search model and matching",
            "Minh Tran",
            "3h ago",
            1,
            vec![9],
            vec![],
            vec![file("M", "src/app/model.rs")],
        ),
        // 8
        commit(
            "3c9b8f6",
            "Merge branch 'feature/graph-ui' into main",
            "Minh Tran",
            "4h ago",
            0,
            vec![10, 11],
            vec![],
            vec![file("A", "src/app/views/graph.rs"), file("M", "README.md")],
        ),
        // 9
        commit(
            "b02ef15",
            "Scaffold search module",
            "Minh Tran",
            "5h ago",
            1,
            vec![12],
            vec![],
            vec![file("A", "src/search.rs")],
        ),
        // 10
        commit(
            "51aa07c",
            "Cache commit topology for faster rendering",
            "Priya Raman",
            "6h ago",
            0,
            vec![12],
            vec![],
            vec![file("A", "src/cache/topology.rs")],
        ),
        // 11
        commit(
            "08d4c6e",
            "Polish graph badge colors and merge routing",
            "Minh Tran",
            "7h ago",
            2,
            vec![12],
            vec![local("feature/graph-ui")],
            vec![file("M", "src/app/views/theme.rs")],
        ),
        // 12
        commit(
            "4e7a930",
            "Add zoomable timeline lanes",
            "Priya Raman",
            "9h ago",
            0,
            vec![13],
            vec![],
            vec![file("A", "src/ui/timeline.rs")],
        ),
        // 13
        commit(
            "91c2d58",
            "Merge branch 'release/v0.2'",
            "Jordan Lee",
            "12h ago",
            0,
            vec![14, 15],
            vec![tag("v0.2.0")],
            vec![file("M", "src/release.rs"), file("M", "src/version.rs")],
        ),
        // 14
        commit(
            "c06f3b2",
            "Prepare release notes for v0.2",
            "Jordan Lee",
            "14h ago",
            0,
            vec![16],
            vec![],
            vec![file("M", "docs/release-notes.md")],
        ),
        // 15
        commit(
            "e9110a4",
            "Stabilize release branch before demo",
            "Jordan Lee",
            "15h ago",
            1,
            vec![16],
            vec![local("release/v0.2"), remote("origin/release/v0.2")],
            vec![file("M", "Cargo.toml"), file("M", "src/release.rs")],
        ),
        // 16
        commit(
            "5a9d0c4",
            "Add staging summary panel scaffolding",
            "Daria Gomez",
            "20h ago",
            0,
            vec![17],
            vec![],
            vec![file("A", "src/ui/staging.rs"), file("M", "src/main.rs")],
        ),
        // 17
        commit(
            "703dd2a",
            "Initial UI shell for repository overview",
            "Minh Tran",
            "1d ago",
            0,
            vec![],
            vec![tag("v0.1.0")],
            vec![file("A", "src/main.rs"), file("A", "Cargo.toml")],
        ),
    ]
}

