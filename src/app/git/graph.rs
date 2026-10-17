use super::client::GitClient;
use super::error::GitResult;
use super::history::LogOptions;
use super::types::Commit;

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

/// Assigns lanes to commits. `commits` must be ordered children-before-parents (`LogOrder::Topo`).
/// Parents missing from the slice (truncated history) simply leave their lane open at the bottom.
pub fn build_graph(commits: Vec<Commit>) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<Lane>> = Vec::new();
    let mut next_color = 0usize;
    let mut rows = Vec::with_capacity(commits.len());

    for commit in commits {
        let width_before = lanes.len();
        let waiting: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.as_ref().filter(|l| l.id == commit.id).map(|_| i))
            .collect();

        let (lane, color) = match waiting.first() {
            Some(&i) => (i, lanes[i].as_ref().map_or(0, |l| l.color)),
            None => {
                let i = free_slot(&mut lanes);
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

        rows.push(GraphRow { commit, lane, color, width, incoming, passthrough, outgoing });
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
