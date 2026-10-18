use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use cacao::button::Button;
use cacao::color::Color;
use cacao::input::{TextField, TextFieldDelegate};
use cacao::select::Select;
use cacao::switch::Switch;
use cacao::text::Label;

use crate::app::{
    actions::{AppAction, GitOperation},
    model::AppModel,
    send,
};

#[derive(Debug, Default)]
pub struct SearchDelegate;

impl TextFieldDelegate for SearchDelegate {
    const NAME: &'static str = "RockGitSearchField";

    fn text_did_change(&self, value: &str) {
        send(AppAction::SetSearchQuery(value.to_owned()));
    }
}

pub struct Toolbar {
    pub fetch: Button,
    pub pull: Button,
    pub push: Button,
    pub refresh: Button,
    pub search: TextField<SearchDelegate>,
    pub count: Label,
    pub branches_label: Label,
    pub branch_select: Select,
    pub remote_switch: Switch,
    branch_signature: Cell<Option<u64>>,
}

fn operation_button(operation: GitOperation) -> Button {
    let mut button = Button::new(operation.label());
    button.set_action(move || send(AppAction::RunGitOperation(operation)));
    button
}

impl Toolbar {
    pub fn new() -> Self {
        let search = TextField::with(SearchDelegate);
        search.set_placeholder_text("commit message / hash / author");

        let count = Label::new();
        count.set_text_color(Color::SystemBlue);

        let branches_label = Label::new();
        branches_label.set_text("Branches:");

        let mut branch_select = Select::new();
        branch_select.set_action(|| send(AppAction::BranchFilterChanged));

        let mut remote_switch = Switch::new("Show Remote Branches");
        remote_switch.set_checked(true);
        remote_switch.set_action(|| send(AppAction::ToggleShowRemote));

        Self {
            fetch: operation_button(GitOperation::Fetch),
            pull: operation_button(GitOperation::Pull),
            push: operation_button(GitOperation::Push),
            refresh: operation_button(GitOperation::Refresh),
            search,
            count,
            branches_label,
            branch_select,
            remote_switch,
            branch_signature: Cell::new(None),
        }
    }

    /// Fills the branch popup once ("Show All" followed by every branch).
    pub fn fill_branches(&self, model: &AppModel) {
        let mut hasher = DefaultHasher::new();
        model.branches.len().hash(&mut hasher);
        for branch in &model.branches {
            branch.name.hash(&mut hasher);
            branch.ahead.hash(&mut hasher);
            branch.behind.hash(&mut hasher);
            branch.is_remote.hash(&mut hasher);
            branch.is_current.hash(&mut hasher);
        }
        let signature = hasher.finish();

        if self.branch_signature.get() != Some(signature) {
            self.branch_select.remove_all_items();
            self.branch_select.add_item("Show All");
            for branch in &model.branches {
                self.branch_select.add_item(&branch.name);
            }
            self.branch_signature.set(Some(signature));
        }

        let selected = model.branch_filter.map(|index| index + 1).unwrap_or(0);
        if selected < self.branch_select.len() {
            self.branch_select.set_selected_index(selected as _);
        } else if self.branch_select.len() > 0 {
            self.branch_select.set_selected_index(0);
        }
    }

    pub fn update(&self, model: &AppModel) {
        self.count.set_text(format!(
            "{} / {} commits visible",
            model.matching_commit_count(),
            model.commits.len()
        ));
    }
}

