mod actions;
mod bus;
mod controller;
mod git;
mod git_service;
mod model;
pub(crate) mod persistent;
mod sample_data;
mod views;

use std::sync::{Arc, Mutex};

use cacao::appkit::menu::{Menu, MenuItem};
use cacao::appkit::window::{Window, WindowConfig};
use cacao::appkit::{App, AppDelegate};
use cacao::button::Button;
use cacao::color::Color;
use cacao::events::EventModifierFlag;
use cacao::foundation::{NSString, nil};
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::ListView;
use cacao::notification_center::Dispatcher;
use cacao::objc::{msg_send, sel, sel_impl};
use cacao::text::{Label, LineBreakMode};
use cacao::view::View;

use actions::AppAction;
use controller::AppController;
use model::{AppModel, SharedModel};
use views::{
    details::Details,
    graph::{CommitList, selected_row},
    header::CommitHeader,
    settings::SettingsDialog,
    sidebar::{BranchList, RepoList, open_repository_button},
    toolbar::Toolbar,
};

const STATUS_BRANCH_MAX_CHARS: usize = 48;

fn abbreviate_branch_name(branch_name: &str, max_chars: usize) -> String {
    let branch_len = branch_name.chars().count();
    if branch_len <= max_chars {
        return branch_name.to_owned();
    }

    let parts: Vec<&str> = branch_name.split('/').collect();
    if parts.len() > 1 {
        if let (Some(root), Some(leaf)) = (parts.first(), parts.last()) {
            let compact = format!("{root}/…/{leaf}");
            if compact.chars().count() <= max_chars {
                return compact;
            }
        }
    }

    abbreviate_middle(branch_name, max_chars)
}

fn abbreviate_middle(text: &str, max_chars: usize) -> String {
    let text_len = text.chars().count();
    if text_len <= max_chars {
        return text.to_owned();
    }

    if max_chars <= 1 {
        return "…".to_owned();
    }

    let visible_chars = max_chars - 1;
    let prefix_len = visible_chars * 2 / 3;
    let suffix_len = visible_chars - prefix_len;
    let chars: Vec<char> = text.chars().collect();
    let prefix: String = chars[..prefix_len].iter().collect();
    let suffix: String = chars[chars.len() - suffix_len..].iter().collect();
    format!("{prefix}…{suffix}")
}

fn set_label_tooltip(label: &Label, tooltip: Option<&str>) {
    label.objc.with_mut(|obj| unsafe {
        match tooltip {
            Some(tooltip) => {
                let tooltip = NSString::new(tooltip);
                let _: () = msg_send![obj, setToolTip: tooltip];
            }
            None => {
                let _: () = msg_send![obj, setToolTip: nil];
            }
        }
    });
}

/// Publishes an action on the event bus (for the git service) and queues it to be applied on
/// the main thread by the app delegate.
pub fn send(action: AppAction) {
    bus::publish(action.clone());
    send_ui(action);
}

/// Queues an action for the main thread only (used for results coming from the git service).
pub fn send_ui(action: AppAction) {
    App::<RockGitApp, AppAction>::dispatch_main(action);
}

pub fn run() {
    let model: SharedModel = Arc::new(Mutex::new(AppModel::new()));
    let config = model.lock().unwrap().config.clone();

    git_service::start(config);
    App::new("com.rockgit.app", RockGitApp::new(model)).run();
}

struct RockGitApp {
    model: SharedModel,
    window: Window,
    content: View,
    toolbar: Toolbar,
    open_repo: Button,
    repos: ListView<RepoList>,
    branches: ListView<BranchList>,
    commits: ListView<CommitList>,
    header: CommitHeader,
    details: Details,
    settings: SettingsDialog,
    status: Label,
}

impl RockGitApp {
    fn new(model: SharedModel) -> Self {
        Self {
            window: Window::new(WindowConfig::default()),
            content: View::new(),
            toolbar: Toolbar::new(),
            open_repo: open_repository_button(),
            repos: ListView::with(RepoList::new(model.clone())),
            branches: ListView::with(BranchList::new(model.clone())),
            commits: ListView::with(CommitList::new(model.clone())),
            header: CommitHeader::new(),
            details: Details::new(model.clone()),
            settings: SettingsDialog::new(),
            status: Label::new(),
            model,
        }
    }

    fn build_layout(&self) {
        let content = &self.content;
        let toolbar = &self.toolbar;

        content.add_subview(&toolbar.fetch);
        content.add_subview(&toolbar.pull);
        content.add_subview(&toolbar.push);
        content.add_subview(&toolbar.refresh);
        content.add_subview(&toolbar.search);
        content.add_subview(&toolbar.count);
        content.add_subview(&toolbar.branches_label);
        content.add_subview(&toolbar.branch_select);
        content.add_subview(&toolbar.remote_switch);
        content.add_subview(&self.open_repo);
        content.add_subview(&self.repos);
        content.add_subview(&self.branches);
        content.add_subview(&self.header.container);
        content.add_subview(&self.commits);
        content.add_subview(&self.details.container);
        content.add_subview(&self.status);
        self.details.layout();
        self.header.layout();

        self.status.set_text_color(Color::SystemGray);
        self.status.set_max_number_of_lines(1);
        self.status.set_line_break_mode(LineBreakMode::TruncateTail);

        LayoutConstraint::activate(&[
            // Toolbar row.
            toolbar
                .fetch
                .top
                .constraint_equal_to(&content.top)
                .offset(46.),
            toolbar
                .fetch
                .leading
                .constraint_equal_to(&content.leading)
                .offset(12.),
            toolbar
                .pull
                .top
                .constraint_equal_to(&content.top)
                .offset(46.),
            toolbar
                .pull
                .leading
                .constraint_equal_to(&toolbar.fetch.trailing)
                .offset(6.),
            toolbar
                .push
                .top
                .constraint_equal_to(&content.top)
                .offset(46.),
            toolbar
                .push
                .leading
                .constraint_equal_to(&toolbar.pull.trailing)
                .offset(6.),
            toolbar
                .refresh
                .top
                .constraint_equal_to(&content.top)
                .offset(46.),
            toolbar
                .refresh
                .leading
                .constraint_equal_to(&toolbar.push.trailing)
                .offset(6.),
            toolbar
                .search
                .top
                .constraint_equal_to(&content.top)
                .offset(47.),
            toolbar
                .search
                .leading
                .constraint_equal_to(&toolbar.refresh.trailing)
                .offset(16.),
            toolbar.search.width.constraint_equal_to_constant(260.),
            toolbar
                .count
                .top
                .constraint_equal_to(&content.top)
                .offset(49.),
            toolbar
                .count
                .leading
                .constraint_equal_to(&toolbar.search.trailing)
                .offset(16.),
            // Left column: repositories above branches.
            self.open_repo
                .top
                .constraint_equal_to(&toolbar.fetch.bottom)
                .offset(10.),
            self.open_repo
                .leading
                .constraint_equal_to(&content.leading)
                .offset(12.),
            self.repos
                .top
                .constraint_equal_to(&self.open_repo.bottom)
                .offset(6.),
            self.repos.leading.constraint_equal_to(&content.leading),
            self.repos.width.constraint_equal_to_constant(260.),
            self.repos.height.constraint_equal_to_constant(220.),
            self.branches
                .top
                .constraint_equal_to(&self.repos.bottom)
                .offset(8.),
            self.branches.leading.constraint_equal_to(&content.leading),
            self.branches.width.constraint_equal_to_constant(260.),
            self.branches
                .bottom
                .constraint_equal_to(&self.status.top)
                .offset(-8.),
            // Right column: commit details.
            self.details
                .container
                .top
                .constraint_equal_to(&toolbar.fetch.bottom)
                .offset(10.),
            self.details
                .container
                .trailing
                .constraint_equal_to(&content.trailing),
            self.details
                .container
                .bottom
                .constraint_equal_to(&self.status.top)
                .offset(-8.),
            // Center: branch filter bar, column header, then the commit graph.
            toolbar
                .branches_label
                .top
                .constraint_equal_to(&toolbar.fetch.bottom)
                .offset(16.),
            toolbar
                .branches_label
                .leading
                .constraint_equal_to(&self.repos.trailing)
                .offset(16.),
            toolbar
                .branch_select
                .top
                .constraint_equal_to(&toolbar.fetch.bottom)
                .offset(10.),
            toolbar
                .branch_select
                .leading
                .constraint_equal_to(&toolbar.branches_label.trailing)
                .offset(8.),
            toolbar
                .branch_select
                .width
                .constraint_equal_to_constant(260.),
            toolbar
                .remote_switch
                .top
                .constraint_equal_to(&toolbar.fetch.bottom)
                .offset(13.),
            toolbar
                .remote_switch
                .leading
                .constraint_equal_to(&toolbar.branch_select.trailing)
                .offset(16.),
            self.header
                .container
                .top
                .constraint_equal_to(&toolbar.branch_select.bottom)
                .offset(8.),
            self.header
                .container
                .leading
                .constraint_equal_to(&self.repos.trailing)
                .offset(8.),
            self.header
                .container
                .trailing
                .constraint_equal_to(&self.details.container.leading)
                .offset(-8.),
            self.header
                .container
                .height
                .constraint_equal_to_constant(22.),
            self.commits
                .top
                .constraint_equal_to(&self.header.container.bottom),
            self.commits
                .leading
                .constraint_equal_to(&self.repos.trailing)
                .offset(8.),
            self.commits
                .trailing
                .constraint_equal_to(&self.details.container.leading)
                .offset(-8.),
            self.commits
                .bottom
                .constraint_equal_to(&self.status.top)
                .offset(-8.),
            // Status bar.
            self.status
                .leading
                .constraint_equal_to(&content.leading)
                .offset(12.),
            self.status
                .trailing
                .constraint_equal_to(&content.trailing)
                .offset(-12.),
            self.status
                .bottom
                .constraint_equal_to(&content.bottom)
                .offset(-8.),
        ]);
    }

    /// Pushes the current model state into every view.
    fn refresh(&self) {
        // Snapshot so the lock isn't held while list views call back into their delegates.
        let model = self.model.lock().unwrap().clone();

        self.toolbar.fill_branches(&model);
        self.toolbar.update(&model);
        self.details.update(&model);

        let repo = model
            .active_repository()
            .map(|r| r.name.as_str())
            .unwrap_or("-");
        let branch = model
            .active_branch()
            .map(|b| b.name.as_str())
            .unwrap_or("-");
        let branch_label = abbreviate_branch_name(branch, STATUS_BRANCH_MAX_CHARS);
        self.status.set_text(format!(
            "{}  |  {}  |  {}",
            model.last_action, repo, branch_label
        ));
        set_label_tooltip(
            &self.status,
            (branch_label != branch && branch != "-").then_some(branch),
        );

        self.repos.reload();
        if model.active_repo < model.repositories.len() {
            self.repos.select_row_indexes(&[model.active_repo], false);
        } else {
            self.repos.select_row_indexes(&[], false);
        }
        self.branches.reload();
        if let Some(row) = model.sidebar_row_of_branch(model.active_branch) {
            self.branches.select_row_indexes(&[row], false);
        } else {
            self.branches.select_row_indexes(&[], false);
        }
        self.header
            .set_graph_width(views::graph::GraphGeometry::for_model(&model).width);
        self.commits.reload();
        if let Some(row) = selected_row(&model) {
            self.commits.select_row_indexes(&[row], false);
        } else {
            self.commits.select_row_indexes(&[], false);
        }
    }

    fn open_settings_dialog(&self) {
        let config = self.model.lock().unwrap().config.clone();
        self.settings.show(&config);
    }

    fn save_settings_dialog(&self) -> Result<AppAction, String> {
        let config = self.settings.read_config()?;
        persistent::save_config(&config)?;
        self.settings.close();
        Ok(AppAction::UpdateConfig(config))
    }
}

#[cfg(test)]
mod tests;

impl AppDelegate for RockGitApp {
    fn did_finish_launching(&self) {
        App::set_menu(vec![
            Menu::new(
                "",
                vec![
                    MenuItem::new("Settings…")
                        .key(",")
                        .modifiers(&[EventModifierFlag::Command])
                        .action(|| send_ui(AppAction::OpenSettingsDialog)),
                    MenuItem::Separator,
                    MenuItem::Hide,
                    MenuItem::HideOthers,
                    MenuItem::ShowAll,
                    MenuItem::Separator,
                    MenuItem::Quit,
                ],
            ),
            Menu::new("File", vec![MenuItem::CloseWindow]),
            Menu::new(
                "Edit",
                vec![
                    MenuItem::Undo,
                    MenuItem::Redo,
                    MenuItem::Separator,
                    MenuItem::Cut,
                    MenuItem::Copy,
                    MenuItem::Paste,
                    MenuItem::Separator,
                    MenuItem::SelectAll,
                ],
            ),
            Menu::new("View", vec![MenuItem::EnterFullScreen]),
            Menu::new("Window", vec![MenuItem::Minimize, MenuItem::Zoom]),
        ]);

        App::activate();

        self.window.set_title("RockGit");
        self.window.set_content_size(1480., 920.);
        self.window.set_minimum_content_size(1100., 720.);
        self.window.set_content_view(&self.content);

        self.build_layout();
        self.refresh();
        self.window.show();
    }

    fn should_terminate_after_last_window_closed(&self) -> bool {
        true
    }
}

impl Dispatcher for RockGitApp {
    type Message = AppAction;

    fn on_ui_message(&self, action: AppAction) {
        // The popup can't carry its selection in the callback, so read it here.
        let action = match action {
            AppAction::OpenSettingsDialog => {
                self.open_settings_dialog();
                return;
            }
            AppAction::CloseSettingsDialog => {
                self.settings.close();
                return;
            }
            AppAction::SaveSettingsDialog => match self.save_settings_dialog() {
                Ok(action) => {
                    bus::publish(action.clone());
                    action
                }
                Err(message) => {
                    self.settings.show_error(&message);
                    return;
                }
            },
            AppAction::BranchFilterChanged => {
                let selected = self.toolbar.branch_select.get_selected_index();
                AppAction::SetBranchFilter(selected.checked_sub(1))
            }
            other => other,
        };

        let navigated = matches!(
            action,
            AppAction::SelectOlderCommit | AppAction::SelectNewerCommit
        );

        let selected_after = {
            let mut model = self.model.lock().unwrap();
            let before = model.selected_commit;
            AppController.dispatch(&mut model, action);
            (model.selected_commit != before).then_some(model.selected_commit)
        };
        // Keyboard-style navigation is resolved by the controller; tell the git service too.
        if let (true, Some(index)) = (navigated, selected_after) {
            bus::publish(AppAction::SelectCommit(index));
        }
        self.refresh();
    }
}
