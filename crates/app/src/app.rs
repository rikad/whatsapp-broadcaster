//! Root view: the login screen until WhatsApp is ready, then the shell with
//! the sidebar and the Send / Broadcast pages.

use crate::connection::{Connection, ConnectionEntity};
use crate::theme::{self, color};
use crate::views::{broadcast::BroadcastView, login::LoginView, send::SendView};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{IconName, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FontWeight, Image, ImageFormat,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div, img, px,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::broadcast::error::RecvError;
use wb_core::{ConnState, WaEvent, WaHandle};

fn logo() -> Arc<Image> {
    static LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../../assets/icon.png").to_vec(),
        ))
    });
    LOGO.clone()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Send,
    Broadcast,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Send => "Send Message",
            Page::Broadcast => "Broadcast",
        }
    }
}

pub struct AppView {
    conn: ConnectionEntity,
    login: Entity<LoginView>,
    send: Entity<SendView>,
    broadcast: Entity<BroadcastView>,
    page: Page,
    _events: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(
        wa: WaHandle,
        notice: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut events = wa.subscribe();
        let conn = cx.new(|cx| Connection::new(wa, cx));
        let login = cx.new(|cx| LoginView::new(conn.clone(), window, cx));
        let send = cx.new(|cx| SendView::new(conn.clone(), window, cx));
        let broadcast = cx.new(|cx| BroadcastView::new(conn.clone(), window, cx));

        let events_task = cx.spawn_in(window, async move |this, cx| {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                };
                if this
                    .update_in(cx, |_, window, cx| notify_event(&event, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        if let Some(notice) = notice {
            window.push_notification(Notification::warning(notice), cx);
        }
        AppView {
            _subscriptions: vec![cx.observe(&conn, |_, _, cx| cx.notify())],
            conn,
            login,
            send,
            broadcast,
            page: Page::Send,
            _events: events_task,
        }
    }

    fn logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wa = self.conn.read(cx).wa().clone();
        cx.spawn_in(window, async move |_, _| wa.logout().await)
            .detach();
    }

    fn sidebar(&self, state: &ConnState, cx: &mut Context<Self>) -> impl IntoElement {
        let info = match state {
            ConnState::Ready(info) => Some(info.clone()),
            _ => None,
        };
        let name = info
            .as_ref()
            .map(|i| i.pushname.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "WhatsApp User".into());
        let phone = info
            .map(|i| format!("+{}", i.phone_number))
            .unwrap_or_else(|| "—".into());
        let initial = name
            .trim()
            .chars()
            .next()
            .unwrap_or('W')
            .to_uppercase()
            .to_string();

        let nav = |page: Page, icon: IconName, cx: &mut Context<Self>| {
            let active = self.page == page;
            h_flex()
                .id(page.title())
                .gap_3()
                .px_3()
                .py_2()
                .rounded(px(8.))
                .cursor_pointer()
                .text_color(color(if active { theme::GREEN } else { theme::TEXT }))
                .when(active, |el| el.bg(color(theme::SURFACE)))
                .when(!active, |el| el.hover(|s| s.bg(color(theme::HOVER))))
                .child(gpui_kit::component::Icon::new(icon).size_4())
                .child(page.title())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.page = page;
                    cx.notify();
                }))
        };

        v_flex()
            .w(px(240.))
            .h_full()
            .flex_none()
            .bg(color(theme::PANEL))
            .border_r_1()
            .border_color(color(theme::SURFACE))
            .child(
                h_flex()
                    .gap_3()
                    .p_4()
                    .border_b_1()
                    .border_color(color(theme::SURFACE))
                    .child(img(logo()).size(px(32.)).flex_none())
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("WhatsApp Broadcaster"),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .gap_1()
                    .p_3()
                    .child(nav(Page::Send, IconName::Inbox, cx))
                    .child(nav(Page::Broadcast, IconName::FileText, cx)),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .border_t_1()
                    .border_color(color(theme::SURFACE))
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(36.))
                                    .flex_none()
                                    .rounded_full()
                                    .bg(color(theme::GREEN))
                                    .text_color(color(theme::PANEL))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(initial),
                            )
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .child(div().truncate().text_sm().child(name))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(color(theme::MUTED))
                                            .child(phone),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("logout")
                            .danger()
                            .outline()
                            .label("Logout")
                            .on_click(cx.listener(|this, _, window, cx| this.logout(window, cx))),
                    ),
            )
    }

    fn header(&self, state: &ConnState) -> impl IntoElement {
        let (label, fg, bg): (SharedString, _, _) = match state {
            ConnState::Reconnecting(_) => ("Reconnecting…".into(), theme::BLUE, theme::BLUE_BG),
            _ => ("Connected".into(), theme::GREEN, theme::GREEN_BG),
        };
        h_flex()
            .h(px(56.))
            .flex_none()
            .px_6()
            .justify_between()
            .bg(color(theme::PANEL))
            .border_b_1()
            .border_color(color(theme::SURFACE))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.page.title()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .text_xs()
                    .bg(color(bg))
                    .text_color(color(fg))
                    .child(div().size(px(6.)).rounded_full().bg(color(fg)))
                    .child(label),
            )
    }
}

fn notify_event(event: &WaEvent, window: &mut Window, cx: &mut gpui_kit::App) {
    let note = match event {
        WaEvent::Error(msg) => Notification::error(msg.clone()),
        WaEvent::AuthFailure(msg) => Notification::error(format!("Authentication failed: {msg}")),
        WaEvent::Disconnected(_) => Notification::warning("Connection lost — reconnecting…"),
        WaEvent::LoggedOut(_) => {
            Notification::info("Logged out. Scan the new QR code to link again.")
        }
        WaEvent::Ready(_) => Notification::success("Connected to WhatsApp"),
        _ => return,
    };
    window.push_notification(note, cx);
}

impl Render for AppView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.conn.read(cx).state().clone();
        let content: AnyElement = match state {
            ConnState::Ready(_) | ConnState::Reconnecting(_) => h_flex()
                .size_full()
                .child(self.sidebar(&state, cx))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.header(&state))
                        .child(
                            v_flex()
                                .id("page")
                                .flex_1()
                                .min_h_0()
                                .p_6()
                                .overflow_y_scroll()
                                .map(|el| match self.page {
                                    Page::Send => el.child(self.send.clone()),
                                    Page::Broadcast => el.child(self.broadcast.clone()),
                                }),
                        ),
                )
                .into_any_element(),
            _ => self.login.clone().into_any_element(),
        };
        div()
            .size_full()
            .bg(color(theme::BACKGROUND))
            .text_color(color(theme::TEXT))
            .child(content)
    }
}

/// Shown instead of the app when another copy already holds the lock.
pub struct AlreadyRunning;

impl Render for AlreadyRunning {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .p_6()
            .bg(color(theme::BACKGROUND))
            .text_color(color(theme::TEXT))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("WhatsApp Broadcaster is already running"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(color(theme::MUTED))
                    .child("Switch to the open window instead of starting a second copy."),
            )
            .child(
                Button::new("quit")
                    .primary()
                    .label("Close")
                    .on_click(|_, _, cx| cx.quit()),
            )
    }
}
