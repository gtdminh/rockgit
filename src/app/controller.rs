use crate::app::{
    actions::{AppAction, GitOperation},
    model::{AppModel, LoadedRepo, Repository},
};

#[derive(Default)]
pub struct AppController;

impl AppController {
    pub fn dispatch(&mut self, model: &mut AppModel, action: AppAction) {
        match action {
            AppAction::SelectRepository(index) => self.select_repository(model, index),
            AppAction::SelectBranch(index) => self.select_branch(model, index),
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
        let repository = Repository {
            name: loaded.name.clone(),
            path: loaded.path.clone(),
            branch_count: loaded.branches.len(),
            has_uncommitted_changes: false,
        };
        match model.repositories.iter().position(|r| r.path == loaded.path) {
            Some(index) => {
                model.repositories[index] = repository;
                model.active_repo = index;
            }
            None => {
                model.repositories.push(repository);
                model.active_repo = model.repositories.len() - 1;
            }
        }

        model.branches = loaded.branches;
        model.tags = loaded.tags;
        model.active_branch = loaded.active_branch;
        model.commits = loaded.commits;
        model.selected_commit = loaded.selected_commit;
        model.branch_filter = None;
        model.last_action = format!("Loaded repository {}", loaded.name);
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

    fn select_commit(&mut self, model: &mut AppModel, index: usize) {
        if let Some(commit) = model.commits.get(index) {
            model.selected_commit = index;
            model.last_action = format!("Selected commit {}", commit.short_id);
        }
    }

    fn select_older_commit(&mut self, model: &mut AppModel) {
        if let Some(index) = model.older_commit_index() {
            self.select_commit(model, index);
        }
    }

    fn select_newer_commit(&mut self, model: &mut AppModel) {
        if let Some(index) = model.newer_commit_index() {
            self.select_commit(model, index);
        }
    }

    fn set_branch_filter(&mut self, model: &mut AppModel, filter: Option<usize>) {
        model.branch_filter = filter.filter(|&i| i < model.branches.len());

        let visible = model.visible_commits();
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
                format!("{} requested for {repo_name} on {branch_name}", operation.label())
            }
            GitOperation::Refresh => format!("{} requested for {repo_name}", operation.label()),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        actions::{AppAction, GitOperation},
        model::AppModel,
    };

    #[test]
    fn select_repository_updates_model() {
        let mut model = AppModel::new();
        let mut controller = AppController;

        controller.dispatch(&mut model, AppAction::SelectRepository(2));

        assert_eq!(model.active_repo, 2);
        assert!(model.last_action.contains("infra-config"));
    }

    #[test]
    fn selecting_older_commit_stops_at_end() {
        let mut model = AppModel::new();
        let mut controller = AppController;
        model.selected_commit = model.commits.len() - 1;

        controller.dispatch(&mut model, AppAction::SelectOlderCommit);

        assert_eq!(model.selected_commit, model.commits.len() - 1);
    }

    #[test]
    fn run_operation_uses_active_context() {
        let mut model = AppModel::new();
        let mut controller = AppController;
        model.active_repo = 0;
        model.active_branch = 1;

        controller.dispatch(&mut model, AppAction::RunGitOperation(GitOperation::Push));

        assert!(model.last_action.contains("Push requested"));
        assert!(model.last_action.contains("RockGit"));
        assert!(model.last_action.contains("feature/graph-ui"));
    }
}

