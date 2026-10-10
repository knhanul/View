use std::io::Cursor;

use crate::{Kind, Result};
use gif::ColorOutput;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

#[derive(Clone, Debug)]
pub struct Still {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct GifFrame {
    pub image: Still,
    pub delay_ms: u32,
}

pub fn decode_jpeg(bytes: &[u8]) -> Result<Still> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(bytes, options);
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or_else(|| "JPEG 헤더가 없습니다".to_string())?;
    let width = info.width as u32;
    let height = info.height as u32;
    let rgba = to_rgba(&pixels, width, height)?;
    Ok(Still { width, height, rgba })
}

pub fn decode_png(bytes: &[u8]) -> Result<Still> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 1024 * 1024 * 1024 });
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let n = output.buffer_size();
    let rgba = to_rgba(&buf[..n], output.width, output.height)?;
    Ok(Still {
        width: output.width,
        height: output.height,
        rgba,
    })
}

pub fn decode_gif(bytes: &[u8]) -> Result<Vec<GifFrame>> {
    let mut frames = Vec::new();
    visit_gif(bytes, |frame| frames.push(frame))?;
    if frames.is_empty() {
        return Err("GIF 프레임이 없습니다".into());
    }
    Ok(frames)
}

pub fn visit_gif(bytes: &[u8], mut visit: impl FnMut(GifFrame)) -> Result<()> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let mut decoder = options.read_info(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let width = decoder.width() as usize;
    let height = decoder.height() as usize;
    if width == 0 || height == 0 {
        return Err("GIF 크기가 없습니다".into());
    }
    let mut canvas = vec![0u8; width * height * 4];
    let mut previous = canvas.clone();
    while let Some(frame) = decoder.read_next_frame().map_err(|e| e.to_string())? {
        let dispose = frame.dispose;
        previous.clone_from(&canvas);
        blit(
            &mut canvas,
            width,
            &frame.buffer,
            frame.width as usize,
            frame.height as usize,
            frame.left as usize,
            frame.top as usize,
        );
        let delay_ms = u32::from(frame.delay).saturating_mul(10).max(20);
        visit(GifFrame {
            image: Still {
                width: width as u32,
                height: height as u32,
                rgba: canvas.clone(),
            },
            delay_ms,
        });
        match dispose {
            gif::DisposalMethod::Background => clear_rect(
                &mut canvas,
                width,
                frame.left as usize,
                frame.top as usize,
                frame.width as usize,
                frame.height as usize,
            ),
            gif::DisposalMethod::Previous => canvas.clone_from(&previous),
            _ => {}
        }
    }
    Ok(())
}

pub fn raster_svg(bytes: &[u8], max_edge: u32) -> Result<Still> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut width = size.width();
    let mut height = size.height();
    let mut transform = tiny_skia::Transform::identity();
    let long = width.max(height);
    if max_edge > 0 && long > max_edge {
        let scale = max_edge as f32 / long as f32;
        width = ((width as f32) * scale).round().max(1.0) as u32;
        height = ((height as f32) * scale).round().max(1.0) as u32;
        transform = tiny_skia::Transform::from_scale(scale, scale);
    }
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| "SVG 버퍼를 만들 수 없습니다".to_string())?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(Still {
        width,
        height,
        rgba: pixmap.take(),
    })
}

pub fn decode_still(name: &str, bytes: &[u8]) -> Result<Still> {
    match crate::sniff(name, bytes) {
        Kind::Jpeg => decode_jpeg(bytes),
        Kind::Png => decode_png(bytes),
        Kind::Svg => raster_svg(bytes, 0),
        Kind::Gif => decode_gif(bytes)?
            .into_iter()
            .next()
            .map(|frame| frame.image)
            .ok_or_else(|| "GIF 프레임이 없습니다".to_string()),
        Kind::Webp => Err("WebP는 변환 결과로만 씁니다".into()),
        Kind::Other => Err("이 형식은 열 수 없습니다".into()),
    }
}

fn to_rgba(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let n = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| "이미지가 너무 큽니다".to_string())?;
    if pixels.len() == n * 4 {
        return Ok(pixels.to_vec());
    }
    if pixels.len() == n * 3 {
        let mut rgba = Vec::with_capacity(n * 4);
        for px in pixels.chunks_exact(3) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
        return Ok(rgba);
    }
    if pixels.len() == n {
        let mut rgba = Vec::with_capacity(n * 4);
        for &g in pixels {
            rgba.extend_from_slice(&[g, g, g, 255]);
        }
        return Ok(rgba);
    }
    if pixels.len() == n * 2 {
        let mut rgba = Vec::with_capacity(n * 4);
        for px in pixels.chunks_exact(2) {
            rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
        }
        return Ok(rgba);
    }
    Err(format!("픽셀 수가 맞지 않습니다 ({})", pixels.len()))
}

fn blit(canvas: &mut [u8], cw: usize, frame: &[u8], fw: usize, fh: usize, left: usize, top: usize) {
    let ch = canvas.len() / (cw.saturating_mul(4).max(1));
    for y in 0..fh {
        let dy = top + y;
        if dy >= ch {
            break;
        }
        for x in 0..fw {
            let dx = left + x;
            if dx >= cw {
                continue;
            }
            let si = (y * fw + x) * 4;
            if si + 3 >= frame.len() || frame[si + 3] == 0 {
                continue;
            }
            let di = (dy * cw + dx) * 4;
            canvas[di..di + 4].copy_from_slice(&frame[si..si + 4]);
        }
    }
}

fn clear_rect(canvas: &mut [u8], cw: usize, left: usize, top: usize, fw: usize, fh: usize) {
    let ch = canvas.len() / (cw.saturating_mul(4).max(1));
    for y in 0..fh {
        let dy = top + y;
        if dy >= ch {
            break;
        }
        for x in 0..fw {
            let dx = left + x;
            if dx >= cw {
                continue;
            }
            let di = (dy * cw + dx) * 4;
            canvas[di..di + 4].fill(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    #[test]
    fn jpeg_roundtrip_size() {
        let rgb = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90)
            .write_image(&rgb, 2, 2, image::ExtendedColorType::Rgb8)
            .unwrap();
        let still = decode_jpeg(&bytes).unwrap();
        assert_eq!((still.width, still.height), (2, 2));
        assert_eq!(still.rgba.len(), 16);
    }

    #[test]
    fn png_roundtrip_pixels() {
        let rgba = [10, 20, 30, 255, 1, 2, 3, 128, 9, 9, 9, 0, 4, 5, 6, 255];
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&rgba, 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        let still = decode_png(&bytes).unwrap();
        assert_eq!(still.rgba, rgba);
    }

    #[test]
    fn svg_raster_is_red() {
        let svg = br#"<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><rect width='8' height='8' fill='red'/></svg>"#;
        let still = raster_svg(svg, 0).unwrap();
        assert_eq!((still.width, still.height), (8, 8));
        assert!(still.rgba[0] > 200 && still.rgba[1] < 40);
    }

    #[test]
    fn gif_first_frame() {
        let palette = [255, 0, 0, 0, 255, 0];
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 2, 2, &palette).unwrap();
            let mut frame = gif::Frame::from_indexed_pixels(2, 2, vec![0, 0, 1, 1], None);
            frame.delay = 10;
            encoder.write_frame(&frame).unwrap();
        }
        let frames = decode_gif(&bytes).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].image.width, 2);
        assert!(frames[0].image.rgba[0] > 200);
    }
}

