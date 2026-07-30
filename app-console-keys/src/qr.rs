use image::{DynamicImage, GrayImage};
use qrcode::{render::unicode, QrCode};

/// Render `content` as a Unicode half-block QR code string.
/// Each line uses Dense1x2 (▀ ▄ █ space) — works on any terminal without image protocol.
/// Returns an empty string if `content` cannot be encoded (should not happen for short ASCII).
pub fn qr_unicode(content: &str) -> String {
    match QrCode::new(content.as_bytes()) {
        Ok(code) => code.render::<unicode::Dense1x2>().quiet_zone(true).build(),
        Err(_) => String::new(),
    }
}

/// Render `content` as a `DynamicImage` suitable for Kitty/Sixel rendering via ratatui-image.
/// Returns `None` if encoding fails (should not happen for short ASCII codes).
pub fn qr_image(content: &str) -> Option<DynamicImage> {
    let code = QrCode::new(content.as_bytes()).ok()?;
    let img: GrayImage = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .module_dimensions(4, 4)
        .build();
    Some(DynamicImage::ImageLuma8(img))
}
