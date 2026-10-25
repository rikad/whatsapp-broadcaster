use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub phone_number: String,
    pub pushname: String,
}

/// Current connection state, published on a `watch` channel. New subscribers
/// (UI, SSE clients) read it once instead of replaying past events.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConnState {
    #[default]
    Starting,
    Pairing {
        qr: Option<String>,
        pair_code: Option<String>,
    },
    /// Paired; waiting for the first `Connected`.
    Authenticating,
    Ready(AccountInfo),
    /// Was ready, connection dropped; the client reconnects on its own.
    Reconnecting(String),
    /// Needs the user: banned, outdated client, session replaced, store error.
    Failed(String),
}

impl ConnState {
    pub fn is_ready(&self) -> bool {
        matches!(self, ConnState::Ready(_))
    }

    pub fn account(&self) -> Option<&AccountInfo> {
        match self {
            ConnState::Ready(info) => Some(info),
            _ => None,
        }
    }
}

/// One-off notifications, published on a `broadcast` channel. Names follow the
/// v1 SSE events where one existed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaEvent {
    Qr(String),
    PairCode(String),
    Authenticated,
    Ready(AccountInfo),
    AuthFailure(String),
    Disconnected(String),
    LoggedOut(String),
    Error(String),
}

impl WaEvent {
    pub fn name(&self) -> &'static str {
        match self {
            WaEvent::Qr(_) => "qr",
            WaEvent::PairCode(_) => "pair-code",
            WaEvent::Authenticated => "authenticated",
            WaEvent::Ready(_) => "ready",
            WaEvent::AuthFailure(_) => "auth-failure",
            WaEvent::Disconnected(_) | WaEvent::LoggedOut(_) => "disconnected",
            WaEvent::Error(_) => "error",
        }
    }
}
