use cacao::button::Button;
use cacao::color::Color;
use cacao::control::Control;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::text::{Font, Label, LineBreakMode};
use cacao::view::View;

use crate::app::{actions::AppAction, model::AppModel, send};

const PANEL_WIDTH: f64 = 340.;
const CONTENT_WIDTH: f64 = PANEL_WIDTH - 24.;

pub struct Details {
    pub container: View,
    newer: Button,
    older: Button,
    summary: Label,
    meta: Label,
    refs: Label,
    files_title: Label,
    files: Label,
}

fn wrapped_label() -> Label {
    let label = Label::new();
    label.set_max_number_of_lines(0);
    label.set_line_break_mode(LineBreakMode::WrapWords);
    label
}

impl Details {
    pub fn new() -> Self {
        let mut newer = Button::new("Newer");
        newer.set_action(|| send(AppAction::SelectNewerCommit));
        let mut older = Button::new("Older");
        older.set_action(|| send(AppAction::SelectOlderCommit));

        let summary = wrapped_label();
        summary.set_font(&Font::bold_system(18.));
        let files_title = Label::new();
        files_title.set_text("Changed files");
        files_title.set_font(&Font::bold_system(13.));
        let refs = wrapped_label();
        refs.set_text_color(Color::SystemGray);

        Self {
            container: View::new(),
            newer,
            older,
            summary,
            meta: wrapped_label(),
            refs,
            files_title,
            files: wrapped_label(),
        }
    }

    /// Adds the panel's subviews and layout constraints.
    pub fn layout(&self) {
        let c = &self.container;
        c.add_subview(&self.newer);
        c.add_subview(&self.older);
        c.add_subview(&self.summary);
        c.add_subview(&self.meta);
        c.add_subview(&self.refs);
        c.add_subview(&self.files_title);
        c.add_subview(&self.files);

        LayoutConstraint::activate(&[
            c.width.constraint_equal_to_constant(PANEL_WIDTH),
            self.newer.top.constraint_equal_to(&c.top),
            self.newer.leading.constraint_equal_to(&c.leading).offset(12.),
            self.older.top.constraint_equal_to(&c.top),
            self.older.leading.constraint_equal_to(&self.newer.trailing).offset(8.),
            self.summary.top.constraint_equal_to(&self.newer.bottom).offset(14.),
            self.summary.leading.constraint_equal_to(&c.leading).offset(12.),
            self.summary.width.constraint_equal_to_constant(CONTENT_WIDTH),
            self.meta.top.constraint_equal_to(&self.summary.bottom).offset(10.),
            self.meta.leading.constraint_equal_to(&c.leading).offset(12.),
            self.meta.width.constraint_equal_to_constant(CONTENT_WIDTH),
            self.refs.top.constraint_equal_to(&self.meta.bottom).offset(10.),
            self.refs.leading.constraint_equal_to(&c.leading).offset(12.),
            self.refs.width.constraint_equal_to_constant(CONTENT_WIDTH),
            self.files_title.top.constraint_equal_to(&self.refs.bottom).offset(16.),
            self.files_title.leading.constraint_equal_to(&c.leading).offset(12.),
            self.files.top.constraint_equal_to(&self.files_title.bottom).offset(6.),
            self.files.leading.constraint_equal_to(&c.leading).offset(12.),
            self.files.width.constraint_equal_to_constant(CONTENT_WIDTH),
        ]);
    }

    pub fn update(&self, model: &AppModel) {
        self.newer.set_enabled(model.can_select_newer_commit());
        self.older.set_enabled(model.can_select_older_commit());

        let Some(commit) = model.selected_commit() else {
            self.summary.set_text("No commit selected");
            self.meta.set_text("");
            self.refs.set_text("");
            self.files.set_text("");
            return;
        };

        self.summary.set_text(&commit.summary);
        self.meta.set_text(format!(
            "Commit: {}\nAuthor: {}\nWhen: {}\nParents: {}",
            commit.short_id,
            commit.author,
            commit.relative_time,
            commit.parents.len()
        ));

        self.refs.set_text(if commit.refs.is_empty() {
            "No refs on this commit".to_owned()
        } else {
            let labels: Vec<&str> = commit.refs.iter().map(|r| r.label.as_str()).collect();
            format!("Refs: {}", labels.join(", "))
        });

        self.files.set_text(
            commit
                .files
                .iter()
                .map(|f| format!("{}  {}", f.status, f.path))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
}

