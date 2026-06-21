use anyhow::{Context, Result, bail};
use base64::Engine;
use screenshots::{Screen, image::RgbaImage};
use std::io::Cursor;
use tracing::{info, warn};

const MAX_WIDTH: u32 = 1280;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTarget {
    /// Monitor under the mouse cursor (default).
    Cursor,
    /// OS primary monitor.
    Primary,
    /// 1-based monitor index (`1` = first display).
    Index(usize),
}

#[derive(Debug, Clone)]
pub struct ScreenInfo {
    pub index:      usize,
    pub width:      u32,
    pub height:     u32,
    pub is_primary: bool,
    pub x:          i32,
    pub y:          i32,
}

#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub image_base64:  String,
    pub monitor_label: String,
    pub width:         u32,
    pub height:        u32,
    pub png_bytes:     usize,
}

pub fn list_screens() -> Result<Vec<ScreenInfo>> {
    let screens = Screen::all().context("Failed to enumerate displays")?;
    Ok(screens
        .iter()
        .enumerate()
        .map(|(i, screen)| {
            let info = &screen.display_info;
            ScreenInfo {
                index:      i + 1,
                width:      info.width,
                height:     info.height,
                is_primary: info.is_primary,
                x:          info.x,
                y:          info.y,
            }
        })
        .collect())
}

/// Capture the monitor under the cursor (falls back through other monitors on failure).
pub fn capture_screen() -> Result<String> {
    capture_screen_with_target(CaptureTarget::Cursor).map(|r| r.image_base64)
}

pub fn capture_screen_with_target(target: CaptureTarget) -> Result<CaptureResult> {
    let screens = Screen::all().context("Failed to enumerate displays")?;
    if screens.is_empty() {
        bail!("No displays found");
    }

    info!("Found {} display(s)", screens.len());
    for (i, screen) in screens.iter().enumerate() {
        let info = &screen.display_info;
        info!(
            "  Monitor {}: {}x{} at ({},{}) primary={}",
            i + 1,
            info.width,
            info.height,
            info.x,
            info.y,
            info.is_primary,
        );
    }

    let indices = resolve_capture_indices(&screens, target);
    let mut errors = Vec::new();

    for idx in indices {
        let screen = &screens[idx];
        let info = &screen.display_info;
        let label = format!(
            "Monitor {} ({}x{}, primary={})",
            idx + 1,
            info.width,
            info.height,
            info.is_primary,
        );

        match try_capture(screen) {
            Ok((image_base64, width, height, png_bytes)) => {
                info!("Captured {label}");
                return Ok(CaptureResult {
                    image_base64,
                    monitor_label: label,
                    width,
                    height,
                    png_bytes,
                });
            }
            Err(e) => {
                warn!("Capture failed on {label}: {e:#}");
                errors.push(format!("{label}: {e:#}"));
            }
        }
    }

    bail!(
        "Screen capture failed on all attempted monitors.\n{}\n\
         Tips: move the cursor onto the target display, try `/screen 2`, \
         or check that no other app is blocking screen capture.",
        errors.join("\n")
    );
}

pub fn capture_screen_to_file(path: &str) -> Result<()> {
    let screens = Screen::all()?;
    let indices = resolve_capture_indices(&screens, CaptureTarget::Cursor);
    let idx = indices
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("No display to capture"))?;
    let image = screens[idx].capture()?;
    image.save(path)?;
    Ok(())
}

fn resolve_capture_indices(screens: &[Screen], target: CaptureTarget) -> Vec<usize> {
    match target {
        CaptureTarget::Index(n) if n >= 1 && n <= screens.len() => {
            let preferred = n - 1;
            fallback_indices(screens, preferred)
        }
        CaptureTarget::Index(n) => {
            warn!("Monitor index {n} is out of range (1-{}), using cursor", screens.len());
            resolve_capture_indices(screens, CaptureTarget::Cursor)
        }
        CaptureTarget::Primary => {
            let preferred = screens
                .iter()
                .position(|s| s.display_info.is_primary)
                .unwrap_or(0);
            fallback_indices(screens, preferred)
        }
        CaptureTarget::Cursor => {
            if let Some(idx) = screen_index_at_cursor(screens) {
                fallback_indices(screens, idx)
            } else {
                let preferred = screens
                    .iter()
                    .position(|s| s.display_info.is_primary)
                    .unwrap_or(0);
                fallback_indices(screens, preferred)
            }
        }
    }
}

fn fallback_indices(screens: &[Screen], preferred: usize) -> Vec<usize> {
    let mut indices = vec![preferred];
    for i in 0..screens.len() {
        if i != preferred {
            indices.push(i);
        }
    }
    indices
}

#[cfg(windows)]
fn screen_index_at_cursor(screens: &[Screen]) -> Option<usize> {
    let (cx, cy) = cursor_position()?;
    let screen = Screen::from_point(cx, cy).ok()?;
    screens.iter().position(|s| {
        s.display_info.id == screen.display_info.id
    })
}

#[cfg(not(windows))]
fn screen_index_at_cursor(_screens: &[Screen]) -> Option<usize> {
    None
}

#[cfg(windows)]
fn cursor_position() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut point = POINT::default();
        GetCursorPos(&mut point).ok()?;
        Some((point.x, point.y))
    }
}

fn try_capture(screen: &Screen) -> Result<(String, u32, u32, usize)> {
    let image = screen
        .capture()
        .context("Display driver returned an error")?;

    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        bail!("Capture returned an empty image");
    }

    let png_bytes = encode_png_resized(&image, MAX_WIDTH)?;
    if png_bytes.len() < 1000 {
        warn!(
            "Captured PNG is very small ({} bytes) — display may be blank or blocked",
            png_bytes.len()
        );
    }

    let (out_w, out_h) = scaled_dimensions(w, h, MAX_WIDTH);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Ok((encoded, out_w, out_h, png_bytes.len()))
}

fn scaled_dimensions(w: u32, h: u32, max_width: u32) -> (u32, u32) {
    if w <= max_width {
        (w, h)
    } else {
        let ratio = max_width as f32 / w as f32;
        (max_width, (h as f32 * ratio).round() as u32)
    }
}

fn encode_png_resized(image: &RgbaImage, max_width: u32) -> Result<Vec<u8>> {
    use screenshots::image::imageops::FilterType;

    let (w, h) = image.dimensions();
    let img = if w > max_width {
        let (new_w, new_h) = scaled_dimensions(w, h, max_width);
        info!("Resizing capture from {w}x{h} to {new_w}x{new_h}");
        screenshots::image::imageops::resize(image, new_w, new_h, FilterType::Triangle)
    } else {
        image.clone()
    };

    let mut png_bytes = Vec::new();
    img.write_to(
        &mut Cursor::new(&mut png_bytes),
        screenshots::image::ImageFormat::Png,
    )?;
    Ok(png_bytes)
}
