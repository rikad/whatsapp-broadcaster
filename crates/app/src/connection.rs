//! The app's view of the WhatsApp connection: mirrors `WaHandle`'s state
//! channel into a gpui entity that views observe.

use gpui_kit::{Context, Entity, Image, ImageFormat, Task};
use std::sync::Arc;
use wb_core::{ConnState, WaHandle};

pub struct Connection {
    wa: WaHandle,
    state: ConnState,
    qr: Option<(String, Arc<Image>)>,
    _pump: Task<()>,
}

impl Connection {
    pub fn new(wa: WaHandle, cx: &mut Context<Self>) -> Self {
        let mut rx = wa.state();
        let state = rx.borrow_and_update().clone();
        let pump = cx.spawn(async move |this, cx| {
            while rx.changed().await.is_ok() {
                let state = rx.borrow_and_update().clone();
                if this
                    .update(cx, |conn, cx| conn.set_state(state, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut conn = Connection {
            wa,
            state: ConnState::Starting,
            qr: None,
            _pump: pump,
        };
        conn.state = state;
        conn.refresh_qr();
        conn
    }

    pub fn wa(&self) -> &WaHandle {
        &self.wa
    }

    pub fn state(&self) -> &ConnState {
        &self.state
    }

    pub fn qr_image(&self) -> Option<Arc<Image>> {
        self.qr.as_ref().map(|(_, image)| image.clone())
    }

    fn set_state(&mut self, state: ConnState, cx: &mut Context<Self>) {
        self.state = state;
        self.refresh_qr();
        cx.notify();
    }

    /// Re-renders the QR PNG only when the payload changes (every ~20 s).
    fn refresh_qr(&mut self) {
        let ConnState::Pairing {
            qr: Some(payload), ..
        } = &self.state
        else {
            self.qr = None;
            return;
        };
        if self.qr.as_ref().is_some_and(|(p, _)| p == payload) {
            return;
        }
        self.qr = match wb_core::qr::png(payload, 8) {
            Ok(png) => Some((
                payload.clone(),
                Arc::new(Image::from_bytes(ImageFormat::Png, png)),
            )),
            Err(e) => {
                log::error!("QR render failed: {e}");
                None
            }
        };
    }
}

pub type ConnectionEntity = Entity<Connection>;
