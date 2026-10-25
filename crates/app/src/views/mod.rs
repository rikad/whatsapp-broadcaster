pub mod broadcast;
pub mod login;
pub mod send;

use crate::theme::{self, color};
use gpui_kit::component::v_flex;
use gpui_kit::{Div, Styled as _, px};

/// Bordered panel used for each section of a page.
pub fn card() -> Div {
    v_flex()
        .w_full()
        .p_5()
        .bg(color(theme::PANEL))
        .border_1()
        .border_color(color(theme::SURFACE))
        .rounded(px(10.))
}
