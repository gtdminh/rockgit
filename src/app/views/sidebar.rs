use std::path::PathBuf;

use cacao::button::Button;
use cacao::filesystem::FileSelectPanel;
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};

use crate::app::{
    actions::AppAction,
    model::{SharedModel, SidebarItem},
    send,
    views::rows::SidebarRow,
};

const ROW_ID: &str = "RockGitSidebarRowCell";

pub struct RepoList {
    model: SharedModel,
    view: Option<ListView>,
}

impl RepoList {
    pub fn new(model: SharedModel) -> Self {
        Self { model, view: None }
    }
}

impl ListViewDelegate for RepoList {
    const NAME: &'static str = "RockGitRepoList";

    fn did_load(&mut self, view: ListView) {
        view.register(ROW_ID, SidebarRow::default);
        view.set_row_height(48.);
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.model.lock().unwrap().repositories.len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let view = self.view.as_ref().unwrap().dequeue::<SidebarRow>(ROW_ID);
        if let Some(cell) = &view.delegate {
            let model = self.model.lock().unwrap();
            if let Some(repo) = model.repositories.get(row) {
                let dirty = if repo.has_uncommitted_changes { " • dirty" } else { "" };
                cell.configure(
                    &format!("{}{dirty}", repo.name),
                    &format!("{} • {} branches", repo.path, repo.branch_count),
                    model.active_repo == row,
                );
            }
        }
        view.into_row()
    }

    fn item_selected(&self, row: Option<usize>) {
        if let Some(row) = row {
            let model = self.model.lock().unwrap();
            if model.active_repo != row {
                // Real repositories are (re)loaded by the git service; samples just switch.
                match model.repositories.get(row).map(|r| PathBuf::from(&r.path)) {
                    Some(path) if path.is_dir() => send(AppAction::OpenRepository(path)),
                    _ => send(AppAction::SelectRepository(row)),
                }
            }
        }
    }
}

/// Button that lets the user pick a repository folder to open.
pub fn open_repository_button() -> Button {
    let mut button = Button::new("Open Repository…");
    button.set_action(|| {
        let mut panel = FileSelectPanel::new();
        panel.set_can_choose_files(false);
        panel.set_can_choose_directories(true);
        panel.set_allows_multiple_selection(false);
        panel.set_message("Choose a git repository folder");
        panel.show(|urls| {
            if let Some(url) = urls.first() {
                send(AppAction::OpenRepository(url.pathbuf()));
            }
        });
    });
    button
}

pub struct BranchList {
    model: SharedModel,
    view: Option<ListView>,
}

impl BranchList {
    pub fn new(model: SharedModel) -> Self {
        Self { model, view: None }
    }
}

impl ListViewDelegate for BranchList {
    const NAME: &'static str = "RockGitBranchList";

    fn did_load(&mut self, view: ListView) {
        view.register(ROW_ID, SidebarRow::default);
        view.set_row_height(28.);
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.model.lock().unwrap().sidebar_items().len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let view = self.view.as_ref().unwrap().dequeue::<SidebarRow>(ROW_ID);
        if let Some(cell) = &view.delegate {
            let model = self.model.lock().unwrap();
            if let Some(item) = model.sidebar_items().get(row) {
                match item {
                    SidebarItem::Section {
                        title, expanded, ..
                    } => {
                        let arrow = if *expanded { "▾" } else { "▸" };
                        cell.configure_header(&format!("{arrow} {title}"));
                    }
                    SidebarItem::Group {
                        title,
                        depth,
                        expanded,
                        ..
                    } => {
                        let arrow = if *expanded { "▾" } else { "▸" };
                        let indent = "  ".repeat(*depth);
                        cell.configure_tree_item(&format!("{indent}{arrow} {title}"), false, true, None);
                    }
                    SidebarItem::Branch {
                        index,
                        title,
                        depth,
                        head_copy,
                    } => {
                        let indent = "  ".repeat(*depth);
                        let branch = &model.branches[*index];
                        let icon = if *head_copy || branch.is_current {
                            "✦"
                        } else if branch.is_remote {
                            "⎇"
                        } else {
                            "◦"
                        };
                        let selected = model.active_branch == *index && !*head_copy;
                        cell.configure_tree_item(
                            &format!("{indent}{icon} {title}"),
                            selected,
                            branch.is_current,
                            Some(&branch.name),
                        );
                    }
                    SidebarItem::Tag { index, depth } => {
                        let indent = "  ".repeat(*depth);
                        if let Some(tag) = model.tags.get(*index) {
                            cell.configure_tree_item(&format!("{indent}◇ {tag}"), false, false, None);
                        }
                    }
                }
            }
        }
        view.into_row()
    }

    fn item_selected(&self, row: Option<usize>) {
        let Some(row) = row else {
            return;
        };

        let item = {
            let model = self.model.lock().unwrap();
            model.sidebar_items().get(row).cloned()
        };

        match item {
            Some(SidebarItem::Branch { index, .. }) => {
                if self.model.lock().unwrap().active_branch != index {
                    send(AppAction::SelectBranch(index));
                }
            }
            Some(SidebarItem::Section { id, .. }) | Some(SidebarItem::Group { id, .. }) => {
                send(AppAction::ToggleBranchListNode(id));
            }
            Some(SidebarItem::Tag { .. }) | None => {}
        }
    }
}

