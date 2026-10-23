use std::{ffi::c_void, path::Path, sync::OnceLock};

use cacao::color::Color;
use cacao::foundation::{id, nil, NSString};
use cacao::objc::{class, msg_send, sel, sel_impl};
use cacao::text::Font;
use objc_id::ShareId;

use crate::app::model::RefKind;

static FONT_AWESOME_REGISTERED: OnceLock<bool> = OnceLock::new();

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontManagerRegisterFontsForURL(
        font_url: *const c_void,
        scope: u32,
        error: *mut *const c_void,
    ) -> bool;
}

pub type Rgb = (u8, u8, u8);

pub fn font_awesome_font(size: f64) -> Option<Font> {
    if !*FONT_AWESOME_REGISTERED.get_or_init(register_font_awesome) {
        return None;
    }

    let name = NSString::new("FontAwesome5Free-Solid");
    let font: id = unsafe { msg_send![class!(NSFont), fontWithName: name size: size] };
    if font == nil {
        None
    } else {
        Some(Font(unsafe { ShareId::from_ptr(font) }))
    }
}

fn register_font_awesome() -> bool {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/fa-solid-900.ttf");
    if !Path::new(path).is_file() {
        return false;
    }

    let path = NSString::new(path);
    let url: id = unsafe { msg_send![class!(NSURL), fileURLWithPath: path] };
    if url == nil {
        return false;
    }

    unsafe { CTFontManagerRegisterFontsForURL(url as *const c_void, 1, std::ptr::null_mut()) }
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
