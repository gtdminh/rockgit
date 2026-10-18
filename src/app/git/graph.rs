use std::collections::{HashMap, HashSet};

use super::client::GitClient;
use super::error::GitResult;
use super::history::LogOptions;
use super::types::{Commit, DecorationKind};

/// A line segment between two lane columns within one graph row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: usize,
    pub to: usize,
    /// Stable per-branch-line color index; take it modulo your palette size.
    pub color: usize,
}

/// One row of a commit graph: the node plus the lines to draw around it.
///
/// Draw the top half of the row from `incoming` and `passthrough`, the bottom half from
/// `outgoing` and `passthrough`. Edges go from `from` (column at the row boundary) to `to`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRow {
    pub commit: Commit,
    /// Column of the commit's node.
    pub lane: usize,
    pub color: usize,
    pub is_mainline: bool,
    /// Number of columns this row needs.
    pub width: usize,
    /// Lines from the row above that end at this node (branches converging).
    pub incoming: Vec<GraphEdge>,
    /// Lines that run straight through this row, untouched by this commit.
    pub passthrough: Vec<GraphEdge>,
    /// Lines from this node down to each parent's column in the next row.
    pub outgoing: Vec<GraphEdge>,
}

struct Lane {
    /// Commit this lane is waiting to reach.
    id: String,
    color: usize,
}

fn free_slot(lanes: &mut Vec<Option<Lane>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        },
    }
}

fn preferred_main_ref_score(commit: &Commit) -> Option<usize> {
    commit
        .refs
        .iter()
        .filter_map(|reference| {
            let name = reference.name.as_str();
            match (reference.kind, name) {
                (DecorationKind::LocalBranch, "main") => Some(0),
                (DecorationKind::LocalBranch, "master") => Some(1),
                (DecorationKind::LocalBranch, "trunk") => Some(2),
                (DecorationKind::RemoteBranch, "origin/main") => Some(3),
                (DecorationKind::RemoteBranch, "origin/master") => Some(4),
                (DecorationKind::RemoteBranch, "origin/trunk") => Some(5),
                _ => None,
            }
        })
        .min()
}

fn mainline_commit_ids(commits: &[Commit]) -> HashSet<String> {
    let Some((tip_id, _)) = commits
        .iter()
        .filter_map(|commit| preferred_main_ref_score(commit).map(|score| (commit.id.as_str(), score)))
        .min_by_key(|(_, score)| *score)
    else {
        return HashSet::new();
    };

    let first_parent_of: HashMap<&str, &str> = commits
        .iter()
        .filter_map(|commit| commit.parents.first().map(|parent| (commit.id.as_str(), parent.as_str())))
        .collect();
    let known_ids: HashSet<&str> = commits.iter().map(|commit| commit.id.as_str()).collect();

    let mut ids = HashSet::new();
    let mut current = tip_id;
    while known_ids.contains(current) && ids.insert(current.to_owned()) {
        let Some(parent) = first_parent_of.get(current).copied() else {
            break;
        };
        current = parent;
    }
    ids
}

fn reserve_leftmost_lane_for_mainline(lanes: &mut Vec<Option<Lane>>, waiting: &mut [usize]) {
    if waiting.contains(&0) {
        return;
    }

    if lanes.is_empty() {
        lanes.push(None);
    }

    if lanes[0].is_some() {
        let slot = free_slot(lanes);
        lanes[slot] = lanes[0].take();
    }

    if let Some(position) = waiting.iter().position(|&lane| lane != 0) {
        let from = waiting[position];
        lanes[0] = lanes[from].take();
    }
}

/// Assigns lanes to commits. `commits` must be ordered children-before-parents (`LogOrder::Topo`).
/// Parents missing from the slice (truncated history) simply leave their lane open at the bottom.
pub fn build_graph(commits: Vec<Commit>) -> Vec<GraphRow> {
    let mainline_ids = mainline_commit_ids(&commits);
    let mut lanes: Vec<Option<Lane>> = Vec::new();
    let mut next_color = 0usize;
    let mut rows = Vec::with_capacity(commits.len());

    for commit in commits {
        let is_mainline = mainline_ids.contains(&commit.id);
        let width_before = lanes.len();
        let mut waiting: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.as_ref().filter(|l| l.id == commit.id).map(|_| i))
            .collect();

        if is_mainline {
            reserve_leftmost_lane_for_mainline(&mut lanes, &mut waiting);
            if waiting.is_empty() {
                waiting.push(0);
            } else if let Some(first) = waiting.first_mut() {
                *first = 0;
            }
        }

        let (lane, color) = match waiting.first() {
            Some(&i) => (i, lanes[i].as_ref().map_or(0, |l| l.color)),
            None => {
                let i = if is_mainline {
                    if lanes.is_empty() {
                        lanes.push(None);
                    }
                    0
                } else {
                    free_slot(&mut lanes)
                };
                let color = next_color;
                next_color += 1;
                (i, color)
            },
        };

        let incoming: Vec<GraphEdge> = waiting
            .iter()
            .map(|&j| GraphEdge {
                from: j,
                to: lane,
                color: lanes[j].as_ref().map_or(color, |l| l.color),
            })
            .collect();
        let passthrough: Vec<GraphEdge> = lanes
            .iter()
            .enumerate()
            .filter_map(|(j, l)| {
                l.as_ref()
                    .filter(|l| l.id != commit.id)
                    .map(|l| GraphEdge { from: j, to: j, color: l.color })
            })
            .collect();

        for &j in &waiting {
            lanes[j] = None;
        }

        let mut outgoing = Vec::new();
        for (n, parent) in commit.parents.iter().enumerate() {
            if let Some(existing) = lanes.iter().position(|l| l.as_ref().is_some_and(|l| &l.id == parent)) {
                let c = lanes[existing].as_ref().map_or(color, |l| l.color);
                outgoing.push(GraphEdge { from: lane, to: existing, color: c });
            } else if n == 0 {
                lanes[lane] = Some(Lane { id: parent.clone(), color });
                outgoing.push(GraphEdge { from: lane, to: lane, color });
            } else {
                let slot = free_slot(&mut lanes);
                let c = next_color;
                next_color += 1;
                lanes[slot] = Some(Lane { id: parent.clone(), color: c });
                outgoing.push(GraphEdge { from: lane, to: slot, color: c });
            }
        }

        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        let width = width_before
            .max(lanes.len())
            .max(lane + 1)
            .max(outgoing.iter().map(|e| e.to + 1).max().unwrap_or(0));

        rows.push(GraphRow {
            commit,
            lane,
            color,
            is_mainline,
            width,
            incoming,
            passthrough,
            outgoing,
        });
    }
    rows
}

impl GitClient {
    /// History with lane layout, ready for rendering. Defaults to topological order.
    pub fn graph(&self, o: &LogOptions) -> GitResult<Vec<GraphRow>> {
        let mut o = o.clone();
        if o.order == super::history::LogOrder::Plain {
            o.order = super::history::LogOrder::Topo;
        }
        Ok(build_graph(self.log(&o)?))
    }
}
