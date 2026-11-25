//! PNG artwork for native instrument panels.
use std::io::BufRead;

/// An instrument image with top-down, unpremultiplied BGRA8 pixels.
pub struct Image {
    pub width: i32,
    pub height: i32,
    /// Four bytes per pixel in blue, green, red, alpha order.
    pub pixels: Vec<u8>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    Unreadable,
    Dimensions,
}

/// Decode the first PNG frame and optionally require an exact panel size.
/// Images larger than 4096 pixels in either dimension are rejected before decoding.
pub fn decode(input: impl BufRead, expected: Option<(i32, i32)>) -> Result<Image, LoadError> {
    let mut decoder = png::Decoder::new(input);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|_| LoadError::Unreadable)?;
    let (w, h) = reader.info().size();
    if w > 4096 || h > 4096 || expected.is_some_and(|size| size != (w as i32, h as i32)) {
        return Err(LoadError::Dimensions);
    }
    let mut raw = vec![0; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut raw)
        .map_err(|_| LoadError::Unreadable)?;
    if (frame.width, frame.height) != (w, h) {
        return Err(LoadError::Dimensions);
    }
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for p in raw[..frame.buffer_size()].chunks_exact(frame.color_type.samples()) {
        let bgra = match p {
            [r, g, b, a] => [*b, *g, *r, *a],
            [r, g, b] => [*b, *g, *r, 255],
            [v, a] => [*v, *v, *v, *a],
            [v] => [*v, *v, *v, 255],
            _ => return Err(LoadError::Unreadable),
        };
        pixels.extend_from_slice(&bgra);
    }
    Ok(Image {
        width: w as i32,
        height: h as i32,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_transparency_is_expanded_to_top_down_bgra() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 2);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(&[10, 20, 30, 40, 50, 60]);
            encoder.set_trns(&[130, 0]);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0, 1]).unwrap();
        }
        let image = decode(bytes.as_slice(), Some((1, 2))).unwrap();
        assert_eq!((image.width, image.height), (1, 2));
        assert_eq!(image.pixels, [30, 20, 10, 130, 60, 50, 40, 0]);
        assert!(matches!(
            decode(bytes.as_slice(), Some((2, 1))),
            Err(LoadError::Dimensions)
        ));
    }
}
