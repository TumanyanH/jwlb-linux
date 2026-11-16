//! Colours of the original, decoded from its compiled colour dictionary (resources.pri, XBF at
//! 0x583bc0: base colours plus Light/Dark theme dictionaries). Names in comments are the
//! original's resource keys.
use egui::Color32;

pub const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// jwlibrary_purple — the brand accent (buttons, emphasized text and icons, light theme).
pub const PRIMARY: Color32 = hex(0x5b3c88);
/// violet_200 — the accent in the dark theme.
pub const PRIMARY_DARK_MODE: Color32 = hex(0xc1a7e2);
pub const PRIMARY_LIGHT: Color32 = hex(0xc1a7e2);
/// violet_50 / ToggleMenuFlyoutItemRevealBackgroundPointerOver — light purple fills.
pub const PURPLE_50: Color32 = hex(0xf1eafa);

/// Highlight colours 1..=6 (yellow, green, blue, pink, orange, purple), light theme.
pub const HIGHLIGHT_LIGHT: [Color32; 6] = [hex(0xfff37a), hex(0xb7e492), hex(0x98d8ff), hex(0xf698bc), hex(0xffba8a), hex(0xc1a7e2)];
/// The same six colours in the dark theme.
pub const HIGHLIGHT_DARK: [Color32; 6] = [hex(0xe9c600), hex(0x66a333), hex(0x4aa1dd), hex(0xc64677), hex(0xe96d00), hex(0x7b57a7)];
/// Order of the colour buttons in the original's highlight picker (JWLibrary.dll 0x185e1dd40):
/// Yellow=1, Green=2, Blue=3, Purple=6, Pink=4, Orange=5 (ColorIndex values).
pub const HIGHLIGHT_PICKER_ORDER: [i64; 6] = [1, 2, 3, 6, 4, 5];
pub const HIGHLIGHT_NAMES: [&str; 6] = ["Yellow", "Green", "Blue", "Pink", "Orange", "Purple"];

/// bookmark_1..bookmark_10, light theme.
pub const BOOKMARKS: [Color32; 10] = [
    hex(0x9b1a4b),
    hex(0x664194),
    hex(0x4a6da7),
    hex(0x37a5af),
    hex(0x1d8642),
    hex(0x47c681),
    hex(0xbbd72e),
    hex(0xc3673c),
    hex(0xa43d3b),
    hex(0x814800),
];
/// bookmark_1..bookmark_10, dark theme.
pub const BOOKMARKS_DARK: [Color32; 10] = [
    hex(0xb32d61),
    hex(0x7b57a7),
    hex(0x6385bf),
    hex(0x51bdc7),
    hex(0x179e54),
    hex(0x67da9c),
    hex(0xd2eb5c),
    hex(0xd97e54),
    hex(0xbd5856),
    hex(0x9d5d07),
];

pub fn bookmark(slot: usize, dark: bool) -> Color32 {
    if dark {
        BOOKMARKS_DARK[slot.min(9)]
    } else {
        BOOKMARKS[slot.min(9)]
    }
}

/// Bible book tiles: bible_books_one/two/three_purple = bible_books_block1/2/3 (#3b3547, #746b84,
/// #544c63; same in both themes). Measured on the person's screenshot of the original: Pentateuch,
/// Prophets, Gospels and Revelation use "one" (darkest), History and Acts "two" (lightest),
/// Poetic books and the Letters "three" (middle). Groups are the MEPS BibleBookGroup ids 0..7.
pub fn book_group_color(group: u8, _dark: bool) -> Color32 {
    const ONE: Color32 = hex(0x3b3547);
    const TWO: Color32 = hex(0x746b84);
    const THREE: Color32 = hex(0x544c63);
    [ONE, TWO, THREE, ONE, ONE, TWO, THREE, ONE][(group as usize).min(7)]
}

pub fn highlight(idx: i64, dark: bool) -> Color32 {
    let i = ((idx.max(1) - 1) as usize).min(5);
    if dark {
        HIGHLIGHT_DARK[i]
    } else {
        HIGHLIGHT_LIGHT[i]
    }
}

/// background_default: page background behind cards.
pub fn page_bg(dark: bool) -> Color32 {
    if dark { hex(0x000000) } else { hex(0xf1f1f1) }
}
/// background_content: reading pages.
pub fn content_bg(dark: bool) -> Color32 {
    if dark { hex(0x000000) } else { hex(0xffffff) }
}
/// accent: jwlibrary_purple / violet_200.
pub fn accent(dark: bool) -> Color32 {
    if dark { PRIMARY_DARK_MODE } else { PRIMARY }
}
/// link_default: links inside publications.
pub fn link(dark: bool) -> Color32 {
    if dark { hex(0x9fb9e3) } else { hex(0x4a6da7) }
}
/// menu_selected_windows: selected navigation item.
pub fn nav_selected(dark: bool) -> Color32 {
    if dark { hex(0x3c3c3c) } else { hex(0xc0c0c0) }
}
/// separator_default.
pub fn separator(dark: bool) -> Color32 {
    if dark { hex(0x8e8e8e) } else { hex(0xc0c0c0) }
}
/// icon_default.
pub fn icon(dark: bool) -> Color32 {
    if dark { hex(0xd8d8d8) } else { hex(0x4f4f4f) }
}
/// background_placeholder (tiles without a picture).
pub fn placeholder(dark: bool) -> Color32 {
    if dark { hex(0x4f4f4f) } else { hex(0x8e8e8e) }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let acc = accent(dark);
    v.hyperlink_color = link(dark);
    v.selection.bg_fill = if dark { hex(0x2a1048) } else { PURPLE_50 };
    v.selection.stroke.color = acc;
    v.panel_fill = page_bg(dark);
    // background_modal
    v.window_fill = if dark { hex(0x3c3c3c) } else { hex(0xffffff) };
    // TextControlBackground
    v.extreme_bg_color = if dark { hex(0x292929) } else { hex(0xffffff) };
    v.faint_bg_color = if dark { hex(0x292929) } else { hex(0xf1f1f1) };
    v.override_text_color = Some(text(dark));
    // buttons: background_menu_windows with a subtle border, like WinUI
    let btn = if dark { hex(0x3c3c3c) } else { hex(0xfbfbfb) };
    v.widgets.inactive.weak_bg_fill = btn;
    v.widgets.inactive.bg_fill = btn;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, if dark { hex(0x4f4f4f) } else { hex(0xd8d8d8) });
    v.widgets.hovered.weak_bg_fill = if dark { hex(0x4f4f4f) } else { hex(0xf1f1f1) };
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, separator(dark));
    v.widgets.active.weak_bg_fill = if dark { hex(0x626262) } else { hex(0xd8d8d8) };
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, separator(dark));
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = egui::CornerRadius::same(4);
    }
    v.window_corner_radius = egui::CornerRadius::same(8);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(11.0, 5.0);
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.5));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.5));
    });
}

/// button_subdued / text_tabs: secondary text.
pub fn muted(dark: bool) -> Color32 {
    if dark { hex(0xa7a7a7) } else { hex(0x757575) }
}

/// text_default.
pub fn text(dark: bool) -> Color32 {
    if dark { hex(0xffffff) } else { hex(0x121212) }
}

/// background_card.
pub fn card(dark: bool) -> Color32 {
    if dark { hex(0x292929) } else { hex(0xffffff) }
}

/// background_navigation_windows.
pub fn sidebar(dark: bool) -> Color32 {
    if dark { hex(0x121212) } else { hex(0xf1f1f1) }
}

/// Tiles without a cover picture use the original's placeholder grey.
pub fn tile_color(_seed: &str) -> Color32 {
    hex(0x8e8e8e)
}
