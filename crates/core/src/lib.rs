//! WhatsApp connection, sending and broadcast logic shared by the desktop app
//! and the HTTP API. Nothing in here touches the UI.

pub mod broadcast;
pub mod events;
pub mod media;
pub mod paths;
pub mod phone;
pub mod qr;
pub mod service;

pub use events::{AccountInfo, ConnState, WaEvent};
pub use service::{GroupInfo, SendError, SendOutcome, WaHandle};
