//! Bounded local bitmap subset of T3 v0.1.11 ProjectFaviconResolver.ts.
//! Never searches source/markup, resolves URLs, or loads SVG and ICNS.

use incular::image::ImageHandle;
use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Component,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use rustix::fs::{self, AtFlags, CWD, FileType, Mode, OFlags};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const CANDIDATES: &[&str] = &[
    "favicon.ico",
    "favicon.png",
    "public/favicon.ico",
    "public/favicon.png",
    "app/favicon.ico",
    "app/favicon.png",
    "app/icon.png",
    "app/icon.ico",
    "src/favicon.ico",
    "src/app/favicon.ico",
    "src/app/icon.png",
    "assets/icon.png",
    "assets/logo.png",
];

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_BYTES: usize = 512 * 1024;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_SIDE: u32 = 256;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Reads and decodes the first supported favicon among T3's conventional
/// project PNG/ICO paths, preserving source order. Accepts an absolute, already
/// trusted local workspace root. Every path component must be nonsymlink on
/// Linux/macOS; other platforms fail closed. Call from a worker, not a widget
/// build. The result owns decoded straight-alpha RGBA8, not a path or URL.
pub fn bitmap(root: &Path) -> Option<ImageHandle> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let root = open_root(root)?;
        CANDIDATES.iter().find_map(|candidate| {
            let bytes = read_candidate(&root, candidate)?;
            let bitmap = if candidate.ends_with(".png") {
                decode_png(&bytes, None)?
            } else {
                decode_ico(&bytes)?
            };
            ImageHandle::from_rgba8(bitmap.width, bitmap.height, bitmap.pixels).ok()
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = root;
        None
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn dir_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_root(path: &Path) -> Option<File> {
    let mut parts = path.components();
    if parts.next()? != Component::RootDir {
        return None;
    }
    let mut root = File::from(fs::openat(CWD, "/", dir_flags(), Mode::empty()).ok()?);
    for part in parts {
        let Component::Normal(name) = part else {
            return None;
        };
        root = File::from(fs::openat(&root, name, dir_flags(), Mode::empty()).ok()?);
    }
    Some(root)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_candidate(root: &File, candidate: &str) -> Option<Vec<u8>> {
    let mut parts = Path::new(candidate).components().peekable();
    let mut directory = root.try_clone().ok()?;
    loop {
        let Component::Normal(name) = parts.next()? else {
            return None;
        };
        if parts.peek().is_some() {
            directory = File::from(fs::openat(&directory, name, dir_flags(), Mode::empty()).ok()?);
            continue;
        }
        // Check the directory entry before opening (skip FIFOs, devices, links,
        // etc.). NONBLOCK + NOFOLLOW still apply to the atomically opened fd.
        let before = fs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
        if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
            || u64::try_from(before.st_size).ok()? > MAX_BYTES as u64
        {
            return None;
        }
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let fd = fs::openat(&directory, name, flags, Mode::empty()).ok()?;
        let opened = fs::fstat(&fd).ok()?;
        if FileType::from_raw_mode(opened.st_mode) != FileType::RegularFile
            || (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            || u64::try_from(opened.st_size).ok()? > MAX_BYTES as u64
        {
            return None;
        }
        let mut bytes = Vec::new();
        File::from(fd)
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        return (bytes.len() <= MAX_BYTES).then_some(bytes);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug)]
struct Bitmap {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn decode_png(bytes: &[u8], expected: Option<(u32, u32)>) -> Option<Bitmap> {
    if !bytes.starts_with(PNG_MAGIC) || bytes.len() > MAX_BYTES {
        return None;
    }
    let mut decoder =
        png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: 1024 * 1024 });
    // Untrusted repository comments and ICC payloads need not be inflated.
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let header = decoder.read_header_info().ok()?;
    let (width, height) = (header.width, header.height);
    if width == 0
        || height == 0
        || width > MAX_SIDE
        || height > MAX_SIDE
        || expected.is_some_and(|size| size != (width, height))
    {
        return None;
    }
    let mut reader = decoder.read_info().ok()?;
    if reader.info().animation_control.is_some() {
        return None;
    }

    let texels = (width * height) as usize;
    let max_rgba = texels * 4;
    let len = reader.output_buffer_size()?;
    if len > max_rgba {
        return None;
    }
    let mut buffer = vec![0u8; len];
    let output = reader.next_frame(&mut buffer).ok()?;
    reader.finish().ok()?;
    let channels = match output.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return None,
    };
    if (output.width, output.height) != (width, height)
        || output.bit_depth != png::BitDepth::Eight
        || output.buffer_size() != texels * channels
    {
        return None;
    }
    buffer.truncate(output.buffer_size());
    let pixels = if channels == 4 {
        buffer
    } else {
        let mut rgba = Vec::with_capacity(max_rgba);
        for pixel in buffer.chunks_exact(channels) {
            let encoded = match pixel {
                [y] => [*y, *y, *y, 255],
                [y, a] => [*y, *y, *y, *a],
                [r, g, b] => [*r, *g, *b, 255],
                _ => return None,
            };
            rgba.extend_from_slice(&encoded);
        }
        rgba
    };
    Some(Bitmap {
        width,
        height,
        pixels,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn le16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn directory_side(value: u8) -> u32 {
    if value == 0 { 256 } else { u32::from(value) }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn decode_ico(bytes: &[u8]) -> Option<Bitmap> {
    if bytes.len() > MAX_BYTES || bytes.get(..4)? != [0, 0, 1, 0] {
        return None; // ICO only, not CUR.
    }
    let count = usize::from(le16(bytes, 4)?);
    if !(1..=16).contains(&count) {
        return None;
    }
    let table_end = 6 + count * 16;
    let mut entries: Vec<_> = bytes
        .get(6..table_end)?
        .chunks_exact(16)
        .filter_map(|entry| {
            let width = directory_side(entry[0]);
            let height = directory_side(entry[1]);
            // Valid ICO directory metadata is small; PNG often omits depth.
            let bits = le16(entry, 6)?;
            if entry[3] != 0 || le16(entry, 4)? > 1 || bits > 32 {
                return None;
            }
            let length = usize::try_from(le32(entry, 8)?).ok()?;
            let start = usize::try_from(le32(entry, 12)?).ok()?;
            if start < table_end || length < 8 {
                return None;
            }
            let data = bytes.get(start..start.checked_add(length)?)?;
            Some((width, height, bits, data))
        })
        .collect();
    // Display-sized use is 14–16px; retain the best available source image for
    // later scaling rather than exposing arbitrary dimensions or all frames.
    entries.sort_by_key(|&(width, height, bits, _)| std::cmp::Reverse((bits, width * height)));
    entries
        .into_iter()
        .take(3)
        .find_map(|(width, height, _, data)| {
            if data.starts_with(PNG_MAGIC) {
                decode_png(data, Some((width, height)))
            } else {
                decode_dib(data, width, height)
            }
        })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn decode_dib(bytes: &[u8], width: u32, height: u32) -> Option<Bitmap> {
    const HEADER: usize = 40;
    // Small conventional BITMAPINFOHEADER only: uncompressed 24/32bpp,
    // bottom-up rows, no palette or profile. ICO height includes the AND mask.
    let depth = le16(bytes, 14)?;
    if le32(bytes, 0)? != HEADER as u32
        || le32(bytes, 4)? != width
        || le32(bytes, 8)? != 2 * height
        || le16(bytes, 12)? != 1
        || !matches!(depth, 24 | 32)
        || le32(bytes, 16)? != 0
        || le32(bytes, 32)? != 0
        || le32(bytes, 36)? != 0
    {
        return None;
    }
    let stride = (width * u32::from(depth)).div_ceil(32) as usize * 4;
    let alpha_stride = width.div_ceil(32) as usize * 4;
    let bitmap_end = HEADER + stride * height as usize;
    let pixels_src = bytes.get(HEADER..bitmap_end)?;
    let mask = match bytes.len().checked_sub(bitmap_end)? {
        0 => None,
        len if len == alpha_stride * height as usize => Some(bytes.get(bitmap_end..)?),
        _ => return None,
    };

    let use_alpha = depth == 32 && pixels_src.chunks_exact(4).any(|pixel| pixel[3] != 0);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in (0..height as usize).rev() {
        for x in 0..width as usize {
            let at = row * stride + x * usize::from(depth / 8);
            let masked =
                mask.is_some_and(|m| m[row * alpha_stride + x / 8] & (0x80 >> (x % 8)) != 0);
            let alpha = if masked {
                0
            } else if use_alpha {
                pixels_src[at + 3]
            } else {
                255
            };
            pixels.extend_from_slice(&[
                pixels_src[at + 2],
                pixels_src[at + 1],
                pixels_src[at],
                alpha,
            ]);
        }
    }
    Some(Bitmap {
        width,
        height,
        pixels,
    })
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::{fs as files, os::unix::fs::symlink, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("butler-favicon-{}", uuid::Uuid::new_v4()));
            files::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = files::remove_dir_all(&self.0);
        }
    }

    fn png(width: u32, height: u32, value: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&vec![value; (width * height * 4) as usize])
                .unwrap();
        }
        bytes
    }

    fn ico(bytes: &[u8], width: u8, height: u8) -> Vec<u8> {
        let mut file = vec![0, 0, 1, 0, 1, 0, width, height, 0, 0, 1, 0, 32, 0];
        file.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        file.extend_from_slice(&22_u32.to_le_bytes());
        file.extend_from_slice(bytes);
        file
    }

    #[test]
    fn follows_t3_order_but_skips_unsupported_and_invalid_assets() {
        let root = Fixture::new();
        files::create_dir(root.0.join("public")).unwrap();
        files::write(root.0.join("favicon.svg"), "<svg/>").unwrap();
        files::write(root.0.join("favicon.ico"), png(1, 1, 1)).unwrap();
        files::write(root.0.join("favicon.png"), png(1, 1, 2)).unwrap();
        files::write(root.0.join("public/favicon.ico"), ico(&png(1, 1, 3), 1, 1)).unwrap();
        assert_eq!(
            bitmap(&root.0).unwrap().decoded().pixels().as_ref(),
            &[2; 4]
        );
        files::remove_file(root.0.join("favicon.png")).unwrap();
        assert_eq!(
            bitmap(&root.0).unwrap().decoded().pixels().as_ref(),
            &[3; 4]
        );
    }

    #[test]
    fn refuses_links_in_file_parent_and_workspace_root() {
        let root = Fixture::new();
        let outside = Fixture::new();
        files::write(outside.0.join("favicon.png"), png(1, 1, 42)).unwrap();
        symlink(outside.0.join("favicon.png"), root.0.join("favicon.png")).unwrap();
        symlink(&outside.0, root.0.join("public")).unwrap();
        assert!(bitmap(&root.0).is_none());
        assert!(bitmap(&root.0.join("public")).is_none());
    }

    #[test]
    fn bounds_file_dimensions_and_ico_entry_slices() {
        let root = Fixture::new();
        let oversized = root.0.join("favicon.png");
        files::write(&oversized, vec![0; MAX_BYTES + 1]).unwrap();
        assert!(bitmap(&root.0).is_none());
        files::write(&oversized, png(257, 1, 3)).unwrap();
        assert!(bitmap(&root.0).is_none());

        let image = png(2, 2, 3);
        assert!(decode_ico(&ico(&image, 1, 1)).is_none());
        let mut bad_offset = ico(&image, 2, 2);
        bad_offset[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_ico(&bad_offset).is_none());
        let mut bad_len = ico(&image, 2, 2);
        bad_len[14..18].copy_from_slice(&8_u32.to_le_bytes());
        assert!(decode_ico(&bad_len).is_none());
        let mut bad_count = ico(&image, 2, 2);
        bad_count[4..6].copy_from_slice(&17_u16.to_le_bytes());
        assert!(decode_ico(&bad_count).is_none());
    }

    #[test]
    fn decodes_uncompressed_ico_dib_with_bottom_up_alpha_mask() {
        let mut dib = vec![0u8; 40];
        dib[..4].copy_from_slice(&40_u32.to_le_bytes());
        dib[4..8].copy_from_slice(&1_u32.to_le_bytes());
        dib[8..12].copy_from_slice(&4_u32.to_le_bytes());
        dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32_u16.to_le_bytes());
        dib.extend_from_slice(&[
            8, 7, 6, 0, // bottom BGRA
            3, 2, 1, 0, // top BGRA
            0x80, 0, 0, 0, // bottom masked
            0, 0, 0, 0, // top visible
        ]);
        let image = decode_ico(&ico(&dib, 1, 2)).unwrap();
        assert_eq!((image.width, image.height), (1, 2));
        assert_eq!(image.pixels, [1, 2, 3, 255, 6, 7, 8, 0]);
    }
}
