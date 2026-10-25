//! Bulk sending from a JSON file.
//!
//! Only `message` and the phone (`phone`, falling back to `customerPhone`) are
//! read from each row. Every other key is kept untouched and written back in
//! the report, alongside the `broadcast*` result fields.

use crate::service::WaHandle;
use chrono::{SecondsFormat, Utc};
use serde_json::{Map, Value};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const MIN_DELAY: Duration = Duration::from_millis(500);
pub const DEFAULT_DELAY: Duration = Duration::from_millis(3000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    Pending,
    Sending,
    Success,
    Failed,
}

impl RowStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RowStatus::Pending => "pending",
            RowStatus::Sending => "sending",
            RowStatus::Success => "success",
            RowStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub phone: String,
    pub message: String,
    pub status: RowStatus,
    pub error: Option<String>,
    pub sent_at: Option<String>,
    pub message_id: Option<String>,
    raw: Map<String, Value>,
}

impl Row {
    fn to_report_value(&self) -> Value {
        let mut obj = self.raw.clone();
        let opt = |v: &Option<String>| v.clone().map_or(Value::Null, Value::String);
        obj.insert("broadcastStatus".into(), self.status.as_str().into());
        obj.insert("broadcastError".into(), opt(&self.error));
        obj.insert("broadcastSentAt".into(), opt(&self.sent_at));
        obj.insert("broadcastMessageId".into(), opt(&self.message_id));
        Value::Object(obj)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("{0}")]
    Json(String),
    #[error("JSON root must be an array")]
    NotAnArray,
    #[error("Entry #{0} is missing phone or message")]
    MissingField(usize),
}

fn non_empty_str<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

pub fn parse(json: &str) -> Result<Vec<Row>, ParseError> {
    let value: Value = serde_json::from_str(json).map_err(|e| ParseError::Json(e.to_string()))?;
    let Value::Array(items) = value else {
        return Err(ParseError::NotAnArray);
    };
    items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let Value::Object(raw) = item else {
                return Err(ParseError::MissingField(idx + 1));
            };
            let phone =
                non_empty_str(&raw, "phone").or_else(|| non_empty_str(&raw, "customerPhone"));
            let message = non_empty_str(&raw, "message");
            let (Some(phone), Some(message)) = (phone, message) else {
                return Err(ParseError::MissingField(idx + 1));
            };
            let (phone, message) = (phone.to_string(), message.to_string());

            // Rows already sent in an earlier run keep their result and are
            // skipped; anything else starts over as pending.
            let done = raw.get("broadcastStatus").and_then(Value::as_str) == Some("success");
            let keep = |key: &str| {
                done.then(|| raw.get(key).and_then(Value::as_str).map(str::to_string))
                    .flatten()
            };
            Ok(Row {
                phone,
                message,
                status: if done {
                    RowStatus::Success
                } else {
                    RowStatus::Pending
                },
                error: keep("broadcastError"),
                sent_at: keep("broadcastSentAt"),
                message_id: keep("broadcastMessageId"),
                raw,
            })
        })
        .collect()
}

/// Pretty-printed report, every row or only the failed ones.
pub fn report_json(rows: &[Row], failed_only: bool) -> String {
    let values: Vec<Value> = rows
        .iter()
        .filter(|r| !failed_only || r.status == RowStatus::Failed)
        .map(Row::to_report_value)
        .collect();
    serde_json::to_string_pretty(&values).expect("JSON values always serialize")
}

/// `{source}-report-{stamp}.json` / `{source}-failed-{stamp}.json`, as in v1.
pub fn report_file_name(source: &str, failed_only: bool) -> String {
    let stamp = Utc::now()
        .to_rfc3339_opts(SecondsFormat::Millis, true)
        .replace([':', '.'], "-");
    let suffix = if failed_only { "failed" } else { "report" };
    format!("{source}-{suffix}-{stamp}.json")
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub success: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    Sending {
        index: usize,
        counts: Counts,
    },
    Sent {
        index: usize,
        message_id: String,
        sent_at: String,
    },
    Failed {
        index: usize,
        error: String,
        sent_at: String,
    },
    Finished {
        counts: Counts,
        stopped: bool,
    },
}

pub struct Job {
    pub index: usize,
    pub phone: String,
    pub message: String,
}

/// Running broadcast. Drop or `stop()` to end it after the current message.
pub struct Run {
    pub updates: mpsc::UnboundedReceiver<Update>,
    stop: CancellationToken,
}

impl Run {
    pub fn stop(&self) {
        self.stop.cancel();
    }

    pub fn stop_token(&self) -> CancellationToken {
        self.stop.clone()
    }
}

/// Jobs for every row that still needs sending. `skipped` counts the rest.
pub fn jobs(rows: &[Row]) -> (Vec<Job>, usize) {
    let mut skipped = 0;
    let jobs = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            if row.status == RowStatus::Success {
                skipped += 1;
                return None;
            }
            Some(Job {
                index,
                phone: row.phone.clone(),
                message: row.message.clone(),
            })
        })
        .collect();
    (jobs, skipped)
}

/// Abstraction over sending so the loop can be tested without WhatsApp.
pub trait Sender: Send + Sync + 'static {
    fn send(
        &self,
        phone: String,
        message: String,
    ) -> impl Future<Output = Result<String, String>> + Send;
}

impl Sender for WaHandle {
    async fn send(&self, phone: String, message: String) -> Result<String, String> {
        self.send_text(&phone, &message)
            .await
            .map(|outcome| outcome.id)
            .map_err(|e| e.to_string())
    }
}

/// Starts sending on the current tokio runtime.
pub fn start<S: Sender>(sender: S, jobs: Vec<Job>, skipped: usize, delay: Duration) -> Run {
    let (tx, updates) = mpsc::unbounded_channel();
    let stop = CancellationToken::new();
    tokio::spawn(run_loop(
        sender,
        jobs,
        skipped,
        delay.max(MIN_DELAY),
        stop.clone(),
        tx,
    ));
    Run { updates, stop }
}

async fn run_loop<S: Sender>(
    sender: S,
    jobs: Vec<Job>,
    skipped: usize,
    delay: Duration,
    stop: CancellationToken,
    tx: mpsc::UnboundedSender<Update>,
) {
    let mut counts = Counts {
        skipped,
        ..Counts::default()
    };
    let total = jobs.len();
    for (n, job) in jobs.into_iter().enumerate() {
        if stop.is_cancelled() {
            break;
        }
        let _ = tx.send(Update::Sending {
            index: job.index,
            counts,
        });
        let result = sender.send(job.phone, job.message).await;
        let sent_at = now_iso();
        let _ = tx.send(match result {
            Ok(message_id) => {
                counts.success += 1;
                Update::Sent {
                    index: job.index,
                    message_id,
                    sent_at,
                }
            }
            Err(error) => {
                counts.failed += 1;
                Update::Failed {
                    index: job.index,
                    error,
                    sent_at,
                }
            }
        });
        if n + 1 < total {
            tokio::select! {
                _ = stop.cancelled() => break,
                _ = tokio::time::sleep(delay) => {}
            }
        }
    }
    let _ = tx.send(Update::Finished {
        counts,
        stopped: stop.is_cancelled(),
    });
}

/// Applies an update to the UI's copy of the rows.
pub fn apply(rows: &mut [Row], update: &Update) {
    match update {
        Update::Sending { index, .. } => {
            if let Some(row) = rows.get_mut(*index) {
                row.status = RowStatus::Sending;
                row.error = None;
            }
        }
        Update::Sent {
            index,
            message_id,
            sent_at,
        } => {
            if let Some(row) = rows.get_mut(*index) {
                row.status = RowStatus::Success;
                row.error = None;
                row.message_id = Some(message_id.clone());
                row.sent_at = Some(sent_at.clone());
            }
        }
        Update::Failed {
            index,
            error,
            sent_at,
        } => {
            if let Some(row) = rows.get_mut(*index) {
                row.status = RowStatus::Failed;
                row.error = Some(error.clone());
                row.sent_at = Some(sent_at.clone());
            }
        }
        Update::Finished { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn phone_is_default_customer_phone_is_fallback() {
        let rows = parse(
            r#"[
                {"phone": "111", "customerPhone": "222", "message": "a"},
                {"customerPhone": "333", "message": "b"},
                {"phone": "", "customerPhone": "444", "message": "c"}
            ]"#,
        )
        .unwrap();
        let phones: Vec<_> = rows.iter().map(|r| r.phone.as_str()).collect();
        assert_eq!(phones, ["111", "333", "444"]);
    }

    #[test]
    fn rejects_rows_missing_phone_or_message() {
        assert_eq!(
            parse(r#"[{"phone": "1", "message": "ok"}, {"phone": "2"}]"#).unwrap_err(),
            ParseError::MissingField(2)
        );
        assert_eq!(
            parse(r#"[{"message": "no phone"}]"#).unwrap_err(),
            ParseError::MissingField(1)
        );
        assert_eq!(
            parse(r#"[{"phone": 62812, "message": "x"}]"#).unwrap_err(),
            ParseError::MissingField(1)
        );
        assert_eq!(
            parse(r#"{"phone": "1"}"#).unwrap_err(),
            ParseError::NotAnArray
        );
        assert!(matches!(parse("not json"), Err(ParseError::Json(_))));
    }

    #[test]
    fn other_fields_survive_round_trip_in_order() {
        let rows = parse(
            r#"[{"customerName": "Neli", "phone": "1", "amount": 100000, "nested": {"a": [1, 2]}, "message": "hi"}]"#,
        )
        .unwrap();
        let report: Value = serde_json::from_str(&report_json(&rows, false)).unwrap();
        let obj = report[0].as_object().unwrap();
        let keys: Vec<_> = obj.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "customerName",
                "phone",
                "amount",
                "nested",
                "message",
                "broadcastStatus",
                "broadcastError",
                "broadcastSentAt",
                "broadcastMessageId"
            ]
        );
        assert_eq!(obj["amount"], 100000);
        assert_eq!(obj["nested"]["a"][1], 2);
        assert_eq!(obj["broadcastStatus"], "pending");
        assert_eq!(obj["broadcastError"], Value::Null);
    }

    #[test]
    fn previous_success_is_kept_other_results_reset() {
        let rows = parse(
            r#"[
                {"phone": "1", "message": "a", "broadcastStatus": "success", "broadcastMessageId": "X1", "broadcastSentAt": "t"},
                {"phone": "2", "message": "b", "broadcastStatus": "failed", "broadcastError": "boom"}
            ]"#,
        )
        .unwrap();
        assert_eq!(rows[0].status, RowStatus::Success);
        assert_eq!(rows[0].message_id.as_deref(), Some("X1"));
        assert_eq!(rows[1].status, RowStatus::Pending);
        assert_eq!(rows[1].error, None);

        let (jobs, skipped) = jobs(&rows);
        assert_eq!(skipped, 1);
        assert_eq!(jobs.iter().map(|j| j.index).collect::<Vec<_>>(), [1]);
    }

    #[test]
    fn failed_only_report() {
        let mut rows =
            parse(r#"[{"phone": "1", "message": "a"}, {"phone": "2", "message": "b"}]"#).unwrap();
        rows[1].status = RowStatus::Failed;
        let report: Vec<Value> = serde_json::from_str(&report_json(&rows, true)).unwrap();
        assert_eq!(report.len(), 1);
        assert_eq!(report[0]["phone"], "2");
    }

    #[test]
    fn report_file_name_shape() {
        let name = report_file_name("invoices", true);
        assert!(name.starts_with("invoices-failed-"));
        assert!(name.ends_with("Z.json"));
        assert!(!name.trim_end_matches(".json").contains([':', '.']));
    }

    #[derive(Clone, Default)]
    struct FakeSender {
        sent: Arc<Mutex<Vec<String>>>,
    }

    impl Sender for FakeSender {
        async fn send(&self, phone: String, _message: String) -> Result<String, String> {
            self.sent.lock().unwrap().push(phone.clone());
            if phone == "bad" {
                Err("not on WhatsApp".into())
            } else {
                Ok(format!("id-{phone}"))
            }
        }
    }

    async fn collect(mut run: Run) -> Vec<Update> {
        let mut out = Vec::new();
        while let Some(u) = run.updates.recv().await {
            let done = matches!(u, Update::Finished { .. });
            out.push(u);
            if done {
                break;
            }
        }
        out
    }

    #[tokio::test(start_paused = true)]
    async fn sends_pending_rows_and_counts_results() {
        let mut rows = parse(
            r#"[
                {"phone": "1", "message": "a", "broadcastStatus": "success"},
                {"phone": "bad", "message": "b"},
                {"phone": "3", "message": "c"}
            ]"#,
        )
        .unwrap();
        let sender = FakeSender::default();
        let (jobs, skipped) = jobs(&rows);
        let updates = collect(start(
            sender.clone(),
            jobs,
            skipped,
            Duration::from_millis(10),
        ))
        .await;
        for u in &updates {
            apply(&mut rows, u);
        }
        assert_eq!(*sender.sent.lock().unwrap(), ["bad", "3"]);
        assert_eq!(
            updates.last(),
            Some(&Update::Finished {
                counts: Counts {
                    success: 1,
                    failed: 1,
                    skipped: 1
                },
                stopped: false
            })
        );
        assert_eq!(rows[1].status, RowStatus::Failed);
        assert_eq!(rows[1].error.as_deref(), Some("not on WhatsApp"));
        assert_eq!(rows[2].status, RowStatus::Success);
        assert_eq!(rows[2].message_id.as_deref(), Some("id-3"));
    }

    #[tokio::test(start_paused = true)]
    async fn delay_never_below_floor() {
        let rows =
            parse(r#"[{"phone": "1", "message": "a"}, {"phone": "2", "message": "b"}]"#).unwrap();
        let (jobs, skipped) = jobs(&rows);
        let started = tokio::time::Instant::now();
        collect(start(
            FakeSender::default(),
            jobs,
            skipped,
            Duration::from_millis(1),
        ))
        .await;
        assert!(started.elapsed() >= MIN_DELAY);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_ends_after_current_message() {
        let rows = parse(
            r#"[{"phone": "1", "message": "a"}, {"phone": "2", "message": "b"}, {"phone": "3", "message": "c"}]"#,
        )
        .unwrap();
        let sender = FakeSender::default();
        let (jobs, skipped) = jobs(&rows);
        let mut run = start(sender.clone(), jobs, skipped, Duration::from_secs(60));
        // First message goes out, then the loop sleeps; stopping cuts the sleep short.
        assert!(matches!(
            run.updates.recv().await,
            Some(Update::Sending { index: 0, .. })
        ));
        assert!(matches!(
            run.updates.recv().await,
            Some(Update::Sent { index: 0, .. })
        ));
        run.stop();
        let rest = collect(run).await;
        assert_eq!(
            rest,
            [Update::Finished {
                counts: Counts {
                    success: 1,
                    failed: 0,
                    skipped: 0
                },
                stopped: true
            }]
        );
        assert_eq!(*sender.sent.lock().unwrap(), ["1"]);
    }
}
