use super::*;
use crate::app::{
    actions::{AppAction, GitOperation},
    model::{AiConfig, AiProviderCli, AppConfig, AppModel},
};

#[test]
fn select_repository_updates_model() {
    let mut model = AppModel::sample();
    let mut controller = AppController;

    controller.dispatch(&mut model, AppAction::SelectRepository(2));

    assert_eq!(model.active_repo, 2);
    assert!(model.last_action.contains("infra-config"));
}

#[test]
fn selecting_older_commit_stops_at_end() {
    let mut model = AppModel::sample();
    let mut controller = AppController;
    model.selected_commit = model.commits.len() - 1;

    controller.dispatch(&mut model, AppAction::SelectOlderCommit);

    assert_eq!(model.selected_commit, model.commits.len() - 1);
}

#[test]
fn selecting_newer_commit_from_tip_moves_to_uncommitted_row() {
    let mut model = AppModel::sample();
    let mut controller = AppController;
    model.selected_commit = 0;

    controller.dispatch(&mut model, AppAction::SelectNewerCommit);

    assert!(model.selected_uncommitted);
}

#[test]
fn selecting_older_commit_from_uncommitted_row_moves_back_to_tip() {
    let mut model = AppModel::sample();
    let mut controller = AppController;
    model.selected_uncommitted = true;
    model.selected_commit = 3;

    controller.dispatch(&mut model, AppAction::SelectOlderCommit);

    assert!(!model.selected_uncommitted);
    assert_eq!(model.selected_commit, 0);
}

#[test]
fn run_operation_uses_active_context() {
    let mut model = AppModel::sample();
    let mut controller = AppController;
    model.active_repo = 0;
    model.active_branch = 1;

    controller.dispatch(&mut model, AppAction::RunGitOperation(GitOperation::Push));

    assert!(model.last_action.contains("Push requested"));
    assert!(model.last_action.contains("RockGit"));
    assert!(model.last_action.contains("feature/graph-ui"));
}

#[test]
fn commit_working_tree_requires_message() {
    let mut model = AppModel::sample();
    let mut controller = AppController;

    controller.dispatch(
        &mut model,
        AppAction::CommitWorkingTree {
            message: "   ".to_owned(),
            push_after: false,
        },
    );

    assert_eq!(model.last_action, "Commit message is required");
}

#[test]
fn update_config_replaces_model_config() {
    let mut model = AppModel::sample();
    let mut controller = AppController;
    let config = AppConfig {
        autorefresh_time: 45,
        ai: AiConfig {
            provider_cli: AiProviderCli::Gemini,
            cli_path: "/opt/homebrew/bin/gemini".to_owned(),
        },
    };

    controller.dispatch(&mut model, AppAction::UpdateConfig(config.clone()));

    assert_eq!(model.config, config);
    assert_eq!(model.last_action, "Saved settings");
}

#[test]
fn toggling_working_tree_group_collapses_and_expands() {
    let mut model = AppModel::sample();
    let mut controller = AppController;

    controller.dispatch(
        &mut model,
        AppAction::ToggleWorkingTreeListNode("working_tree/changed".to_owned()),
    );

    assert!(
        model
            .collapsed_working_tree_nodes
            .contains("working_tree/changed")
    );
    assert_eq!(model.last_action, "Collapsed changed");

    controller.dispatch(
        &mut model,
        AppAction::ToggleWorkingTreeListNode("working_tree/changed".to_owned()),
    );

    assert!(
        !model
            .collapsed_working_tree_nodes
            .contains("working_tree/changed")
    );
    assert_eq!(model.last_action, "Expanded changed");
}

