use super::{abbreviate_ref_label, pills_for};
use crate::app::model::{CommitNode, RefBadge, RefKind};

fn commit_with_refs(refs: Vec<RefBadge>) -> CommitNode {
    CommitNode {
        id: String::new(),
        short_id: "abc1234".to_owned(),
        summary: "summary".to_owned(),
        author: "author".to_owned(),
        relative_time: "now".to_owned(),
        lane: 0,
        parents: Vec::new(),
        refs,
        files: Vec::new(),
        is_head: false,
        is_mainline: false,
    }
}

#[test]
fn abbreviates_graph_ref_labels_using_path_context_when_possible() {
    let label =
        "origin/feature/docker/idp-release/goldenimage-python/python-idp-v3.13-2026_07_19-00cd882";
    let abbreviated = abbreviate_ref_label(label, 24);

    assert!(abbreviated.starts_with("…/python-idp"));
    assert!(abbreviated.ends_with("00cd882"));
    assert!(abbreviated.contains('…'));
    assert!(abbreviated.chars().count() <= 24);
}

#[test]
fn graph_pills_include_full_name_tooltip_only_when_abbreviated() {
    let commit = commit_with_refs(vec![
        RefBadge {
            label: "main".to_owned(),
            kind: RefKind::LocalBranch,
        },
        RefBadge {
            label: "origin/feature/docker/idp-release/goldenimage-python/python-idp-v3.13-2026_07_19-00cd882"
                .to_owned(),
            kind: RefKind::RemoteBranch,
        },
    ]);

    let pills = pills_for(&commit, true);

    assert_eq!(pills[0].text, "main");
    assert_eq!(pills[0].tooltip, None);
    assert_eq!(
        pills[1].tooltip.as_deref(),
        Some(
            "origin/feature/docker/idp-release/goldenimage-python/python-idp-v3.13-2026_07_19-00cd882"
        )
    );
    assert!(pills[1].text.contains('…'));
    assert!(pills[1].text.chars().count() <= 24);
}
