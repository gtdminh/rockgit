use cacao::color::Color;
use cacao::objc::{class, msg_send, sel, sel_impl};
use cacao::text::Font;
use objc_id::ShareId;

use crate::app::model::RefKind;

pub type Rgb = (u8, u8, u8);

/// Monospaced system font so columns line up.
pub fn monospace_font(size: f64) -> Font {
    Font(unsafe {
        ShareId::from_ptr(
            msg_send![class!(NSFont), monospacedSystemFontOfSize: size weight: 0.0f64],
        )
    })
}

/// Colour of a graph lane / branch line.
pub fn lane_rgb(lane: usize) -> Rgb {
    const COLORS: [Rgb; 6] = [
        (52, 152, 219),
        (46, 204, 64),
        (255, 133, 27),
        (177, 13, 201),
        (231, 76, 60),
        (0, 188, 188),
    ];
    COLORS[lane % COLORS.len()]
}

/// Background colour of a ref pill.
pub fn ref_rgb(kind: RefKind) -> Rgb {
    match kind {
        RefKind::Head => (127, 82, 219),
        RefKind::LocalBranch => (39, 174, 96),
        RefKind::RemoteBranch => (41, 128, 185),
        RefKind::Tag => (230, 145, 18),
    }
}

pub fn color(rgb: Rgb) -> Color {
    Color::rgb(rgb.0, rgb.1, rgb.2)
}
