use cacao::button::Button;
use cacao::color::Color;
use cacao::foundation::{id, nil, NSString};
use cacao::image::{Image, ImageView};
use cacao::layout::{Layout, LayoutConstraint};
use cacao::objc::{msg_send, sel, sel_impl};
use cacao::text::{Font, Label, LineBreakMode, TextAlign};
use cacao::view::{View, ViewDelegate};
use std::sync::{Arc, Mutex};

use crate::app::{
    actions::AppAction,
    send,
    views::theme::{color, font_awesome_font, Rgb},
};

pub const ROW_HEIGHT: f64 = 28.;
pub const MIN_GRAPH_WIDTH: f64 = 96.;
pub const MAX_GRAPH_WIDTH: f64 = 320.;
pub const DATE_WIDTH: f64 = 110.;
pub const AUTHOR_WIDTH: f64 = 130.;
pub const COMMIT_WIDTH: f64 = 70.;
pub const MAX_PILLS: usize = 3;
pub const WORKING_TREE_ROW_HEIGHT: f64 = 26.;

/// Two-line row used by the repository and branch lists.
#[derive(Default)]
pub struct SidebarRow {
    view: Option<View>,
    title: Label,
    icon: Label,
    tree_title: Label,
    subtitle: Label,
}

impl SidebarRow {
    fn set_tooltip(&self, tooltip: Option<&str>) {
        if let Some(view) = &self.view {
            view.objc.with_mut(|obj| unsafe {
                match tooltip {
                    Some(tooltip) => {
                        let tooltip = NSString::new(tooltip);
                        let _: () = msg_send![obj, setToolTip: tooltip];
                    }
                    None => {
                        let _: () = msg_send![obj, setToolTip: nil];
                    }
                }
            });
        }
    }

    pub fn configure_header(&self, title: &str, icon: &str) {
        if let Some(view) = &self.view {
            view.set_background_color(Color::Clear);
        }
        self.title.set_hidden(true);
        self.icon.set_hidden(false);
        self.tree_title.set_hidden(false);
        self.icon
            .set_font(&font_awesome_font(10.).unwrap_or_else(|| Font::system(10.)));
        self.icon.set_text_color(Color::SystemGray);
        self.icon.set_text(icon);
        self.tree_title.set_font(&Font::bold_system(10.));
        self.tree_title.set_text(title);
        self.subtitle.set_hidden(true);
        self.subtitle.set_text("");
        self.set_tooltip(None);
    }

    pub fn configure(&self, title: &str, subtitle: &str, current: bool) {
        if let Some(view) = &self.view {
            view.set_background_color(if current {
                Color::rgba(41, 128, 185, 70)
            } else {
                Color::Clear
            });
        }
        self.title.set_hidden(false);
        self.icon.set_hidden(true);
        self.tree_title.set_hidden(true);
        self.title.set_font(&if current {
            Font::bold_system(13.)
        } else {
            Font::system(13.)
        });
        self.title.set_text(title);
        self.subtitle.set_hidden(subtitle.is_empty());
        self.subtitle.set_text(subtitle);
        self.set_tooltip(None);
    }

    pub fn configure_tree_item(
        &self,
        title: &str,
        depth: usize,
        icon: &str,
        selected: bool,
        emphasized: bool,
        tooltip: Option<&str>,
    ) {
        if let Some(view) = &self.view {
            view.set_background_color(if selected {
                Color::rgba(113, 76, 86, 180)
            } else {
                Color::Clear
            });
        }
        self.title.set_hidden(true);
        self.icon.set_hidden(false);
        self.tree_title.set_hidden(false);
        self.icon
            .set_font(&font_awesome_font(11.).unwrap_or_else(|| Font::system(11.)));
        self.icon.set_text_color(Color::SystemGray);
        self.icon.set_text(&format!("{}{icon}", "  ".repeat(depth)));
        self.tree_title.set_font(&if selected || emphasized {
            Font::bold_system(12.)
        } else {
            Font::system(12.)
        });
        self.tree_title
            .set_text(&format!("{}{title}", "  ".repeat(depth)));
        self.subtitle.set_hidden(true);
        self.subtitle.set_text("");
        self.set_tooltip(tooltip);
    }
}

impl ViewDelegate for SidebarRow {
    const NAME: &'static str = "RockGitSidebarRow";

    fn did_load(&mut self, view: View) {
        view.add_subview(&self.title);
        view.add_subview(&self.icon);
        view.add_subview(&self.tree_title);
        view.add_subview(&self.subtitle);
        self.view = Some(view.clone_as_handle());

        self.title.set_line_break_mode(LineBreakMode::TruncateTail);
        self.tree_title
            .set_line_break_mode(LineBreakMode::TruncateTail);
        self.subtitle
            .set_line_break_mode(LineBreakMode::TruncateTail);
        self.subtitle.set_font(&Font::system(10.));
        self.subtitle.set_text_color(Color::SystemGray);

        LayoutConstraint::activate(&[
            self.title.top.constraint_equal_to(&view.top).offset(6.),
            self.icon.top.constraint_equal_to(&view.top).offset(6.),
            self.tree_title
                .top
                .constraint_equal_to(&view.top)
                .offset(6.),
            self.title
                .leading
                .constraint_equal_to(&view.leading)
                .offset(12.),
            self.icon
                .leading
                .constraint_equal_to(&view.leading)
                .offset(12.),
            self.tree_title
                .leading
                .constraint_equal_to(&view.leading)
                .offset(30.),
            self.title
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
            self.tree_title
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
            self.subtitle
                .top
                .constraint_equal_to(&self.title.bottom)
                .offset(2.),
            self.subtitle
                .leading
                .constraint_equal_to(&view.leading)
                .offset(12.),
            self.subtitle
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
        ]);
    }
}

/// Everything a commit row needs to display.
pub struct CommitPill {
    pub text: String,
    pub tooltip: Option<String>,
    pub rgb: Rgb,
}

pub struct CommitRowData<'a> {
    pub graph: Image,
    pub graph_width: f64,
    pub pills: &'a [CommitPill],
    pub summary: &'a str,
    pub is_head: bool,
    pub date: &'a str,
    pub author: &'a str,
    pub short_id: &'a str,
    pub selected: bool,
}

/// Table-like commit row: graph | ref pills + description | date | author | commit.
#[derive(Default)]
pub struct CommitRow {
    view: Option<View>,
    graph: ImageView,
    graph_width: Option<LayoutConstraint>,
    pills: [Label; MAX_PILLS],
    pill_widths: Vec<LayoutConstraint>,
    summary: Label,
    date: Label,
    author: Label,
    short_id: Label,
}

impl CommitRow {
    fn set_pill_tooltip(pill: &Label, tooltip: Option<&str>) {
        pill.objc.with_mut(|obj| unsafe {
            match tooltip {
                Some(tooltip) => {
                    let tooltip = NSString::new(tooltip);
                    let _: () = msg_send![obj, setToolTip: tooltip];
                }
                None => {
                    let _: () = msg_send![obj, setToolTip: nil];
                }
            }
        });
    }

    pub fn configure(&self, data: &CommitRowData) {
        if let Some(view) = &self.view {
            view.set_background_color(if data.selected {
                Color::rgba(41, 128, 185, 70)
            } else {
                Color::Clear
            });
        }
        self.graph.set_image(&data.graph);
        if let Some(width) = &self.graph_width {
            width.set_offset(data.graph_width);
        }

        for (index, pill) in self.pills.iter().enumerate() {
            match data.pills.get(index) {
                Some(item) => {
                    if let Some(width) = self.pill_widths.get(index) {
                        width.set_offset(item.text.chars().count() as f64 * 6.6 + 14.);
                    }
                    pill.set_text(&item.text);
                    pill.set_background_color(color(item.rgb));
                    pill.set_hidden(false);
                    Self::set_pill_tooltip(pill, item.tooltip.as_deref());
                }
                None => {
                    if let Some(width) = self.pill_widths.get(index) {
                        width.set_offset(0.);
                    }
                    pill.set_text("");
                    pill.set_background_color(Color::Clear);
                    pill.set_hidden(true);
                    Self::set_pill_tooltip(pill, None);
                }
            }
        }

        self.summary.set_text(data.summary);
        self.summary.set_font(&if data.is_head {
            Font::bold_system(12.)
        } else {
            Font::system(12.)
        });
        self.date.set_text(data.date);
        self.author.set_text(data.author);
        self.short_id.set_text(data.short_id);
    }
}

impl ViewDelegate for CommitRow {
    const NAME: &'static str = "RockGitCommitRow";

    fn did_load(&mut self, view: View) {
        self.view = Some(view.clone_as_handle());
        view.add_subview(&self.graph);
        for pill in &self.pills {
            view.add_subview(pill);
            pill.set_font(&Font::bold_system(10.));
            pill.set_text_alignment(TextAlign::Center);
            pill.set_max_number_of_lines(1);
            pill.set_line_break_mode(LineBreakMode::TruncateTail);
            pill.set_text_color(Color::rgb(255, 255, 255));
            pill.objc.with_mut(|obj| unsafe {
                let layer: id = msg_send![obj, layer];
                let _: () = msg_send![layer, setCornerRadius: 4.0f64];
            });
        }
        view.add_subview(&self.summary);
        view.add_subview(&self.date);
        view.add_subview(&self.author);
        view.add_subview(&self.short_id);

        for label in [&self.summary, &self.date, &self.author, &self.short_id] {
            label.set_max_number_of_lines(1);
            label.set_line_break_mode(LineBreakMode::TruncateTail);
        }
        for label in [&self.date, &self.author, &self.short_id] {
            label.set_font(&Font::system(11.));
            label.set_text_color(Color::SystemGray);
        }

        self.pill_widths = self
            .pills
            .iter()
            .map(|pill| pill.width.constraint_equal_to_constant(0.))
            .collect();
        LayoutConstraint::activate(&self.pill_widths);

        let graph_width = self
            .graph
            .width
            .constraint_equal_to_constant(MIN_GRAPH_WIDTH);
        LayoutConstraint::activate(&[graph_width.clone()]);
        self.graph_width = Some(graph_width);
        LayoutConstraint::activate(&[
            self.graph.top.constraint_equal_to(&view.top),
            self.graph.leading.constraint_equal_to(&view.leading),
            self.graph.height.constraint_equal_to_constant(ROW_HEIGHT),
            self.pills[0]
                .leading
                .constraint_equal_to(&self.graph.trailing)
                .offset(8.),
            self.pills[0].center_y.constraint_equal_to(&view.center_y),
            self.pills[1]
                .leading
                .constraint_equal_to(&self.pills[0].trailing)
                .offset(4.),
            self.pills[1].center_y.constraint_equal_to(&view.center_y),
            self.pills[2]
                .leading
                .constraint_equal_to(&self.pills[1].trailing)
                .offset(4.),
            self.pills[2].center_y.constraint_equal_to(&view.center_y),
            self.summary
                .leading
                .constraint_equal_to(&self.pills[2].trailing)
                .offset(6.),
            self.summary
                .trailing
                .constraint_less_than_or_equal_to(&self.date.leading)
                .offset(-8.),
            self.summary.center_y.constraint_equal_to(&view.center_y),
            self.date
                .trailing
                .constraint_equal_to(&self.author.leading)
                .offset(-8.),
            self.date.width.constraint_equal_to_constant(DATE_WIDTH),
            self.date.center_y.constraint_equal_to(&view.center_y),
            self.author
                .trailing
                .constraint_equal_to(&self.short_id.leading)
                .offset(-8.),
            self.author.width.constraint_equal_to_constant(AUTHOR_WIDTH),
            self.author.center_y.constraint_equal_to(&view.center_y),
            self.short_id
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
            self.short_id
                .width
                .constraint_equal_to_constant(COMMIT_WIDTH),
            self.short_id.center_y.constraint_equal_to(&view.center_y),
        ]);
    }
}

pub struct WorkingTreeRow {
    view: Option<View>,
    icon: Label,
    name: Label,
    parent_path: Label,
    section_count: Label,
    status: Label,
    revert: Button,
    action_path: Arc<Mutex<Option<String>>>,
}

pub struct WorkingTreeRowData<'a> {
    pub icon: &'a str,
    pub status: &'a str,
    pub name: &'a str,
    pub parent_path: &'a str,
    pub display_path: &'a str,
    pub repo_path: &'a str,
}

impl Default for WorkingTreeRow {
    fn default() -> Self {
        Self {
            view: None,
            icon: Label::new(),
            name: Label::new(),
            parent_path: Label::new(),
            section_count: Label::new(),
            status: Label::new(),
            revert: Button::new("↺"),
            action_path: Arc::new(Mutex::new(None)),
        }
    }
}

impl WorkingTreeRow {
    pub fn configure_header(&self, title: &str, count: usize, expanded: bool) {
        if let Some(view) = &self.view {
            view.set_background_color(Color::Clear);
        }
        self.icon.set_hidden(true);
        self.parent_path.set_hidden(true);
        self.section_count.set_hidden(false);
        self.status.set_hidden(true);
        self.revert.set_hidden(true);
        self.name.set_font(&Font::bold_system(12.));
        self.name.set_text(&format!("{} {title}", if expanded { "▾" } else { "▸" }));
        self.section_count.set_text(&count.to_string());
        if let Ok(mut action_path) = self.action_path.lock() {
            *action_path = None;
        }
    }

    pub fn configure_file(&self, data: &WorkingTreeRowData) {
        if let Some(view) = &self.view {
            view.set_background_color(Color::Clear);
        }

        self.icon.set_hidden(false);
        self.parent_path.set_hidden(data.parent_path.is_empty());
        self.section_count.set_hidden(true);
        self.status.set_hidden(false);
        self.revert.set_hidden(false);
        self.icon.set_text(data.icon);
        self.status.set_text(data.status);
        self.name.set_font(&Font::system(12.));
        self.name.set_text(data.name);
        self.parent_path.set_text(data.parent_path);
        if let Some(view) = &self.view {
            view.objc.with_mut(|obj| unsafe {
                let tooltip = NSString::new(data.display_path);
                let _: () = msg_send![obj, setToolTip: tooltip];
            });
        }
        if let Ok(mut action_path) = self.action_path.lock() {
            *action_path = Some(data.repo_path.to_owned());
        }
    }
}

impl ViewDelegate for WorkingTreeRow {
    const NAME: &'static str = "RockGitWorkingTreeRow";

    fn did_load(&mut self, view: View) {
        self.view = Some(view.clone_as_handle());
        self.icon.set_font(&Font::system(11.));
        self.icon.set_text_color(Color::SystemOrange);
        self.name.set_max_number_of_lines(1);
        self.name.set_line_break_mode(LineBreakMode::TruncateTail);
        self.parent_path.set_font(&Font::system(11.));
        self.parent_path.set_text_color(Color::SystemGray);
        self.parent_path.set_max_number_of_lines(1);
        self.parent_path
            .set_line_break_mode(LineBreakMode::TruncateTail);
        self.section_count.set_font(&Font::bold_system(11.));
        self.section_count.set_text_color(Color::SystemGray);
        self.status.set_font(&Font::bold_system(11.));
        self.status.set_text_color(Color::SystemGray);
        self.revert.set_font(&Font::system(11.));

        let action_path = self.action_path.clone();
        self.revert.set_action(move || {
            let path = action_path.lock().ok().and_then(|guard| guard.clone());
            if let Some(path) = path {
                send(AppAction::RevertWorkingTreeFile(path));
            }
        });

        view.add_subview(&self.icon);
        view.add_subview(&self.name);
        view.add_subview(&self.parent_path);
        view.add_subview(&self.section_count);
        view.add_subview(&self.status);
        view.add_subview(&self.revert);

        LayoutConstraint::activate(&[
            self.icon.center_y.constraint_equal_to(&view.center_y),
            self.icon
                .leading
                .constraint_equal_to(&view.leading)
                .offset(10.),
            self.icon.width.constraint_equal_to_constant(14.),
            self.name.center_y.constraint_equal_to(&view.center_y),
            self.name
                .leading
                .constraint_equal_to(&self.icon.trailing)
                .offset(6.),
            self.parent_path
                .center_y
                .constraint_equal_to(&view.center_y),
            self.parent_path
                .leading
                .constraint_equal_to(&self.name.trailing)
                .offset(6.),
            self.section_count
                .center_y
                .constraint_equal_to(&view.center_y),
            self.section_count
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-12.),
            self.status.center_y.constraint_equal_to(&view.center_y),
            self.status
                .trailing
                .constraint_equal_to(&self.revert.leading)
                .offset(-8.),
            self.status.width.constraint_equal_to_constant(16.),
            self.revert.center_y.constraint_equal_to(&view.center_y),
            self.revert
                .trailing
                .constraint_equal_to(&view.trailing)
                .offset(-8.),
            self.revert.width.constraint_equal_to_constant(24.),
            self.name
                .trailing
                .constraint_less_than_or_equal_to(&self.status.leading)
                .offset(-8.),
            self.parent_path
                .trailing
                .constraint_less_than_or_equal_to(&self.status.leading)
                .offset(-8.),
        ]);
    }
}
