use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use crate::app::persistent;

#[cfg(test)]
use crate::app::sample_data;

/// Model shared between the app delegate and the list view delegates.
pub type SharedModel = Arc<Mutex<AppModel>>;

#[derive(Clone, Debug)]
pub struct AppModel {
    pub config: AppConfig,
    pub repositories: Vec<Repository>,
    pub active_repo: usize,
    pub branches: Vec<Branch>,
    pub tags: Vec<String>,
    pub active_branch: usize,
    pub working_tree: WorkingTreeChanges,
    pub commits: Vec<CommitNode>,
    pub selected_commit: usize,
    pub selected_uncommitted: bool,
    /// Index into `branches`; `None` shows every commit.
    pub branch_filter: Option<usize>,
    pub show_remote: bool,
    pub search_query: String,
    pub last_action: String,
    pub collapsed_sidebar_nodes: BTreeSet<String>,
    pub collapsed_working_tree_nodes: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfig {
    pub autorefresh_time: u64,
    pub ai: AiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            autorefresh_time: 0,
            ai: AiConfig::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiConfig {
    pub provider_cli: AiProviderCli,
    pub cli_path: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider_cli: AiProviderCli::OpenAi,
            cli_path: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiProviderCli {
    OpenAi,
    Gemini,
    Anthropic,
    Copilot,
}

impl AiProviderCli {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
            Self::Anthropic => "anthropic",
            Self::Copilot => "copilot",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAi),
            "gemini" => Some(Self::Gemini),
            "anthropic" => Some(Self::Anthropic),
            "copilot" => Some(Self::Copilot),
            _ => None,
        }
    }
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
    pub working_tree: WorkingTreeChanges,
    pub commits: Vec<CommitNode>,
    pub selected_commit: usize,
}

#[derive(Clone, Debug, Default)]
pub struct WorkingTreeChanges {
    pub files: Vec<WorkingTreeFile>,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
}

impl WorkingTreeChanges {
    pub fn has_changes(&self) -> bool {
        !self.files.is_empty()
    }

    pub fn total_count(&self) -> usize {
        self.files.len()
    }

    pub fn staged_files(&self) -> Vec<&WorkingTreeFile> {
        self.files.iter().filter(|file| file.staged).collect()
    }

    pub fn unstaged_files(&self) -> Vec<&WorkingTreeFile> {
        self.files
            .iter()
            .filter(|file| file.unstaged || file.untracked)
            .collect()
    }

    pub fn changed_files(&self) -> Vec<&WorkingTreeFile> {
        self.files.iter().filter(|file| !file.untracked).collect()
    }

    pub fn unversioned_files(&self) -> Vec<&WorkingTreeFile> {
        self.files.iter().filter(|file| file.untracked).collect()
    }
}

/// One row of the branch sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarItem {
    Section {
        id: String,
        title: String,
        expanded: bool,
    },
    Group {
        id: String,
        title: String,
        depth: usize,
        expanded: bool,
    },
    /// Index into `AppModel::branches`.
    Branch {
        index: usize,
        title: String,
        depth: usize,
        head_copy: bool,
    },
    /// Index into `AppModel::tags`.
    Tag { index: usize, depth: usize },
}

#[derive(Clone, Debug, Default)]
struct SidebarTree {
    nodes: Vec<SidebarTreeNode>,
}

#[derive(Clone, Debug)]
enum SidebarTreeNode {
    Group(SidebarTreeGroup),
    Branch { index: usize, title: String },
}

#[derive(Clone, Debug)]
struct SidebarTreeGroup {
    id: String,
    title: String,
    children: SidebarTree,
}

#[derive(Clone, Debug)]
pub struct Branch {
    pub name: String,
    pub ahead: u8,
    pub behind: u8,
    pub is_remote: bool,
    pub is_current: bool,
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
    pub is_mainline: bool,
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

#[derive(Clone, Debug)]
pub struct WorkingTreeFile {
    pub repo_path: String,
    pub display_path: String,
    pub status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
}

impl WorkingTreeFile {
    pub fn staged_status_label(&self) -> String {
        if self.untracked {
            return "??".to_owned();
        }

        self.status
            .chars()
            .next()
            .filter(|ch| !ch.is_whitespace())
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| self.status.trim().to_owned())
    }

    pub fn unstaged_status_label(&self) -> String {
        if self.untracked {
            return "??".to_owned();
        }

        self.status
            .chars()
            .nth(1)
            .filter(|ch| !ch.is_whitespace())
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| self.status.trim().to_owned())
    }
}

impl AppModel {
    pub fn new() -> Self {
        let (config, config_error) = match persistent::load_config() {
            Ok(config) => (config, None),
            Err(message) => (AppConfig::default(), Some(message)),
        };

        match persistent::load_recent_repositories() {
            Ok(repositories) => {
                let active_repo = repositories.len();
                let last_action = if let Some(message) = config_error {
                    format!("Config unavailable: {message}")
                } else if repositories.is_empty() {
                    "Open a repository to get started".to_owned()
                } else {
                    format!("Loaded {} recent repositories", repositories.len())
                };

                Self {
                    config,
                    repositories,
                    active_repo,
                    branches: Vec::new(),
                    tags: Vec::new(),
                    active_branch: 0,
                    working_tree: WorkingTreeChanges::default(),
                    commits: Vec::new(),
                    selected_commit: 0,
                    selected_uncommitted: false,
                    branch_filter: None,
                    show_remote: true,
                    search_query: String::new(),
                    last_action,
                    collapsed_sidebar_nodes: BTreeSet::new(),
                    collapsed_working_tree_nodes: BTreeSet::new(),
                }
            }
            Err(message) => Self {
                config,
                repositories: Vec::new(),
                active_repo: 0,
                branches: Vec::new(),
                tags: Vec::new(),
                active_branch: 0,
                working_tree: WorkingTreeChanges::default(),
                commits: Vec::new(),
                selected_commit: 0,
                selected_uncommitted: false,
                branch_filter: None,
                show_remote: true,
                search_query: String::new(),
                last_action: format!("Recent repositories unavailable: {message}"),
                collapsed_sidebar_nodes: BTreeSet::new(),
                collapsed_working_tree_nodes: BTreeSet::new(),
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn sample() -> Self {
        let repositories = sample_data::sample_repositories();
        let branches = sample_data::sample_branches();
        let commits = sample_data::sample_commits();

        Self {
            config: AppConfig::default(),
            repositories,
            active_repo: 0,
            branches,
            tags: Vec::new(),
            active_branch: 0,
            working_tree: sample_data::sample_working_tree_changes(),
            commits,
            selected_commit: 0,
            selected_uncommitted: false,
            branch_filter: None,
            show_remote: true,
            search_query: String::new(),
            last_action: "Loaded mock repository graph".to_owned(),
            collapsed_sidebar_nodes: BTreeSet::new(),
            collapsed_working_tree_nodes: BTreeSet::new(),
        }
    }

    pub fn active_repository(&self) -> Option<&Repository> {
        self.repositories.get(self.active_repo)
    }

    pub fn active_branch(&self) -> Option<&Branch> {
        self.branches.get(self.active_branch)
    }

    pub fn current_branch(&self) -> Option<&Branch> {
        self.current_branch_index()
            .and_then(|index| self.branches.get(index))
    }

    pub fn selected_commit(&self) -> Option<&CommitNode> {
        if self.selected_uncommitted {
            return None;
        }
        self.commits.get(self.selected_commit)
    }

    pub fn has_uncommitted_changes(&self) -> bool {
        self.working_tree.has_changes()
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
        if self.selected_uncommitted {
            return false;
        }

        let visible = self.visible_commits();
        match visible.iter().position(|&i| i == self.selected_commit) {
            Some(0) => self.has_uncommitted_changes(),
            Some(position) => position > 0,
            None => false,
        }
    }

    pub fn can_select_older_commit(&self) -> bool {
        if self.selected_uncommitted {
            return !self.visible_commits().is_empty();
        }
        self.older_commit_index().is_some()
    }

    pub fn commit_list_len(&self) -> usize {
        self.visible_commits().len() + usize::from(self.has_uncommitted_changes())
    }

    pub fn commit_index_for_row(&self, row: usize) -> Option<usize> {
        let offset = usize::from(self.has_uncommitted_changes());
        self.visible_commits()
            .get(row.checked_sub(offset)?)
            .copied()
    }

    pub fn selected_commit_row(&self) -> Option<usize> {
        if self.selected_uncommitted {
            return self.has_uncommitted_changes().then_some(0);
        }

        let offset = usize::from(self.has_uncommitted_changes());
        self.visible_commits()
            .iter()
            .position(|&i| i == self.selected_commit)
            .map(|row| row + offset)
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

    pub fn current_branch_index(&self) -> Option<usize> {
        self.branches.iter().position(|branch| branch.is_current)
    }

    /// Sidebar rows for the tree-style branch list.
    pub fn sidebar_items(&self) -> Vec<SidebarItem> {
        let mut items = Vec::new();

        if let Some(index) = self.current_branch_index() {
            let id = "section/head";
            let expanded = self.sidebar_node_expanded(id);
            items.push(SidebarItem::Section {
                id: id.to_owned(),
                title: "HEAD (Current Branch)".to_owned(),
                expanded,
            });
            if expanded {
                items.push(SidebarItem::Branch {
                    index,
                    title: branch_leaf_title(&self.branches[index]),
                    depth: 1,
                    head_copy: true,
                });
            }
        }

        let local = self.build_branch_tree(false);
        if !local.nodes.is_empty() {
            self.push_tree_section(&mut items, "section/local", "Local", &local);
        }

        if self.show_remote {
            let remote = self.build_branch_tree(true);
            if !remote.nodes.is_empty() {
                self.push_tree_section(&mut items, "section/remote", "Remote", &remote);
            }
        }

        if !self.tags.is_empty() {
            let id = "section/tags";
            let expanded = self.sidebar_node_expanded(id);
            items.push(SidebarItem::Section {
                id: id.to_owned(),
                title: "Tags".to_owned(),
                expanded,
            });
            if expanded {
                items
                    .extend((0..self.tags.len()).map(|index| SidebarItem::Tag { index, depth: 1 }));
            }
        }

        items
    }

    /// Sidebar row index showing the given branch.
    pub fn sidebar_row_of_branch(&self, branch: usize) -> Option<usize> {
        let mut head_row = None;
        for (row, item) in self.sidebar_items().iter().enumerate() {
            if let SidebarItem::Branch {
                index, head_copy, ..
            } = item
            {
                if *index != branch {
                    continue;
                }
                if !head_copy {
                    return Some(row);
                }
                if head_row.is_none() {
                    head_row = Some(row);
                }
            }
        }
        head_row
    }

    pub fn toggle_sidebar_node(&mut self, id: &str) -> bool {
        if !self.collapsed_sidebar_nodes.remove(id) {
            self.collapsed_sidebar_nodes.insert(id.to_owned());
            return false;
        }
        true
    }

    pub fn toggle_working_tree_node(&mut self, id: &str) -> bool {
        if !self.collapsed_working_tree_nodes.remove(id) {
            self.collapsed_working_tree_nodes.insert(id.to_owned());
            return false;
        }
        true
    }

    pub fn branch_status(branch: &Branch) -> String {
        match (branch.ahead, branch.behind) {
            (0, 0) => "up to date".to_owned(),
            (ahead, 0) => format!("ahead {ahead}"),
            (0, behind) => format!("behind {behind}"),
            (ahead, behind) => format!("ahead {ahead} • behind {behind}"),
        }
    }

    fn sidebar_node_expanded(&self, id: &str) -> bool {
        !self.collapsed_sidebar_nodes.contains(id)
    }

    pub(crate) fn working_tree_node_expanded(&self, id: &str) -> bool {
        !self.collapsed_working_tree_nodes.contains(id)
    }

    fn push_tree_section(
        &self,
        items: &mut Vec<SidebarItem>,
        id: &str,
        title: &str,
        tree: &SidebarTree,
    ) {
        let expanded = self.sidebar_node_expanded(id);
        items.push(SidebarItem::Section {
            id: id.to_owned(),
            title: title.to_owned(),
            expanded,
        });
        if expanded {
            self.append_tree_items(items, &tree.nodes, 1);
        }
    }

    fn append_tree_items(
        &self,
        items: &mut Vec<SidebarItem>,
        nodes: &[SidebarTreeNode],
        depth: usize,
    ) {
        for node in nodes {
            match node {
                SidebarTreeNode::Group(group) => {
                    let expanded = self.sidebar_node_expanded(&group.id);
                    items.push(SidebarItem::Group {
                        id: group.id.clone(),
                        title: group.title.clone(),
                        depth,
                        expanded,
                    });
                    if expanded {
                        self.append_tree_items(items, &group.children.nodes, depth + 1);
                    }
                }
                SidebarTreeNode::Branch { index, title } => items.push(SidebarItem::Branch {
                    index: *index,
                    title: title.clone(),
                    depth,
                    head_copy: false,
                }),
            }
        }
    }

    fn build_branch_tree(&self, remote: bool) -> SidebarTree {
        let mut tree = SidebarTree::default();
        for (index, branch) in self.branches.iter().enumerate() {
            if branch.is_remote != remote {
                continue;
            }

            let parts: Vec<&str> = branch.name.split('/').collect();
            let title = branch_leaf_title(branch);
            insert_sidebar_branch(&mut tree, &section_prefix(remote), &parts, index, title);
        }
        tree
    }
}

fn section_prefix(remote: bool) -> String {
    if remote {
        "remote".to_owned()
    } else {
        "local".to_owned()
    }
}

fn branch_leaf_title(branch: &Branch) -> String {
    branch
        .name
        .rsplit('/')
        .next()
        .unwrap_or(branch.name.as_str())
        .to_owned()
}

fn insert_sidebar_branch(
    tree: &mut SidebarTree,
    prefix: &str,
    parts: &[&str],
    index: usize,
    title: String,
) {
    if parts.len() <= 1 {
        tree.nodes.push(SidebarTreeNode::Branch { index, title });
        return;
    }

    let name = parts[0];
    let id = format!("{prefix}/{name}");
    let group_index = tree.nodes.iter().position(|node| match node {
        SidebarTreeNode::Group(group) => group.title == name,
        SidebarTreeNode::Branch { .. } => false,
    });

    let child_prefix = id.clone();
    let group = match group_index {
        Some(i) => match &mut tree.nodes[i] {
            SidebarTreeNode::Group(group) => group,
            SidebarTreeNode::Branch { .. } => unreachable!(),
        },
        None => {
            tree.nodes.push(SidebarTreeNode::Group(SidebarTreeGroup {
                id,
                title: name.to_owned(),
                children: SidebarTree::default(),
            }));
            match tree.nodes.last_mut() {
                Some(SidebarTreeNode::Group(group)) => group,
                _ => unreachable!(),
            }
        }
    };

    insert_sidebar_branch(
        &mut group.children,
        &child_prefix,
        &parts[1..],
        index,
        title,
    );
}

#[cfg(test)]
mod tests;
