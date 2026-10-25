//! Local HTTP API, route-compatible with v1 (`api.js`). Listens on
//! 127.0.0.1 only.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use futures::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use wb_core::{ConnState, SendError, SendOutcome, WaEvent, WaHandle};

pub const DEFAULT_PORT: u16 = 1111;

/// `PORT` env var, else 1111.
pub fn port_from_env() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub async fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await
}

pub async fn serve(listener: TcpListener, wa: WaHandle) -> std::io::Result<()> {
    if let Ok(addr) = listener.local_addr() {
        log::info!("HTTP API listening on http://{addr}");
    }
    axum::serve(listener, router(wa)).await
}

pub fn router(wa: WaHandle) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/info", get(info))
        .route("/api/send", post(send))
        .route("/api/send-media", post(send_media))
        .route("/api/chats", get(chats))
        .route("/api/logout", post(logout))
        .route("/api/events", get(events))
        .fallback(|| async { error(StatusCode::NOT_FOUND, "Endpoint not found") })
        .with_state(wa)
}

fn error(code: StatusCode, message: impl Into<String>) -> Response {
    (code, Json(json!({ "error": message.into() }))).into_response()
}

fn not_ready() -> Response {
    error(StatusCode::SERVICE_UNAVAILABLE, "Client not ready")
}

fn send_error(e: SendError) -> Response {
    let code = match e {
        SendError::NotReady => StatusCode::SERVICE_UNAVAILABLE,
        SendError::InvalidPhone => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    error(code, e.to_string())
}

fn sent(outcome: SendOutcome) -> Response {
    Json(json!({
        "success": true,
        "id": outcome.id,
        "to": outcome.to,
        "timestamp": wb_core::broadcast::now_iso(),
    }))
    .into_response()
}

/// v1 read `req.body || {}`, so a missing or malformed body is treated as
/// empty and reported as missing fields rather than a parse error.
fn body<T: for<'de> Deserialize<'de> + Default>(bytes: &Bytes) -> T {
    serde_json::from_slice(bytes).unwrap_or_default()
}

/// Accepts `"628…"` or `628…` and treats blanks as missing.
fn text(value: &Option<Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

async fn status(State(wa): State<WaHandle>) -> Response {
    let state = wa.current_state();
    let info = state.account();
    Json(json!({
        "status": if info.is_some() { "ready" } else { "not_ready" },
        "info": info,
    }))
    .into_response()
}

async fn info(State(wa): State<WaHandle>) -> Response {
    match wa.current_state().account() {
        Some(info) => Json(info).into_response(),
        None => not_ready(),
    }
}

#[derive(Default, Deserialize)]
struct SendBody {
    phone: Option<Value>,
    message: Option<Value>,
}

async fn send(State(wa): State<WaHandle>, bytes: Bytes) -> Response {
    if !wa.current_state().is_ready() {
        return not_ready();
    }
    let req: SendBody = body(&bytes);
    let (Some(phone), Some(message)) = (text(&req.phone), text(&req.message)) else {
        return error(StatusCode::BAD_REQUEST, "Missing phone or message");
    };
    match wa.send_text(&phone, &message).await {
        Ok(outcome) => sent(outcome),
        Err(e) => send_error(e),
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendMediaBody {
    phone: Option<Value>,
    media_path: Option<Value>,
    caption: Option<Value>,
}

async fn send_media(State(wa): State<WaHandle>, bytes: Bytes) -> Response {
    if !wa.current_state().is_ready() {
        return not_ready();
    }
    let req: SendMediaBody = body(&bytes);
    let (Some(phone), Some(path)) = (text(&req.phone), text(&req.media_path)) else {
        return error(StatusCode::BAD_REQUEST, "Missing phone or mediaPath");
    };
    let caption = text(&req.caption);
    match wa
        .send_media(&phone, &PathBuf::from(path), caption.as_deref())
        .await
    {
        Ok(outcome) => sent(outcome),
        Err(e) => send_error(e),
    }
}

async fn chats(State(wa): State<WaHandle>) -> Response {
    if !wa.current_state().is_ready() {
        return not_ready();
    }
    match wa.groups().await {
        Ok(groups) => Json(
            groups
                .into_iter()
                .map(|g| json!({ "id": g.id, "name": g.name, "isGroup": true }))
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(e) => send_error(e),
    }
}

async fn logout(State(wa): State<WaHandle>) -> Response {
    wa.logout().await;
    Json(json!({ "success": true })).into_response()
}

fn qr_data_url(code: &str) -> Value {
    match wb_core::qr::png(code, 6) {
        Ok(png) => format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
        .into(),
        Err(_) => Value::Null,
    }
}

fn sse_event(event: &WaEvent) -> Event {
    let data = match event {
        // v1 sent the QR as a PNG data URL; keep that so existing clients work.
        WaEvent::Qr(code) => qr_data_url(code),
        WaEvent::PairCode(code) => code.clone().into(),
        WaEvent::Authenticated => Value::Null,
        WaEvent::Ready(info) => json!(info),
        WaEvent::AuthFailure(msg)
        | WaEvent::Disconnected(msg)
        | WaEvent::LoggedOut(msg)
        | WaEvent::Error(msg) => msg.clone().into(),
    };
    Event::default().event(event.name()).data(data.to_string())
}

/// What a client connecting now would have seen, replacing v1's `lastEvents`.
fn replay(state: &ConnState) -> Vec<WaEvent> {
    match state {
        ConnState::Pairing { qr, pair_code } => qr
            .iter()
            .map(|q| WaEvent::Qr(q.clone()))
            .chain(pair_code.iter().map(|c| WaEvent::PairCode(c.clone())))
            .collect(),
        ConnState::Authenticating => vec![WaEvent::Authenticated],
        ConnState::Ready(info) => vec![WaEvent::Authenticated, WaEvent::Ready(info.clone())],
        ConnState::Failed(msg) => vec![WaEvent::Error(msg.clone())],
        ConnState::Starting | ConnState::Reconnecting(_) => vec![],
    }
}

async fn events(State(wa): State<WaHandle>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribe before reading the state so nothing falls in between.
    let live = wa.subscribe();
    let initial = replay(&wa.current_state());
    let live = stream::unfold(live, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => return Some((event, rx)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    let stream = stream::iter(initial)
        .chain(live)
        .map(|event| Ok(sse_event(&event)));
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(25))
            .text("ping"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use wb_core::AccountInfo;

    fn account() -> AccountInfo {
        AccountInfo {
            phone_number: "628123".into(),
            pushname: "Rikad".into(),
        }
    }

    async fn call(state: ConnState, method: &str, uri: &str, body: &str) -> (StatusCode, Value) {
        let wa = WaHandle::offline(&tokio::runtime::Handle::current(), state);
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let res = router(wa).oneshot(req).await.unwrap();
        let code = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (code, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    #[tokio::test]
    async fn status_reports_readiness() {
        let (code, body) = call(ConnState::Starting, "GET", "/api/status", "").await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body, json!({ "status": "not_ready", "info": null }));

        let (_, body) = call(ConnState::Ready(account()), "GET", "/api/status", "").await;
        assert_eq!(body["status"], "ready");
        assert_eq!(
            body["info"],
            json!({ "phoneNumber": "628123", "pushname": "Rikad" })
        );
    }

    #[tokio::test]
    async fn info_is_503_until_ready() {
        let (code, body) = call(ConnState::Starting, "GET", "/api/info", "").await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body, json!({ "error": "Client not ready" }));
        let (code, _) = call(ConnState::Ready(account()), "GET", "/api/info", "").await;
        assert_eq!(code, StatusCode::OK);
    }

    #[tokio::test]
    async fn send_checks_readiness_then_fields() {
        let ok_body = r#"{"phone":"628123","message":"hi"}"#;
        let (code, _) = call(ConnState::Starting, "POST", "/api/send", ok_body).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);

        for body in [
            "",
            "{}",
            r#"{"phone":"1"}"#,
            r#"{"phone":"1","message":"  "}"#,
            "not json",
        ] {
            let (code, json) = call(ConnState::Ready(account()), "POST", "/api/send", body).await;
            assert_eq!(code, StatusCode::BAD_REQUEST, "body {body:?}");
            assert_eq!(json["error"], "Missing phone or message");
        }

        let (code, json) = call(
            ConnState::Ready(account()),
            "POST",
            "/api/send-media",
            r#"{"phone":"1"}"#,
        )
        .await;
        assert_eq!(code, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "Missing phone or mediaPath");
    }

    #[tokio::test]
    async fn unknown_endpoint_is_json_404() {
        let (code, body) = call(ConnState::Starting, "GET", "/api/nope", "").await;
        assert_eq!(code, StatusCode::NOT_FOUND);
        assert_eq!(body, json!({ "error": "Endpoint not found" }));
    }

    #[test]
    fn replay_matches_state() {
        let names = |s: ConnState| replay(&s).iter().map(WaEvent::name).collect::<Vec<_>>();
        assert_eq!(names(ConnState::Starting), Vec::<&str>::new());
        assert_eq!(
            names(ConnState::Pairing {
                qr: Some("x".into()),
                pair_code: None
            }),
            ["qr"]
        );
        assert_eq!(
            names(ConnState::Ready(account())),
            ["authenticated", "ready"]
        );
        assert_eq!(names(ConnState::Failed("banned".into())), ["error"]);
    }

    #[test]
    fn qr_event_is_png_data_url() {
        let data = qr_data_url("2@abc");
        assert!(
            data.as_str()
                .unwrap()
                .starts_with("data:image/png;base64,iVBOR")
        );
    }
}
