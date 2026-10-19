use cacao::button::Button;
use cacao::color::Color;
use cacao::control::Control;
use cacao::input::{TextField, TextFieldDelegate};
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};
use cacao::text::{Font, Label, LineBreakMode};
use cacao::view::View;
use std::sync::{Arc, Mutex};

use crate::app::{
    actions::AppAction,
    model::{AppModel, SharedModel},
    send,
    views::rows::{WORKING_TREE_ROW_HEIGHT, WorkingTreeRow, WorkingTreeRowData},
};

const PANEL_WIDTH: f64 = 340.;
const CONTENT_WIDTH: f64 = PANEL_WIDTH - 24.;
const COMMIT_FIELD_HEIGHT: f64 = 116.;
const MIN_WORKING_TREE_LIST_HEIGHT: f64 = 140.;
const WORKING_TREE_ROW_ID: &str = "RockGitWorkingTreeRowCell";
const WORKING_TREE_CHANGED_ID: &str = "working_tree/changed";
const WORKING_TREE_UNVERSIONS_ID: &str = "working_tree/unversions";

#[derive(Clone, Debug)]
enum WorkingTreeListItem {
    Header {
        id: String,
        title: String,
        count: usize,
        expanded: bool,
    },
    File {
        icon: String,
        status: String,
        name: String,
        parent_path: String,
        display_path: String,
        repo_path: String,
    },
}

#[derive(Clone, Debug)]
struct CommitMessageDelegate {
    value: Arc<Mutex<String>>,
}

impl TextFieldDelegate for CommitMessageDelegate {
    const NAME: &'static str = "RockGitCommitMessageField";

    fn text_did_change(&self, value: &str) {
        if let Ok(mut text) = self.value.lock() {
            *text = value.to_owned();
        }
    }
}

pub struct WorkingTreeList {
    model: SharedModel,
    view: Option<ListView>,
}

impl WorkingTreeList {
    pub fn new(model: SharedModel) -> Self {
        Self { model, view: None }
    }
}

impl ListViewDelegate for WorkingTreeList {
    const NAME: &'static str = "RockGitWorkingTreeList";

    fn did_load(&mut self, view: ListView) {
        view.register(WORKING_TREE_ROW_ID, WorkingTreeRow::default);
        view.set_row_height(WORKING_TREE_ROW_HEIGHT);
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        let model = self.model.lock().unwrap();
        working_tree_list_items(&model).len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let view = self
            .view
            .as_ref()
            .unwrap()
            .dequeue::<WorkingTreeRow>(WORKING_TREE_ROW_ID);
        if let Some(cell) = &view.delegate {
            let model = self.model.lock().unwrap();
            if let Some(item) = working_tree_list_items(&model).get(row) {
                match item {
                    WorkingTreeListItem::Header {
                        title,
                        count,
                        expanded,
                        ..
                    } => cell.configure_header(title, *count, *expanded),
                    WorkingTreeListItem::File {
                        icon,
                        status,
                        name,
                        parent_path,
                        display_path,
                        repo_path,
                    } => cell.configure_file(&WorkingTreeRowData {
                        icon,
                        status,
                        name,
                        parent_path,
                        display_path,
                        repo_path,
                    }),
                }
            }
        }
        view.into_row()
    }

    fn item_selected(&self, row: Option<usize>) {
        let Some(row) = row else {
            return;
        };

        let model = self.model.lock().unwrap();
        if let Some(WorkingTreeListItem::Header { id, .. }) = working_tree_list_items(&model).get(row)
        {
            send(AppAction::ToggleWorkingTreeListNode(id.clone()));
        }
    }
}

pub struct Details {
    pub container: View,
    newer: Button,
    older: Button,
    summary: Label,
    meta: Label,
    refs: Label,
    files_title: Label,
    files: Label,
    revert_all: Button,
    working_tree_list: ListView<WorkingTreeList>,
    commit_message_title: Label,
    commit_message: TextField<CommitMessageDelegate>,
    commit_help: Label,
    commit: Button,
    commit_and_push: Button,
}

fn wrapped_label() -> Label {
    let label = Label::new();
    label.set_max_number_of_lines(0);
    label.set_line_break_mode(LineBreakMode::WrapWords);
    label
}

impl Details {
    pub fn new(model: SharedModel) -> Self {
        let mut newer = Button::new("Newer");
        newer.set_action(|| send(AppAction::SelectNewerCommit));
        let mut older = Button::new("Older");
        older.set_action(|| send(AppAction::SelectOlderCommit));

        let summary = wrapped_label();
        summary.set_font(&Font::bold_system(18.));
        let files_title = Label::new();
        files_title.set_text("Changed files");
        files_title.set_font(&Font::bold_system(13.));
        let refs = wrapped_label();
        refs.set_text_color(Color::SystemGray);
        let mut revert_all = Button::new("Revert All");
        revert_all.set_action(|| send(AppAction::RevertAllWorkingTreeChanges));
        let working_tree_list = ListView::with(WorkingTreeList::new(model));
        let commit_message_title = Label::new();
        commit_message_title.set_text("Commit message");
        commit_message_title.set_font(&Font::bold_system(13.));
        let commit_message_value = Arc::new(Mutex::new(String::new()));
        let commit_message = TextField::with(CommitMessageDelegate {
            value: commit_message_value.clone(),
        });
        commit_message.set_placeholder_text("Summarize the changes you want to commit");
        commit_message.set_uses_single_line(false);
        commit_message.set_wraps(true);
        commit_message.set_max_number_of_lines(0);
        commit_message.set_font(&Font::system(12.));
        let commit_help = wrapped_label();
        commit_help.set_text_color(Color::LabelSecondary);
        let mut commit = Button::new("Commit");
        {
            let commit_message_value = commit_message_value.clone();
            commit.set_action(move || {
                let message = commit_message_value
                    .lock()
                    .map(|value| value.clone())
                    .unwrap_or_default();
                send(AppAction::CommitWorkingTree {
                    message,
                    push_after: false,
                });
            });
        }
        let mut commit_and_push = Button::new("Commit & Push");
        commit_and_push.set_action(move || {
            let message = commit_message_value
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default();
            send(AppAction::CommitWorkingTree {
                message,
                push_after: true,
            });
        });

        Self {
            container: View::new(),
            newer,
            older,
            summary,
            meta: wrapped_label(),
            refs,
            files_title,
            files: wrapped_label(),
            revert_all,
            working_tree_list,
            commit_message_title,
            commit_message,
            commit_help,
            commit,
            commit_and_push,
        }
    }

    /// Adds the panel's subviews and layout constraints.
    pub fn layout(&self) {
        let c = &self.container;
        c.add_subview(&self.newer);
        c.add_subview(&self.older);
        c.add_subview(&self.summary);
        c.add_subview(&self.meta);
        c.add_subview(&self.refs);
        c.add_subview(&self.files_title);
        c.add_subview(&self.files);
        c.add_subview(&self.revert_all);
        c.add_subview(&self.working_tree_list);
        c.add_subview(&self.commit_message_title);
        c.add_subview(&self.commit_message);
        c.add_subview(&self.commit_help);
        c.add_subview(&self.commit);
        c.add_subview(&self.commit_and_push);

        LayoutConstraint::activate(&[
            c.width.constraint_equal_to_constant(PANEL_WIDTH),
            self.newer.top.constraint_equal_to(&c.top),
            self.newer
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.older.top.constraint_equal_to(&c.top),
            self.older
                .leading
                .constraint_equal_to(&self.newer.trailing)
                .offset(8.),
            self.summary
                .top
                .constraint_equal_to(&self.newer.bottom)
                .offset(14.),
            self.summary
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.summary
                .width
                .constraint_equal_to_constant(CONTENT_WIDTH),
            self.meta
                .top
                .constraint_equal_to(&self.summary.bottom)
                .offset(10.),
            self.meta
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.meta.width.constraint_equal_to_constant(CONTENT_WIDTH),
            self.refs
                .top
                .constraint_equal_to(&self.meta.bottom)
                .offset(10.),
            self.refs
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.refs.width.constraint_equal_to_constant(CONTENT_WIDTH),
            self.files_title
                .top
                .constraint_equal_to(&self.refs.bottom)
                .offset(16.),
            self.files_title
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.revert_all
                .center_y
                .constraint_equal_to(&self.files_title.center_y),
            self.revert_all
                .trailing
                .constraint_equal_to(&c.trailing)
                .offset(-12.),
            self.files
                .top
                .constraint_equal_to(&self.files_title.bottom)
                .offset(6.),
            self.files
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.files.width.constraint_equal_to_constant(CONTENT_WIDTH),
            self.working_tree_list
                .top
                .constraint_equal_to(&self.files_title.bottom)
                .offset(6.),
            self.working_tree_list
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.working_tree_list
                .trailing
                .constraint_equal_to(&c.trailing)
                .offset(-12.),
            self.working_tree_list
                .height
                .constraint_greater_than_or_equal_to_constant(MIN_WORKING_TREE_LIST_HEIGHT),
            self.commit_message_title
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.working_tree_list
                .bottom
                .constraint_equal_to(&self.commit_message_title.top)
                .offset(-14.),
            self.commit_message_title
                .top
                .constraint_equal_to(&self.working_tree_list.bottom)
                .offset(14.),
            self.commit_message
                .trailing
                .constraint_equal_to(&c.trailing)
                .offset(-12.),
            self.commit_message
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.commit_message
                .top
                .constraint_equal_to(&self.commit_message_title.bottom)
                .offset(6.),
            self.commit_message
                .height
                .constraint_equal_to_constant(COMMIT_FIELD_HEIGHT),
            self.commit_help
                .top
                .constraint_equal_to(&self.commit_message.bottom)
                .offset(8.),
            self.commit_help
                .leading
                .constraint_equal_to(&c.leading)
                .offset(12.),
            self.commit_help
                .trailing
                .constraint_equal_to(&c.trailing)
                .offset(-12.),
            self.commit_help
                .bottom
                .constraint_equal_to(&self.commit.top)
                .offset(-10.),
            self.commit
                .bottom
                .constraint_equal_to(&c.bottom)
                .offset(-12.),
            self.commit
                .trailing
                .constraint_equal_to(&self.commit_and_push.leading)
                .offset(-8.),
            self.commit_and_push
                .bottom
                .constraint_equal_to(&c.bottom)
                .offset(-12.),
            self.commit_and_push
                .trailing
                .constraint_equal_to(&c.trailing)
                .offset(-12.),
        ]);

        self.set_commit_form_visible(false);
    }

    pub fn update(&self, model: &AppModel) {
        if model.selected_uncommitted {
            self.newer.set_hidden(true);
            self.older.set_hidden(true);
            self.set_commit_form_visible(true);
            self.files.set_hidden(true);
            self.revert_all.set_hidden(false);
            self.working_tree_list.set_hidden(false);
            self.working_tree_list.reload();

            let changed = model.working_tree.changed_files();
            let unversions = model.working_tree.unversioned_files();

            let repository = model
                .active_repository()
                .map(|repo| repo.name.as_str())
                .unwrap_or("-");
            let branch = model
                .current_branch()
                .map(|branch| branch.name.as_str())
                .unwrap_or("-");
            let change_count = model.working_tree.total_count();
            let change_label = if change_count == 1 { "file" } else { "files" };

            self.summary.set_text("Uncommitted changes");
            self.meta.set_text(format!(
                "Repository: {repository}\nBranch: {branch}\n{change_count} pending {change_label}"
            ));
            self.refs.set_text(format!(
                "Changed: {} • Unversions: {}",
                changed.len(),
                unversions.len()
            ));
            self.files_title.set_text("Files");
            self.commit_help.set_text(
                "Commit stages the current working tree before creating the commit. Use Revert or Revert All to discard changes.",
            );
            return;
        }

        self.newer.set_hidden(false);
        self.older.set_hidden(false);
        self.newer.set_enabled(model.can_select_newer_commit());
        self.older.set_enabled(model.can_select_older_commit());
        self.set_commit_form_visible(false);
        self.revert_all.set_hidden(true);
        self.working_tree_list.set_hidden(true);
        self.files.set_hidden(false);
        self.files_title.set_text("Changed files");

        let Some(commit) = model.selected_commit() else {
            self.summary.set_text("No commit selected");
            self.meta.set_text("");
            self.refs.set_text("");
            self.files.set_text("");
            return;
        };

        self.summary.set_text(&commit.summary);
        self.meta.set_text(format!(
            "Commit: {}\nAuthor: {}\nWhen: {}\nParents: {}",
            commit.short_id,
            commit.author,
            commit.relative_time,
            commit.parents.len()
        ));

        self.refs.set_text(if commit.refs.is_empty() {
            "No refs on this commit".to_owned()
        } else {
            let labels: Vec<&str> = commit.refs.iter().map(|r| r.label.as_str()).collect();
            format!("Refs: {}", labels.join(", "))
        });

        self.files.set_text(format_file_list(&commit.files));
    }

    fn set_commit_form_visible(&self, visible: bool) {
        self.commit_message_title.set_hidden(!visible);
        self.commit_message.set_hidden(!visible);
        self.commit_help.set_hidden(!visible);
        self.commit.set_hidden(!visible);
        self.commit_and_push.set_hidden(!visible);
    }
}

fn working_tree_list_items(model: &AppModel) -> Vec<WorkingTreeListItem> {
    let changed = model.working_tree.changed_files();
    let unversions = model.working_tree.unversioned_files();
    let changed_expanded = model.working_tree_node_expanded(WORKING_TREE_CHANGED_ID);
    let unversions_expanded = model.working_tree_node_expanded(WORKING_TREE_UNVERSIONS_ID);
    let mut items = Vec::with_capacity(changed.len() + unversions.len() + 2);

    items.push(WorkingTreeListItem::Header {
        id: WORKING_TREE_CHANGED_ID.to_owned(),
        title: "changed".to_owned(),
        count: changed.len(),
        expanded: changed_expanded,
    });
    if changed_expanded {
        items.extend(changed.into_iter().map(working_tree_file_item_for_changed));
    }

    items.push(WorkingTreeListItem::Header {
        id: WORKING_TREE_UNVERSIONS_ID.to_owned(),
        title: "unversions".to_owned(),
        count: unversions.len(),
        expanded: unversions_expanded,
    });
    if unversions_expanded {
        items.extend(unversions.into_iter().map(working_tree_file_item_for_unversioned));
    }

    items
}

fn working_tree_file_item_for_changed(
    file: &crate::app::model::WorkingTreeFile,
) -> WorkingTreeListItem {
    let (name, parent_path) = split_display_path(&file.display_path);
    WorkingTreeListItem::File {
        icon: "⦿".to_owned(),
        status: file.staged_status_label(),
        name,
        parent_path,
        display_path: file.display_path.clone(),
        repo_path: file.repo_path.clone(),
    }
}

fn working_tree_file_item_for_unversioned(
    file: &crate::app::model::WorkingTreeFile,
) -> WorkingTreeListItem {
    let (name, parent_path) = split_display_path(&file.display_path);
    WorkingTreeListItem::File {
        icon: if file.untracked { "◎" } else { "⦿" }.to_owned(),
        status: file.unstaged_status_label(),
        name,
        parent_path,
        display_path: file.display_path.clone(),
        repo_path: file.repo_path.clone(),
    }
}

fn split_display_path(path: &str) -> (String, String) {
    if path.contains(" → ") {
        return (path.to_owned(), String::new());
    }

    match path.rsplit_once('/') {
        Some((parent, name)) => (name.to_owned(), parent.to_owned()),
        None => (path.to_owned(), String::new()),
    }
}

fn format_file_list(files: &[crate::app::model::FileChange]) -> String {
    if files.is_empty() {
        return "No files to show".to_owned();
    }

    files
        .iter()
        .map(|file| format!("{}  {}", file.status.trim_end(), file.path))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests;

