use cacao::core_graphics::geometry::{CGPoint, CGRect, CGSize};
use cacao::image::{DrawConfig, Image, ResizeBehavior};
use cacao::listview::{ListView, ListViewDelegate, ListViewRow};
use cacao::objc::{msg_send, sel, sel_impl};

use crate::app::{
    actions::AppAction,
    model::{AppModel, CommitNode, RefKind, SharedModel},
    send,
    views::{
        rows::{CommitRow, CommitRowData, MAX_GRAPH_WIDTH, MAX_PILLS, MIN_GRAPH_WIDTH, ROW_HEIGHT},
        theme::{lane_rgb, ref_rgb, Rgb},
    },
};

const ROW_ID: &str = "RockGitCommitRowCell";
const LANE_WIDTH: f64 = 16.;
const LANE_OFFSET: f64 = 14.;
const MIN_LANE_WIDTH: f64 = 5.;
const NODE_RADIUS: f64 = 5.;

/// A drawing primitive in row-local coordinates (origin top-left).
enum Prim {
    Line { from: (f64, f64), to: (f64, f64), rgb: Rgb },
    Curve { from: (f64, f64), c1: (f64, f64), c2: (f64, f64), to: (f64, f64), rgb: Rgb },
}

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
fn row_primitives(geo: GraphGeometry, rows: &[&CommitNode], positions: &[Option<usize>], all: &[CommitNode], row: usize) -> Vec<Prim> {
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
            let (xc, xp, xe) = (geo.x(child.lane), geo.x(parent.lane), geo.x(edge_lane));

            if child_pos < row && row < parent_pos {
                prims.push(Prim::Line { from: (xe, top), to: (xe, bottom), rgb });
            } else if row == child_pos {
                if parent.lane > child.lane {
                    // Merge: leave the child lane and curve into the parent lane.
                    prims.push(Prim::Curve {
                        from: (xc, mid),
                        c1: (xc, bottom),
                        c2: (xp, mid),
                        to: (xp, bottom),
                        rgb,
                    });
                } else {
                    prims.push(Prim::Line { from: (xc, mid), to: (xc, bottom), rgb });
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
                    });
                } else {
                    prims.push(Prim::Line { from: (xp, top), to: (xp, mid), rgb });
                }
            }
        }
    }

    prims
}

fn graph_image(geo: GraphGeometry, prims: Vec<Prim>, node_lane: usize, is_head: bool) -> Image {
    let node_x = geo.x(node_lane);
    let node_rgb = lane_rgb(node_lane);
    let size = (geo.width, ROW_HEIGHT);

    Image::draw(
        DrawConfig { source: size, target: size, resize: ResizeBehavior::Stretch },
        move |_rect, ctx| {
            let channel = |v: u8| f64::from(v) / 255.;

            ctx.set_line_width(2.);
            for prim in &prims {
                match prim {
                    Prim::Line { from, to, rgb } => {
                        ctx.set_rgb_stroke_color(channel(rgb.0), channel(rgb.1), channel(rgb.2), 1.);
                        ctx.begin_path();
                        ctx.move_to_point(from.0, from.1);
                        ctx.add_line_to_point(to.0, to.1);
                        ctx.stroke_path();
                    },
                    Prim::Curve { from, c1, c2, to, rgb } => {
                        ctx.set_rgb_stroke_color(channel(rgb.0), channel(rgb.1), channel(rgb.2), 1.);
                        ctx.begin_path();
                        ctx.move_to_point(from.0, from.1);
                        ctx.add_curve_to_point(c1.0, c1.1, c2.0, c2.1, to.0, to.1);
                        ctx.stroke_path();
                    },
                }
            }

            let mid = ROW_HEIGHT / 2.;
            let rect = |r: f64| {
                CGRect::new(&CGPoint::new(node_x - r, mid - r), &CGSize::new(r * 2., r * 2.))
            };
            if is_head {
                // HEAD: hollow node with a thick ring.
                ctx.set_rgb_fill_color(1., 1., 1., 1.);
                ctx.fill_ellipse_in_rect(rect(NODE_RADIUS + 1.));
                ctx.set_line_width(3.);
                ctx.set_rgb_stroke_color(channel(node_rgb.0), channel(node_rgb.1), channel(node_rgb.2), 1.);
                ctx.stroke_ellipse_in_rect(rect(NODE_RADIUS));
            } else {
                ctx.set_rgb_fill_color(channel(node_rgb.0), channel(node_rgb.1), channel(node_rgb.2), 1.);
                ctx.fill_ellipse_in_rect(rect(NODE_RADIUS));
            }
            true
        },
    )
}

/// Ref pills for a commit, honouring the "Show Remote Branches" toggle.
fn pills_for(commit: &CommitNode, show_remote: bool) -> Vec<(String, Rgb)> {
    commit
        .refs
        .iter()
        .filter(|badge| show_remote || badge.kind != RefKind::RemoteBranch)
        .take(MAX_PILLS)
        .map(|badge| (badge.label.clone(), ref_rgb(badge.kind)))
        .collect()
}

pub struct CommitList {
    model: SharedModel,
    view: Option<ListView>,
}

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
        self.model.lock().unwrap().visible_commits().len()
    }

    fn item_for(&self, row: usize) -> ListViewRow {
        let view = self.view.as_ref().unwrap().dequeue::<CommitRow>(ROW_ID);
        if let Some(cell) = &view.delegate {
            let model = self.model.lock().unwrap();
            let visible = model.visible_commits();

            if let Some(&index) = visible.get(row) {
                let rows: Vec<&CommitNode> = visible.iter().map(|&i| &model.commits[i]).collect();
                let mut positions = vec![None; model.commits.len()];
                for (pos, &i) in visible.iter().enumerate() {
                    positions[i] = Some(pos);
                }

                let commit = &model.commits[index];
                let geo = GraphGeometry::for_max_lane(rows.iter().map(|c| c.lane).max().unwrap_or(0));
                let prims = row_primitives(geo, &rows, &positions, &model.commits, row);
                let marker = if model.commit_matches_query(commit) { "▸ " } else { "" };
                let summary = format!("{marker}{}", commit.summary);

                cell.configure(&CommitRowData {
                    graph: graph_image(geo, prims, commit.lane, commit.is_head),
                    graph_width: geo.width,
                    pills: &pills_for(commit, model.show_remote),
                    summary: &summary,
                    is_head: commit.is_head,
                    date: &commit.relative_time,
                    author: &commit.author,
                    short_id: &commit.short_id,
                    selected: model.selected_commit == index,
                });
            }
        }
        view.into_row()
    }

    fn item_selected(&self, row: Option<usize>) {
        let Some(row) = row else { return };
        let model = self.model.lock().unwrap();
        if let Some(&index) = model.visible_commits().get(row) {
            if model.selected_commit != index {
                send(AppAction::SelectCommit(index));
            }
        }
    }
}

/// Position of the selected commit within the visible list, if it is visible.
pub fn selected_row(model: &AppModel) -> Option<usize> {
    model
        .visible_commits()
        .iter()
        .position(|&i| i == model.selected_commit)
}

