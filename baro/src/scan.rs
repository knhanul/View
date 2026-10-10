use std::path::{Path, PathBuf};

use crate::kind::Kind;
use crate::{kind_from_name, Result, Still};

pub fn list_images(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_file() && is_image(&path) {
            files.push(path);
        }
    }
    files.sort();
    files
}

pub fn is_image(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
        let kind = kind_from_name(name);
        kind != Kind::Other && kind != Kind::Webp
    })
}

pub fn load_thumb(path: &Path) -> Result<Still> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let head = &bytes[..bytes.len().min(256)];
    if crate::sniff(name, head) == Kind::Jpeg {
        let prefix = &bytes[..bytes.len().min(131_072)];
        if let Some(thumb) = crate::extract_jpeg_thumbnail(prefix) {
            if let Ok(still) = crate::decode_jpeg(&thumb) {
                return Ok(thumb_edge(&still, 96));
            }
        }
    }
    if crate::sniff(name, head) == Kind::Gif {
        let frames = crate::decode_gif(&bytes)?;
        return frames
            .into_iter()
            .next()
            .map(|frame| thumb_edge(&frame.image, 96))
            .ok_or_else(|| "GIF 프레임이 없습니다".to_string());
    }
    Ok(thumb_edge(&crate::decode_still(name, &bytes)?, 96))
}

fn thumb_edge(still: &Still, edge: u32) -> Still {
    if still.width.max(still.height) <= edge {
        return still.clone();
    }
    let scale = edge as f32 / still.width.max(still.height) as f32;
    let width = ((still.width as f32) * scale).round().max(1.0) as u32;
    let height = ((still.height as f32) * scale).round().max(1.0) as u32;
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height {
        let sy = (y as u64 * still.height as u64 / height as u64) as u32;
        for x in 0..width {
            let sx = (x as u64 * still.width as u64 / width as u64) as u32;
            let si = (sy as usize * still.width as usize + sx as usize) * 4;
            let di = (y as usize * width as usize + x as usize) * 4;
            rgba[di..di + 4].copy_from_slice(&still.rgba[si..si + 4]);
        }
    }
    Still { width, height, rgba }
}
