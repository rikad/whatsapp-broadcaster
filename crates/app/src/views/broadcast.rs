use crate::connection::ConnectionEntity;
use crate::theme::{self, color};
use crate::views::card;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Disableable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, PathPromptOptions, Render, SharedString, Styled as _, Task, Window, div,
    px, uniform_list,
};
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wb_core::broadcast::{self, Counts, Row, RowStatus, Update};

const ROW_HEIGHT: f32 = 36.;

/// Column widths: #, phone, message (flex), status, sent at, error.
const COL_INDEX: f32 = 48.;
const COL_PHONE: f32 = 150.;
const COL_STATUS: f32 = 90.;
const COL_TIME: f32 = 90.;
const COL_ERROR: f32 = 220.;

struct Running {
    stop: CancellationToken,
    _task: Task<()>,
}

pub struct BroadcastView {
    conn: ConnectionEntity,
    delay: Entity<InputState>,
    rows: Vec<Row>,
    source_name: String,
    summary: Option<(bool, SharedString)>,
    progress: Option<SharedString>,
    running: Option<Running>,
}

impl BroadcastView {
    pub fn new(conn: ConnectionEntity, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delay = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(broadcast::DEFAULT_DELAY.as_millis().to_string())
                .validate(|text, _| text.chars().all(|c| c.is_ascii_digit()))
        });
        BroadcastView {
            conn,
            delay,
            rows: Vec::new(),
            source_name: "broadcast-report".into(),
            summary: None,
            progress: None,
            running: None,
        }
    }

    fn is_running(&self) -> bool {
        self.running.is_some()
    }

    fn has_failed(&self) -> bool {
        self.rows.iter().any(|r| r.status == RowStatus::Failed)
    }

    fn has_pending(&self) -> bool {
        self.rows.iter().any(|r| r.status != RowStatus::Success)
    }

    fn choose_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open broadcast JSON".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let text = std::fs::read_to_string(&path);
            this.update(cx, |this, cx| this.load(path, text, cx)).ok();
        })
        .detach();
    }

    fn load(&mut self, path: PathBuf, text: std::io::Result<String>, cx: &mut Context<Self>) {
        let parsed = text
            .map_err(|e| e.to_string())
            .and_then(|t| broadcast::parse(&t).map_err(|e| e.to_string()));
        self.progress = None;
        match parsed {
            Ok(rows) => {
                self.source_name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "broadcast-report".into());
                let done = rows
                    .iter()
                    .filter(|r| r.status == RowStatus::Success)
                    .count();
                self.summary = Some((
                    true,
                    format!(
                        "{} — {} entries: {} to send, {} already marked success (will be skipped).",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        rows.len(),
                        rows.len() - done,
                        done
                    )
                    .into(),
                ));
                self.rows = rows;
            }
            Err(e) => {
                self.rows.clear();
                self.summary = Some((false, format!("Invalid JSON: {e}").into()));
            }
        }
        cx.notify();
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_running() || !self.has_pending() {
            return;
        }
        let delay_ms: u64 = self.delay.read(cx).value().parse().unwrap_or(3000);
        let delay = Duration::from_millis(delay_ms).max(broadcast::MIN_DELAY);
        self.delay.update(cx, |s, cx| {
            s.set_value(delay.as_millis().to_string(), window, cx)
        });

        let wa = self.conn.read(cx).wa().clone();
        let (jobs, skipped) = broadcast::jobs(&self.rows);
        let total = self.rows.len();
        let mut run = {
            let _rt = wa.runtime().enter();
            broadcast::start(wa.clone(), jobs, skipped, delay)
        };
        let stop = run.stop_token();
        let task = cx.spawn(async move |this, cx| {
            while let Some(update) = run.updates.recv().await {
                let finished = matches!(update, Update::Finished { .. });
                let alive = this.update(cx, |this, cx| {
                    broadcast::apply(&mut this.rows, &update);
                    this.progress = Some(progress_text(&update, total).into());
                    if finished {
                        this.running = None;
                    }
                    cx.notify();
                });
                if finished || alive.is_err() {
                    break;
                }
            }
        });
        self.running = Some(Running { stop, _task: task });
        self.progress = Some("Starting…".into());
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(running) = &self.running {
            running.stop.cancel();
            self.progress = Some("Stopping after the current message…".into());
            cx.notify();
        }
    }

    fn download(&mut self, failed_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        if failed_only && !self.has_failed() {
            window.push_notification(Notification::warning("No failed entries to export."), cx);
            return;
        }
        let json = broadcast::report_json(&self.rows, failed_only);
        let name = broadcast::report_file_name(&self.source_name, failed_only);
        let dir = directories::UserDirs::new()
            .and_then(|d| d.download_dir().map(PathBuf::from))
            .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()))
            .unwrap_or_default();
        let picked = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            let note = match std::fs::write(&path, json) {
                Ok(()) => Notification::success(format!("Saved {}", path.display())),
                Err(e) => Notification::error(format!("Could not save report: {e}")),
            };
            cx.update(|window, cx| window.push_notification(note, cx))
                .ok();
        })
        .detach();
    }

    fn table(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let header = h_flex()
            .h(px(ROW_HEIGHT))
            .px_3()
            .bg(color(theme::ROW_HOVER))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color(theme::LABEL))
            .child(cell(COL_INDEX, "#"))
            .child(cell(COL_PHONE, "Phone"))
            .child(div().flex_1().min_w_0().child("Message"))
            .child(cell(COL_STATUS, "Status"))
            .child(cell(COL_TIME, "Sent at"))
            .child(cell(COL_ERROR, "Error"));
        let list = uniform_list(
            "broadcast-rows",
            self.rows.len(),
            cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                range
                    .map(|i| table_row(i, &this.rows[i]).into_any_element())
                    .collect::<Vec<_>>()
            }),
        )
        .flex_1()
        .min_h(px(ROW_HEIGHT * 4.));
        v_flex()
            .flex_1()
            .min_h_0()
            .border_1()
            .border_color(color(theme::SURFACE))
            .rounded(px(8.))
            .overflow_hidden()
            .child(header)
            .child(list)
    }
}

fn progress_text(update: &Update, total: usize) -> String {
    let counts = |c: &Counts| {
        format!(
            "success: {}, failed: {}, skipped: {}",
            c.success, c.failed, c.skipped
        )
    };
    match update {
        Update::Sending { index, counts: c } => {
            format!("Sending {} of {total}… ({})", index + 1, counts(c))
        }
        Update::Sent { index, .. } | Update::Failed { index, .. } => {
            format!("Sent {} of {total}, waiting…", index + 1)
        }
        Update::Finished { counts: c, stopped } => {
            let note = if *stopped { " (stopped)" } else { "" };
            format!(
                "Done{note}. Success: {}, Failed: {}, Skipped: {}.",
                c.success, c.failed, c.skipped
            )
        }
    }
}

fn cell(width: f32, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .truncate()
        .pr_2()
        .child(text.into())
}

fn local_time(iso: &Option<String>) -> String {
    iso.as_deref()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}

fn badge(status: RowStatus) -> impl IntoElement {
    let (fg, bg) = match status {
        RowStatus::Pending => (theme::MUTED, theme::SURFACE),
        RowStatus::Sending => (theme::BLUE, theme::BLUE_BG),
        RowStatus::Success => (theme::GREEN, theme::GREEN_BG),
        RowStatus::Failed => (theme::RED, theme::RED_BG),
    };
    div()
        .px_2()
        .py_0p5()
        .rounded(px(10.))
        .text_xs()
        .bg(color(bg))
        .text_color(color(fg))
        .child(status.as_str())
}

fn table_row(index: usize, row: &Row) -> impl IntoElement {
    // Rows have no stable id of their own; the index is their identity
    // within one loaded file.
    h_flex()
        .id(("row", index))
        .h(px(ROW_HEIGHT))
        .px_3()
        .text_sm()
        .text_color(color(theme::TEXT))
        .border_t_1()
        .border_color(color(theme::SURFACE))
        .hover(|s| s.bg(color(theme::ROW_HOVER)))
        .child(cell(COL_INDEX, (index + 1).to_string()))
        .child(cell(COL_PHONE, row.phone.clone()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .pr_2()
                .child(row.message.replace('\n', " ")),
        )
        .child(div().w(px(COL_STATUS)).flex_none().child(badge(row.status)))
        .child(cell(COL_TIME, local_time(&row.sent_at)))
        .child(
            div()
                .w(px(COL_ERROR))
                .flex_none()
                .truncate()
                .text_color(color(theme::RED))
                .child(row.error.clone().unwrap_or_default()),
        )
}

impl Render for BroadcastView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.is_running();
        let loaded = !self.rows.is_empty();
        v_flex()
            .size_full()
            .gap_4()
            .child(
                card().child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .gap_3()
                                .items_end()
                                .child(
                                    Button::new("choose")
                                        .label("Choose JSON file…")
                                        .disabled(running)
                                        .on_click(cx.listener(|this, _, window, cx| this.choose_file(window, cx))),
                                )
                                .child(
                                    v_flex()
                                        .gap_1()
                                        .child(div().text_xs().text_color(color(theme::LABEL)).child("Delay between messages (ms)"))
                                        .child(div().w(px(160.)).child(Input::new(&self.delay).disabled(running))),
                                ),
                        )
                        .child(div().text_xs().text_color(color(theme::MUTED)).child(
                            "Each entry needs \"message\" and \"phone\" (\"customerPhone\" also works). Other fields are kept in the report. Minimum delay 500 ms.",
                        ))
                        .when_some(self.summary.clone(), |el, (ok, text)| {
                            el.child(div().text_sm().text_color(color(if ok { theme::MUTED } else { theme::RED })).child(text))
                        })
                        .child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .child(
                                    Button::new("start")
                                        .primary()
                                        .label("Start Broadcast")
                                        .disabled(running || !self.has_pending())
                                        .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
                                )
                                .child(
                                    Button::new("stop")
                                        .label("Stop")
                                        .disabled(!running)
                                        .on_click(cx.listener(|this, _, _, cx| this.stop(cx))),
                                )
                                .child(
                                    Button::new("report")
                                        .label("Download Report")
                                        .disabled(!loaded)
                                        .on_click(cx.listener(|this, _, window, cx| this.download(false, window, cx))),
                                )
                                .child(
                                    Button::new("report-failed")
                                        .label("Download Failed Only")
                                        .disabled(!self.has_failed())
                                        .on_click(cx.listener(|this, _, window, cx| this.download(true, window, cx))),
                                ),
                        )
                        .when_some(self.progress.clone(), |el, text| {
                            el.child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded(px(6.))
                                    .bg(color(theme::GREEN_BG))
                                    .text_color(color(theme::GREEN))
                                    .text_sm()
                                    .child(text),
                            )
                        }),
                ),
            )
            .when(loaded, |el| el.child(self.table(cx)))
    }
}
