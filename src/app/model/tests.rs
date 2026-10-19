use super::*;

#[test]
fn sidebar_items_build_tree_sections_and_nested_groups() {
    let model = AppModel::sample();
    let items = model.sidebar_items();

    assert!(items.iter().any(|item| matches!(
        item,
        SidebarItem::Section { title, .. } if title == "HEAD (Current Branch)"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SidebarItem::Section { title, .. } if title == "Local"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SidebarItem::Group { title, .. } if title == "feature"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SidebarItem::Group { title, .. } if title == "origin"
    )));
}

#[test]
fn sidebar_row_prefers_tree_branch_over_head_copy() {
    let model = AppModel::sample();
    let row = model.sidebar_row_of_branch(0).unwrap();

    assert!(matches!(
        model.sidebar_items().get(row),
        Some(SidebarItem::Branch {
            head_copy: false,
            ..
        })
    ));
}

#[test]
fn toggling_sidebar_node_collapses_and_expands() {
    let mut model = AppModel::sample();

    assert!(!model.toggle_sidebar_node("section/remote"));
    assert!(
        model
            .sidebar_items()
            .iter()
            .all(|item| !matches!(item, SidebarItem::Group { title, .. } if title == "origin"))
    );

    assert!(model.toggle_sidebar_node("section/remote"));
    assert!(
        model
            .sidebar_items()
            .iter()
            .any(|item| matches!(item, SidebarItem::Group { title, .. } if title == "origin"))
    );
}

#[test]
fn selected_commit_is_none_when_uncommitted_row_is_active() {
    let mut model = AppModel::sample();

    model.selected_uncommitted = true;

    assert!(model.selected_commit().is_none());
}

#[test]
fn commit_list_row_helpers_include_uncommitted_row() {
    let mut model = AppModel::sample();
    model.selected_commit = 0;

    assert_eq!(model.commit_index_for_row(0), None);
    assert_eq!(model.commit_index_for_row(1), Some(0));
    assert_eq!(model.selected_commit_row(), Some(1));

    model.selected_uncommitted = true;
    assert_eq!(model.selected_commit_row(), Some(0));
}

#[test]
fn working_tree_group_helpers_split_changed_and_unversions_files() {
    let model = AppModel::sample();

    assert_eq!(model.working_tree.changed_files().len(), 4);
    assert_eq!(model.working_tree.unversioned_files().len(), 1);
    assert!(
        model
            .working_tree
            .changed_files()
            .iter()
            .all(|file| !file.untracked)
    );
    assert!(
        model
            .working_tree
            .unversioned_files()
            .iter()
            .all(|file| file.untracked)
    );
}
