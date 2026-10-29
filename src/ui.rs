//! Small GDI helpers shared by the panel and settings windows.
//!
//! The notch paints itself with its own copy of these primitives; this module
//! keeps the utility windows in the same visual language (dark card, Segoe UI,
//! soft rounded shapes, green accent).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND, DWMWINDOWATTRIBUTE,
};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen, CreateSolidBrush, DeleteDC,
    DeleteObject, DrawTextW, FillRect, GetStockObject, LineTo, MoveToEx, RoundRect, SelectObject,
    SetBkMode, SetBrushOrgEx, SetStretchBltMode, SetTextColor, StretchBlt, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DIB_RGB_COLORS, DRAW_TEXT_FORMAT,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, HALFTONE, HDC, HFONT, HGDIOBJ, NULL_PEN,
    PS_SOLID, SRCCOPY, TRANSPARENT,
};

pub const BG: u32 = 0x00161618;
pub const CARD: u32 = 0x001F1F22;
pub const ROW_HOVER: u32 = 0x0028282C;
pub const HOVER: u32 = 0x002F2F33;
pub const TEXT: u32 = 0x00FFFFFF;
pub const DIM: u32 = 0x00938E8E;
pub const FAINT: u32 = 0x003A3A3C;
pub const GREEN: u32 = 0x0058D130;
pub const GREEN_HI: u32 = 0x006DE24B;
pub const TRACK: u32 = 0x00303030;
pub const DONE: u32 = 0x006A6A6E;

/// Dark title bar and rounded corners (Windows 11; ignored on older builds).
pub fn dark_frame(hwnd: HWND) {
    unsafe {
        let dark: i32 = 1;
        let ptr = &dark as *const i32 as *const core::ffi::c_void;
        if DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ptr, 4).is_err() {
            // Older Windows 10 builds used attribute 19.
            let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(19), ptr, 4);
        }
        let round = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &round as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<windows::Win32::Graphics::Dwm::DWM_WINDOW_CORNER_PREFERENCE>()
                as u32,
        );
    }
}

pub unsafe fn font(size: i32, weight: u32) -> HFONT {
    let face: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
    CreateFontW(
        -size,
        0,
        0,
        0,
        weight as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        0,
        0,
        CLEARTYPE_QUALITY.0 as u32,
        FF_DONTCARE.0 as u32,
        PCWSTR(face.as_ptr()),
    )
}

pub unsafe fn text(
    hdc: HDC,
    font: HFONT,
    color: u32,
    s: &str,
    rc: RECT,
    flags: DRAW_TEXT_FORMAT,
) {
    if s.is_empty() {
        return;
    }
    let mut v: Vec<u16> = s.encode_utf16().collect();
    let old = SelectObject(hdc, HGDIOBJ(font.0));
    SetTextColor(hdc, COLORREF(color));
    SetBkMode(hdc, TRANSPARENT);
    let mut r = rc;
    DrawTextW(hdc, &mut v, &mut r, DT_SINGLELINE | DT_VCENTER | flags);
    SelectObject(hdc, old);
}

pub unsafe fn fill(hdc: HDC, l: i32, t: i32, r: i32, b: i32, color: u32) {
    let brush = CreateSolidBrush(COLORREF(color));
    let rc = RECT { left: l, top: t, right: r, bottom: b };
    FillRect(hdc, &rc, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

pub unsafe fn round(hdc: HDC, l: i32, t: i32, r: i32, b: i32, radius: i32, color: u32) {
    let brush = CreateSolidBrush(COLORREF(color));
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = RoundRect(hdc, l, t, r, b, radius, radius);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

pub unsafe fn outline(
    hdc: HDC,
    l: i32,
    t: i32,
    r: i32,
    b: i32,
    radius: i32,
    border: u32,
    fill_color: u32,
) {
    let brush = CreateSolidBrush(COLORREF(fill_color));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(border));
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = RoundRect(hdc, l, t, r, b, radius, radius);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

pub unsafe fn checkbox(hdc: HDC, x: i32, y: i32, checked: bool, accent: u32) {
    let size = 14;
    if checked {
        round(hdc, x, y, x + size, y + size, 8, accent);
        let pen = CreatePen(PS_SOLID, 2, COLORREF(0x00FFFFFF));
        let old = SelectObject(hdc, HGDIOBJ(pen.0));
        MoveToEx(hdc, x + 3, y + 7, None);
        let _ = LineTo(hdc, x + 6, y + 10);
        let _ = LineTo(hdc, x + 11, y + 4);
        SelectObject(hdc, old);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    } else {
        outline(hdc, x, y, x + size, y + size, 8, FAINT, CARD);
    }
}

pub unsafe fn progress(hdc: HDC, l: i32, t: i32, r: i32, pct: f32, fg: u32, bg: u32) {
    let h = 6;
    round(hdc, l, t, r, t + h, h, bg);
    let width = ((r - l) as f32 * pct.clamp(0.0, 1.0)) as i32;
    if width > 1 {
        round(hdc, l, t, l + width.max(h), t + h, h, fg);
    }
}

#[allow(clippy::too_many_arguments)]
pub unsafe fn pill(
    hdc: HDC,
    l: i32,
    t: i32,
    w: i32,
    h: i32,
    bg: u32,
    fg: u32,
    label: &str,
    font: HFONT,
) {
    round(hdc, l, t, l + w, t + h, h.min(14), bg);
    text(
        hdc,
        font,
        fg,
        label,
        RECT { left: l, top: t, right: l + w, bottom: t + h },
        DT_CENTER,
    );
}

// ------------------------------------------------------------------ aurora

static AURORA: AtomicBool = AtomicBool::new(false);
static AURORA_START: OnceLock<Instant> = OnceLock::new();
static AURORA_BG: std::sync::Mutex<Option<(i32, i32, Vec<u32>)>> = std::sync::Mutex::new(None);

pub fn set_aurora(on: bool) {
    AURORA.store(on, Ordering::Relaxed);
    if let Ok(mut cache) = AURORA_BG.lock() {
        *cache = None;
    }
}

pub fn is_aurora() -> bool {
    AURORA.load(Ordering::Relaxed)
}

/// Seconds since the process started; drives the slow colour drift.
pub fn aurora_phase() -> f32 {
    AURORA_START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

fn mix(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

/// Render a small aurora field as 32-bpp top-down BGRA pixels.
///
/// The notch uses a deliberately low-resolution field and lets HALFTONE stretch
/// it, keeping the 15 fps animation cheap. Utility windows ask for `aurora_grid`
/// at native size instead. `t` is in seconds and moves the ribbons and glows.
pub fn aurora_field(dst_w: i32, dst_h: i32, t: f32) -> (Vec<u32>, i32, i32) {
    let dst_w = dst_w.max(8);
    let dst_h = dst_h.max(8);
    let fh = 72.min(dst_h);
    let fw = ((dst_w as f32 * fh as f32 / dst_h as f32).round() as i32).clamp(12, 234);
    (aurora_grid(fw, fh, t), fw, fh)
}

/// Render the aurora at an explicit grid size.
pub fn aurora_grid(fw: i32, fh: i32, t: f32) -> Vec<u32> {
    let fw = fw.max(1);
    let fh = fh.max(1);

    // Cool palette only: teal/cyan drift on one ribbon, violet/magenta on the
    // other. The mix breathes slowly, so the card changes colour without
    // passing through the muddy olive/brown hues a full hue rotation creates.
    let teal = (46.0, 226.0, 196.0);
    let cyan = (56.0, 170.0, 244.0);
    let violet = (128.0, 100.0, 252.0);
    let rose = (206.0, 86.0, 224.0);
    let cool = mix(teal, cyan, 0.5 + 0.5 * (t * 0.14).sin());
    let warm = mix(violet, rose, 0.5 + 0.5 * (t * 0.11 + 1.3).sin());
    let glow_cool = mix(cyan, violet, 0.5 + 0.5 * (t * 0.07 + 0.4).cos());
    let glow_warm = mix(rose, violet, 0.5 + 0.5 * (t * 0.10 + 2.0).sin());

    // Breathing amplitudes: the dominant ribbon changes over time.
    let a1 = 0.55 + 0.45 * (t * 0.33).sin();
    let a2 = 0.50 + 0.50 * (t * 0.25 + 2.2).sin();
    let a3 = 0.40 + 0.40 * (t * 0.18 + 4.1).sin();
    let a4 = 0.60 + 0.40 * (t * 0.20 + 0.8).sin();

    let mut out = vec![0u32; (fw * fh) as usize];
    for y in 0..fh {
        let v = (y as f32 + 0.5) / fh as f32;
        let row = (y * fw) as usize;
        for x in 0..fw {
            let u = (x as f32 + 0.5) / fw as f32;

            // Two soft ribbons crossing the card at different speeds.
            let y1 = 0.34 + 0.20 * (2.2 * u + 0.45 * t).sin()
                + 0.07 * (5.3 * u - 0.70 * t + 1.1).sin();
            let d1 = (v - y1) / 0.19;
            let b1 = (-d1 * d1 * 2.4).exp();
            let core1 = (-d1 * d1 * 14.0).exp() * 34.0;
            let y2 = 0.60 + 0.18 * (1.7 * u - 0.34 * t + 2.0).sin()
                + 0.06 * (4.1 * u + 0.55 * t).sin();
            let d2 = (v - y2) / 0.17;
            let b2 = (-d2 * d2 * 2.2).exp();
            let core2 = (-d2 * d2 * 12.0).exp() * 26.0;

            // Two slow radial glows, one cool and one warm.
            let gx1 = 0.27 + 0.16 * (0.16 * t).sin();
            let gy1 = 0.30 + 0.13 * (0.13 * t + 0.7).cos();
            let q1 = (u - gx1) * (u - gx1) + (v - gy1) * (v - gy1) * 0.8;
            let glow1 = (-q1 / 0.09).exp();
            let gx2 = 0.78 + 0.12 * (0.18 * t + 1.6).cos();
            let gy2 = 0.58 + 0.14 * (0.15 * t).sin();
            let q2 = (u - gx2) * (u - gx2) * 0.9 + (v - gy2) * (v - gy2) * 0.8;
            let glow2 = (-q2 / 0.11).exp();

            // Glass: a cool top sheen plus a faint diagonal streak.
            let sheen = (-v / 0.10).exp() * 0.9
                + (-((v - (0.055 + 0.06 * u)) / 0.028).powi(2)).exp() * 0.35;
            let shimmer = 0.5 + 0.5 * (18.0 * u + 7.0 * v + 1.7 * t).sin();
            let shimmer = shimmer * shimmer * shimmer * 0.018;

            let mut r = 8.0 + 5.0 * v;
            let mut g = 12.0 + 6.0 * v;
            let mut b = 22.0 + 9.0 * v;
            let w1 = 0.34 * a1 * b1;
            r += cool.0 * w1;
            g += cool.1 * w1;
            b += cool.2 * w1;
            let w2 = 0.26 * a2 * b2;
            r += warm.0 * w2;
            g += warm.1 * w2;
            b += warm.2 * w2;
            let w3 = 0.22 * a3 * glow1;
            r += glow_cool.0 * w3;
            g += glow_cool.1 * w3;
            b += glow_cool.2 * w3;
            let w4 = 0.15 * a4 * glow2;
            r += glow_warm.0 * w4;
            g += glow_warm.1 * w4;
            b += glow_warm.2 * w4;
            r += 0.10 * core1 + 0.55 * core2;
            g += 0.80 * core1 + 0.30 * core2;
            b += 0.65 * core1 + 0.85 * core2;
            r += 12.6 * sheen + 255.0 * shimmer;
            g += 18.0 * sheen + 255.0 * shimmer;
            b += 22.0 * sheen + 255.0 * shimmer;

            // Vignette for depth, then clamp so white text keeps its contrast.
            let du = u - 0.5;
            let dv = v - 0.5;
            let vig = 1.0 - 0.30 * (du * du * 2.0 + dv * dv * 1.2);
            let r = (r * vig).clamp(0.0, 104.0) as u32;
            let g = (g * vig).clamp(0.0, 124.0) as u32;
            let b = (b * vig).clamp(0.0, 144.0) as u32;
            // 32-bpp BI_RGB is BGRA in memory (little-endian).
            out[row + x as usize] = b | (g << 8) | (r << 16);
        }
    }
    out
}

/// Native-resolution field for the utility windows. The phase is fixed, so the
/// result is cached per window size; it is also slightly dimmed because those
/// windows are taller and have more text sitting directly on the glass.
fn utility_field(w: i32, h: i32) -> Vec<u32> {
    if let Ok(cache) = AURORA_BG.lock() {
        if let Some((cw, ch, field)) = cache.as_ref() {
            if *cw == w && *ch == h {
                return field.clone();
            }
        }
    }
    let mut field = aurora_grid(w, h, 8.0);
    for p in field.iter_mut() {
        let b = ((*p & 0xff) * 184) >> 8;
        let g = (((*p >> 8) & 0xff) * 184) >> 8;
        let r = (((*p >> 16) & 0xff) * 184) >> 8;
        *p = b | (g << 8) | (r << 16);
    }
    if let Ok(mut cache) = AURORA_BG.lock() {
        *cache = Some((w, h, field.clone()));
    }
    field
}

/// Stretch an aurora field into `l,t,r,b`. Callers paint their own content on
/// top, so this is only the glass base. `detail` renders at native resolution
/// (utility windows, painted rarely) instead of the cheap notch grid.
pub unsafe fn aurora_bg(
    hdc: HDC,
    l: i32,
    t: i32,
    r: i32,
    b: i32,
    phase: f32,
    detail: bool,
) {
    let w = r - l;
    let h = b - t;
    if w <= 0 || h <= 0 {
        return;
    }
    let (field, fw, fh) = if detail {
        (utility_field(w, h), w, h)
    } else {
        aurora_field(w, h, phase)
    };
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: fw,
        biHeight: -fh,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let Ok(hbmp) = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0)
    else {
        return;
    };
    if bits.is_null() {
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        return;
    }
    std::ptr::copy_nonoverlapping(field.as_ptr(), bits as *mut u32, field.len());

    let mem = CreateCompatibleDC(hdc);
    if mem.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        return;
    }
    let old = SelectObject(mem, HGDIOBJ(hbmp.0));
    // HALFTONE averages the low-res field into smooth glass; the brush origin
    // must be reset after changing the stretch mode.
    let _ = SetStretchBltMode(hdc, HALFTONE);
    let _ = SetBrushOrgEx(hdc, 0, 0, None);
    let _ = StretchBlt(hdc, l, t, w, h, mem, 0, 0, fw, fh, SRCCOPY);
    SelectObject(mem, old);
    let _ = DeleteObject(HGDIOBJ(hbmp.0));
    let _ = DeleteDC(mem);
}

/// Paint a window background: flat dark, or a static aurora-glass field.
pub unsafe fn paint_bg(hdc: HDC, w: i32, h: i32) {
    if is_aurora() {
        aurora_bg(hdc, 0, 0, w, h, 8.0, true);
    } else {
        fill(hdc, 0, 0, w, h, BG);
    }
}
