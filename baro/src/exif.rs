/// JPEG APP1의 IFD1 썸네일만 꺼낸다. 원본 픽셀은 디코드하지 않는다.
pub fn extract_jpeg_thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut p = 2usize;
    while p + 4 < bytes.len() {
        if bytes[p] != 0xff {
            break;
        }
        let mut marker = bytes[p + 1];
        while marker == 0xff && p + 2 < bytes.len() {
            p += 1;
            marker = bytes[p + 1];
        }
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            p += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[p + 2], bytes[p + 3]]) as usize;
        if len < 2 || p + 2 + len > bytes.len() {
            break;
        }
        if marker == 0xe1 {
            if let Some(thumb) = thumb_from_exif(&bytes[p + 4..p + 2 + len]) {
                return Some(thumb);
            }
        }
        p += 2 + len;
    }
    None
}

fn thumb_from_exif(app1: &[u8]) -> Option<Vec<u8>> {
    if app1.len() < 16 || &app1[0..6] != b"Exif\0\0" {
        return None;
    }
    let tiff = 6usize;
    let le = app1[tiff] == b'I' && app1[tiff + 1] == b'I';
    let be = app1[tiff] == b'M' && app1[tiff + 1] == b'M';
    if !le && !be || u16_at(app1, tiff, 2, le)? != 42 {
        return None;
    }
    let mut ifd = u32_at(app1, tiff, 4, le)? as usize;
    for _ in 0..4 {
        if ifd == 0 {
            break;
        }
        if let Some(found) = read_ifd_thumb(app1, tiff, ifd, le) {
            return Some(found);
        }
        let count = u16_at(app1, tiff, ifd, le)? as usize;
        let next = u32_at(app1, tiff, ifd + 2 + count * 12, le)? as usize;
        if next == ifd {
            break;
        }
        ifd = next;
    }
    None
}

fn read_ifd_thumb(app1: &[u8], tiff: usize, ifd: usize, le: bool) -> Option<Vec<u8>> {
    let count = u16_at(app1, tiff, ifd, le)? as usize;
    let mut jpeg_off = None;
    let mut jpeg_len = None;
    for i in 0..count {
        let e = ifd + 2 + i * 12;
        let tag = u16_at(app1, tiff, e, le)?;
        let value = u32_at(app1, tiff, e + 8, le)?;
        if tag == 0x0201 {
            jpeg_off = Some(value as usize);
        }
        if tag == 0x0202 {
            jpeg_len = Some(value as usize);
        }
    }
    let start = tiff + jpeg_off?;
    let end = start + jpeg_len?;
    let slice = app1.get(start..end)?;
    if slice.len() < 4 || slice[0] != 0xff || slice[1] != 0xd8 {
        return None;
    }
    Some(slice.to_vec())
}

fn u16_at(buf: &[u8], base: usize, offset: usize, le: bool) -> Option<u16> {
    let i = base + offset;
    let pair = buf.get(i..i + 2)?;
    Some(if le {
        u16::from_le_bytes([pair[0], pair[1]])
    } else {
        u16::from_be_bytes([pair[0], pair[1]])
    })
}

fn u32_at(buf: &[u8], base: usize, offset: usize, le: bool) -> Option<u32> {
    let i = base + offset;
    let quad = buf.get(i..i + 4)?;
    Some(if le {
        u32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]])
    } else {
        u32::from_be_bytes([quad[0], quad[1], quad[2], quad[3]])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ifd1_thumbnail() {
        // TIFF: little-endian, IFD0 empty, next IFD has JPEG offset/length.
        let mut app1 = b"Exif\0\0II*\0".to_vec();
        app1.extend_from_slice(&8u32.to_le_bytes()); // first IFD at offset 8
        // IFD0 count 0, next IFD at 14
        app1.extend_from_slice(&0u16.to_le_bytes());
        app1.extend_from_slice(&14u32.to_le_bytes());
        // IFD1 count 2
        app1.extend_from_slice(&2u16.to_le_bytes());
        // tag 0x0201 long, count 1, value = offset of jpeg from TIFF start
        let jpeg_at = 14 + 2 + 24 + 4; // after IFD1 entries and next pointer
        let mut entry = [0u8; 12];
        entry[0..2].copy_from_slice(&0x0201u16.to_le_bytes());
        entry[2..4].copy_from_slice(&4u16.to_le_bytes());
        entry[4..8].copy_from_slice(&1u32.to_le_bytes());
        entry[8..12].copy_from_slice(&(jpeg_at as u32).to_le_bytes());
        app1.extend_from_slice(&entry);
        let mut entry2 = [0u8; 12];
        entry2[0..2].copy_from_slice(&0x0202u16.to_le_bytes());
        entry2[2..4].copy_from_slice(&4u16.to_le_bytes());
        entry2[4..8].copy_from_slice(&1u32.to_le_bytes());
        entry2[8..12].copy_from_slice(&4u32.to_le_bytes());
        app1.extend_from_slice(&entry2);
        app1.extend_from_slice(&0u32.to_le_bytes());
        app1.extend_from_slice(&[0xff, 0xd8, 0xff, 0xd9]);

        let mut jpeg = vec![0xff, 0xd8];
        jpeg.extend_from_slice(&[0xff, 0xe1]);
        let len = (app1.len() + 2) as u16;
        jpeg.extend_from_slice(&len.to_be_bytes());
        jpeg.extend_from_slice(&app1);
        jpeg.extend_from_slice(&[0xff, 0xd9]);

        let thumb = extract_jpeg_thumbnail(&jpeg).unwrap();
        assert_eq!(thumb, vec![0xff, 0xd8, 0xff, 0xd9]);
    }
}
