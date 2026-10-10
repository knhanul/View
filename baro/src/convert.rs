use image::ImageEncoder;

use crate::{Result, Still};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Jpeg,
    Webp,
}

impl OutFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }
}

pub fn convert(src: &Still, format: OutFormat, quality: u8, max_long: Option<u32>) -> Result<Vec<u8>> {
    let fitted = match max_long {
        Some(edge) if edge > 0 && src.width.max(src.height) > edge => resize_nearest(src, edge),
        _ => src.clone(),
    };
    let mut out = Vec::new();
    match format {
        OutFormat::Png => image::codecs::png::PngEncoder::new(&mut out)
            .write_image(&fitted.rgba, fitted.width, fitted.height, image::ExtendedColorType::Rgba8)
            .map_err(|e| e.to_string())?,
        OutFormat::Jpeg => {
            let rgb = flatten_white(&fitted.rgba);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
                .write_image(&rgb, fitted.width, fitted.height, image::ExtendedColorType::Rgb8)
                .map_err(|e| e.to_string())?;
        }
        OutFormat::Webp => image::codecs::webp::WebPEncoder::new_lossless(&mut out)
            .write_image(&fitted.rgba, fitted.width, fitted.height, image::ExtendedColorType::Rgba8)
            .map_err(|e| e.to_string())?,
    }
    Ok(out)
}

fn flatten_white(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.chunks_exact(4) {
        let a = u32::from(px[3]);
        let blend = |c: u8| -> u8 { ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8 };
        rgb.extend_from_slice(&[blend(px[0]), blend(px[1]), blend(px[2])]);
    }
    rgb
}

fn resize_nearest(src: &Still, max_long: u32) -> Still {
    let scale = max_long as f32 / src.width.max(src.height) as f32;
    let width = ((src.width as f32) * scale).round().max(1.0) as u32;
    let height = ((src.height as f32) * scale).round().max(1.0) as u32;
    let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
    for y in 0..height {
        let sy = ((y as u64 * src.height as u64) / height as u64) as u32;
        for x in 0..width {
            let sx = ((x as u64 * src.width as u64) / width as u64) as u32;
            let si = ((sy as usize) * (src.width as usize) + sx as usize) * 4;
            let di = ((y as usize) * (width as usize) + x as usize) * 4;
            rgba[di..di + 4].copy_from_slice(&src.rgba[si..si + 4]);
        }
    }
    Still { width, height, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_convert_starts_with_signature() {
        let src = Still {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 255],
        };
        let bytes = convert(&src, OutFormat::Png, 90, None).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
    }
}
