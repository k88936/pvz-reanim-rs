use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::{RgbaImage, Rgba};
use image_webp::{ColorType, WebPEncoder};
use reanim_parser::*;

const IMAGE_PREFIXES: &[&str] = &["IMAGE_REANIM_", "IMAGE_"];

/// Loaded image with cel info.
pub(crate) struct LoadedImage {
    pixels: RgbaImage,
    cel_w: u32,
    cel_h: u32,
}

// LoadedImage intentionally minimal; cel fields accessed directly.

/// Per-image lazy-load entry.
struct ImageEntry {
    path: PathBuf,
    loaded: OnceLock<Option<LoadedImage>>,
}

/// Image database: maps XML image refs -> loaded PNGs (lazily loaded).
pub struct ImageDb {
    entries: HashMap<String, ImageEntry>,
}

impl ImageDb {
    /// Build the database from one or more directories containing .png/.jpg files.
    /// Later directories override earlier ones for same-key entries.
    /// Images are **not** loaded eagerly — loading happens on first `get()` access.
    pub fn from_directories(dirs: &[&Path]) -> Self {
        let mut entries = HashMap::new();
        for dir in dirs {
            let read_dir = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in read_dir.flatten() {
                let path = entry.path();
                let ext = match path.extension().and_then(|s| s.to_str()) {
                    Some(e) => e,
                    None => continue,
                };
                if ext != "png" && ext != "jpg" {
                    continue;
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let lower = stem.to_lowercase();
                entries.insert(
                    lower,
                    ImageEntry {
                        path,
                        loaded: OnceLock::new(),
                    },
                );
            }
        }
        Self { entries }
    }

    /// Build the database from a single directory containing .png/.jpg files.
    /// Images are **not** loaded eagerly — loading happens on first `get()` access.
    pub fn from_directory(dir: &Path) -> Self {
        Self::from_directories(&[dir])
    }

    /// Look up an XML image ref (e.g. "IMAGE_REANIM_BLOVER_HEAD").
    /// Loads the image from disk on first access and caches it thereafter.
    pub(crate) fn get(&self, xml_ref: &str) -> Option<&LoadedImage> {
        for prefix in IMAGE_PREFIXES {
            if let Some(stripped) = xml_ref.strip_prefix(prefix) {
                let key = stripped.to_lowercase();
                if let Some(entry) = self.entries.get(&key) {
                    return entry
                        .loaded
                        .get_or_init(|| Self::load_image(&entry.path))
                        .as_ref();
                }
            }
        }
        // Try the whole ref lowercased as a last resort
        self.entries
            .get(&xml_ref.to_lowercase())
            .and_then(|entry| {
                entry
                    .loaded
                    .get_or_init(|| Self::load_image(&entry.path))
                    .as_ref()
            })
    }

    /// Load a single image file from disk. Returns `None` if the file is corrupt.
    fn load_image(path: &Path) -> Option<LoadedImage> {
        let img_data = match image::open(path) {
            Ok(i) => i.to_rgba8(),
            Err(_) => return None,
        };
        let (w, h) = img_data.dimensions();
        Some(LoadedImage {
            pixels: img_data,
            cel_w: w,
            cel_h: h,
        })
    }

    /// Return cel dimensions for an XML image ref, or a default.
    pub fn cel_size(&self, xml_ref: &str, default: (u32, u32)) -> (u32, u32) {
        self.get(xml_ref)
            .map(|i| (i.cel_w, i.cel_h))
            .unwrap_or(default)
    }
}

#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub render_w: i32,
    pub render_h: i32,
    pub center_off_x: f32,
    pub center_off_y: f32,
}

pub fn compute_bounding_box(
    def: &ReanimatorDefinition,
    images: &ImageDb,
    default_cel: (u32, u32),
) -> BoundingBox {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for track in &def.tracks {
        for tr in &track.transforms {
            let img_name = match &tr.image {
                Some(n) => n,
                None => continue,
            };
            let (cw, ch) = images.cel_size(img_name, default_cel);
            let cw = cw as f32;
            let ch = ch as f32;
            let m = from_transform(tr);
            for &(x, y) in &get_corners(&m, cw, ch) {
                if x < min_x { min_x = x; }
                if y < min_y { min_y = y; }
                if x > max_x { max_x = x; }
                if y > max_y { max_y = y; }
            }
        }
    }

    if min_x == f32::MAX {
        min_x = 0.0; min_y = 0.0; max_x = 100.0; max_y = 100.0;
    }

    let render_w = ((max_x - min_x + 2.0) as i32).max(1);
    let render_h = ((max_y - min_y + 2.0) as i32).max(1);
    let center_off_x = render_w as f32 * 0.5 - (min_x + max_x) * 0.5;
    let center_off_y = render_h as f32 * 0.5 - (min_y + max_y) * 0.5;

    BoundingBox { min_x, min_y, max_x, max_y, render_w, render_h, center_off_x, center_off_y }
}

fn get_corners(
    m: &glam::Mat3,
    cw: f32,
    ch: f32,
) -> [(f32, f32); 4] {
    let c = [
        (m.z_axis.x, m.z_axis.y),
        (m.x_axis.x * cw + m.z_axis.x, m.x_axis.y * cw + m.z_axis.y),
        (m.x_axis.x * cw + m.y_axis.x * ch + m.z_axis.x, m.x_axis.y * cw + m.y_axis.y * ch + m.z_axis.y),
        (m.y_axis.x * ch + m.z_axis.x, m.y_axis.y * ch + m.z_axis.y),
    ];
    log::debug!("[get_corners] cw={} ch={}", cw, ch);
    for (i, (x, y)) in c.iter().enumerate() {
        log::debug!("  corner[{}] = ({}, {})", i, x, y);
    }
    c
}

fn sign(p1x: f32, p1y: f32, p2x: f32, p2y: f32, p3x: f32, p3y: f32) -> f32 {
    (p1x - p3x) * (p2y - p3y) - (p2x - p3x) * (p1y - p3y)
}

fn point_in_triangle(px: f32, py: f32, tri: &[(f32, f32); 3]) -> bool {
    let d1 = sign(px, py, tri[0].0, tri[0].1, tri[1].0, tri[1].1);
    let d2 = sign(px, py, tri[1].0, tri[1].1, tri[2].0, tri[2].1);
    let d3 = sign(px, py, tri[2].0, tri[2].1, tri[0].0, tri[0].1);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn fill_quad_textured(
    img: &mut RgbaImage,
    corners: &[(f32, f32); 4],
    source: &LoadedImage,
    alpha: f32,
) {
    let min_x = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min).floor() as i32;
    let min_y = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min).floor() as i32;
    let max_x = corners.iter().map(|c| c.0).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
    let max_y = corners.iter().map(|c| c.1).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;

    let tri1 = [corners[0], corners[1], corners[3]];
    let tri2 = [corners[1], corners[2], corners[3]];
    let tri1_uv = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
    let tri2_uv = [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];

    let sw = source.cel_w as f32;
    let sh = source.cel_h as f32;

    let w = img.width() as i32;
    let h = img.height() as i32;

    for py in (min_y.max(0))..=(max_y.min(h - 1)) {
        for px in (min_x.max(0))..=(max_x.min(w - 1)) {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;

            let (tri_corners, tri_uv) = if point_in_triangle(fx, fy, &tri1) {
                (&tri1, &tri1_uv)
            } else if point_in_triangle(fx, fy, &tri2) {
                (&tri2, &tri2_uv)
            } else {
                continue;
            };

            // Barycentric: u = λ_A, v = λ_B, w = λ_C
            let (ax, ay) = tri_corners[0];
            let (bx, by) = tri_corners[1];
            let (cx, cy) = tri_corners[2];

            let denom = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
            if denom.abs() < 1e-10 { continue; }

            let lambda_a = ((by - cy) * (fx - cx) + (cx - bx) * (fy - cy)) / denom;
            let lambda_b = ((cy - ay) * (fx - cx) + (ax - cx) * (fy - cy)) / denom;
            let lambda_c = 1.0 - lambda_a - lambda_b;

            let uv_u = tri_uv[0].0 * lambda_a + tri_uv[1].0 * lambda_b + tri_uv[2].0 * lambda_c;
            let uv_v = tri_uv[0].1 * lambda_a + tri_uv[1].1 * lambda_b + tri_uv[2].1 * lambda_c;

            let sx = (uv_u * sw).clamp(0.0, sw - 1.0) as u32;
            let sy = (uv_v * sh).clamp(0.0, sh - 1.0) as u32;

            let src = source.pixels.get_pixel(sx, sy);
            let sa = (src[3] as f32 * alpha) as u8;
            if sa == 0 { continue; }

            let dst = img.get_pixel_mut(px as u32, py as u32);
            if sa == 255 {
                *dst = Rgba([src[0], src[1], src[2], 255]);
            } else {
                let a = sa as f32 / 255.0;
                let inv = 1.0 - a;
                *dst = Rgba([
                    (src[0] as f32 * a + dst[0] as f32 * inv) as u8,
                    (src[1] as f32 * a + dst[1] as f32 * inv) as u8,
                    (src[2] as f32 * a + dst[2] as f32 * inv) as u8,
                    dst[3].max(sa),
                ]);
            }
        }
    }
}

fn render_track(
    img: &mut RgbaImage,
    transform: &ReanimatorTransform,
    images: &ImageDb,
    center_off_x: f32,
    center_off_y: f32,
) {
    let img_name = match &transform.image {
        Some(n) => n,
        None => return,
    };
    if transform.frame < 0.0 {
        return;
    }
    let source = match images.get(img_name) {
        Some(s) => s,
        None => return,
    };
    let cw = source.cel_w as f32;
    let ch = source.cel_h as f32;

    let m = from_transform(transform);
    let corners = get_corners(&m, cw, ch);
    let tx = center_off_x - 1.0;
    let ty = center_off_y - 1.0;
    let shifted = corners.map(|(x, y)| (x + tx, y + ty));

    log::debug!("[render_track] alpha={} -> factor={}", transform.alpha, (transform.alpha / 255.0).clamp(0.0, 1.0));
    log::debug!("[render_track] center_off=({}, {}) tx=({}, {})", center_off_x, center_off_y, tx, ty);
    for (i, (x, y)) in shifted.iter().enumerate() {
        log::debug!("  shifted[{}] = ({}, {})", i, x, y);
    }
    let alpha_factor = if transform.alpha > 1.0 { 1.0 } else { transform.alpha.max(0.0) };
    fill_quad_textured(img, &shifted, source, alpha_factor);
}

fn render_frame(
    def: &ReanimatorDefinition,
    frame_idx: i32,
    total_frames: i32,
    images: &ImageDb,
    bbox: &BoundingBox,
) -> RgbaImage {
    let anim_time = if total_frames > 1 {
        frame_idx as f32 / (total_frames - 1) as f32
    } else {
        0.0
    };

    let ft = ReanimatorFrameTime::from_anim_time(anim_time, 0, total_frames, false);
    let mut img = RgbaImage::new(bbox.render_w as u32, bbox.render_h as u32);

    for track in &def.tracks {
        let before = &track.transforms[ft.frame_before as usize];
        let after = &track.transforms[ft.frame_after as usize];
        let transform = lerp_transform(before, after, ft.fraction);
        render_track(&mut img, &transform, images,
                     bbox.center_off_x, bbox.center_off_y);
    }

    img
}


/// An animation mask: a named frame range from an `anim_*` track.
#[derive(Debug, Clone)]
pub struct AnimMask {
    pub name: String,
    pub frame_start: i32,
    pub frame_end: i32,
}

/// Find all `anim_*` mask tracks and their visible frame ranges.
pub fn find_anim_masks(def: &ReanimatorDefinition) -> Vec<AnimMask> {
    let mut masks = Vec::new();
    for track in &def.tracks {
        if !track.name.starts_with("anim_") {
            continue;
        }
        let mut i = 0;
        let tx = &track.transforms;
        while i < tx.len() {
            if tx[i].frame >= 0.0 {
                let start = i;
                while i < tx.len() && tx[i].frame >= 0.0 {
                    i += 1;
                }
                masks.push(AnimMask {
                    name: track.name.clone(),
                    frame_start: start as i32,
                    frame_end: (i - 1) as i32,
                });
            } else {
                i += 1;
            }
        }
    }
    masks
}

/// Encode a single RGBA frame to a VP8L bitstream (extracted from a simple lossless WebP).
fn encode_vp8l_frame(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    WebPEncoder::new(&mut buf)
        .encode(data, width, height, ColorType::Rgba8)
        .unwrap();

    // Simple lossless WebP layout:
    // [0..4]   "RIFF"
    // [4..8]   file size (little-endian u32)
    // [8..12]  "WEBP"
    // [12..16] "VP8L"
    // [16..20] VP8L chunk data size (little-endian u32)
    // [20..]   VP8L bitstream data
    let vp8l_data_size = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
    buf[20..20 + vp8l_data_size].to_vec()
}

/// Build a complete animated WebP from frame VP8L bitstreams.
fn build_animated_webp(
    frames: &[Vec<u8>],
    width: u32,
    height: u32,
    loop_count: u16,
    frame_duration_ms: u32,
) -> Vec<u8> {
    let mut output = Vec::new();

    // Reserve space for RIFF header (will patch the file size later)
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0u8; 4]); // placeholder for file size
    output.extend_from_slice(b"WEBP");

    // --- VP8X chunk (10 bytes of data) ---
    let mut vp8x_data = Vec::with_capacity(10);
    vp8x_data.push(0b0000_0010); // flags: animation bit set
    vp8x_data.extend_from_slice(&[0u8; 3]); // reserved
    vp8x_data.extend_from_slice(&(width - 1).to_le_bytes()[..3]); // canvas width - 1
    vp8x_data.extend_from_slice(&(height - 1).to_le_bytes()[..3]); // canvas height - 1

    output.extend_from_slice(b"VP8X");
    output.extend_from_slice(&(vp8x_data.len() as u32).to_le_bytes());
    output.extend_from_slice(&vp8x_data);

    // --- ANIM chunk (6 bytes of data) ---
    let mut anim_data = Vec::with_capacity(6);
    anim_data.extend_from_slice(&[0u8; 4]); // background color (BGRA) - default black
    anim_data.extend_from_slice(&loop_count.to_le_bytes());

    output.extend_from_slice(b"ANIM");
    output.extend_from_slice(&(anim_data.len() as u32).to_le_bytes());
    output.extend_from_slice(&anim_data);

    // --- ANMF chunks ---
    for vp8l_data in frames {
        // ANMF header (16 bytes before subchunk)
        let mut anmf_data = Vec::with_capacity(32);

        // Frame position and size (3 bytes each, little-endian)
        anmf_data.extend_from_slice(&[0u8; 3]); // frame X offset
        anmf_data.extend_from_slice(&[0u8; 3]); // frame Y offset
        anmf_data.extend_from_slice(&(width - 1).to_le_bytes()[..3]); // frame width - 1
        anmf_data.extend_from_slice(&(height - 1).to_le_bytes()[..3]); // frame height - 1
        anmf_data.extend_from_slice(&frame_duration_ms.to_le_bytes()[..3]); // frame duration (3 bytes)
        anmf_data.push(0); // flags (no dispose, no blend)

        // VP8L sub-chunk inside the ANMF
        anmf_data.extend_from_slice(b"VP8L");
        anmf_data.extend_from_slice(&(vp8l_data.len() as u32).to_le_bytes());
        anmf_data.extend_from_slice(vp8l_data);
        // VP8L sub-chunk padding
        if vp8l_data.len() % 2 == 1 {
            anmf_data.push(0);
        }

        // ANMF chunk: header + data
        output.extend_from_slice(b"ANMF");
        output.extend_from_slice(&(anmf_data.len() as u32).to_le_bytes());
        output.extend_from_slice(&anmf_data);
        // ANMF chunk padding
        if anmf_data.len() % 2 == 1 {
            output.push(0);
        }
    }

    // Patch the RIFF file size
    let file_size = (output.len() as u32).wrapping_sub(8);
    output[4..8].copy_from_slice(&file_size.to_le_bytes());

    output
}

/// Render a specific frame range to an animated WebP.
pub fn render_range_to_webp<P: AsRef<Path>>(
    def: &ReanimatorDefinition,
    images: &ImageDb,
    default_cel: (u32, u32),
    output: P,
    frame_start: i32,
    frame_end: i32,
) -> anyhow::Result<()> {
    let bbox = compute_bounding_box(def, images, default_cel);
    let total_frames = def.frame_count();
    let range_len = frame_end - frame_start + 1;
    let frame_duration_ms = if def.fps > 0.0 {
        (1000.0 / def.fps as f32).ceil() as u32
    } else {
        30
    };

    log::info!(
        "Rendering {} frames ({}..{}) at {}x{} (bbox [{:.0},{:.0}]-[{:.0},{:.0}]) center: ({:.1},{:.1}) delay={}ms",
        range_len, frame_start, frame_end,
        bbox.render_w, bbox.render_h,
        bbox.min_x, bbox.min_y, bbox.max_x, bbox.max_y,
        bbox.center_off_x, bbox.center_off_y,
        frame_duration_ms,
    );

    // Render all frames to RGBA, then encode to VP8L bitstreams
    let mut vp8l_frames: Vec<Vec<u8>> = Vec::with_capacity(range_len as usize);
    for abs_idx in frame_start..=frame_end {
        let rgba = render_frame(def, abs_idx, total_frames, images, &bbox);
        let (w, h) = rgba.dimensions();
        let vp8l = encode_vp8l_frame(&rgba.into_raw(), w, h);
        vp8l_frames.push(vp8l);
        log::info!(
            "Frame {}/{} (abs {}) encoded ({} bytes VP8L)",
            abs_idx - frame_start + 1,
            range_len,
            abs_idx,
            vp8l_frames.last().map(|v| v.len()).unwrap_or(0),
        );
    }

    // Assemble animated WebP and write to disk
    let webp_data = build_animated_webp(&vp8l_frames, bbox.render_w as u32, bbox.render_h as u32, 0, frame_duration_ms);
    std::fs::write(output.as_ref(), &webp_data)?;

    Ok(())
}

pub fn render_to_webp<P: AsRef<Path>>(
    def: &ReanimatorDefinition,
    images: &ImageDb,
    default_cel: (u32, u32),
    output: P,
) -> anyhow::Result<()> {
    let total_frames = def.frame_count();
    render_range_to_webp(def, images, default_cel, output, 0, total_frames - 1)
}
