use super::abbreviate_branch_name;

#[test]
fn keeps_short_branch_names_unchanged() {
    assert_eq!(abbreviate_branch_name("feature/login", 48), "feature/login");
}

#[test]
fn abbreviates_long_branch_names_in_the_middle() {
    let branch =
        "origin/feature/docker/idp-release/goldenimage-python/python-idp-v3.13-2026_07_19-00cd882";
    let abbreviated = abbreviate_branch_name(branch, 48);

    assert!(abbreviated.chars().count() <= 48);
    assert_eq!(abbreviated, "origin/…/python-idp-v3.13-2026_07_19-00cd882");
    assert!(abbreviated.contains('…'));
}
