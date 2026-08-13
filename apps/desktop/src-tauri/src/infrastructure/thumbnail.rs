//! Bounded image thumbnails (P1-CAP-007 / Workstream G "thumbnail variants").
//!
//! Pure PNG pipeline using the `png` crate (no new dependencies):
//! decode (any color type → RGBA8) → box-filter downscale → encode.
//! Thumbnails are bounded by construction: output never exceeds max_dim on
//! either axis, and oversized inputs are refused by the caller.

/// Computes the downscale target size. Returns None when the image already
/// fits within `max_dim` on both axes (caller should use the original).
pub fn compute_target_size(width: u32, height: u32, max_dim: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 || max_dim == 0 {
        return None;
    }
    if width <= max_dim && height <= max_dim {
        return None;
    }
    let scale = (max_dim as f64) / (width.max(height) as f64);
    let w = ((width as f64) * scale).round() as u32;
    let h = ((height as f64) * scale).round() as u32;
    Some((w.max(1), h.max(1)))
}

/// Decodes a PNG of any color type into an RGBA8 buffer.
fn decode_png_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = png::Decoder::new(bytes);
    // Normalize palette/grayscale/bit depths to 8-bit, and always produce an
    // alpha channel so the output is uniformly RGBA8. (png 0.17: set on the
    // Decoder before read_info.)
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("png decode: {e}"))?;
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("png frame: {e}"))?;
    buf.truncate(frame.buffer_size());
    if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
        return Err("unexpected decoded format".to_string());
    }
    Ok((buf, width, height))
}

/// Box-filter downscale of an RGBA8 buffer. Integer arithmetic only; each
/// output pixel is the average of its source block.
fn box_downscale(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dst_w * dst_h * 4) as usize];
    for oy in 0..dst_h {
        // Source rows covered by this output row.
        let y0 = (oy as u64 * src_h as u64 / dst_h as u64) as u32;
        let y1 = (((oy + 1) as u64 * src_h as u64 / dst_h as u64) as u32).max(y0 + 1);
        for ox in 0..dst_w {
            let x0 = (ox as u64 * src_w as u64 / dst_w as u64) as u32;
            let x1 = (((ox + 1) as u64 * src_w as u64 / dst_w as u64) as u32).max(x0 + 1);

            let mut r = 0u64;
            let mut g = 0u64;
            let mut b = 0u64;
            let mut a = 0u64;
            let mut n = 0u64;
            for sy in y0..y1 {
                let row = (sy as usize) * (src_w as usize) * 4;
                for sx in x0..x1 {
                    let i = row + (sx as usize) * 4;
                    r += src[i] as u64;
                    g += src[i + 1] as u64;
                    b += src[i + 2] as u64;
                    a += src[i + 3] as u64;
                    n += 1;
                }
            }
            let o = ((oy * dst_w + ox) * 4) as usize;
            out[o] = (r / n) as u8;
            out[o + 1] = (g / n) as u8;
            out[o + 2] = (b / n) as u8;
            out[o + 3] = (a / n) as u8;
        }
    }
    out
}

/// Encodes an RGBA8 buffer as PNG. Shared by the thumbnail pipeline and the
/// clipboard-image capture path (CAP-001).
pub fn encode_png_rgba(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("png encode: {e}"))?;
        writer
            .write_image_data(pixels)
            .map_err(|e| format!("png write: {e}"))?;
    }
    Ok(out)
}

/// Downscale a PNG so neither axis exceeds `max_dim`.
/// Returns Err when the image already fits (caller should use the original)
/// or when decoding fails.
pub fn downscale_png(bytes: &[u8], max_dim: u32) -> Result<Vec<u8>, String> {
    let (rgba, w, h) = decode_png_rgba(bytes)?;
    let (tw, th) = compute_target_size(w, h, max_dim)
        .ok_or_else(|| "image already within bounds".to_string())?;
    let scaled = box_downscale(&rgba, w, h, tw, th);
    encode_png_rgba(&scaled, tw, th)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_png(width: u32, height: u32) -> Vec<u8> {
        let pixels = vec![200u8; (width * height * 4) as usize];
        encode_png_rgba(&pixels, width, height).expect("encode")
    }

    #[test]
    fn target_size_scales_longest_axis_to_max() {
        assert_eq!(compute_target_size(2000, 1000, 400), Some((400, 200)));
        assert_eq!(compute_target_size(1000, 2000, 400), Some((200, 400)));
        assert_eq!(compute_target_size(800, 800, 400), Some((400, 400)));
    }

    #[test]
    fn target_size_none_when_already_fits() {
        assert_eq!(compute_target_size(400, 300, 400), None);
        assert_eq!(compute_target_size(100, 100, 400), None);
    }

    #[test]
    fn target_size_rejects_degenerate_input() {
        assert_eq!(compute_target_size(0, 100, 400), None);
        assert_eq!(compute_target_size(100, 0, 400), None);
        assert_eq!(compute_target_size(100, 100, 0), None);
    }

    #[test]
    fn downscale_produces_bounded_valid_png() {
        let src = make_png(64, 32);
        let out = downscale_png(&src, 16).expect("downscale");

        // Decode the result and verify dimensions are bounded.
        let (rgba, w, h) = decode_png_rgba(&out).expect("re-decode");
        assert_eq!((w, h), (16, 8));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        // Uniform source → average stays ~200 on every channel.
        assert!(rgba.iter().all(|&v| (199..=201).contains(&v)));
    }

    #[test]
    fn downscale_refuses_already_small_images() {
        let src = make_png(8, 8);
        assert!(downscale_png(&src, 16).is_err());
    }

    #[test]
    fn downscale_refuses_non_png() {
        assert!(downscale_png(b"not a png at all", 16).is_err());
    }
}
