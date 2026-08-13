//! Screenshot capture using Windows GDI BitBlt.
//! Captures the entire virtual screen or a selected region as lossless PNG.
//!
//! Per AGENTS.md: All Windows-only code lives behind platform abstraction.
//! Per GOAL.md: Screenshot capture is a P0 feature.
//! Per PRD DAT-001: clipboard/screen bitmaps are saved as lossless PNG.

/// Encodes BGRA pixel data to lossless PNG (RGBA color type).
/// Public for reuse by clipboard bitmap capture.
pub fn encode_bgra_to_png(bgra: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let expected = (width * height * 4) as usize;
    if bgra.len() != expected {
        return Err(format!(
            "buffer size mismatch: expected {expected}, got {}",
            bgra.len()
        ));
    }

    // Convert BGRA → RGBA
    let mut rgba = bgra.to_vec();
    for chunk in rgba.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("png header: {e}"))?;
        writer
            .write_image_data(&rgba)
            .map_err(|e| format!("png data: {e}"))?;
    }
    Ok(out)
}

#[cfg(target_os = "windows")]
struct VirtualScreen {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[cfg(target_os = "windows")]
fn get_virtual_screen() -> Result<VirtualScreen, String> {
    unsafe extern "system" {
        fn GetSystemMetrics(index: i32) -> i32;
    }
    const SM_XVIRTUALSCREEN: i32 = 76;
    const SM_YVIRTUALSCREEN: i32 = 77;
    const SM_CXVIRTUALSCREEN: i32 = 78;
    const SM_CYVIRTUALSCREEN: i32 = 79;

    unsafe {
        let vs = VirtualScreen {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
        };
        if vs.width <= 0 || vs.height <= 0 {
            return Err("invalid screen dimensions".to_string());
        }
        Ok(vs)
    }
}

/// Captures a region of the virtual screen (all monitors) as PNG.
/// Coordinates are virtual-screen absolute (same space as GetSystemMetrics(76..79)).
#[cfg(target_os = "windows")]
#[allow(clippy::upper_case_acronyms)]
pub fn capture_region(rx: i32, ry: i32, rw: i32, rh: i32) -> Result<Vec<u8>, String> {
    use std::mem;

    type HDC = *mut std::ffi::c_void;
    type HBITMAP = *mut std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(clippy::upper_case_acronyms)]
    struct BitmapInfoHeader {
        bi_size: u32,
        bi_width: i32,
        bi_height: i32,
        bi_planes: u16,
        bi_bit_count: u16,
        bi_compression: u32,
        bi_size_image: u32,
        bi_x_pels_per_meter: i32,
        bi_y_pels_per_meter: i32,
        bi_clr_used: u32,
        bi_clr_important: u32,
    }

    #[repr(C)]
    #[allow(clippy::upper_case_acronyms)]
    struct BitmapInfo {
        bmi_header: BitmapInfoHeader,
        bmi_colors: [u32; 1],
    }

    unsafe extern "system" {
        fn GetDC(hwnd: *mut std::ffi::c_void) -> HDC;
        fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: HDC) -> i32;
        fn CreateCompatibleDC(hdc: HDC) -> HDC;
        fn CreateCompatibleBitmap(hdc: HDC, w: i32, h: i32) -> HBITMAP;
        fn SelectObject(hdc: HDC, obj: HBITMAP) -> HBITMAP;
        fn BitBlt(
            dst: HDC,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            src: HDC,
            sx: i32,
            sy: i32,
            rop: u32,
        ) -> i32;
        fn GetDIBits(
            hdc: HDC,
            hbm: HBITMAP,
            start: u32,
            lines: u32,
            bits: *mut u8,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
        fn DeleteObject(obj: HBITMAP) -> i32;
        fn DeleteDC(hdc: HDC) -> i32;
    }

    const SRCCOPY: u32 = 0x00CC0020;
    const DIB_RGB_COLORS: u32 = 0;
    const BI_RGB: u32 = 0;

    if rw <= 0 || rh <= 0 {
        return Err("invalid region dimensions".to_string());
    }

    // Validate region lies within the virtual screen
    let vs = get_virtual_screen()?;
    if rx < vs.x || ry < vs.y || rx + rw > vs.x + vs.width || ry + rh > vs.y + vs.height {
        return Err("region outside virtual screen bounds".to_string());
    }

    unsafe {
        let screen_dc = GetDC(std::ptr::null_mut());
        if screen_dc.is_null() {
            return Err("failed to get screen DC".to_string());
        }

        let mem_dc = CreateCompatibleDC(screen_dc);
        let bitmap = CreateCompatibleBitmap(screen_dc, rw, rh);
        let _old = SelectObject(mem_dc, bitmap);

        // BitBlt: copy the requested region from screen to memory DC
        BitBlt(mem_dc, 0, 0, rw, rh, screen_dc, rx, ry, SRCCOPY);

        let mut bmi = BitmapInfo {
            bmi_header: BitmapInfoHeader {
                bi_size: mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: rw,
                bi_height: -rh, // Top-down
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: BI_RGB,
                bi_size_image: 0,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            bmi_colors: [0; 1],
        };

        let buf_size = (rw * rh * 4) as usize;
        let mut buffer = vec![0u8; buf_size];

        let lines = GetDIBits(
            mem_dc,
            bitmap,
            0,
            rh as u32,
            buffer.as_mut_ptr(),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(mem_dc, _old);
        DeleteObject(bitmap);
        DeleteDC(mem_dc);
        ReleaseDC(std::ptr::null_mut(), screen_dc);

        if lines == 0 {
            return Err("GetDIBits failed".to_string());
        }

        encode_bgra_to_png(&buffer, rw as u32, rh as u32)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn capture_region(_rx: i32, _ry: i32, _rw: i32, _rh: i32) -> Result<Vec<u8>, String> {
    Err("screenshot capture only supported on Windows".to_string())
}

/// Captures the entire virtual screen (all monitors) as PNG.
#[cfg(target_os = "windows")]
pub fn capture_screen() -> Result<Vec<u8>, String> {
    let vs = get_virtual_screen()?;
    capture_region(vs.x, vs.y, vs.width, vs.height)
}

#[cfg(not(target_os = "windows"))]
pub fn capture_screen() -> Result<Vec<u8>, String> {
    Err("screenshot capture only supported on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_bgra_to_png_produces_valid_png() {
        // 2x2 red image in BGRA (B=0, G=0, R=255, A=255)
        let bgra: Vec<u8> = vec![
            0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
        ];
        let png_data = encode_bgra_to_png(&bgra, 2, 2).expect("encode");

        // PNG magic bytes
        assert_eq!(
            &png_data[0..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );

        // Decode and verify pixels are RGBA red (R=255, G=0, B=0)
        let decoder = png::Decoder::new(std::io::Cursor::new(&png_data));
        let mut reader = decoder.read_info().expect("decode info");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("decode frame");
        assert_eq!(info.width, 2);
        assert_eq!(info.height, 2);
        assert_eq!(info.color_type, png::ColorType::Rgba);
        let pixels = &buf[..info.buffer_size()];
        assert_eq!(&pixels[0..4], &[255, 0, 0, 255]); // First pixel = red RGBA
    }

    #[test]
    fn encode_rejects_size_mismatch() {
        let bgra = vec![0u8; 10]; // Not a multiple of 4 for any dims
        assert!(encode_bgra_to_png(&bgra, 2, 2).is_err());
    }

    #[test]
    fn encode_empty_region() {
        let result = encode_bgra_to_png(&[], 0, 0);
        // 0x0 PNG is technically valid but we reject zero-dim upstream; encoder handles it
        assert!(result.is_ok() || result.is_err()); // no panic
    }
}
