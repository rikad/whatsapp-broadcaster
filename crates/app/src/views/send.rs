use crate::connection::ConnectionEntity;
use crate::theme::{self, color};
use crate::views::card;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::{Disableable as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};

pub struct SendView {
    conn: ConnectionEntity,
    phone: Entity<InputState>,
    message: Entity<TextareaState>,
    sending: bool,
    result: Option<(bool, SharedString)>,
}

impl SendView {
    pub fn new(conn: ConnectionEntity, window: &mut Window, cx: &mut Context<Self>) -> Self {
        SendView {
            conn,
            phone: cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 6281234567890")),
            message: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .rows(6)
                    .placeholder("Type your message here…")
            }),
            sending: false,
            result: None,
        }
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let phone = self.phone.read(cx).value().trim().to_string();
        let message = self.message.read(cx).value().trim().to_string();
        if phone.is_empty() || message.is_empty() {
            self.result = Some((false, "Please enter both phone number and message".into()));
            cx.notify();
            return;
        }
        self.sending = true;
        self.result = None;
        cx.notify();
        let wa = self.conn.read(cx).wa().clone();
        cx.spawn_in(window, async move |this, cx| {
            let outcome = wa.send_text(&phone, &message).await;
            this.update_in(cx, |this, window, cx| {
                this.sending = false;
                this.result = Some(match outcome {
                    Ok(sent) => {
                        this.phone.update(cx, |s, cx| s.set_value("", window, cx));
                        this.message.update(cx, |s, cx| s.set_value("", window, cx));
                        (true, format!("Message sent! ID: {}", sent.id).into())
                    }
                    Err(e) => (false, format!("Failed to send message: {e}").into()),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for SendView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        card().max_w(px(640.)).child(
            v_flex()
                .gap_4()
                .child(field(
                    "Phone number",
                    "Digits with country code — no + or spaces needed.",
                    Input::new(&self.phone),
                ))
                .child(field("Message", "", Textarea::new(&self.message)))
                .child(
                    div().child(
                        Button::new("send")
                            .primary()
                            .label("Send Message")
                            .loading(self.sending)
                            .disabled(self.sending)
                            .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                    ),
                )
                .when_some(self.result.clone(), |el, (ok, text)| {
                    let (fg, bg) = if ok {
                        (theme::GREEN, theme::GREEN_BG)
                    } else {
                        (theme::RED, theme::RED_BG)
                    };
                    el.child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded(px(6.))
                            .bg(color(bg))
                            .text_color(color(fg))
                            .text_sm()
                            .child(text),
                    )
                }),
        )
    }
}

fn field(label: &'static str, hint: &'static str, input: impl IntoElement) -> impl IntoElement {
    v_flex()
        .gap_1p5()
        .child(div().text_sm().text_color(color(theme::LABEL)).child(label))
        .child(input)
        .when(!hint.is_empty(), |el| {
            el.child(div().text_xs().text_color(color(theme::MUTED)).child(hint))
        })
}
