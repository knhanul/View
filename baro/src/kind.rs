use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jpeg,
    Png,
    Gif,
    Svg,
    Webp,
    Other,
}

pub fn kind_from_name(name: &str) -> Kind {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Kind::Jpeg,
        "png" => Kind::Png,
        "gif" => Kind::Gif,
        "svg" => Kind::Svg,
        "webp" => Kind::Webp,
        _ => Kind::Other,
    }
}

pub fn sniff(name: &str, head: &[u8]) -> Kind {
    if head.len() >= 3 && head[0] == 0xff && head[1] == 0xd8 {
        return Kind::Jpeg;
    }
    if head.len() >= 8 && head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Kind::Png;
    }
    if head.len() >= 6 && head.starts_with(b"GIF8") {
        return Kind::Gif;
    }
    if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WEBP" {
        return Kind::Webp;
    }
    let text = String::from_utf8_lossy(&head[..head.len().min(256)]);
    let trimmed = text.trim_start().to_ascii_lowercase();
    if trimmed.starts_with("<svg") || trimmed.contains("<svg") || kind_from_name(name) == Kind::Svg {
        return Kind::Svg;
    }
    kind_from_name(name)
}
