//! Phase 0 check without the UI: link by QR in the terminal, then send a text
//! (and optionally a file) to one number.
//!
//!   cargo run -p wb-core --example spike -- 628123456789 "Hello" [path/to/file.jpg]
//!
//! Uses its own data folder (`./spike-data`) so it never touches the app's session.

use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use std::path::PathBuf;
use wb_core::{ConnState, WaHandle};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut args = std::env::args().skip(1);
    let (Some(phone), Some(text)) = (args.next(), args.next()) else {
        eprintln!("usage: spike <phone> <message> [file]");
        std::process::exit(2);
    };
    let file = args.next().map(PathBuf::from);

    let data = PathBuf::from("spike-data");
    std::fs::create_dir_all(&data).unwrap();
    let wa = WaHandle::start(&tokio::runtime::Handle::current(), data);
    let mut state = wa.state();
    loop {
        let current = state.borrow_and_update().clone();
        match current {
            ConnState::Pairing { qr: Some(qr), .. } => {
                let code = QrCode::new(qr.as_bytes()).unwrap();
                println!(
                    "\nScan with WhatsApp > Linked devices:\n{}",
                    code.render::<Dense1x2>().build()
                );
            }
            ConnState::Ready(info) => {
                println!("Connected as {} (+{})", info.pushname, info.phone_number);
                break;
            }
            ConnState::Failed(msg) => {
                eprintln!("Failed: {msg}");
                std::process::exit(1);
            }
            other => println!("{other:?}"),
        }
        state.changed().await.unwrap();
    }

    match wa.send_text(&phone, &text).await {
        Ok(sent) => println!("Text sent: id={} to={}", sent.id, sent.to),
        Err(e) => eprintln!("Text failed: {e}"),
    }
    if let Some(file) = file {
        match wa
            .send_media(&phone, &file, Some("Sent by the spike"))
            .await
        {
            Ok(sent) => println!("File sent: id={} to={}", sent.id, sent.to),
            Err(e) => eprintln!("File failed: {e}"),
        }
    }
    wa.shutdown().await;
}
