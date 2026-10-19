use cacao::core_graphics::geometry::{CGPoint, CGRect, CGSize};
use cacao::image::{DrawConfig, Image, ResizeBehavior};
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};
use cacao::objc::{msg_send, sel, sel_impl};

use crate::app::{
    actions::AppAction,
    model::{AppModel, CommitNode, RefBadge, RefKind, SharedModel},
    send,
    views::{
        rows::{
            CommitPill, CommitRow, CommitRowData, MAX_GRAPH_WIDTH, MAX_PILLS, MIN_GRAPH_WIDTH,
            ROW_HEIGHT,
        },
        theme::{Rgb, lane_rgb, ref_rgb},
    },
};

const ROW_ID: &str = "RockGitCommitRowCell";
const LANE_WIDTH: f64 = 16.;
const LANE_OFFSET: f64 = 14.;
const MIN_LANE_WIDTH: f64 = 5.;
const NODE_RADIUS: f64 = 5.;
const MAX_PILL_CHARS: usize = 24;

/// A drawing primitive in row-local coordinates (origin top-left).
enum Prim {
    Line {
        from: (f64, f64),
        to: (f64, f64),
        rgb: Rgb,
        is_mainline: bool,
    },
    Curve {
        from: (f64, f64),
        c1: (f64, f64),
        c2: (f64, f64),
        to: (f64, f64),
        rgb: Rgb,
        is_mainline: bool,
    },
}

const DEFAULT_LINE_WIDTH: f64 = 2.;
const MAINLINE_LINE_WIDTH: f64 = 3.;
const MAINLINE_NODE_RADIUS: f64 = 6.;

/// Horizontal layout of the graph column, derived from the number of lanes in use.
#[derive(Clone, Copy)]
pub struct GraphGeometry {
    pub spacing: f64,
    pub width: f64,
}
impl GraphGeometry {
    /// Grows with the widest lane; once `MAX_GRAPH_WIDTH` is reached lanes are packed closer.
    pub fn for_max_lane(max_lane: usize) -> Self {
        let wanted = LANE_OFFSET * 2. + max_lane as f64 * LANE_WIDTH;
        let width = wanted.clamp(MIN_GRAPH_WIDTH, MAX_GRAPH_WIDTH);
        let spacing = if max_lane == 0 {
            LANE_WIDTH
        } else {
            ((width - LANE_OFFSET * 2.) / max_lane as f64).clamp(MIN_LANE_WIDTH, LANE_WIDTH)
        };
        Self { spacing, width }
    }
    pub fn for_model(model: &AppModel) -> Self {
        let max_lane = model
            .visible_commits()
            .iter()
            .map(|&i| model.commits[i].lane)
            .max()
            .unwrap_or(0);
        Self::for_max_lane(max_lane)
    }
    fn x(&self, lane: usize) -> f64 {
        LANE_OFFSET + lane as f64 * self.spacing
    }
}

/// Computes the lines crossing one row of the graph. `rows` are the visible commits,
/// newest first, and `row` is the position being drawn.
fn row_primitives(
    geo: GraphGeometry,
    rows: &[&CommitNode],
    positions: &[Option<usize>],
    all: &[CommitNode],
    row: usize,
) -> Vec<Prim> {
    let (top, mid, bottom) = (0., ROW_HEIGHT / 2., ROW_HEIGHT);
    let mut prims = Vec::new();

    for (child_pos, child) in rows.iter().enumerate() {
        for &parent_index in &child.parents {
            let Some(parent_pos) = positions.get(parent_index).copied().flatten() else {
                continue;
            };
            let parent = &all[parent_index];
            // Branch edges run down the child's lane, merge edges down the parent's lane.
            let edge_lane = child.lane.max(parent.lane);
            let rgb = lane_rgb(edge_lane);
            let is_mainline =
                child.is_mainline && parent.is_mainline && child.lane == 0 && parent.lane == 0;
            let (xc, xp, xe) = (geo.x(child.lane), geo.x(parent.lane), geo.x(edge_lane));

            if child_pos < row && row < parent_pos {
                prims.push(Prim::Line {
                    from: (xe, top),
                    to: (xe, bottom),
                    rgb,
                    is_mainline,
                });
            } else if row == child_pos {
                if parent.lane > child.lane {
                    // Merge: leave the child lane and curve into the parent lane.
                    prims.push(Prim::Curve {
                        from: (xc, mid),
                        c1: (xc, bottom),
                        c2: (xp, mid),
                        to: (xp, bottom),
                        rgb,
                        is_mainline,
                    });
                } else {
                    prims.push(Prim::Line {
                        from: (xc, mid),
                        to: (xc, bottom),
                        rgb,
                        is_mainline,
                    });
                }
            } else if row == parent_pos {
                if child.lane > parent.lane {
                    // Branch point: curve from the child lane into the parent node.
                    prims.push(Prim::Curve {
                        from: (xc, top),
                        c1: (xc, mid),
                        c2: (xp, top),
                        to: (xp, mid),
                        rgb,
                        is_mainline,
                    });
                } else {
                    prims.push(Prim::Line {
                        from: (xp, top),
                        to: (xp, mid),
                        rgb,
                        is_mainline,
                    });
                }
            }
        }
    }

    prims
}

fn graph_image(
    geo: GraphGeometry,
    prims: Vec<Prim>,
    node_lane: usize,
    is_head: bool,
    is_mainline: bool,
) -> Image {
    let node_x = geo.x(node_lane);
    let node_rgb = lane_rgb(node_lane);
    let size = (geo.width, ROW_HEIGHT);

    Image::draw(
        DrawConfig {
            source: size,
            target: size,
            resize: ResizeBehavior::Stretch,
        },
        move |_rect, ctx| {
            let channel = |v: u8| f64::from(v) / 255.;

            for prim in &prims {
                match prim {
                    Prim::Line {
                        from,
                        to,
                        rgb,
                        is_mainline,
                    } => {
                        ctx.set_line_width(if *is_mainline {
                            MAINLINE_LINE_WIDTH
                        } else {
                            DEFAULT_LINE_WIDTH
                        });
                        ctx.set_rgb_stroke_color(
                            channel(rgb.0),
                            channel(rgb.1),
                            channel(rgb.2),
                            1.,
                        );
                        ctx.begin_path();
                        ctx.move_to_point(from.0, from.1);
                        ctx.add_line_to_point(to.0, to.1);
                        ctx.stroke_path();
                    }
                    Prim::Curve {
                        from,
                        c1,
                        c2,
                        to,
                        rgb,
                        is_mainline,
                    } => {
                        ctx.set_line_width(if *is_mainline {
                            MAINLINE_LINE_WIDTH
                        } else {
                            DEFAULT_LINE_WIDTH
                        });
                        ctx.set_rgb_stroke_color(
                            channel(rgb.0),
                            channel(rgb.1),
                            channel(rgb.2),
                            1.,
                        );
                        ctx.begin_path();
                        ctx.move_to_point(from.0, from.1);
                        ctx.add_curve_to_point(c1.0, c1.1, c2.0, c2.1, to.0, to.1);
                        ctx.stroke_path();
                    }
                }
            }

            let mid = ROW_HEIGHT / 2.;
            let node_radius = if is_mainline {
                MAINLINE_NODE_RADIUS
            } else {
                NODE_RADIUS
            };
            let rect = |r: f64| {
                CGRect::new(
                    &CGPoint::new(node_x - r, mid - r),
                    &CGSize::new(r * 2., r * 2.),
                )
            };
            if is_head {
                // HEAD: hollow node with a thick ring.
                ctx.set_rgb_fill_color(1., 1., 1., 1.);
                ctx.fill_ellipse_in_rect(rect(node_radius + 1.));
                ctx.set_line_width(if is_mainline {
                    MAINLINE_LINE_WIDTH + 1.
                } else {
                    MAINLINE_LINE_WIDTH
                });
                ctx.set_rgb_stroke_color(
                    channel(node_rgb.0),
                    channel(node_rgb.1),
                    channel(node_rgb.2),
                    1.,
                );
                ctx.stroke_ellipse_in_rect(rect(node_radius));
            } else {
                ctx.set_rgb_fill_color(
                    channel(node_rgb.0),
                    channel(node_rgb.1),
                    channel(node_rgb.2),
                    1.,
                );
                ctx.fill_ellipse_in_rect(rect(node_radius));
            }
            true
        },
    )
}

fn working_tree_graph_image(geo: GraphGeometry) -> Image {
    graph_image(
        geo,
        vec![Prim::Line {
            from: (geo.x(0), ROW_HEIGHT / 2.),
            to: (geo.x(0), ROW_HEIGHT),
            rgb: lane_rgb(0),
            is_mainline: true,
        }],
        0,
        false,
        true,
    )
}

/// Ref pills for a commit, honouring the "Show Remote Branches" toggle.
fn pills_for(commit: &CommitNode, show_remote: bool) -> Vec<CommitPill> {
    commit
        .refs
        .iter()
        .filter(|badge| show_remote || badge.kind != RefKind::RemoteBranch)
        .take(MAX_PILLS)
        .map(commit_pill)
        .collect()
}

fn commit_pill(badge: &RefBadge) -> CommitPill {
    let text = abbreviate_ref_label(&badge.label, MAX_PILL_CHARS);
    CommitPill {
        tooltip: (text != badge.label).then(|| badge.label.clone()),
        text,
        rgb: ref_rgb(badge.kind),
    }
}

fn working_tree_pills(model: &AppModel) -> Vec<CommitPill> {
    let mut pills = Vec::new();
    let changed_count = model.working_tree.changed_files().len();
    let unversioned_count = model.working_tree.unversioned_files().len();

    if changed_count > 0 {
        pills.push(CommitPill {
            text: format!("changed {}", changed_count),
            tooltip: None,
            rgb: (46, 204, 113),
        });
    }
    if unversioned_count > 0 {
        pills.push(CommitPill {
            text: format!("unversions {}", unversioned_count),
            tooltip: None,
            rgb: (241, 196, 15),
        });
    }
    if model.working_tree.untracked_count > 0 {
        pills.push(CommitPill {
            text: format!("untracked {}", model.working_tree.untracked_count),
            tooltip: None,
            rgb: (149, 165, 166),
        });
    }

    pills.truncate(MAX_PILLS);
    pills
}

fn abbreviate_ref_label(label: &str, max_chars: usize) -> String {
    let label_len = label.chars().count();
    if label_len <= max_chars {
        return label.to_owned();
    }

    let parts: Vec<&str> = label.split('/').collect();
    if parts.len() > 1 {
        if let (Some(root), Some(leaf)) = (parts.first(), parts.last()) {
            let root_and_leaf = format!("{root}/…/{leaf}");
            if root_and_leaf.chars().count() <= max_chars {
                return root_and_leaf;
            }

            let leaf_only = format!("…/{leaf}");
            if leaf_only.chars().count() <= max_chars {
                return leaf_only;
            }

            if max_chars > 2 {
                return format!("…/{}", abbreviate_middle(leaf, max_chars - 2));
            }
        }
    }

    abbreviate_middle(label, max_chars)
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
    let prefix_len = visible_chars / 2;
    let suffix_len = visible_chars - prefix_len;
    let chars: Vec<char> = text.chars().collect();
    let prefix: String = chars[..prefix_len].iter().collect();
    let suffix: String = chars[chars.len() - suffix_len..].iter().collect();
    format!("{prefix}…{suffix}")
}

pub struct CommitList {
    model: SharedModel,
    view: Option<ListView>,
}

#[cfg(test)]
mod tests;

impl CommitList {
    pub fn new(model: SharedModel) -> Self {
        Self { model, view: None }
    }
}

impl ListViewDelegate for CommitList {
    const NAME: &'static str = "RockGitCommitList";

    fn did_load(&mut self, view: ListView) {
        view.register(ROW_ID, CommitRow::default);
        view.set_row_height(ROW_HEIGHT);
        view.objc.with_mut(|obj| unsafe {
            // Remove the gap between rows so graph lines connect.
            let _: () = msg_send![obj, setIntercellSpacing: CGSize::new(0., 0.)];
        });
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.model.lock().unwrap().commit_list_len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let view = self.view.as_ref().unwrap().dequeue::<CommitRow>(ROW_ID);
        if let Some(cell) = &view.delegate {
            let model = self.model.lock().unwrap();
            let visible = model.visible_commits();
            let geo = GraphGeometry::for_max_lane(
                visible
                    .iter()
                    .map(|&i| model.commits[i].lane)
                    .max()
                    .unwrap_or(0),
            );

            if model.has_uncommitted_changes() && row == 0 {
                let pills = working_tree_pills(&model);
                let file_count = model.working_tree.total_count();
                let date_text = if file_count == 1 {
                    "1 file".to_owned()
                } else {
                    format!("{file_count} files")
                };

                cell.configure(&CommitRowData {
                    graph: working_tree_graph_image(geo),
                    graph_width: geo.width,
                    pills: &pills,
                    summary: "Uncommitted changes",
                    is_head: false,
                    date: &date_text,
                    author: "Working tree",
                    short_id: "",
                    selected: model.selected_uncommitted,
                });
                return view.into_row();
            }

            if let Some(index) = model.commit_index_for_row(row) {
                let rows: Vec<&CommitNode> = visible.iter().map(|&i| &model.commits[i]).collect();
                let mut positions = vec![None; model.commits.len()];
                for (pos, &i) in visible.iter().enumerate() {
                    positions[i] = Some(pos);
                }

                let commit = &model.commits[index];
                let graph_row = row - usize::from(model.has_uncommitted_changes());
                let prims = row_primitives(geo, &rows, &positions, &model.commits, graph_row);
                let marker = if model.commit_matches_query(commit) {
                    "▸ "
                } else {
                    ""
                };
                let summary = format!("{marker}{}", commit.summary);

                cell.configure(&CommitRowData {
                    graph: graph_image(geo, prims, commit.lane, commit.is_head, commit.is_mainline),
                    graph_width: geo.width,
                    pills: &pills_for(commit, model.show_remote),
                    summary: &summary,
                    is_head: commit.is_head,
                    date: &commit.relative_time,
                    author: &commit.author,
                    short_id: &commit.short_id,
                    selected: !model.selected_uncommitted && model.selected_commit == index,
                });
            }
        }
        view.into_row()
    }

    fn item_selected(&self, row: Option<usize>) {
        let Some(row) = row else { return };
        let model = self.model.lock().unwrap();
        if model.has_uncommitted_changes() && row == 0 {
            if !model.selected_uncommitted {
                send(AppAction::SelectUncommittedChanges);
            }
            return;
        }
        if let Some(index) = model.commit_index_for_row(row) {
            if model.selected_commit != index {
                send(AppAction::SelectCommit(index));
            }
        }
    }
}

/// Position of the selected commit within the visible list, if it is visible.
pub fn selected_row(model: &AppModel) -> Option<usize> {
    model.selected_commit_row()
}
