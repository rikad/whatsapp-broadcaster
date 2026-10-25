#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod connection;
mod theme;
mod views;

use gpui_kit::{AppContext as _, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use std::sync::{Arc, OnceLock};
use wb_core::WaHandle;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn init_logging() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    // Release builds on Windows have no console; write to a file instead.
    if !cfg!(debug_assertions)
        && let Ok(file) = wb_core::paths::logs_dir()
            .and_then(|dir| std::fs::File::create(dir.join("whatsapp-broadcaster.log")))
    {
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }
    builder.init();
}

fn window_options(cx: &gpui_kit::App, width: f32, height: f32) -> WindowOptions {
    let icon = image::load_from_memory(include_bytes!("../../../assets/icon-512.png"))
        .ok()
        .map(|i| Arc::new(i.to_rgba8()));
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("WhatsApp Broadcaster".into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::centered(size(px(width), px(height)), cx)),
        window_min_size: Some(size(px(860.), px(600.))),
        app_id: Some("whatsapp-broadcaster".into()),
        icon,
        ..Default::default()
    }
}

fn main() {
    init_logging();
    let data_dir = wb_core::paths::data_dir().expect("cannot create the app data folder");
    let lock = wb_core::paths::lock_instance(&data_dir).unwrap_or_else(|e| {
        log::warn!("Instance lock unavailable: {e}");
        None
    });

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::apply(cx);

            let Some(lock) = lock else {
                gpui_kit::open_window(window_options(cx, 480., 260.), cx, |_, cx| {
                    cx.new(|_| app::AlreadyRunning)
                })
                .expect("failed to open window");
                return;
            };

            let rt = RUNTIME.get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("wb-tokio")
                    .build()
                    .expect("failed to start tokio")
            });
            let wa = WaHandle::start(rt.handle(), data_dir.clone());

            let port = wb_api::port_from_env();
            let notice = match rt.block_on(wb_api::bind(port)) {
                Ok(listener) => {
                    rt.spawn(wb_api::serve(listener, wa.clone()));
                    None
                }
                Err(e) => {
                    log::warn!("HTTP API disabled, port {port}: {e}");
                    Some(format!(
                        "HTTP API is off: port {port} is in use. Set PORT to use another port."
                    ))
                }
            };

            let quit_wa = wa.clone();
            cx.on_app_quit(move |_| {
                let wa = quit_wa.clone();
                async move { wa.shutdown().await }
            })
            .detach();

            // Keep the lock file open until the process exits.
            std::mem::forget(lock);
            gpui_kit::open_window(window_options(cx, 1120., 760.), cx, move |window, cx| {
                cx.new(|cx| app::AppView::new(wa, notice, window, cx))
            })
            .expect("failed to open window");
        });
}
