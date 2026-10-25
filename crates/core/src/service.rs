//! Owns the whatsapp-rust `Bot`: starts it, maps its events onto
//! [`ConnState`]/[`WaEvent`], and rebuilds it after logout or failures.
//!
//! Everything runs on the tokio runtime passed to [`WaHandle::start`]. The
//! public async methods hop onto that runtime themselves, so they can be
//! awaited from any executor (gpui's included).

use crate::events::{AccountInfo, ConnState, WaEvent};
use crate::{media, phone};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::prelude::*;
use whatsapp_rust::upload::UploadOptions;
use whatsapp_rust::wacore::store::DevicePropsOverride;

const DEVICE_NAME: &str = "WhatsApp Broadcaster";
const RESTART_DELAY: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SendError {
    #[error("Client not ready")]
    NotReady,
    #[error("Invalid phone number")]
    InvalidPhone,
    #[error("{0} is not on WhatsApp")]
    NotOnWhatsApp(String),
    #[error("Cannot read {path}: {reason}")]
    File { path: String, reason: String },
    #[error("{0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendOutcome {
    pub id: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone)]
pub struct WaHandle {
    inner: Arc<Inner>,
}

struct Inner {
    rt: Handle,
    client: Mutex<Option<Arc<Client>>>,
    state: watch::Sender<ConnState>,
    events: broadcast::Sender<WaEvent>,
    commands: mpsc::UnboundedSender<Command>,
}

enum Command {
    Logout(oneshot::Sender<()>),
    Retry,
    Shutdown(oneshot::Sender<()>),
}

/// Raised by the event handler for things only the supervisor can act on.
enum Signal {
    LoggedOut,
    QrExhausted,
    Fatal(String),
}

enum Exit {
    LoggedOut,
    Restart,
    Failed,
    Shutdown,
}

impl WaHandle {
    /// Spawns the supervisor on `rt`. Session data lives in `data_dir`.
    pub fn start(rt: &Handle, data_dir: PathBuf) -> Self {
        let (commands, command_rx) = mpsc::unbounded_channel();
        let handle = WaHandle {
            inner: Arc::new(Inner {
                rt: rt.clone(),
                client: Mutex::new(None),
                state: watch::Sender::new(ConnState::Starting),
                events: broadcast::channel(64).0,
                commands,
            }),
        };
        rt.spawn(supervise(
            handle.clone(),
            Sessions::new(data_dir),
            command_rx,
        ));
        handle
    }

    /// A handle with no WhatsApp connection behind it, for tests of code that
    /// only reads state (e.g. the HTTP API).
    #[doc(hidden)]
    pub fn offline(rt: &Handle, state: ConnState) -> Self {
        let (commands, _) = mpsc::unbounded_channel();
        WaHandle {
            inner: Arc::new(Inner {
                rt: rt.clone(),
                client: Mutex::new(None),
                state: watch::Sender::new(state),
                events: broadcast::channel(64).0,
                commands,
            }),
        }
    }

    pub fn state(&self) -> watch::Receiver<ConnState> {
        self.inner.state.subscribe()
    }

    pub fn current_state(&self) -> ConnState {
        self.inner.state.borrow().clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WaEvent> {
        self.inner.events.subscribe()
    }

    pub fn runtime(&self) -> &Handle {
        &self.inner.rt
    }

    pub async fn send_text(&self, phone: &str, text: &str) -> Result<SendOutcome, SendError> {
        let digits = phone::normalize(phone).ok_or(SendError::InvalidPhone)?;
        let client = self.ready_client()?;
        let text = text.to_string();
        self.on_runtime(async move {
            let jid = resolve(&client, &digits).await?;
            let sent = client
                .send_message(jid, wa::Message::text(text))
                .await
                .map_err(|e| SendError::Failed(e.to_string()))?;
            Ok(SendOutcome {
                id: sent.message_id,
                to: sent.to.to_string(),
            })
        })
        .await
    }

    pub async fn send_media(
        &self,
        phone: &str,
        path: &Path,
        caption: Option<&str>,
    ) -> Result<SendOutcome, SendError> {
        let digits = phone::normalize(phone).ok_or(SendError::InvalidPhone)?;
        let client = self.ready_client()?;
        let path = path.to_path_buf();
        let caption = caption.filter(|c| !c.is_empty()).map(str::to_string);
        self.on_runtime(async move {
            let data = tokio::fs::read(&path).await.map_err(|e| SendError::File {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
            let jid = resolve(&client, &digits).await?;
            let kind = media::classify(&path);
            let upload = client
                .upload(data, kind.media_type, UploadOptions::new())
                .await
                .map_err(|e| SendError::Failed(format!("Upload failed: {e}")))?;
            let message = media::build_message(&kind, upload, &path, caption);
            let sent = client
                .send_message(jid, message)
                .await
                .map_err(|e| SendError::Failed(e.to_string()))?;
            Ok(SendOutcome {
                id: sent.message_id,
                to: sent.to.to_string(),
            })
        })
        .await
    }

    /// Groups the account is in. whatsapp-rust has no server-side chat list,
    /// so this replaces v1's `getChats()`.
    pub async fn groups(&self) -> Result<Vec<GroupInfo>, SendError> {
        let client = self.ready_client()?;
        self.on_runtime(async move {
            let groups = client
                .groups()
                .get_participating()
                .await
                .map_err(|e| SendError::Failed(e.to_string()))?;
            let mut list: Vec<_> = groups
                .into_values()
                .map(|g| GroupInfo {
                    id: g.id.to_string(),
                    name: g.subject,
                })
                .collect();
            list.sort_by_key(|g| g.name.to_lowercase());
            Ok(list)
        })
        .await
    }

    /// Asks for an 8-character code to link by phone number instead of QR.
    pub async fn request_pair_code(&self, phone: &str) -> Result<String, SendError> {
        let digits = phone::normalize(phone).ok_or(SendError::InvalidPhone)?;
        let client = self
            .inner
            .client
            .lock()
            .unwrap()
            .clone()
            .ok_or(SendError::NotReady)?;
        self.on_runtime(async move {
            let options = PairCodeOptions {
                phone_number: digits,
                ..Default::default()
            };
            client
                .pair_with_code(options)
                .await
                .map_err(|e| SendError::Failed(e.to_string()))
        })
        .await
    }

    /// Unlinks this device. A fresh QR follows once the new session starts.
    pub async fn logout(&self) {
        let (done, wait) = oneshot::channel();
        if self.inner.commands.send(Command::Logout(done)).is_ok() {
            let _ = wait.await;
        }
    }

    /// Starts a new session after [`ConnState::Failed`].
    pub fn retry(&self) {
        let _ = self.inner.commands.send(Command::Retry);
    }

    /// Disconnects cleanly so the session state is flushed to disk.
    pub async fn shutdown(&self) {
        let (done, wait) = oneshot::channel();
        if self.inner.commands.send(Command::Shutdown(done)).is_ok() {
            // The timer needs tokio; callers may be on another executor.
            let _ = self
                .inner
                .rt
                .spawn(tokio::time::timeout(Duration::from_secs(5), wait))
                .await;
        }
    }

    fn ready_client(&self) -> Result<Arc<Client>, SendError> {
        if !self.inner.state.borrow().is_ready() {
            return Err(SendError::NotReady);
        }
        self.inner
            .client
            .lock()
            .unwrap()
            .clone()
            .ok_or(SendError::NotReady)
    }

    async fn on_runtime<T: Send + 'static>(
        &self,
        work: impl Future<Output = Result<T, SendError>> + Send + 'static,
    ) -> Result<T, SendError> {
        self.inner
            .rt
            .spawn(work)
            .await
            .map_err(|e| SendError::Failed(e.to_string()))?
    }

    fn set_client(&self, client: Option<Arc<Client>>) {
        *self.inner.client.lock().unwrap() = client;
    }

    fn set_state(&self, state: ConnState) {
        self.inner.state.send_replace(state);
    }

    fn emit(&self, event: WaEvent) {
        let _ = self.inner.events.send(event);
    }

    fn handle_event(
        &self,
        event: &Event,
        client: &Client,
        signals: &mpsc::UnboundedSender<Signal>,
    ) {
        let state = self.current_state();
        match event {
            Event::PairingQrCode(qr) => {
                let pair_code = match state {
                    ConnState::Pairing { pair_code, .. } => pair_code,
                    _ => None,
                };
                self.set_state(ConnState::Pairing {
                    qr: Some(qr.code.clone()),
                    pair_code,
                });
                self.emit(WaEvent::Qr(qr.code.clone()));
            }
            Event::PairingCode(code) => {
                let qr = match state {
                    ConnState::Pairing { qr, .. } => qr,
                    _ => None,
                };
                self.set_state(ConnState::Pairing {
                    qr,
                    pair_code: Some(code.code.clone()),
                });
                self.emit(WaEvent::PairCode(code.code.clone()));
            }
            Event::PairingCodeError(e) => {
                self.emit(WaEvent::Error(format!("Pairing code failed: {}", e.error)));
            }
            Event::PairSuccess(_) => {
                self.set_state(ConnState::Authenticating);
                self.emit(WaEvent::Authenticated);
            }
            Event::PairError(e) => self.emit(WaEvent::AuthFailure(e.error.clone())),
            Event::PairingQrCodesExhausted(_) => {
                let _ = signals.send(Signal::QrExhausted);
            }
            Event::Connected(_) => {
                let info = account_info(client);
                self.set_state(ConnState::Ready(info.clone()));
                self.emit(WaEvent::Ready(info));
            }
            Event::SelfPushNameUpdated(_) => {
                if state.is_ready() {
                    self.set_state(ConnState::Ready(account_info(client)));
                }
            }
            Event::Disconnected(d) => {
                if matches!(state, ConnState::Ready(_) | ConnState::Authenticating) {
                    let reason = d.reason.to_string();
                    self.set_state(ConnState::Reconnecting(reason.clone()));
                    self.emit(WaEvent::Disconnected(reason));
                }
            }
            Event::LoggedOut(l) => {
                self.emit(WaEvent::LoggedOut(format!("{:?}", l.reason)));
                let _ = signals.send(Signal::LoggedOut);
            }
            Event::ClientOutdated(_) => {
                let _ = signals.send(Signal::Fatal(
                    "WhatsApp rejected this app version. Update WhatsApp Broadcaster.".into(),
                ));
            }
            Event::TemporaryBan(ban) => {
                let _ = signals.send(Signal::Fatal(format!(
                    "This number is temporarily banned ({}). Try again in {} minutes.",
                    ban.code,
                    (ban.expire.num_seconds().max(0) as u64).div_ceil(60)
                )));
            }
            Event::StreamReplaced(_) => {
                let _ = signals.send(Signal::Fatal(
                    "This WhatsApp session was opened somewhere else.".into(),
                ));
            }
            Event::ConnectFailure(f) => {
                let detail = f
                    .message
                    .as_deref()
                    .map(|m| format!(": {m}"))
                    .unwrap_or_default();
                self.emit(WaEvent::Error(format!(
                    "Connection failed ({:?}){detail}",
                    f.reason
                )));
            }
            _ => {}
        }
    }
}

fn account_info(client: &Client) -> AccountInfo {
    AccountInfo {
        phone_number: client
            .pn()
            .map(|jid| jid.user.to_string())
            .unwrap_or_default(),
        pushname: client.push_name(),
    }
}

/// Checks the number is registered and returns the JID to send to. The server
/// may hand back a corrected number (e.g. Brazil's extra 9).
async fn resolve(client: &Arc<Client>, digits: &str) -> Result<Jid, SendError> {
    let jid = Jid::pn(digits.to_string());
    let results = client
        .contacts()
        .is_on_whatsapp(std::slice::from_ref(&jid))
        .await
        .map_err(|e| SendError::Failed(format!("Number check failed: {e}")))?;
    match results.into_iter().next() {
        Some(r) if r.is_registered => Ok(if r.jid.is_pn() {
            r.jid
        } else {
            r.pn_jid.unwrap_or(jid)
        }),
        _ => Err(SendError::NotOnWhatsApp(digits.to_string())),
    }
}

async fn supervise(
    handle: WaHandle,
    sessions: Sessions,
    mut commands: mpsc::UnboundedReceiver<Command>,
) {
    sessions.cleanup();
    loop {
        handle.set_state(ConnState::Starting);
        let exit = run_session(&handle, &sessions, &mut commands).await;
        handle.set_client(None);
        let exit = match exit {
            Exit::Restart => idle(&mut commands, Some(RESTART_DELAY)).await,
            Exit::Failed => idle(&mut commands, None).await,
            other => other,
        };
        match exit {
            Exit::Shutdown => return,
            Exit::LoggedOut => sessions.rotate(),
            Exit::Restart | Exit::Failed => {}
        }
    }
}

/// Waits between sessions: for `delay`, or until the user retries.
async fn idle(commands: &mut mpsc::UnboundedReceiver<Command>, delay: Option<Duration>) -> Exit {
    let sleep = async {
        match delay {
            Some(d) => tokio::time::sleep(d).await,
            None => std::future::pending().await,
        }
    };
    tokio::select! {
        _ = sleep => Exit::Restart,
        command = commands.recv() => match command {
            Some(Command::Retry) => Exit::Restart,
            Some(Command::Logout(done)) => {
                let _ = done.send(());
                Exit::LoggedOut
            }
            Some(Command::Shutdown(done)) => {
                let _ = done.send(());
                Exit::Shutdown
            }
            None => Exit::Shutdown,
        },
    }
}

async fn run_session(
    handle: &WaHandle,
    sessions: &Sessions,
    commands: &mut mpsc::UnboundedReceiver<Command>,
) -> Exit {
    let db = sessions.current();
    let store = match SqliteStore::new(&db.to_string_lossy()).await {
        Ok(store) => store,
        Err(e) => {
            fail(handle, format!("Cannot open the session database: {e}"));
            return Exit::Failed;
        }
    };

    let (signal_tx, mut signals) = mpsc::unbounded_channel();
    let events_handle = handle.clone();
    let bot = Bot::builder()
        .with_backend(store)
        .skip_history_sync()
        .with_device_props(DevicePropsOverride::new().with_os(DEVICE_NAME))
        .with_event_delivery(EventDelivery::Ordered { capacity: 256 })
        .on_event_for(
            &[
                EventKind::PairingQrCode,
                EventKind::PairingCode,
                EventKind::PairingCodeError,
                EventKind::PairSuccess,
                EventKind::PairError,
                EventKind::PairingQrCodesExhausted,
                EventKind::Connected,
                EventKind::SelfPushNameUpdated,
                EventKind::Disconnected,
                EventKind::LoggedOut,
                EventKind::ClientOutdated,
                EventKind::TemporaryBan,
                EventKind::StreamReplaced,
                EventKind::ConnectFailure,
            ],
            move |event, client| {
                events_handle.handle_event(&event, &client, &signal_tx);
                std::future::ready(())
            },
        )
        .build()
        .await;
    let bot = match bot {
        Ok(bot) => bot,
        Err(e) => {
            fail(handle, format!("Cannot start WhatsApp: {e}"));
            return Exit::Failed;
        }
    };

    let mut running = bot.spawn();
    let client = running.client();
    handle.set_client(Some(client.clone()));

    enum Wake {
        Ended,
        Signal(Signal),
        Command(Option<Command>),
    }
    loop {
        let wake = tokio::select! {
            _ = &mut running => Wake::Ended,
            Some(signal) = signals.recv() => Wake::Signal(signal),
            command = commands.recv() => Wake::Command(command),
        };
        match wake {
            Wake::Ended => {
                // A logout or fatal error usually ends the run loop itself; the
                // signal that explains it may still be queued.
                return match signals.try_recv() {
                    Ok(Signal::LoggedOut) => Exit::LoggedOut,
                    Ok(Signal::Fatal(msg)) => {
                        fail(handle, msg);
                        Exit::Failed
                    }
                    _ => Exit::Restart,
                };
            }
            Wake::Signal(Signal::LoggedOut) => {
                running.shutdown().await;
                return Exit::LoggedOut;
            }
            Wake::Signal(Signal::QrExhausted) => {
                running.shutdown().await;
                return Exit::Restart;
            }
            Wake::Signal(Signal::Fatal(msg)) => {
                fail(handle, msg);
                running.shutdown().await;
                return Exit::Failed;
            }
            Wake::Command(Some(Command::Logout(done))) => {
                client.logout().await;
                running.shutdown().await;
                let _ = done.send(());
                return Exit::LoggedOut;
            }
            Wake::Command(Some(Command::Shutdown(done))) => {
                running.shutdown().await;
                let _ = done.send(());
                return Exit::Shutdown;
            }
            Wake::Command(None) => {
                running.shutdown().await;
                return Exit::Shutdown;
            }
            Wake::Command(Some(Command::Retry)) => {}
        }
    }
}

fn fail(handle: &WaHandle, message: String) {
    log::error!("{message}");
    handle.set_state(ConnState::Failed(message.clone()));
    handle.emit(WaEvent::Error(message));
}

/// One SQLite file per linked session. Logging out switches to a new file
/// instead of deleting the open one, which Windows refuses while any handle is
/// still alive; stale files are removed best-effort on the next start.
struct Sessions {
    dir: PathBuf,
}

impl Sessions {
    const POINTER: &'static str = "session.txt";
    const DEFAULT: &'static str = "whatsapp.db";

    fn new(dir: PathBuf) -> Self {
        Sessions { dir }
    }

    fn current_name(&self) -> String {
        std::fs::read_to_string(self.dir.join(Self::POINTER))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Self::DEFAULT.to_string())
    }

    fn current(&self) -> PathBuf {
        self.dir.join(self.current_name())
    }

    fn rotate(&self) {
        let name = format!("whatsapp-{}.db", chrono::Utc::now().timestamp_millis());
        if let Err(e) = std::fs::write(self.dir.join(Self::POINTER), &name) {
            log::error!("Cannot switch session file: {e}");
        }
        self.cleanup();
    }

    fn cleanup(&self) {
        let current = self.current_name();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_session = name.starts_with("whatsapp") && name.contains(".db");
            if is_session && !name.starts_with(&current) {
                // Still locked files are retried on the next start.
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sessions;

    #[test]
    fn rotate_switches_file_and_cleans_old_ones() {
        let dir = std::env::temp_dir().join(format!("wb-sessions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sessions = Sessions::new(dir.clone());
        assert!(sessions.current().ends_with("whatsapp.db"));
        for f in [
            "whatsapp.db",
            "whatsapp.db-wal",
            "whatsapp.db-shm",
            "notes.txt",
        ] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        sessions.rotate();
        let current = sessions.current();
        assert!(
            current
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("whatsapp-")
        );
        assert!(!dir.join("whatsapp.db").exists());
        assert!(!dir.join("whatsapp.db-wal").exists());
        assert!(dir.join("notes.txt").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
