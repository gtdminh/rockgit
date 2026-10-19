use super::*;
use crate::app::model::AppModel;

#[test]
fn working_tree_groups_start_expanded() {
    let model = AppModel::sample();
    let items = working_tree_list_items(&model);

    assert_eq!(items.len(), 7);
    assert!(matches!(items[0], WorkingTreeListItem::Header { ref title, expanded: true, .. } if title == "changed"));
    assert!(matches!(items[5], WorkingTreeListItem::Header { ref title, expanded: true, .. } if title == "unversions"));
}

#[test]
fn collapsing_a_working_tree_group_hides_its_children() {
    let mut model = AppModel::sample();
    model
        .collapsed_working_tree_nodes
        .insert("working_tree/changed".to_owned());

    let items = working_tree_list_items(&model);

    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], WorkingTreeListItem::Header { ref title, expanded: false, .. } if title == "changed"));
    assert!(matches!(items[1], WorkingTreeListItem::Header { ref title, expanded: true, .. } if title == "unversions"));
}

