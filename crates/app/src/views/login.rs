use crate::connection::ConnectionEntity;
use crate::theme::{self, color};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window, div, img, px,
};
use wb_core::ConnState;

pub struct LoginView {
    conn: ConnectionEntity,
    phone: Entity<InputState>,
    by_phone: bool,
    requesting: bool,
    _subscriptions: Vec<Subscription>,
}

impl LoginView {
    pub fn new(conn: ConnectionEntity, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let phone = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Phone with country code, e.g. 6281234567890")
        });
        let subscriptions = vec![
            cx.observe(&conn, |_, _, cx| cx.notify()),
            cx.subscribe_in(&phone, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.request_code(window, cx);
                }
            }),
        ];
        LoginView {
            conn,
            phone,
            by_phone: false,
            requesting: false,
            _subscriptions: subscriptions,
        }
    }

    fn request_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let number = self.phone.read(cx).value().to_string();
        if number.trim().is_empty() || self.requesting {
            return;
        }
        self.requesting = true;
        cx.notify();
        let wa = self.conn.read(cx).wa().clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = wa.request_pair_code(&number).await;
            this.update_in(cx, |this, window, cx| {
                this.requesting = false;
                if let Err(e) = result {
                    window.push_notification(
                        Notification::error(format!("Could not get a code: {e}")),
                        cx,
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn body(&mut self, state: &ConnState, cx: &mut Context<Self>) -> AnyElement {
        match state {
            ConnState::Pairing { pair_code, .. } => self.pairing(pair_code.clone(), cx),
            ConnState::Authenticating => waiting("Linked! Loading your account…"),
            ConnState::Failed(message) => v_flex()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .text_color(color(theme::RED))
                        .text_center()
                        .child(message.clone()),
                )
                .child(
                    Button::new("retry")
                        .primary()
                        .label("Try again")
                        .on_click(cx.listener(|this, _, _, cx| this.conn.read(cx).wa().retry())),
                )
                .into_any_element(),
            _ => waiting("Connecting to WhatsApp…"),
        }
    }

    fn pairing(&mut self, pair_code: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let qr = self.conn.read(cx).qr_image();
        let main: AnyElement = if self.by_phone {
            let code = pair_code.map(|c| {
                let (a, b) = c.split_at(c.len().min(4));
                format!("{a}-{b}")
            });
            v_flex()
                .gap_3()
                .w_full()
                .child(Input::new(&self.phone))
                .child(
                    Button::new("get-code")
                        .primary()
                        .label("Get code")
                        .loading(self.requesting)
                        .disabled(self.requesting)
                        .on_click(cx.listener(|this, _, window, cx| this.request_code(window, cx))),
                )
                .children(code.map(|code| {
                    v_flex()
                        .items_center()
                        .gap_1()
                        .pt_2()
                        .child(
                            div()
                                .text_size(px(32.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(color(theme::GREEN))
                                .child(code),
                        )
                        .child(hint("On your phone: Linked devices → Link a device → Link with phone number instead"))
                }))
                .into_any_element()
        } else {
            v_flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size(px(264.))
                        .rounded(px(10.))
                        .bg(gpui_kit::white())
                        .p_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|el| match qr {
                            Some(image) => el.child(img(image).size(px(252.))),
                            None => el.child(Spinner::new().large()),
                        }),
                )
                .child(hint("Open WhatsApp on your phone → Settings → Linked devices → Link a device, then scan this code."))
                .into_any_element()
        };
        let toggle_label = if self.by_phone {
            "Scan a QR code instead"
        } else {
            "Link with phone number instead"
        };
        v_flex()
            .items_center()
            .gap_4()
            .w_full()
            .child(main)
            .child(
                Button::new("toggle-mode")
                    .ghost()
                    .small()
                    .label(toggle_label)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.by_phone = !this.by_phone;
                        if this.by_phone {
                            this.phone.update(cx, |input, cx| input.focus(window, cx));
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

fn waiting(text: &'static str) -> AnyElement {
    v_flex()
        .items_center()
        .gap_3()
        .py_8()
        .child(Spinner::new().large())
        .child(hint(text))
        .into_any_element()
}

fn hint(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(color(theme::MUTED))
        .text_center()
        .child(text.into())
}

impl Render for LoginView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.conn.read(cx).state().clone();
        h_flex()
            .size_full()
            .items_center()
            .justify_center()
            .p_5()
            .bg(color(theme::BACKGROUND))
            .child(
                v_flex()
                    .w(px(400.))
                    .items_center()
                    .gap_5()
                    .px_8()
                    .py_10()
                    .bg(color(theme::PANEL))
                    .border_1()
                    .border_color(color(theme::SURFACE))
                    .rounded(px(14.))
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color(theme::TEXT))
                            .child("WhatsApp Broadcaster"),
                    )
                    .child(self.body(&state, cx)),
            )
    }
}
