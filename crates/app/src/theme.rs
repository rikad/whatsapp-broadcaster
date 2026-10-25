//! WhatsApp-style dark palette, carried over from the v1 web UI.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, rgb};

pub const BACKGROUND: u32 = 0x0b141a;
pub const PANEL: u32 = 0x111b21;
pub const SURFACE: u32 = 0x2a3942;
pub const HOVER: u32 = 0x202c33;
pub const ROW_HOVER: u32 = 0x1a262d;
pub const GREEN: u32 = 0x00a884;
pub const GREEN_DARK: u32 = 0x008f72;
pub const TEXT: u32 = 0xe9edef;
pub const LABEL: u32 = 0xaebac1;
pub const MUTED: u32 = 0x8696a0;
pub const RED: u32 = 0xea4335;
pub const RED_BG: u32 = 0x3d1515;
pub const GREEN_BG: u32 = 0x0c2e26;
pub const BLUE: u32 = 0x53bdeb;
pub const BLUE_BG: u32 = 0x1f3a4d;

pub fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn apply(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    // Colors go in a second update: changing mode reloads the theme's colors.
    Theme::update(cx, |t| {
        t.background = color(BACKGROUND);
        t.foreground = color(TEXT);
        t.border = color(SURFACE);
        t.input = color(SURFACE);
        t.muted = color(SURFACE);
        t.muted_foreground = color(MUTED);
        t.ring = color(GREEN);
        t.caret = color(GREEN);
        t.primary = color(GREEN);
        t.primary_hover = color(GREEN_DARK);
        t.primary_active = color(GREEN_DARK);
        t.primary_foreground = color(PANEL);
        t.button_primary = color(GREEN);
        t.button_primary_hover = color(GREEN_DARK);
        t.button_primary_active = color(GREEN_DARK);
        t.button_primary_foreground = color(PANEL);
        t.secondary = color(SURFACE);
        t.secondary_hover = color(0x3d4f58);
        t.secondary_foreground = color(TEXT);
        t.button_secondary = color(SURFACE);
        t.button_secondary_hover = color(0x3d4f58);
        t.button_secondary_foreground = color(TEXT);
        t.danger = color(RED);
        t.popover = color(PANEL);
        t.popover_foreground = color(TEXT);
        t.sidebar = color(PANEL);
        t.title_bar = color(PANEL);
        t.title_bar_border = color(SURFACE);
        t.list_hover = color(ROW_HOVER);
        t.table_head = color(ROW_HOVER);
        t.table_hover = color(ROW_HOVER);
    });
}
