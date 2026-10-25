use image::{ImageFormat, Luma};
use qrcode::QrCode;
use std::io::Cursor;

/// Renders a pairing QR payload as PNG bytes, `scale` pixels per module.
pub fn png(payload: &str, scale: u32) -> Result<Vec<u8>, String> {
    let code = QrCode::new(payload.as_bytes()).map_err(|e| e.to_string())?;
    let image = code
        .render::<Luma<u8>>()
        .module_dimensions(scale, scale)
        .quiet_zone(true)
        .build();
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    #[test]
    fn renders_png() {
        let bytes = super::png("2@abc,def,ghi", 4).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
