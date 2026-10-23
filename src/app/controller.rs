use crate::app::{
    actions::{AppAction, GitOperation},
    model::{AppModel, LoadedRepo, Repository},
    persistent,
};

#[derive(Default)]
pub struct AppController;

impl AppController {
    pub fn dispatch(&mut self, model: &mut AppModel, action: AppAction) {
        match action {
            AppAction::OpenSettingsDialog
            | AppAction::SaveSettingsDialog
            | AppAction::CloseSettingsDialog => {}
            AppAction::UpdateConfig(config) => self.update_config(model, config),
            AppAction::SelectRepository(index) => self.select_repository(model, index),
            AppAction::SelectBranch(index) => self.select_branch(model, index),
            AppAction::ToggleBranchListNode(id) => self.toggle_branch_list_node(model, &id),
            AppAction::ToggleWorkingTreeListNode(id) => {
                self.toggle_working_tree_list_node(model, &id)
            },
            AppAction::SelectUncommittedChanges => self.select_uncommitted_changes(model),
            AppAction::RevertWorkingTreeFile(path) => self.revert_working_tree_file(model, &path),
            AppAction::RevertAllWorkingTreeChanges => self.revert_all_working_tree_changes(model),
            AppAction::CommitWorkingTree {
                message,
                push_after,
            } => self.commit_working_tree(model, &message, push_after),
            AppAction::SelectCommit(index) => self.select_commit(model, index),
            AppAction::SelectOlderCommit => self.select_older_commit(model),
            AppAction::SelectNewerCommit => self.select_newer_commit(model),
            AppAction::SetSearchQuery(query) => self.set_search_query(model, query),
            // Resolved to `SetBranchFilter` by the app delegate before it gets here.
            AppAction::BranchFilterChanged => {}
            AppAction::SetBranchFilter(filter) => self.set_branch_filter(model, filter),
            AppAction::ToggleShowRemote => {
                model.show_remote = !model.show_remote;
                model.last_action = if model.show_remote {
                    "Showing remote branches".to_owned()
                } else {
                    "Hiding remote branches".to_owned()
                };
            }
            AppAction::RunGitOperation(operation) => self.run_git_operation(model, operation),
            // Handled by the git service.
            AppAction::OpenRepository(path) => {
                model.last_action = format!("Opening {}", path.display());
            }
            AppAction::RepositoryLoaded(loaded) => self.repository_loaded(model, *loaded),
            AppAction::RepositoryLoadFailed(message) => model.last_action = message,
            AppAction::SetLastAction(message) => model.last_action = message,
            AppAction::CommitFilesLoaded { index, id, files } => {
                if let Some(commit) = model.commits.get_mut(index) {
                    if commit.id == id {
                        commit.files = files;
                    }
                }
            }
        }
    }

    /// Adds (or replaces) the repository, then shows its branches and graph with the latest
    /// commit of the current branch selected.
    fn repository_loaded(&mut self, model: &mut AppModel, loaded: LoadedRepo) {
        let keep_uncommitted_selection = model.selected_uncommitted
            && model
                .active_repository()
                .is_some_and(|repo| repo.path == loaded.path)
            && loaded.working_tree.has_changes();
        let repository = Repository {
            name: loaded.name.clone(),
            path: loaded.path.clone(),
            branch_count: loaded.branches.len(),
            has_uncommitted_changes: loaded.working_tree.has_changes(),
        };
        if let Some(index) = model
            .repositories
            .iter()
            .position(|repo| repo.path == loaded.path)
        {
            model.repositories.remove(index);
        }
        model.repositories.insert(0, repository);
        model.active_repo = 0;

        model.branches = loaded.branches;
        model.tags = loaded.tags;
        model.active_branch = loaded.active_branch;
        model.working_tree = loaded.working_tree;
        model.commits = loaded.commits;
        model.selected_commit = loaded.selected_commit;
        model.selected_uncommitted = keep_uncommitted_selection;
        model.branch_filter = None;
        model.last_action = match persistent::save_recent_repositories(&model.repositories) {
            Ok(()) => format!("Loaded repository {}", loaded.name),
            Err(message) => format!(
                "Loaded repository {} (failed to update recent list: {message})",
                loaded.name
            ),
        };
    }

    fn select_repository(&mut self, model: &mut AppModel, index: usize) {
        if let Some(repository) = model.repositories.get(index) {
            model.active_repo = index;
            model.last_action = format!("Switched to repository {}", repository.name);
        }
    }

    fn select_branch(&mut self, model: &mut AppModel, index: usize) {
        if let Some(branch) = model.branches.get(index) {
            model.active_branch = index;
            model.last_action = format!("Checked branch focus on {}", branch.name);
        }
    }

    fn toggle_branch_list_node(&mut self, model: &mut AppModel, id: &str) {
        let expanded = model.toggle_sidebar_node(id);
        model.last_action = if expanded {
            format!("Expanded {}", id.rsplit('/').next().unwrap_or(id))
        } else {
            format!("Collapsed {}", id.rsplit('/').next().unwrap_or(id))
        };
    }

    fn toggle_working_tree_list_node(&mut self, model: &mut AppModel, id: &str) {
        let expanded = model.toggle_working_tree_node(id);
        model.last_action = if expanded {
            format!("Expanded {}", id.rsplit('/').next().unwrap_or(id))
        } else {
            format!("Collapsed {}", id.rsplit('/').next().unwrap_or(id))
        };
    }

    fn select_commit(&mut self, model: &mut AppModel, index: usize) {
        if let Some(commit) = model.commits.get(index) {
            model.selected_uncommitted = false;
            model.selected_commit = index;
            model.last_action = format!("Selected commit {}", commit.short_id);
        }
    }

    fn select_uncommitted_changes(&mut self, model: &mut AppModel) {
        if model.has_uncommitted_changes() {
            model.selected_uncommitted = true;
            model.last_action = format!(
                "Selected {} uncommitted change{}",
                model.working_tree.total_count(),
                if model.working_tree.total_count() == 1 {
                    ""
                } else {
                    "s"
                }
            );
        }
    }

    fn revert_working_tree_file(&mut self, model: &mut AppModel, path: &str) {
        model.last_action = format!("Reverting {path}...");
    }

    fn revert_all_working_tree_changes(&mut self, model: &mut AppModel) {
        model.last_action = "Reverting all working tree changes...".to_owned();
    }

    fn commit_working_tree(&mut self, model: &mut AppModel, message: &str, push_after: bool) {
        let message = message.trim();
        model.last_action = if message.is_empty() {
            "Commit message is required".to_owned()
        } else if push_after {
            "Committing changes and pushing...".to_owned()
        } else {
            "Committing changes...".to_owned()
        };
    }

    fn select_older_commit(&mut self, model: &mut AppModel) {
        if model.selected_uncommitted {
            if let Some(index) = model.visible_commits().first().copied() {
                self.select_commit(model, index);
            }
            return;
        }

        if let Some(index) = model.older_commit_index() {
            self.select_commit(model, index);
        }
    }

    fn select_newer_commit(&mut self, model: &mut AppModel) {
        if model.selected_uncommitted {
            return;
        }

        let visible = model.visible_commits();
        if visible.first().copied() == Some(model.selected_commit)
            && model.has_uncommitted_changes()
        {
            self.select_uncommitted_changes(model);
            return;
        }

        if let Some(index) = model.newer_commit_index() {
            self.select_commit(model, index);
        }
    }

    fn set_branch_filter(&mut self, model: &mut AppModel, filter: Option<usize>) {
        model.branch_filter = filter.filter(|&i| i < model.branches.len());

        let visible = model.visible_commits();
        if let Some(branch) = model.branch_filter.and_then(|i| model.branches.get(i)) {
            let branch_name = branch.name.clone();
            let repo_path = model
                .repositories
                .get(model.active_repo)
                .map(|repo| repo.path.as_str())
                .unwrap_or("<unknown>");
            let tip_matches: Vec<&str> = model
                .commits
                .iter()
                .filter(|commit| commit.refs.iter().any(|r| r.label == branch_name))
                .map(|commit| commit.id.as_str())
                .collect();
            eprintln!(
                "[branch-history] repo={} branch={} tip_matches={:?} included={}/{}",
                repo_path,
                branch_name,
                tip_matches,
                visible.len(),
                model.commits.len()
            );
            for &index in &visible {
                let commit = &model.commits[index];
                let parents: Vec<&str> = commit
                    .parents
                    .iter()
                    .filter_map(|&parent| model.commits.get(parent))
                    .map(|parent| parent.id.as_str())
                    .collect();
                let refs: Vec<&str> = commit.refs.iter().map(|r| r.label.as_str()).collect();
                eprintln!(
                    "[branch-history] include id={} parents={:?} refs={:?} subject={:?}",
                    commit.id, parents, refs, commit.summary
                );
            }
        }
        if !visible.contains(&model.selected_commit) {
            if let Some(&first) = visible.first() {
                self.select_commit(model, first);
            }
        }

        model.last_action = match model.branch_filter.and_then(|i| model.branches.get(i)) {
            Some(branch) => format!("Showing history of {}", branch.name),
            None => "Showing all branches".to_owned(),
        };
    }

    fn set_search_query(&mut self, model: &mut AppModel, query: String) {
        model.search_query = query;
        let trimmed = model.search_query.trim();
        model.last_action = if trimmed.is_empty() {
            "Cleared commit search".to_owned()
        } else {
            format!("Searching commits for \"{trimmed}\"")
        };
    }

    fn update_config(&mut self, model: &mut AppModel, config: crate::app::model::AppConfig) {
        model.config = config;
        model.last_action = "Saved settings".to_owned();
    }

    fn run_git_operation(&mut self, model: &mut AppModel, operation: GitOperation) {
        let repo_name = model
            .active_repository()
            .map(|repo| repo.name.as_str())
            .unwrap_or("current repository");
        let branch_name = model
            .active_branch()
            .map(|branch| branch.name.as_str())
            .unwrap_or("current branch");

        model.last_action = match operation {
            GitOperation::Fetch => format!("{} requested for {repo_name}", operation.label()),
            GitOperation::Pull | GitOperation::Push => {
                format!(
                    "{} requested for {repo_name} on {branch_name}",
                    operation.label()
                )
            }
            GitOperation::Refresh => format!("{} requested for {repo_name}", operation.label()),
        };
    }
}

#[cfg(test)]
mod tests;
