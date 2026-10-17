use std::cell::RefCell;

use cacao::color::Color;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::text::{Font, Label};
use cacao::view::View;

use crate::app::views::rows::{AUTHOR_WIDTH, COMMIT_WIDTH, DATE_WIDTH, MIN_GRAPH_WIDTH};

/// Column titles above the commit list, laid out like `CommitRow`.
pub struct CommitHeader {
    pub container: View,
    graph: Label,
    description: Label,
    date: Label,
    author: Label,
    commit: Label,
    description_offset: RefCell<Option<LayoutConstraint>>,
}

fn title(text: &str) -> Label {
    let label = Label::new();
    label.set_text(text);
    label.set_font(&Font::bold_system(11.));
    label.set_text_color(Color::SystemGray);
    label
}

impl CommitHeader {
    pub fn new() -> Self {
        Self {
            container: View::new(),
            graph: title("Graph"),
            description: title("Description"),
            date: title("Date"),
            author: title("Author"),
            commit: title("Commit"),
            description_offset: RefCell::new(None),
        }
    }

    /// Keeps the "Description" title aligned with the (dynamic) graph column.
    pub fn set_graph_width(&self, width: f64) {
        if let Some(constraint) = &*self.description_offset.borrow() {
            constraint.set_offset(width + 8.);
        }
    }
    pub fn layout(&self) {
        let c = &self.container;
        c.add_subview(&self.graph);
        c.add_subview(&self.description);
        c.add_subview(&self.date);
        c.add_subview(&self.author);
        c.add_subview(&self.commit);

        let description_leading = self
            .description
            .leading
            .constraint_equal_to(&c.leading)
            .offset(MIN_GRAPH_WIDTH + 8.);
        *self.description_offset.borrow_mut() = Some(description_leading.clone());
        LayoutConstraint::activate(&[
            description_leading,
            self.graph.leading.constraint_equal_to(&c.leading).offset(12.),
            self.graph.center_y.constraint_equal_to(&c.center_y),
            self.description.center_y.constraint_equal_to(&c.center_y),
            self.date.trailing.constraint_equal_to(&self.author.leading).offset(-8.),
            self.date.width.constraint_equal_to_constant(DATE_WIDTH),
            self.date.center_y.constraint_equal_to(&c.center_y),
            self.author.trailing.constraint_equal_to(&self.commit.leading).offset(-8.),
            self.author.width.constraint_equal_to_constant(AUTHOR_WIDTH),
            self.author.center_y.constraint_equal_to(&c.center_y),
            self.commit.trailing.constraint_equal_to(&c.trailing).offset(-12.),
            self.commit.width.constraint_equal_to_constant(COMMIT_WIDTH),
            self.commit.center_y.constraint_equal_to(&c.center_y),
        ]);
    }
}

