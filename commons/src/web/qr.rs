//! QR codes for the room page (decision record 0027): the address people in
//! the room open, drawn as SVG so the page needs no image file and no
//! script. Uses the `qrcode` crate with only its SVG renderer (decision D6).

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// The address a QR code holds: `base` (the server's room address, such as
/// `http://192.168.1.23:8080`) followed by `path` (`/@ana`), with exactly
/// one `/` between them. Pure.
pub fn address(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}

/// The QR code for `text` as an SVG drawing, dark modules in ink on white,
/// at least 280 pixels square with the white margin scanners need. Medium
/// error correction (about 15% of the code can be damaged or glared on a
/// projector and still read). `None` if `text` is too long for a QR code,
/// which an address never is. Pure.
pub fn svg(text: &str) -> Option<String> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M).ok()?;
    Some(inline(&draw(&code)))
}

/// Draw `code` as a standalone SVG file (with its `<?xml …?>` line).
fn draw(code: &QrCode) -> String {
    code.render::<svg::Color<'_>>()
        .min_dimensions(280, 280)
        .dark_color(svg::Color("#111111"))
        .light_color(svg::Color("#ffffff"))
        .quiet_zone(true)
        .build()
}

/// The drawing from its `<svg` tag on, so it can sit inside an HTML page: a
/// standalone file's `<?xml …?>` line has no place there. Pure.
fn inline(file: &str) -> String {
    file.find("<svg").map_or(file, |at| &file[at..]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_joins_base_and_path_with_one_slash() {
        assert_eq!(address("http://192.168.1.23:8080", "/@ana"), "http://192.168.1.23:8080/@ana");
        assert_eq!(address("http://192.168.1.23:8080/", "/"), "http://192.168.1.23:8080/");
        assert_eq!(address("http://10.0.0.5:8082/", "@ana"), "http://10.0.0.5:8082/@ana");
    }

    #[test]
    fn the_qr_code_holds_exactly_the_address() {
        // There's no decoder to hand, so compare module by module with a
        // code made directly from the address: the drawing encodes the same
        // bytes, nothing added or changed.
        let text = "http://192.168.1.23:8080/@ana";
        let expected = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M).unwrap();
        let other = QrCode::with_error_correction_level(b"http://192.168.1.23:8080/@ann", EcLevel::M).unwrap();
        assert_ne!(expected.to_colors(), other.to_colors(), "a different address draws a different code");
        let drawing = svg(text).unwrap();
        assert_eq!(drawing, inline(&draw(&expected)));
        assert!(drawing.starts_with("<svg"), "ready to sit inside a page");
    }
}
