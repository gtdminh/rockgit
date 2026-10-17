use std::sync::{Arc, Mutex};

use crate::app::sample_data;

/// Model shared between the app delegate and the list view delegates.
pub type SharedModel = Arc<Mutex<AppModel>>;

#[derive(Clone, Debug)]
pub struct AppModel {
    pub repositories: Vec<Repository>,
    pub active_repo: usize,
    pub branches: Vec<Branch>,
    pub tags: Vec<String>,
    pub active_branch: usize,
    pub commits: Vec<CommitNode>,
    pub selected_commit: usize,
    /// Index into `branches`; `None` shows every commit.
    pub branch_filter: Option<usize>,
    pub show_remote: bool,
    pub search_query: String,
    pub last_action: String,
}

#[derive(Clone, Debug)]
pub struct Repository {
    pub name: String,
    pub path: String,
    pub branch_count: usize,
    pub has_uncommitted_changes: bool,
}

/// A repository read by the git service: branches, graph and the initial selection.
#[derive(Clone, Debug)]
pub struct LoadedRepo {
    pub name: String,
    pub path: String,
    pub branches: Vec<Branch>,
    pub tags: Vec<String>,
    pub active_branch: usize,
    pub commits: Vec<CommitNode>,
    pub selected_commit: usize,
}

/// One row of the sidebar branch list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarItem {
    Header(&'static str),
    /// Index into `AppModel::branches`.
    Branch(usize),
    /// Index into `AppModel::tags`.
    Tag(usize),
}
#[derive(Clone, Debug)]
pub struct Branch {
    pub name: String,
    pub ahead: u8,
    pub behind: u8,
    pub is_remote: bool,
}

#[derive(Clone, Debug)]
pub struct CommitNode {
    /// Full commit id; empty for sample data.
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author: String,
    pub relative_time: String,
    pub lane: usize,
    pub parents: Vec<usize>,
    pub refs: Vec<RefBadge>,
    pub files: Vec<FileChange>,
    pub is_head: bool,
}

#[derive(Clone, Debug)]
pub struct RefBadge {
    pub label: String,
    pub kind: RefKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    Head,
    LocalBranch,
    RemoteBranch,
    Tag,
}

#[derive(Clone, Debug)]
pub struct FileChange {
    pub path: String,
    pub status: String,
}

impl AppModel {
    pub fn new() -> Self {
        let repositories = sample_data::sample_repositories();
        let branches = sample_data::sample_branches();
        let commits = sample_data::sample_commits();

        Self {
            repositories,
            active_repo: 0,
            branches,
            tags: Vec::new(),
            active_branch: 0,
            commits,
            selected_commit: 0,
            branch_filter: None,
            show_remote: true,
            search_query: String::new(),
            last_action: "Loaded mock repository graph".to_owned(),
        }
    }

    pub fn active_repository(&self) -> Option<&Repository> {
        self.repositories.get(self.active_repo)
    }

    pub fn active_branch(&self) -> Option<&Branch> {
        self.branches.get(self.active_branch)
    }

    pub fn selected_commit(&self) -> Option<&CommitNode> {
        self.commits.get(self.selected_commit)
    }

    pub fn commit_matches_query(&self, commit: &CommitNode) -> bool {
        let query = self.search_query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return false;
        }

        commit.summary.to_ascii_lowercase().contains(&query)
            || commit.author.to_ascii_lowercase().contains(&query)
            || commit.short_id.to_ascii_lowercase().contains(&query)
            || commit
                .refs
                .iter()
                .any(|badge| badge.label.to_ascii_lowercase().contains(&query))
    }

    pub fn matching_commit_count(&self) -> usize {
        if self.search_query.trim().is_empty() {
            self.commits.len()
        } else {
            self.commits
                .iter()
                .filter(|commit| self.commit_matches_query(commit))
                .count()
        }
    }

    pub fn can_select_newer_commit(&self) -> bool {
        self.newer_commit_index().is_some()
    }

    pub fn can_select_older_commit(&self) -> bool {
        self.older_commit_index().is_some()
    }

    /// Indexes into `commits` that pass the branch filter, newest first.
    pub fn visible_commits(&self) -> Vec<usize> {
        let all = || (0..self.commits.len()).collect::<Vec<_>>();
        let Some(branch) = self.branch_filter.and_then(|i| self.branches.get(i)) else {
            return all();
        };
        let Some(tip) = self
            .commits
            .iter()
            .position(|c| c.refs.iter().any(|r| r.label == branch.name))
        else {
            return all();
        };

        let mut seen = vec![false; self.commits.len()];
        let mut stack = vec![tip];
        while let Some(index) = stack.pop() {
            if index >= seen.len() || seen[index] {
                continue;
            }
            seen[index] = true;
            stack.extend(self.commits[index].parents.iter().copied());
        }
        (0..seen.len()).filter(|&i| seen[i]).collect()
    }

    pub fn newer_commit_index(&self) -> Option<usize> {
        let visible = self.visible_commits();
        let position = visible.iter().position(|&i| i == self.selected_commit)?;
        position.checked_sub(1).map(|p| visible[p])
    }

    pub fn older_commit_index(&self) -> Option<usize> {
        let visible = self.visible_commits();
        let position = visible.iter().position(|&i| i == self.selected_commit)?;
        visible.get(position + 1).copied()
    }

    /// Sidebar rows: Local, Remote/Branches and Remote/Tags sections (empty sections omitted).
    pub fn sidebar_items(&self) -> Vec<SidebarItem> {
        let mut items = Vec::new();
        let local: Vec<usize> = (0..self.branches.len()).filter(|&i| !self.branches[i].is_remote).collect();
        let remote: Vec<usize> = (0..self.branches.len()).filter(|&i| self.branches[i].is_remote).collect();
        if !local.is_empty() {
            items.push(SidebarItem::Header("LOCAL"));
            items.extend(local.into_iter().map(SidebarItem::Branch));
        }
        if !remote.is_empty() {
            items.push(SidebarItem::Header("REMOTE / BRANCHES"));
            items.extend(remote.into_iter().map(SidebarItem::Branch));
        }
        if !self.tags.is_empty() {
            items.push(SidebarItem::Header("REMOTE / TAGS"));
            items.extend((0..self.tags.len()).map(SidebarItem::Tag));
        }
        items
    }

    /// Sidebar row index showing the given branch.
    pub fn sidebar_row_of_branch(&self, branch: usize) -> Option<usize> {
        self.sidebar_items().iter().position(|i| *i == SidebarItem::Branch(branch))
    }

    pub fn branch_status(branch: &Branch) -> String {
        match (branch.ahead, branch.behind) {
            (0, 0) => "up to date".to_owned(),
            (ahead, 0) => format!("ahead {ahead}"),
            (0, behind) => format!("behind {behind}"),
            (ahead, behind) => format!("ahead {ahead} • behind {behind}"),
        }
    }
}


