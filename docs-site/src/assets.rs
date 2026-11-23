//! Fonts and icons embedded at compile time so the binary is self-contained:
//! no runtime reads from disk, no network at runtime.

use std::collections::HashMap;

pub static FONTS: &[(&str, &[u8])] = &[
    ("inter-latin-400.woff2", include_bytes!("../assets/fonts/inter-latin-400.woff2")),
    ("inter-latin-500.woff2", include_bytes!("../assets/fonts/inter-latin-500.woff2")),
    ("inter-latin-600.woff2", include_bytes!("../assets/fonts/inter-latin-600.woff2")),
    ("inter-latin-700.woff2", include_bytes!("../assets/fonts/inter-latin-700.woff2")),
];

static ICONS: &[(&str, &str)] = &[
    ("home", include_str!("../assets/icons/home.svg")),
    ("calendar", include_str!("../assets/icons/calendar.svg")),
    ("clock", include_str!("../assets/icons/clock.svg")),
    ("list", include_str!("../assets/icons/list.svg")),
    ("zap", include_str!("../assets/icons/zap.svg")),
    ("message-circle", include_str!("../assets/icons/message-circle.svg")),
    ("settings", include_str!("../assets/icons/settings.svg")),
    ("bell", include_str!("../assets/icons/bell.svg")),
    ("lightbulb", include_str!("../assets/icons/lightbulb.svg")),
    ("check", include_str!("../assets/icons/check.svg")),
    ("sliders-horizontal", include_str!("../assets/icons/sliders-horizontal.svg")),
    ("mic", include_str!("../assets/icons/mic.svg")),
    ("puzzle", include_str!("../assets/icons/puzzle.svg")),
    ("brain", include_str!("../assets/icons/brain.svg")),
    ("folder", include_str!("../assets/icons/folder.svg")),
    ("shield", include_str!("../assets/icons/shield.svg")),
    ("heart", include_str!("../assets/icons/heart.svg")),
    ("phone", include_str!("../assets/icons/phone.svg")),
    ("box", include_str!("../assets/icons/box.svg")),
    ("map-pin", include_str!("../assets/icons/map-pin.svg")),
    ("activity", include_str!("../assets/icons/activity.svg")),
    ("map", include_str!("../assets/icons/map.svg")),
    ("send", include_str!("../assets/icons/send.svg")),
    ("plus", include_str!("../assets/icons/plus.svg")),
    ("menu", include_str!("../assets/icons/menu.svg")),
    ("sun", include_str!("../assets/icons/sun.svg")),
    ("moon", include_str!("../assets/icons/moon.svg")),
    ("x", include_str!("../assets/icons/x.svg")),
];

/// All bundled icons, with the leading licence comment stripped, keyed by
/// the Lucide file name.
pub fn load_icons() -> HashMap<String, String> {
    let mut m = HashMap::new();
    for (name, svg) in ICONS {
        m.insert((*name).to_string(), strip_comment(svg));
    }
    m
}

fn strip_comment(s: &str) -> String {
    if let Some(end) = s.find("-->") {
        s[end + 3..].trim_start().to_string()
    } else {
        s.to_string()
    }
}
