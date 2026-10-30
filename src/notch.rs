//! The notch overlay, styled after the original Nusa app:
//! a compact black pill at the top of the screen that expands into a
//! rounded 420x140 card with five pages (media, focus, water, to-do, shelf).

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AlphaBlend, BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen,
    CreatePolygonRgn, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint,
    FillRect, GetStockObject, GetTextExtentPoint32W, InvalidateRect, LineTo, MoveToEx, Polygon,
    RoundRect, SelectObject, SetBkMode, SetTextColor, SetViewportOrgEx, SetWindowRgn, TextOutW,
    AC_SRC_ALPHA, AC_SRC_OVER, ALTERNATE, ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO,
    BITMAPINFOHEADER, BLENDFUNCTION, DEFAULT_CHARSET, DIB_RGB_COLORS, DRAW_TEXT_FORMAT, DT_CENTER,
    DT_END_ELLIPSIS, DT_LEFT, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, FW_NORMAL,
    FW_SEMIBOLD, HDC, HFONT, HGDIOBJ, NULL_BRUSH, NULL_PEN, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetTickCount};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, TrackMouseEvent, LASTINPUTINFO, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
    GetWindowRect, KillTimer, PostQuitMessage, RegisterClassExW, SetTimer, SetWindowPos, TranslateMessage,
    CS_DROPSHADOW, CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, MSG, SM_CXSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    WM_DESTROY, WM_DROPFILES, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_PAINT, WM_RBUTTONDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_ACCEPTFILES, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};

const COLLAPSED_W: i32 = 164;
const COLLAPSED_H: i32 = 30;
const COLLAPSED_BODY_W: i32 = 150;
const COLLAPSED_SHOULDER: i32 = 7;
const COLLAPSED_RADIUS: i32 = 16;
// After a minute of quiet the pill shrinks to just the sleeping cat face.
const DOZE_W: i32 = 56;
const DOZE_BODY_W: i32 = 46;
const DOZE_SHOULDER: i32 = 6;
const DOZE_RADIUS: i32 = 15;
const EXPANDED_W: i32 = 420;
const EXPANDED_H: i32 = 140;
const EXPANDED_SHOULDER: i32 = 17;
const EXPANDED_WINDOW_W: i32 = EXPANDED_W + EXPANDED_SHOULDER * 2;
const EXPANDED_RADIUS: i32 = 37;
const PAGE_COUNT: usize = 5;
const WM_MOUSELEAVE: u32 = 0x02A3;

const PAGE_MEDIA: usize = 0;
const PAGE_FOCUS: usize = 1;
const PAGE_WATER: usize = 2;
const PAGE_TODO: usize = 3;
const PAGE_SHELF: usize = 4;

static EXPANDED: AtomicBool = AtomicBool::new(false);
static PAGE: AtomicUsize = AtomicUsize::new(PAGE_MEDIA);
static WATER_ALERT: AtomicBool = AtomicBool::new(false);
static PLAYING: AtomicBool = AtomicBool::new(false);
static PLAY_POS: AtomicUsize = AtomicUsize::new(0); // seconds
static PLAY_LEN: AtomicUsize = AtomicUsize::new(0); // seconds

static NOTCH_TEXT: Mutex<String> = Mutex::new(String::new());
static LAST_INSIDE: Mutex<Option<Instant>> = Mutex::new(None);
static HOVER_ID: AtomicUsize = AtomicUsize::new(0);
static HOVER_TRACKING: AtomicBool = AtomicBool::new(false);

/// The little mascot lives beside the clock in the closed notch. It waves
/// when the pointer arrives, watches it while it stays, and dozes after a
/// minute of quiet.
#[derive(Clone, Copy)]
struct MascotWave {
    start: Instant,
    until: Instant,
}
static MASCOT_WAVE: Mutex<Option<MascotWave>> = Mutex::new(None);
static MASCOT_DOZING: AtomicBool = AtomicBool::new(false);
static MASCOT_HOVER: AtomicBool = AtomicBool::new(false);
/// Gaze offset in tenths of a pixel, so mouse-move frames only repaint when
/// the eyes actually shift.
static MASCOT_LOOK: Mutex<(i32, i32)> = Mutex::new((0, 0));
/// Centre of the mascot in window coordinates, recorded by the painter.
static MASCOT_CX: AtomicI32 = AtomicI32::new(COLLAPSED_W / 2);

// Hover targets shared with the paint routines.
const HOVER_CHEV_LEFT: usize = 1;
const HOVER_CHEV_RIGHT: usize = 2;
const HOVER_DOT: usize = 10; // + page index
const HOVER_MEDIA_PREV: usize = 30;
const HOVER_MEDIA_PLAY: usize = 31;
const HOVER_MEDIA_NEXT: usize = 32;
const HOVER_FOCUS_START: usize = 40;
const HOVER_FOCUS_RESET: usize = 41;
const HOVER_WATER_DRANK: usize = 50;
const HOVER_WATER_SNOOZE: usize = 51;
const HOVER_WATER_RESET: usize = 52;
const HOVER_SHELF_ZONE: usize = 60;
const HOVER_TODO_CARD: usize = 61;

fn hover_is(id: usize) -> bool {
    HOVER_ID.load(Ordering::Relaxed) == id
}
#[allow(static_mut_refs)]
static mut NOTCH_HWND: Option<HWND> = None;

pub fn set_notch_hwnd(hwnd: HWND) {
    unsafe { NOTCH_HWND = Some(hwnd); }
}

pub fn get_notch_hwnd() -> Option<HWND> {
    unsafe { NOTCH_HWND }
}

pub fn toggle_notch() {
    if let Some(hwnd) = get_notch_hwnd() {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOW};
            if IsWindowVisible(hwnd).as_bool() {
                ShowWindow(hwnd, SW_HIDE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
        }
    }
}

pub static FOCUS: OnceLock<Mutex<crate::focus::FocusTimer>> = OnceLock::new();
static MEDIA_TITLE: Mutex<String> = Mutex::new(String::new());
static LYRICS: Mutex<String> = Mutex::new(String::new());
static WEATHER: Mutex<String> = Mutex::new(String::new());
static SYSMON: Mutex<String> = Mutex::new(String::new());
static SYSMON_INSTANCE: OnceLock<Mutex<crate::sysmon::SystemMonitor>> = OnceLock::new();
static CPU_HISTORY: Mutex<Vec<f32>> = Mutex::new(Vec::new());
static RAM_HISTORY: Mutex<Vec<f32>> = Mutex::new(Vec::new());
static BT_COUNT: Mutex<usize> = Mutex::new(0);
static NOTIF_COUNT: Mutex<u32> = Mutex::new(0);

pub fn init_focus() {
    let mut t = crate::focus::FocusTimer::new();
    t.start();
    let _ = FOCUS.set(Mutex::new(t));
    let _ = SYSMON_INSTANCE.set(Mutex::new(crate::sysmon::SystemMonitor::new()));
}

pub fn set_text(s: &str) {
    if let Ok(mut t) = NOTCH_TEXT.lock() {
        *t = s.to_string();
    }
}

pub fn get_text() -> String {
    NOTCH_TEXT.lock().map(|t| t.clone()).unwrap_or_default()
}

pub fn set_media_title(s: &str) {
    if let Ok(mut m) = MEDIA_TITLE.lock() {
        *m = s.to_string();
    }
}

pub fn set_lyrics(s: &str) {
    if let Ok(mut l) = LYRICS.lock() {
        *l = s.to_string();
    }
}

pub fn set_weather(s: &str) {
    if let Ok(mut w) = WEATHER.lock() {
        *w = s.to_string();
    }
}

pub fn set_sysmon(s: &str) {
    if let Ok(mut m) = SYSMON.lock() {
        *m = s.to_string();
    }
}

pub fn set_bt_count(n: usize) {
    if let Ok(mut c) = BT_COUNT.lock() {
        *c = n;
    }
}

pub fn set_notif_count(n: u32) {
    if let Ok(mut c) = NOTIF_COUNT.lock() {
        *c = n;
    }
}

pub fn set_playback(position: u64, duration: u64, playing: bool) {
    PLAY_POS.store(position as usize, Ordering::Relaxed);
    PLAY_LEN.store(duration as usize, Ordering::Relaxed);
    PLAYING.store(playing, Ordering::Relaxed);
}

pub fn is_expanded() -> bool {
    EXPANDED.load(Ordering::Relaxed)
}

pub fn collapse() {
    if let Some(hwnd) = get_notch_hwnd() {
        layout(hwnd, false);
    }
}

// ---------------------------------------------------------------- geometry

fn layout(hwnd: HWND, expanded: bool) {
    unsafe {
        HOVER_ID.store(0, Ordering::Relaxed);
        if expanded {
            MASCOT_DOZING.store(false, Ordering::Relaxed);
        }
        let (w, h, body_w, shoulder, radius) = if expanded {
            (
                EXPANDED_WINDOW_W,
                EXPANDED_H,
                EXPANDED_W,
                EXPANDED_SHOULDER,
                EXPANDED_RADIUS,
            )
        } else if MASCOT_DOZING.load(Ordering::Relaxed) {
            (DOZE_W, COLLAPSED_H, DOZE_BODY_W, DOZE_SHOULDER, DOZE_RADIUS)
        } else {
            (
                COLLAPSED_W,
                COLLAPSED_H,
                COLLAPSED_BODY_W,
                COLLAPSED_SHOULDER,
                COLLAPSED_RADIUS,
            )
        };
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            (sw - w) / 2,
            0,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        apply_region(hwnd, w, h, body_w, shoulder, radius);
        EXPANDED.store(expanded, Ordering::Relaxed);
        if !expanded {
            *LAST_INSIDE.lock().unwrap() = None;
        } else {
            *LAST_INSIDE.lock().unwrap() = Some(Instant::now());
        }
        let _ = InvalidateRect(hwnd, None, true);
    }
}

/// Nusa's notch silhouette: the top edge flares wider than the body and
/// tapers in through curved shoulders, ending in a rounded bottom.
fn notch_points(w: i32, h: i32, body_w: i32, shoulder: i32, radius: i32) -> Vec<POINT> {
    let left = (w - body_w) / 2;
    let right = left + body_w;
    let r = radius.min(h - shoulder).min(body_w / 2).max(1);
    let steps = 12;
    let mut pts: Vec<POINT> = Vec::with_capacity(steps * 4 + 6);
    pts.push(POINT { x: 0, y: 0 });

    // Left shoulder.
    let ctrl = left * 3 / 7;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let mt = 1.0 - t;
        pts.push(POINT {
            x: (2.0 * mt * t * ctrl as f32 + t * t * left as f32).round() as i32,
            y: (t * t * shoulder as f32).round() as i32,
        });
    }

    pts.push(POINT { x: left, y: h - r });
    // Bottom-left corner.
    for i in 1..=steps {
        let a = std::f32::consts::PI
            - (i as f32) * (std::f32::consts::FRAC_PI_2 / steps as f32);
        pts.push(POINT {
            x: left + r + (r as f32 * a.cos()).round() as i32,
            y: h - r + (r as f32 * a.sin()).round() as i32,
        });
    }
    // Bottom edge and bottom-right corner.
    for i in 1..=steps {
        let a = std::f32::consts::FRAC_PI_2
            - (i as f32) * (std::f32::consts::FRAC_PI_2 / steps as f32);
        pts.push(POINT {
            x: right - r + (r as f32 * a.cos()).round() as i32,
            y: h - r + (r as f32 * a.sin()).round() as i32,
        });
    }
    pts.push(POINT { x: right, y: shoulder });
    // Right shoulder, mirrored from the left.
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let mt = 1.0 - t;
        pts.push(POINT {
            x: (mt * mt * right as f32 + 2.0 * mt * t * (w - ctrl) as f32 + t * t * w as f32)
                .round() as i32,
            y: (mt * mt * shoulder as f32).round() as i32,
        });
    }
    pts
}

unsafe fn apply_region(hwnd: HWND, w: i32, h: i32, body_w: i32, shoulder: i32, radius: i32) {
    let pts = notch_points(w, h, body_w, shoulder, radius);
    let rgn = CreatePolygonRgn(&pts, ALTERNATE);
    if !rgn.is_invalid() {
        SetWindowRgn(hwnd, rgn, true);
    }
}

// ------------------------------------------------------------------ colors

struct Palette {
    bg: u32,
    text: u32,
    dim: u32,
    sub: u32,
    faint: u32,
    accent: u32,
    accent_hi: u32,
    accent_fg: u32,
    hover: u32,
    chev: u32,
    art: u32,
    track: u32,
    media_track: u32,
}

fn palette() -> Palette {
    Palette {
        bg: 0x00000000,
        text: 0x00FFFFFF,
        dim: 0x00938E8E,   // #8E8E93
        sub: 0x00595959,   // #595959 (Nusa's secondary text)
        faint: 0x003C3A3A, // #3A3A3C
        accent: 0x00EEE9E9,    // #E9E9EE silver
        accent_hi: 0x00FFFFFF, // #FFFFFF
        accent_fg: 0x000D0B0B, // near-black glyphs on silver
        hover: 0x004A4A4E,
        chev: 0x00474747,   // #474747 (Nusa's chevron disc)
        art: 0x001E1C1C,   // #1C1C1E
        track: 0x00303030,
        media_track: 0x001B1B1B,
    }
}

// ------------------------------------------------------------ text helpers

unsafe fn make_font(size: i32, weight: u32) -> HFONT {
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
        ANTIALIASED_QUALITY.0 as u32,
        FF_DONTCARE.0 as u32,
        PCWSTR(face.as_ptr()),
    )
}

unsafe fn draw_text(
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

unsafe fn text_width(hdc: HDC, font: HFONT, s: &str) -> i32 {
    if s.is_empty() {
        return 0;
    }
    let v: Vec<u16> = s.encode_utf16().collect();
    let old = SelectObject(hdc, HGDIOBJ(font.0));
    let mut sz = SIZE::default();
    let _ = GetTextExtentPoint32W(hdc, &v, &mut sz);
    SelectObject(hdc, old);
    sz.cx
}

// ---------------------------------------------------------- shape helpers

unsafe fn fill(hdc: HDC, l: i32, t: i32, r: i32, b: i32, color: u32) {
    let brush = CreateSolidBrush(COLORREF(color));
    let rc = RECT { left: l, top: t, right: r, bottom: b };
    FillRect(hdc, &rc, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

unsafe fn ring(hdc: HDC, cx: i32, cy: i32, radius: i32, color: u32, width: i32) {
    let pen = CreatePen(PS_SOLID, width, COLORREF(color));
    let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    let _ = Ellipse(hdc, cx - radius, cy - radius, cx + radius, cy + radius);
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

unsafe fn filled_poly(hdc: HDC, pts: &[POINT], color: u32) {
    let brush = CreateSolidBrush(COLORREF(color));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(color));
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = Polygon(hdc, pts);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

unsafe fn circle(hdc: HDC, cx: i32, cy: i32, r: i32, color: u32) {
    let brush = CreateSolidBrush(COLORREF(color));
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = Ellipse(hdc, cx - r, cy - r, cx + r, cy + r);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

unsafe fn line(hdc: HDC, x1: i32, y1: i32, x2: i32, y2: i32, color: u32, width: i32) {
    let pen = CreatePen(PS_SOLID, width, COLORREF(color));
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    MoveToEx(hdc, x1, y1, None);
    let _ = LineTo(hdc, x2, y2);
    SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

unsafe fn sparkle_mark(hdc: HDC, cx: i32, cy: i32, color: u32) {
    let pts = [
        POINT { x: cx, y: cy - 4 },
        POINT { x: cx + 1, y: cy - 1 },
        POINT { x: cx + 2, y: cy },
        POINT { x: cx + 1, y: cy + 1 },
        POINT { x: cx, y: cy + 3 },
        POINT { x: cx - 1, y: cy + 1 },
        POINT { x: cx - 2, y: cy },
        POINT { x: cx - 1, y: cy - 1 },
    ];
    filled_poly(hdc, &pts, color);
}

unsafe fn bolt_mark(hdc: HDC, cx: i32, cy: i32, color: u32) {
    let pts = [
        POINT { x: cx + 2, y: cy - 5 },
        POINT { x: cx - 2, y: cy },
        POINT { x: cx, y: cy },
        POINT { x: cx - 1, y: cy + 5 },
        POINT { x: cx + 2, y: cy + 1 },
        POINT { x: cx, y: cy + 1 },
    ];
    filled_poly(hdc, &pts, color);
}

/// Draw `frac` of a ring clockwise from 12 o'clock. GDI's Arc direction is
/// awkward to reason about at this size, so the sweep is built from short
/// segments instead.
unsafe fn ring_arc(hdc: HDC, cx: i32, cy: i32, radius: i32, width: i32, color: u32, frac: f32) {
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 0.0 {
        return;
    }
    if frac >= 0.999 {
        ring(hdc, cx, cy, radius, color, width);
        return;
    }
    let pen = CreatePen(PS_SOLID, width, COLORREF(color));
    let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
    let steps = 72;
    let filled = (frac * steps as f32).round() as i32;
    let r = radius as f32;
    for i in 0..filled {
        let a0 = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / steps as f32;
        let a1 = -std::f32::consts::FRAC_PI_2
            + (i + 1) as f32 * std::f32::consts::TAU / steps as f32;
        let _ = MoveToEx(
            hdc,
            cx + (r * a0.cos()).round() as i32,
            cy + (r * a0.sin()).round() as i32,
            None,
        );
        let _ = LineTo(
            hdc,
            cx + (r * a1.cos()).round() as i32,
            cy + (r * a1.sin()).round() as i32,
        );
    }
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

#[derive(Clone, Copy)]
struct Battery {
    percent: Option<u8>,
    charging: bool,
    ac: bool,
}

fn battery_status() -> Battery {
    let mut s = SYSTEM_POWER_STATUS::default();
    let mut b = Battery { percent: None, charging: false, ac: false };
    if unsafe { GetSystemPowerStatus(&mut s) }.is_err() {
        return b;
    }
    b.ac = s.ACLineStatus == 1;
    b.charging = s.BatteryFlag & 0x08 != 0;
    if s.BatteryFlag & 0x80 == 0 && s.BatteryLifePercent <= 100 {
        b.percent = Some(s.BatteryLifePercent);
    }
    b
}

/// The header badge: a battery gauge that fills with the charge level. While
/// charging, a bolt replaces the sparkle; desktops without a battery keep the
/// plain mark and show `AC`.
unsafe fn battery_badge(hdc: HDC, cx: i32, cy: i32, p: &Palette, b: Battery) {
    let frac = b.percent.map(|v| v as f32 / 100.0);
    if let Some(frac) = frac {
        ring(hdc, cx, cy, 8, p.faint, 3);
        ring_arc(hdc, cx, cy, 8, 3, p.accent, frac);
    } else {
        ring(hdc, cx, cy, 8, p.dim, 3);
    }
    if b.charging {
        bolt_mark(hdc, cx, cy, p.text);
    } else {
        sparkle_mark(hdc, cx, cy, p.text);
    }
}

// ------------------------------------------------------------------ mascot

fn system_idle_ms() -> u32 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            GetTickCount().wrapping_sub(info.dwTime)
        } else {
            0
        }
    }
}

fn current_wave() -> Option<MascotWave> {
    MASCOT_WAVE
        .lock()
        .ok()
        .and_then(|w| *w)
        .filter(|w| Instant::now() < w.until)
}

fn start_wave(hwnd: HWND, ms: u64) {
    let now = Instant::now();
    if let Ok(mut w) = MASCOT_WAVE.lock() {
        *w = Some(MascotWave {
            start: now,
            until: now + Duration::from_millis(ms),
        });
    }
    let was_dozing = MASCOT_DOZING.swap(false, Ordering::Relaxed);
    unsafe {
        if was_dozing && !EXPANDED.load(Ordering::Relaxed) {
            layout(hwnd, false);
        }
        SetTimer(hwnd, 4, 55, None);
        let _ = InvalidateRect(hwnd, None, false);
    }
}

/// The mascot sprite is rasterized from signed-distance shapes with a soft
/// bloom, then alpha-blended as one piece. Drawing it with 1px GDI primitives
/// made the ears and eyes read as hard, separate blobs at this size.
const MASCOT_W: i32 = 30;
const MASCOT_PAD: i32 = 13;
const MASCOT_SPRITE_W: i32 = MASCOT_W + MASCOT_PAD * 2;
const MASCOT_H: i32 = 30;
const MASCOT_GAP: i32 = 8;

fn sd_ellipse(px: f32, py: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> f32 {
    let nx = (px - cx) / rx;
    let ny = (py - cy) / ry;
    ((nx * nx + ny * ny).sqrt() - 1.0) * rx.min(ry)
}

fn sd_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, r: f32) -> f32 {
    let vx = bx - ax;
    let vy = by - ay;
    let wx = px - ax;
    let wy = py - ay;
    let t = ((wx * vx + wy * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    let dx = wx - t * vx;
    let dy = wy - t * vy;
    (dx * dx + dy * dy).sqrt() - r
}

/// Signed distance to a triangle, used for the ears and nose. The inside test
/// is done with three cross products so points beyond a vertex cannot flip
/// the sign the way a nearest-edge-only formula can.
fn sd_triangle(
    px: f32,
    py: f32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    cx: f32,
    cy: f32,
) -> f32 {
    let cross = |ux: f32, uy: f32, vx: f32, vy: f32| ux * vy - uy * vx;
    let c0 = cross(bx - ax, by - ay, px - ax, py - ay);
    let c1 = cross(cx - bx, cy - by, px - bx, py - by);
    let c2 = cross(ax - cx, ay - cy, px - cx, py - cy);
    let inside = (c0 >= 0.0 && c1 >= 0.0 && c2 >= 0.0)
        || (c0 <= 0.0 && c1 <= 0.0 && c2 <= 0.0);
    let d = sd_segment(px, py, ax, ay, bx, by, 0.0)
        .min(sd_segment(px, py, bx, by, cx, cy, 0.0))
        .min(sd_segment(px, py, cx, cy, ax, ay, 0.0));
    if inside {
        -d
    } else {
        d
    }
}

/// Signed distance to a rounded rectangle with a different radius per corner.
/// The head is built from two of these so the crown is narrower than the
/// cheeks, the way the app icon's head flares below the ears.
fn sd_rounded_box(
    px: f32,
    py: f32,
    cx: f32,
    cy: f32,
    hx: f32,
    hy: f32,
    r_tl: f32,
    r_tr: f32,
    r_br: f32,
    r_bl: f32,
) -> f32 {
    let r = if px < cx {
        if py < cy {
            r_tl
        } else {
            r_bl
        }
    } else if py < cy {
        r_tr
    } else {
        r_br
    };
    let qx = (px - cx).abs() - hx + r;
    let qy = (py - cy).abs() - hy + r;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// The lower half of the head is a superellipse, which gives the icon's
/// broad, softly squared chin instead of an egg-shaped taper.
fn sd_superellipse(
    px: f32,
    py: f32,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    n: f32,
) -> f32 {
    let nx = ((px - cx) / rx).abs();
    let ny = ((py - cy) / ry).abs();
    ((nx.powf(n) + ny.powf(n)).powf(1.0 / n) - 1.0) * rx.min(ry)
}

/// A vertical capsule: a rounded box whose width is the cap diameter.
fn sd_capsule(px: f32, py: f32, cx: f32, cy: f32, half_h: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - r;
    let qy = (py - cy).abs() - half_h;
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0)
}

/// A closed eye: a smooth arc stroked with a thin black line, used for the
/// wink during the greeting wave.
fn cat_eye_arc(px: f32, py: f32, cx: f32, cy: f32, up: bool) -> f32 {
    let mut d = f32::MAX;
    let mut prev = (cx - 2.2, cy);
    let control_y = if up { cy - 1.5 } else { cy + 1.2 };
    for i in 1..=10 {
        let t = i as f32 / 10.0;
        let mt = 1.0 - t;
        let x = mt * mt * (cx - 2.2) + 2.0 * mt * t * cx + t * t * (cx + 2.2);
        let y = mt * mt * cy + 2.0 * mt * t * control_y + t * t * cy;
        d = d.min(sd_segment(px, py, prev.0, prev.1, x, y, 0.55));
        prev = (x, y);
    }
    d
}

/// The app-icon cat head: a narrower crown, a wider cheek band, a broad
/// superellipse chin, two big rounded ears, and nothing else. No inner ears,
/// nose, mouth, or whiskers, matching `assets\neno.ico`.
fn cat_body_distance(px: f32, py: f32, bob: f32) -> f32 {
    let y = py + bob;
    let mut d = sd_rounded_box(px, y, 28.0, 10.0, 12.0, 2.0, 3.0, 3.0, 0.0, 0.0);
    d = d.min(sd_rounded_box(
        px, y, 28.0, 16.0, 13.0, 4.0, 0.0, 0.0, 0.0, 0.0,
    ));
    d = d.min(sd_superellipse(px, y, 28.0, 18.0, 11.8, 6.8, 3.0));

    // Each ear is a concave-sided quad, built from two triangles plus a
    // small round tip. The concave inner edge is what makes it read as the
    // icon's broad, soft ear rather than a thin devil horn.
    for ((tip_x, tip_y), (ax, ay), (mx, my), (bx, by)) in [
        ((18.0, 3.6), (16.8, 8.0), (21.0, 6.2), (26.0, 7.7)),
        ((38.0, 3.6), (39.2, 8.0), (35.0, 6.2), (30.0, 7.7)),
    ] {
        d = d.min(sd_triangle(px, y, tip_x, tip_y, ax, ay, mx, my));
        d = d.min(sd_triangle(px, y, mx, my, ax, ay, bx, by));
        d = d.min(sd_ellipse(px, y, tip_x, tip_y, 1.0, 1.0));
    }
    d
}

/// The two black capsule eyes from the icon. They are the only cut-outs:
/// they look toward the pointer while hovered, become a wink during the
/// greeting wave, and stay as the icon's open capsules while dozing.
fn cat_feature_distance(px: f32, py: f32, mood: MascotMood, look: (f32, f32)) -> f32 {
    let lx = look.0;
    let ly = look.1;
    let open = |cx: f32, cy: f32| sd_capsule(px, py, cx, cy, 3.6, 1.35);
    match mood {
        MascotMood::Open | MascotMood::Dozing => {
            open(23.5 + lx, 14.3 + ly).min(open(32.5 + lx, 14.3 + ly))
        }
        MascotMood::Waving => open(23.5 + lx, 14.3 + ly)
            .min(cat_eye_arc(px, py, 32.5 + lx, 14.3 + ly, true)),
    }
}

/// Distance to the cat silhouette and, separately, to the black face
/// cut-outs. `look` moves the eyes toward the pointer; the wave adds a
/// small bob to the whole head.
fn mascot_parts(
    px: f32,
    py: f32,
    mood: MascotMood,
    phase: usize,
    look: (f32, f32),
) -> (f32, f32) {
    let bob = match mood {
        MascotMood::Waving => [0.0, 0.4, 0.9, 0.4][phase & 3],
        _ => 0.0,
    };
    (
        cat_body_distance(px, py, bob),
        cat_feature_distance(px, py + bob, mood, look),
    )
}

fn smooth_edge(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// A gaussian falloff outside the shape, used for the bloom without a blur
/// kernel: the glow follows the silhouette instead of turning into a box.
fn bloom_falloff(d: f32, sigma: f32) -> f32 {
    if d <= 0.0 {
        1.0
    } else {
        (-(d * d) / (2.0 * sigma * sigma)).exp()
    }
}

fn bloom_amount(d: f32, glow: f32) -> f32 {
    (bloom_falloff(d, 1.5) * 0.55 + bloom_falloff(d, 3.0) * 0.45) * glow
}

#[derive(Clone, Copy)]
enum MascotMood {
    Open,
    Dozing,
    Waving,
}

unsafe fn draw_mascot(
    hdc: HDC,
    x: i32,
    color: u32,
    mood: MascotMood,
    phase: usize,
    glow: f32,
    look: (f32, f32),
) {
    let w = MASCOT_SPRITE_W as usize;
    let h = MASCOT_H as usize;
    let r = color & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = (color >> 16) & 0xFF;
    let mut sprite = vec![0u32; w * h];
    for py in 0..h {
        for px in 0..w {
            let (body_d, feature_d) =
                mascot_parts(px as f32 + 0.5, py as f32 + 0.5, mood, phase, look);
            let body = smooth_edge(body_d);
            let core = body * (1.0 - smooth_edge(feature_d));
            let bloom = bloom_amount(body_d, glow);
            // Fade the bloom into the transparent padding so the sprite's
            // rectangular bounds never show up as a seam on the glass.
            let edge = ((px as f32 + 0.5) / 4.0)
                .min((w as f32 - 0.5 - px as f32) / 4.0)
                .clamp(0.0, 1.0);
            let out = (core + bloom * edge * (1.0 - body)).clamp(0.0, 1.0);
            let ai = (out * 255.0).round() as u32;
            sprite[py * w + px] = (ai << 24)
                | ((r * ai / 255) << 16)
                | ((g * ai / 255) << 8)
                | (b * ai / 255);
        }
    }

    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = MASCOT_SPRITE_W;
    bmi.bmiHeader.biHeight = -MASCOT_H;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB.0;
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let Ok(dib) = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
        return;
    };
    let mem = CreateCompatibleDC(hdc);
    let old = SelectObject(mem, HGDIOBJ(dib.0));
    std::ptr::copy_nonoverlapping(sprite.as_ptr(), bits as *mut u32, sprite.len());
    let bf = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let _ = AlphaBlend(
        hdc,
        x - MASCOT_PAD,
        0,
        MASCOT_SPRITE_W,
        MASCOT_H,
        mem,
        0,
        0,
        MASCOT_SPRITE_W,
        MASCOT_H,
        bf,
    );
    SelectObject(mem, old);
    let _ = DeleteObject(HGDIOBJ(dib.0));
    let _ = DeleteDC(mem);
}

unsafe fn play_triangle(hdc: HDC, cx: i32, cy: i32, color: u32) {
    let pts = [
        POINT { x: cx - 5, y: cy - 7 },
        POINT { x: cx - 5, y: cy + 7 },
        POINT { x: cx + 7, y: cy },
    ];
    filled_poly(hdc, &pts, color);
}

unsafe fn pause_bars(hdc: HDC, cx: i32, cy: i32, color: u32) {
    fill(hdc, cx - 5, cy - 7, cx - 2, cy + 7, color);
    fill(hdc, cx + 2, cy - 7, cx + 5, cy + 7, color);
}

unsafe fn skip_icon(hdc: HDC, cx: i32, cy: i32, color: u32, dir: i32) {
    // dir = -1 rewind, +1 forward: Nusa uses two filled triangles.
    let (top, bottom) = (cy - 6, cy + 5);
    if dir < 0 {
        filled_poly(
            hdc,
            &[
                POINT { x: cx - 8, y: cy },
                POINT { x: cx - 1, y: top },
                POINT { x: cx - 1, y: bottom },
            ],
            color,
        );
        filled_poly(
            hdc,
            &[
                POINT { x: cx + 1, y: cy },
                POINT { x: cx + 8, y: top },
                POINT { x: cx + 8, y: bottom },
            ],
            color,
        );
    } else {
        filled_poly(
            hdc,
            &[
                POINT { x: cx - 8, y: top },
                POINT { x: cx - 8, y: bottom },
                POINT { x: cx - 1, y: cy },
            ],
            color,
        );
        filled_poly(
            hdc,
            &[
                POINT { x: cx + 1, y: top },
                POINT { x: cx + 1, y: bottom },
                POINT { x: cx + 8, y: cy },
            ],
            color,
        );
    }
}

unsafe fn chevron(hdc: HDC, x: i32, y: i32, dir: i32, color: u32) {
    let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    MoveToEx(hdc, x - 2 * dir, y - 5, None);
    let _ = LineTo(hdc, x + 2 * dir, y);
    let _ = LineTo(hdc, x - 2 * dir, y + 5);
    SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(pen.0));
}

unsafe fn button(
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
    let brush = CreateSolidBrush(COLORREF(bg));
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = RoundRect(hdc, l, t, l + w, t + h, 12, 12);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    draw_text(
        hdc,
        font,
        fg,
        label,
        RECT { left: l, top: t, right: l + w, bottom: t + h },
        DT_CENTER,
    );
}

unsafe fn progress_bar(hdc: HDC, l: i32, t: i32, r: i32, pct: f32, fg: u32, bg: u32) {
    progress_bar_h(hdc, l, t, r, pct, fg, bg, 5);
}

unsafe fn progress_bar_h(
    hdc: HDC,
    l: i32,
    t: i32,
    r: i32,
    pct: f32,
    fg: u32,
    bg: u32,
    height: i32,
) {
    let filled_w = ((r - l) as f32 * pct.clamp(0.0, 1.0)) as i32;
    if height <= 4 {
        // GDI's RoundRect collapses a 4px-tall pill to three scanlines, so
        // build the capsule from fills to keep Nusa's four-row track.
        capsule_h(hdc, l, t, r, height, bg);
        if filled_w > 2 {
            capsule_h(hdc, l, t, l + filled_w.max(2), height, fg);
        }
        return;
    }
    let radius = (height / 2).max(1);
    let track = CreateSolidBrush(COLORREF(bg));
    let old_brush = SelectObject(hdc, HGDIOBJ(track.0));
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = RoundRect(hdc, l, t, r, t + height, radius, radius);
    if filled_w > 2 {
        let fill_brush = CreateSolidBrush(COLORREF(fg));
        SelectObject(hdc, HGDIOBJ(fill_brush.0));
        let _ = RoundRect(hdc, l, t, l + filled_w.max(radius), t + height, radius, radius);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(fill_brush.0));
    } else {
        SelectObject(hdc, old_brush);
    }
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(HGDIOBJ(track.0));
}

/// Small horizontal capsule drawn as scanlines (see progress_bar_h).
unsafe fn capsule_h(hdc: HDC, l: i32, t: i32, r: i32, h: i32, color: u32) {
    let inset = (h / 2).max(1);
    fill(hdc, l + inset, t, r - inset, t + h, color);
    fill(hdc, l, t + 1, r, t + h - 1, color);
}

// ---------------------------------------------------------------- strings

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn now_parts() -> (String, String) {
    let st = unsafe { GetLocalTime() };
    let clock = format!("{:02}:{:02}", st.wHour, st.wMinute);
    let date = format!(
        "{} {} {} {}",
        WEEKDAYS[(st.wDayOfWeek as usize).min(6)],
        st.wDay,
        MONTHS[(st.wMonth as usize).saturating_sub(1).min(11)],
        st.wYear
    );
    (clock, date)
}

fn mmss(total: usize) -> String {
    format!("{:02}:{:02}", total / 60, total % 60)
}

// ------------------------------------------------------------------ paint

unsafe fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let p = palette();
    let expanded = EXPANDED.load(Ordering::Relaxed);

    if !expanded {
        let dozing =
            MASCOT_DOZING.load(Ordering::Relaxed) && !MASCOT_HOVER.load(Ordering::Relaxed);
        let (w, body_w, shoulder, radius) = if dozing {
            (DOZE_W, DOZE_BODY_W, DOZE_SHOULDER, DOZE_RADIUS)
        } else {
            (
                COLLAPSED_W,
                COLLAPSED_BODY_W,
                COLLAPSED_SHOULDER,
                COLLAPSED_RADIUS,
            )
        };
        let pts = notch_points(w, COLLAPSED_H, body_w, shoulder, radius);
        filled_poly(hdc, &pts, p.bg);
        let (clock, _) = now_parts();
        let f_clock = make_font(17, FW_SEMIBOLD.0);
        let wave = current_wave();
        let mood = if wave.is_some() {
            MascotMood::Waving
        } else if dozing {
            MascotMood::Dozing
        } else {
            MascotMood::Open
        };
        let phase = wave
            .map(|w| ((w.start.elapsed().as_millis() / 160) % 4) as usize)
            .unwrap_or(0);
        let look = MASCOT_LOOK
            .lock()
            .map(|l| (l.0 as f32 / 10.0, l.1 as f32 / 10.0))
            .unwrap_or((0.0, 0.0));
        if dozing {
            // After a minute of quiet the pill shrinks to just the sleeping
            // cat face, centered in the small pill.
            let left = (w - MASCOT_W) / 2;
            MASCOT_CX.store(w / 2, Ordering::Relaxed);
            draw_mascot(hdc, left, p.text, mood, 0, 0.0, (0.0, 0.0));
        } else {
            // Center the mascot and clock as one group so the pill does not
            // look left-heavy. The mascot sprite includes room for the bloom.
            let text_w = text_width(hdc, f_clock, &clock);
            let group_w = MASCOT_W + MASCOT_GAP + text_w;
            let left = ((w - group_w) / 2).max(6);
            MASCOT_CX.store(left + MASCOT_W / 2, Ordering::Relaxed);
            draw_mascot(hdc, left, p.text, mood, phase, 0.0, look);
            draw_text(
                hdc,
                f_clock,
                p.text,
                &clock,
                RECT {
                    left: left + MASCOT_W + MASCOT_GAP,
                    top: 0,
                    right: w,
                    bottom: COLLAPSED_H,
                },
                DT_LEFT,
            );
        }
        let _ = DeleteObject(HGDIOBJ(f_clock.0));
        let _ = EndPaint(hwnd, &ps);
        return;
    }

    let pts = notch_points(
        EXPANDED_WINDOW_W,
        EXPANDED_H,
        EXPANDED_W,
        EXPANDED_SHOULDER,
        EXPANDED_RADIUS,
    );
    filled_poly(hdc, &pts, p.bg);
    let _ = SetViewportOrgEx(hdc, EXPANDED_SHOULDER, 0, None);
    let f_date = make_font(13, FW_NORMAL.0);
    let f_time = make_font(14, FW_BOLD.0);
    let f_title = make_font(15, FW_SEMIBOLD.0);
    let f_small = make_font(11, FW_NORMAL.0);
    let f_tiny = make_font(10, FW_NORMAL.0);
    let f_label = make_font(17, FW_NORMAL.0);
    let f_big = make_font(34, FW_SEMIBOLD.0);
    let f_mid = make_font(24, FW_SEMIBOLD.0);
    let f_btn = make_font(11, FW_SEMIBOLD.0);

    let (clock, date) = now_parts();
    let page = PAGE.load(Ordering::Relaxed);

    match page {
        PAGE_MEDIA => paint_media(
            hdc, &p, f_title, f_small, f_label, f_date, f_time, &clock, &date,
        ),
        PAGE_FOCUS => paint_focus(hdc, &p, f_big, f_date, f_time, f_small, f_btn, &clock, &date),
        PAGE_WATER => paint_water(hdc, &p, f_mid, f_date, f_time, f_small, f_btn, &clock, &date),
        PAGE_TODO => paint_todo(hdc, &p, f_title, f_date, f_time, f_small, f_tiny, &clock, &date),
        _ => paint_shelf(hdc, &p, f_date, f_time, f_title, f_small, f_tiny, &clock, &date),
    }

    paint_dots(hdc, &p, page);
    let left_hot = hover_is(HOVER_CHEV_LEFT);
    circle(hdc, 22, 70, 8, if left_hot { p.sub } else { p.chev });
    chevron(hdc, 22, 70, -1, p.text);
    let right_hot = hover_is(HOVER_CHEV_RIGHT);
    circle(hdc, EXPANDED_W - 25, 70, 8, if right_hot { p.sub } else { p.chev });
    chevron(hdc, EXPANDED_W - 25, 70, 1, p.text);

    let _ = SetViewportOrgEx(hdc, 0, 0, None);
    for f in [f_date, f_time, f_title, f_small, f_tiny, f_label, f_big, f_mid, f_btn] {
        let _ = DeleteObject(HGDIOBJ(f.0));
    }
    let _ = EndPaint(hwnd, &ps);
}

unsafe fn paint_header(
    hdc: HDC,
    p: &Palette,
    f_date: HFONT,
    f_time: HFONT,
    f_meta: HFONT,
    clock: &str,
    date: &str,
) {
    let battery = battery_status();
    let weather = WEATHER.lock().map(|w| w.clone()).unwrap_or_default();
    let batt_text = match (battery.percent, battery.ac) {
        (Some(pct), _) => format!("{}%", pct),
        (None, true) => "AC".to_string(),
        _ => String::new(),
    };
    let date_w = text_width(hdc, f_date, date);
    let time_w = text_width(hdc, f_time, clock);
    let weather_w = if weather.is_empty() {
        0
    } else {
        text_width(hdc, f_meta, &weather) + 8
    };
    let batt_w = if batt_text.is_empty() {
        0
    } else {
        text_width(hdc, f_meta, &batt_text) + 8
    };
    let total = 18 + 7 + weather_w + date_w + 8 + time_w + batt_w;
    // The album-art tile owns the left edge of every page, so centre the
    // header in the space to its right instead of the full card.
    let left = 112;
    let right = EXPANDED_W - 14;
    let start = left + ((right - left) - total).max(0) / 2;
    battery_badge(hdc, start + 11, 18, p, battery);
    let mut x = start + 28;
    if !weather.is_empty() {
        draw_text(
            hdc,
            f_meta,
            p.dim,
            &weather,
            RECT { left: x, top: 6, right: x + weather_w, bottom: 30 },
            DT_LEFT,
        );
        x += weather_w;
    }
    draw_text(
        hdc,
        f_date,
        p.dim,
        date,
        RECT { left: x, top: 6, right: x + date_w + 20, bottom: 30 },
        DT_LEFT,
    );
    x += date_w + 8;
    draw_text(
        hdc,
        f_time,
        p.text,
        clock,
        RECT { left: x, top: 6, right: x + time_w + 20, bottom: 30 },
        DT_LEFT,
    );
    x += time_w + 8;
    if !batt_text.is_empty() {
        draw_text(
            hdc,
            f_meta,
            p.dim,
            &batt_text,
            RECT { left: x, top: 6, right: x + batt_w, bottom: 30 },
            DT_LEFT,
        );
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_media(
    hdc: HDC,
    p: &Palette,
    f_title: HFONT,
    f_small: HFONT,
    f_label: HFONT,
    f_date: HFONT,
    f_time: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, f_small, clock, date);

    // Album art placeholder, sized and placed like Nusa's 62px tile.
    let art = CreateSolidBrush(COLORREF(p.art));
    let art_pen = CreatePen(PS_SOLID, 1, COLORREF(p.art));
    let old_brush = SelectObject(hdc, HGDIOBJ(art.0));
    let old_pen = SelectObject(hdc, HGDIOBJ(art_pen.0));
    RoundRect(hdc, 44, 12, 107, 75, 14, 14);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(art_pen.0));
    let _ = DeleteObject(HGDIOBJ(art.0));

    // Beamed music note, traced from Nusa's tile glyph.
    let glyph = 0x00707070;
    fill(hdc, 71, 36, 73, 53, glyph);
    fill(hdc, 84, 31, 86, 49, glyph);
    let beam = [
        POINT { x: 85, y: 31 },
        POINT { x: 85, y: 36 },
        POINT { x: 71, y: 37 },
        POINT { x: 71, y: 34 },
    ];
    filled_poly(hdc, &beam, glyph);
    let head = CreateSolidBrush(COLORREF(glyph));
    let old_head = SelectObject(hdc, HGDIOBJ(head.0));
    let head_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = Ellipse(hdc, 64, 48, 73, 57);
    let _ = Ellipse(hdc, 77, 46, 86, 54);
    SelectObject(hdc, head_pen);
    SelectObject(hdc, old_head);
    let _ = DeleteObject(HGDIOBJ(head.0));

    let media = MEDIA_TITLE.lock().map(|m| m.clone()).unwrap_or_default();
    let (title, artist) = match media.split_once(" \u{2014} ") {
        Some((t, a)) => (t.to_string(), a.to_string()),
        None => (media.clone(), String::new()),
    };
    let (title, artist) = if title.is_empty() {
        ("No media".to_string(), String::new())
    } else {
        (title, artist)
    };

    draw_text(
        hdc,
        f_title,
        p.text,
        &title,
        RECT { left: 122, top: 35, right: EXPANDED_W - 44, bottom: 59 },
        DT_LEFT | DT_END_ELLIPSIS,
    );
    let lyric = LYRICS.lock().map(|l| l.clone()).unwrap_or_default();
    let subtitle = if lyric.is_empty() { artist } else { format!("\u{266A}  {}", lyric) };
    if !subtitle.is_empty() {
        draw_text(
            hdc,
            f_small,
            p.dim,
            &subtitle,
            RECT { left: 122, top: 58, right: EXPANDED_W - 44, bottom: 74 },
            DT_LEFT | DT_END_ELLIPSIS,
        );
    }

    // Progress row: time labels at the edges, a thin track between them.
    let pos = PLAY_POS.load(Ordering::Relaxed);
    let dur = PLAY_LEN.load(Ordering::Relaxed);
    let has = dur > 0;
    let pct = if has { pos as f32 / dur as f32 } else { 0.0 };
    draw_text(
        hdc,
        f_label,
        p.sub,
        &mmss(if has { pos } else { 0 }),
        RECT { left: 44, top: 80, right: 96, bottom: 104 },
        DT_LEFT,
    );
    progress_bar_h(hdc, 96, 89, 318, pct, p.text, p.media_track, 4);
    draw_text(
        hdc,
        f_label,
        p.sub,
        &format!("-{}", mmss(if has { dur.saturating_sub(pos) } else { 0 })),
        RECT { left: 320, top: 80, right: EXPANDED_W - 44, bottom: 104 },
        DT_RIGHT,
    );

    // Controls.
    let cy = 118;
    let enabled = !media.is_empty();
    if hover_is(HOVER_MEDIA_PREV) {
        circle(hdc, 162, cy, 14, p.faint);
    }
    if hover_is(HOVER_MEDIA_PLAY) {
        circle(hdc, 210, cy, 14, p.faint);
    }
    if hover_is(HOVER_MEDIA_NEXT) {
        circle(hdc, 256, cy, 14, p.faint);
    }
    let control = if enabled { p.text } else { p.sub };
    skip_icon(hdc, 162, cy, control, -1);
    if PLAYING.load(Ordering::Relaxed) {
        pause_bars(hdc, 210, cy, control);
    } else {
        play_triangle(hdc, 210, cy, control);
    }
    skip_icon(hdc, 256, cy, control, 1);
    // Lyrics bubble on the left, outlined like the original.
    let pen = CreatePen(PS_SOLID, 1, COLORREF(p.chev));
    let ob = SelectObject(hdc, HGDIOBJ(pen.0));
    let op = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    let _ = RoundRect(hdc, 66, 110, 84, 123, 8, 8);
    SelectObject(hdc, op);
    SelectObject(hdc, ob);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    fill(hdc, 71, 115, 74, 119, p.chev);
    fill(hdc, 76, 115, 79, 119, p.chev);
    let pts = [POINT { x: 69, y: 122 }, POINT { x: 70, y: 126 }, POINT { x: 74, y: 122 }];
    filled_poly(hdc, &pts, p.chev);
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_focus(
    hdc: HDC,
    p: &Palette,
    f_big: HFONT,
    f_date: HFONT,
    f_time: HFONT,
    f_small: HFONT,
    f_btn: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, f_small, clock, date);
    let (remaining, phase, running) = FOCUS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|f| {
            (
                f.remaining().as_secs() as usize,
                match f.phase {
                    crate::focus::Phase::Focus => "Focus",
                    crate::focus::Phase::ShortBreak => "Short break",
                    crate::focus::Phase::LongBreak => "Long break",
                },
                f.is_running(),
            )
        })
        .unwrap_or((25 * 60, "Focus", false));

    draw_text(
        hdc,
        f_big,
        p.text,
        &mmss(remaining),
        RECT { left: 0, top: 34, right: EXPANDED_W, bottom: 76 },
        DT_CENTER,
    );
    draw_text(
        hdc,
        f_small,
        p.dim,
        phase,
        RECT { left: 0, top: 76, right: EXPANDED_W, bottom: 92 },
        DT_CENTER,
    );

    let start_label = if running { "Pause" } else { "Start" };
    let start_bg = if hover_is(HOVER_FOCUS_START) { p.accent_hi } else { p.accent };
    let reset_bg = if hover_is(HOVER_FOCUS_RESET) { p.hover } else { p.faint };
    button(hdc, EXPANDED_W / 2 - 78, 98, 72, 22, start_bg, p.accent_fg, start_label, f_btn);
    button(hdc, EXPANDED_W / 2 + 8, 98, 70, 22, reset_bg, p.text, "Reset", f_btn);

    let _ = WEATHER.lock().map(|w| w.len());
}

unsafe fn paint_water(
    hdc: HDC,
    p: &Palette,
    f_mid: HFONT,
    f_date: HFONT,
    f_time: HFONT,
    f_small: HFONT,
    f_btn: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, f_small, clock, date);
    let w = crate::water::WaterReminder::load();
    let pct = w.progress();
    let label = format!("{:.2} / {:.1} L", w.consumed_ml as f32 / 1000.0, w.goal_liters);
    draw_text(
        hdc,
        f_mid,
        p.text,
        &label,
        RECT { left: 0, top: 40, right: EXPANDED_W, bottom: 70 },
        DT_CENTER,
    );
    progress_bar(hdc, 60, 76, EXPANDED_W - 60, pct, p.accent, p.track);
    draw_text(
        hdc,
        f_small,
        p.dim,
        &format!("{:.0}% of daily goal", pct * 100.0),
        RECT { left: 0, top: 82, right: EXPANDED_W, bottom: 96 },
        DT_CENTER,
    );
    let drank_bg = if hover_is(HOVER_WATER_DRANK) { p.accent_hi } else { p.accent };
    let snooze_bg = if hover_is(HOVER_WATER_SNOOZE) { p.hover } else { p.faint };
    let reset_bg = if hover_is(HOVER_WATER_RESET) { p.hover } else { p.faint };
    button(hdc, EXPANDED_W / 2 - 112, 100, 78, 22, drank_bg, p.accent_fg, "Drank +250", f_btn);
    button(hdc, EXPANDED_W / 2 - 26, 100, 60, 22, snooze_bg, p.text, "Snooze", f_btn);
    button(hdc, EXPANDED_W / 2 + 42, 100, 60, 22, reset_bg, p.text, "Reset", f_btn);
}

unsafe fn paint_todo(
    hdc: HDC,
    p: &Palette,
    f_title: HFONT,
    f_date: HFONT,
    f_time: HFONT,
    f_small: HFONT,
    f_tiny: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, f_tiny, clock, date);
    let list = crate::todo::TodoList::load();
    let open: Vec<_> = list.items.iter().filter(|i| !i.done).collect();

    // Task card uses the same language as the shelf drop zone.
    let hot = hover_is(HOVER_TODO_CARD);
    let (l, t, r, b) = (44, 38, EXPANDED_W - 44, 108);
    let brush = CreateSolidBrush(COLORREF(p.art));
    let pen = CreatePen(
        PS_SOLID,
        if hot { 2 } else { 1 },
        COLORREF(if hot { p.dim } else { p.faint }),
    );
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = RoundRect(hdc, l, t, r, b, 28, 28);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let _ = DeleteObject(HGDIOBJ(brush.0));

    if open.is_empty() {
        // Completed-task mark.
        let cx = EXPANDED_W / 2;
        let icon = CreatePen(PS_SOLID, 1, COLORREF(p.dim));
        let ob = SelectObject(hdc, HGDIOBJ(icon.0));
        let op = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let _ = Ellipse(hdc, cx - 7, 48, cx + 8, 63);
        SelectObject(hdc, op);
        SelectObject(hdc, ob);
        let _ = DeleteObject(HGDIOBJ(icon.0));
        line(hdc, cx - 4, 55, cx - 1, 58, p.dim, 1);
        line(hdc, cx - 1, 58, cx + 4, 52, p.dim, 1);

        let title = if list.items.is_empty() {
            "No tasks yet"
        } else {
            "All caught up"
        };
        draw_text(
            hdc,
            f_title,
            p.text,
            title,
            RECT { left: 0, top: 60, right: EXPANDED_W, bottom: 78 },
            DT_CENTER,
        );
        draw_text(
            hdc,
            f_small,
            p.dim,
            "Click to open the task panel",
            RECT { left: 0, top: 80, right: EXPANDED_W, bottom: 96 },
            DT_CENTER,
        );
    } else {
        for (i, item) in open.iter().take(3).enumerate() {
            let y = 50 + (i as i32) * 18;
            let bx = 60;
            let box_pen = CreatePen(PS_SOLID, 1, COLORREF(p.sub));
            let ob = SelectObject(hdc, HGDIOBJ(box_pen.0));
            let op = SelectObject(hdc, GetStockObject(NULL_BRUSH));
            let _ = RoundRect(hdc, bx, y, bx + 12, y + 12, 4, 4);
            SelectObject(hdc, op);
            SelectObject(hdc, ob);
            let _ = DeleteObject(HGDIOBJ(box_pen.0));
            draw_text(
                hdc,
                f_small,
                p.text,
                &item.text,
                RECT { left: bx + 20, top: y - 2, right: EXPANDED_W - 60, bottom: y + 14 },
                DT_LEFT | DT_END_ELLIPSIS,
            );
        }
        if open.len() > 3 {
            draw_text(
                hdc,
                f_tiny,
                p.dim,
                &format!("+{} more open", open.len() - 3),
                RECT {
                    left: EXPANDED_W - 180,
                    top: 108,
                    right: EXPANDED_W - 44,
                    bottom: 124,
                },
                DT_RIGHT,
            );
        }
    }
}

unsafe fn paint_shelf(
    hdc: HDC,
    p: &Palette,
    f_date: HFONT,
    f_time: HFONT,
    f_title: HFONT,
    f_small: HFONT,
    f_tiny: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, f_tiny, clock, date);

    // Drop zone: a soft card the user can throw files onto.
    let hot = hover_is(HOVER_SHELF_ZONE);
    let (l, t, r, b) = (44, 38, EXPANDED_W - 44, 108);
    let brush = CreateSolidBrush(COLORREF(p.art));
    let pen = CreatePen(PS_SOLID, if hot { 2 } else { 1 }, COLORREF(if hot { p.dim } else { p.faint }));
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = RoundRect(hdc, l, t, r, b, 28, 28);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let _ = DeleteObject(HGDIOBJ(brush.0));

    let shelf = crate::shelf::Shelf::new();
    let count = shelf.files.len();

    if count == 0 {
        // Folder glyph.
        let ix = l + 26;
        let iy = 62;
        let icon_color = if hot { p.text } else { p.dim };
        let icon = CreateSolidBrush(COLORREF(icon_color));
        let ob = SelectObject(hdc, HGDIOBJ(icon.0));
        let op = SelectObject(hdc, GetStockObject(NULL_PEN));
        let _ = RoundRect(hdc, ix, iy - 8, ix + 32, iy + 10, 8, 8);
        fill(hdc, ix + 2, iy - 12, ix + 13, iy - 6, icon_color);
        SelectObject(hdc, op);
        SelectObject(hdc, ob);
        let _ = DeleteObject(HGDIOBJ(icon.0));

        draw_text(
            hdc,
            f_title,
            p.text,
            "Drop files here",
            RECT { left: ix + 46, top: 48, right: r - 14, bottom: 72 },
            DT_LEFT | DT_END_ELLIPSIS,
        );
        draw_text(
            hdc,
            f_small,
            p.dim,
            "Files you drop are copied to your shelf",
            RECT { left: ix + 46, top: 72, right: r - 14, bottom: 92 },
            DT_LEFT | DT_END_ELLIPSIS,
        );
        return;
    }

    // Header with the count; the newest files are listed below it.
    let title = format!("{} file{} on the shelf", count, if count == 1 { "" } else { "s" });
    draw_text(
        hdc,
        f_small,
        p.text,
        &title,
        RECT { left: l + 14, top: 41, right: r - 92, bottom: 57 },
        DT_LEFT | DT_END_ELLIPSIS,
    );
    let hint = if count > 3 {
        format!("+{} more", count - 3)
    } else {
        "click to open".to_string()
    };
    draw_text(
        hdc,
        f_tiny,
        p.dim,
        &hint,
        RECT { left: r - 90, top: 41, right: r - 14, bottom: 57 },
        DT_RIGHT,
    );

    let shown = count.min(3);
    let row_h = 14;
    let start = 60 + (42 - shown as i32 * row_h) / 2;
    for (i, name) in shelf.files.iter().take(3).enumerate() {
        let y = start + i as i32 * row_h;
        file_glyph(hdc, l + 14, y + 1, p);
        let label = name.strip_suffix(".url").unwrap_or(name);
        draw_text(
            hdc,
            f_small,
            p.text,
            label,
            RECT { left: l + 32, top: y - 1, right: r - 14, bottom: y + 13 },
            DT_LEFT | DT_END_ELLIPSIS,
        );
    }
}

/// Small document glyph used for shelf rows.
unsafe fn file_glyph(hdc: HDC, x: i32, y: i32, p: &Palette) {
    let brush = CreateSolidBrush(COLORREF(p.art));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(p.dim));
    let ob = SelectObject(hdc, HGDIOBJ(brush.0));
    let op = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = RoundRect(hdc, x, y, x + 10, y + 12, 4, 4);
    SelectObject(hdc, op);
    SelectObject(hdc, ob);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let _ = DeleteObject(HGDIOBJ(brush.0));
    line(hdc, x + 2, y + 4, x + 8, y + 4, p.dim, 1);
    line(hdc, x + 2, y + 7, x + 8, y + 7, p.dim, 1);
}

fn dot_centers(active: usize) -> Vec<i32> {
    // Active page is a 14px pill, the others are 3px dots (Nusa layout).
    let total = 14 + 6 + 3 + 5 + 3 + 5 + 3 + 5 + 3;
    let mut x = EXPANDED_W / 2 - total / 2 - 1;
    let mut out = Vec::with_capacity(PAGE_COUNT);
    for i in 0..PAGE_COUNT {
        if i == active {
            out.push(x + 7);
            x += 14;
            if i + 1 < PAGE_COUNT {
                x += 6;
            }
        } else {
            out.push(x + 1);
            x += 3;
            if i + 1 < PAGE_COUNT {
                x += 5;
            }
        }
    }
    out
}

unsafe fn paint_dots(hdc: HDC, p: &Palette, page: usize) {
    let centers = dot_centers(page);
    for (i, cx) in centers.iter().enumerate() {
        if i == page {
            fill(hdc, cx - 7, 131, cx + 8, 135, p.text);
        } else {
            if hover_is(HOVER_DOT + i) {
                circle(hdc, *cx, 133, 5, p.faint);
            }
            fill(hdc, cx - 1, 132, cx + 2, 135, 0x00737373);
        }
    }
}

// ------------------------------------------------------------------ input

/// Which control is under the pointer (used for hover feedback).
fn hotspot(x: i32, y: i32) -> usize {
    if !EXPANDED.load(Ordering::Relaxed) {
        return 0;
    }
    let page = PAGE.load(Ordering::Relaxed);
    if y >= 127 {
        for (i, cx) in dot_centers(page).iter().enumerate() {
            if (x - cx).abs() <= 7 {
                return HOVER_DOT + i;
            }
        }
        return 0;
    }
    if (x < 38 || x > EXPANDED_W - 38) && (46..=94).contains(&y) {
        if x < 38 {
            return HOVER_CHEV_LEFT;
        }
        if x > EXPANDED_W - 38 {
            return HOVER_CHEV_RIGHT;
        }
        return 0;
    }
    match page {
        PAGE_MEDIA => {
            let has_media = MEDIA_TITLE.lock().map(|m| !m.is_empty()).unwrap_or(false);
            if has_media && (102..=126).contains(&y) {
                if (140..=184).contains(&x) {
                    return HOVER_MEDIA_PREV;
                }
                if (186..=234).contains(&x) {
                    return HOVER_MEDIA_PLAY;
                }
                if (236..=280).contains(&x) {
                    return HOVER_MEDIA_NEXT;
                }
            }
        }
        PAGE_FOCUS => {
            if (98..=120).contains(&y) {
                let cx = EXPANDED_W / 2;
                if ((cx - 78)..=(cx - 6)).contains(&x) {
                    return HOVER_FOCUS_START;
                }
                if ((cx + 8)..=(cx + 78)).contains(&x) {
                    return HOVER_FOCUS_RESET;
                }
            }
        }
        PAGE_WATER => {
            if (100..=122).contains(&y) {
                let cx = EXPANDED_W / 2;
                if ((cx - 112)..=(cx - 34)).contains(&x) {
                    return HOVER_WATER_DRANK;
                }
                if ((cx - 26)..=(cx + 34)).contains(&x) {
                    return HOVER_WATER_SNOOZE;
                }
                if ((cx + 42)..=(cx + 102)).contains(&x) {
                    return HOVER_WATER_RESET;
                }
            }
        }
        PAGE_SHELF => {
            if (44..=(EXPANDED_W - 44)).contains(&x) && (40..=106).contains(&y) {
                return HOVER_SHELF_ZONE;
            }
        }
        PAGE_TODO => {
            if (44..=(EXPANDED_W - 44)).contains(&x) && (38..=108).contains(&y) {
                return HOVER_TODO_CARD;
            }
        }
        _ => {}
    }
    0
}

fn click(hwnd: HWND, x: i32, y: i32) {
    if !EXPANDED.load(Ordering::Relaxed) {
        layout(hwnd, true);
        return;
    }
    let page = PAGE.load(Ordering::Relaxed);

    // Page dots (bottom strip).
    if y >= 127 {
        for (i, cx) in dot_centers(page).iter().enumerate() {
            if (x - cx).abs() <= 7 {
                PAGE.store(i, Ordering::Relaxed);
                HOVER_ID.store(0, Ordering::Relaxed);
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
                return;
            }
        }
        return;
    }

    // Side chevrons navigate pages.
    if (x < 38 || x > EXPANDED_W - 38) && (46..=94).contains(&y) {
        let mut next = page;
        if x < 38 {
            next = if page == 0 { PAGE_COUNT - 1 } else { page - 1 };
        } else if x > EXPANDED_W - 38 {
            next = (page + 1) % PAGE_COUNT;
        }
        if next != page {
            PAGE.store(next, Ordering::Relaxed);
            HOVER_ID.store(0, Ordering::Relaxed);
            unsafe { let _ = InvalidateRect(hwnd, None, true); }
        }
        return;
    }

    match page {
        PAGE_MEDIA => {
            // Controls row.
            let has_media = MEDIA_TITLE.lock().map(|m| !m.is_empty()).unwrap_or(false);
            if has_media && (102..=126).contains(&y) {
                if (140..=184).contains(&x) {
                    crate::media::MediaSession::prev();
                } else if (186..=234).contains(&x) {
                    crate::media::MediaSession::toggle();
                } else if (236..=280).contains(&x) {
                    crate::media::MediaSession::next();
                }
            }
        }
        PAGE_FOCUS => {
            if (90..=116).contains(&y) {
                if let Some(m) = FOCUS.get() {
                    if let Ok(mut f) = m.lock() {
                        if x < EXPANDED_W / 2 - 2 {
                            if f.is_running() {
                                f.pause();
                            } else {
                                f.start();
                            }
                        } else if x > EXPANDED_W / 2 + 2 {
                            f.reset();
                            f.start();
                        }
                    }
                }
            }
        }
        PAGE_WATER => {
            if (90..=116).contains(&y) {
                let mut w = crate::water::WaterReminder::load();
                if x < EXPANDED_W / 2 - 32 {
                    w.refill(250);
                    WATER_ALERT.store(false, Ordering::Relaxed);
                } else if x < EXPANDED_W / 2 + 38 {
                    w.snooze();
                    WATER_ALERT.store(false, Ordering::Relaxed);
                } else {
                    w.consumed_ml = 0;
                    w.snoozed = false;
                    w.save();
                }
            }
        }
        PAGE_TODO => {
            crate::panel::show_panel();
        }
        _ => {
            let _ = std::process::Command::new("explorer")
                .arg(crate::state::localappdata_shelf_dir())
                .spawn();
        }
    }
    unsafe { let _ = InvalidateRect(hwnd, None, true); }
}

// ------------------------------------------------------------------ window

pub struct Notch {
    pub hwnd: HWND,
}

impl Notch {
    pub fn create() -> Result<Self, windows::core::Error> {
        unsafe {
            let class_name: Vec<u16> = "NenoNotch\0".encode_utf16().collect();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: windows::Win32::Foundation::HINSTANCE(
                    windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?.0,
                ),
                hIcon: Default::default(),
                hCursor: Default::default(),
                hbrBackground: Default::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                hIconSm: Default::default(),
            };
            RegisterClassExW(&wc as *const _);

            let title: Vec<u16> = "Neno Notch\0".encode_utf16().collect();
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let x = (sw - COLLAPSED_W) / 2;
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_ACCEPTFILES,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                x,
                0,
                COLLAPSED_W,
                COLLAPSED_H,
                None,
                None,
                wc.hInstance,
                None,
            )?;
            use windows::Win32::UI::Shell::DragAcceptFiles;
            DragAcceptFiles(hwnd, true);
            SetTimer(hwnd, 1, 1000, None);
            SetTimer(hwnd, 2, 1800000, None);
            set_notch_hwnd(hwnd);
            // Say hello as soon as the notch appears.
            start_wave(hwnd, 3000);
            apply_region(hwnd, COLLAPSED_W, COLLAPSED_H, COLLAPSED_BODY_W, COLLAPSED_SHOULDER, COLLAPSED_RADIUS);
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self) {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOW};
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn run_message_loop() {
        unsafe {
            let mut msg = MSG::default();
            loop {
                let ret = GetMessageW(&mut msg, None, 0, 0);
                if ret.0 <= 0 {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wparam.0 == 4 {
                if current_wave().is_some() {
                    unsafe { let _ = InvalidateRect(hwnd, None, false); }
                } else {
                    unsafe { let _ = KillTimer(hwnd, 4); }
                    if let Ok(mut w) = MASCOT_WAVE.lock() {
                        *w = None;
                    }
                    unsafe { let _ = InvalidateRect(hwnd, None, false); }
                }
                return LRESULT(0);
            }
            if wparam.0 == 2 {
                WATER_ALERT.store(true, Ordering::Relaxed);
                set_text("Time to drink");
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
                return LRESULT(0);
            }

            // Mascot: doze after a quiet spell, then wave on the next input.
            let idle = system_idle_ms();
            if current_wave().is_none() {
                if idle < 1500 {
                    if MASCOT_DOZING.swap(false, Ordering::Relaxed) {
                        layout(hwnd, false);
                        start_wave(hwnd, 2400);
                    }
                } else if idle > 60_000
                    && !EXPANDED.load(Ordering::Relaxed)
                    && !MASCOT_HOVER.load(Ordering::Relaxed)
                {
                    if !MASCOT_DOZING.swap(true, Ordering::Relaxed) {
                        layout(hwnd, false);
                    }
                }
            }

            if let Some(focus_mutex) = FOCUS.get() {
                if let Ok(mut f) = focus_mutex.lock() {
                    if f.tick() {
                        WATER_ALERT.store(false, Ordering::Relaxed);
                    }
                    if let Some(sm) = SYSMON_INSTANCE.get() {
                        if let Ok(mut m) = sm.lock() {
                            m.sample_if_stale(std::time::Duration::from_secs(2));
                            if let Ok(mut sys) = SYSMON.lock() {
                                *sys = format!("CPU {:.0}% RAM {:.0}%", m.cpu_usage, m.ram_usage);
                            }
                            if let Ok(mut h) = CPU_HISTORY.lock() {
                                h.push(m.cpu_usage);
                                if h.len() > 60 { h.remove(0); }
                            }
                            if let Ok(mut h) = RAM_HISTORY.lock() {
                                h.push(m.ram_usage);
                                if h.len() > 60 { h.remove(0); }
                            }
                        }
                    }
                }
            }

            // Collapse when the pointer has been away from the card for a moment.
            if EXPANDED.load(Ordering::Relaxed) {
                let mut pt = POINT::default();
                let mut rc = RECT::default();
                let _ = GetCursorPos(&mut pt);
                let _ = GetWindowRect(hwnd, &mut rc);
                let inside = pt.x >= rc.left && pt.x < rc.right && pt.y >= rc.top && pt.y < rc.bottom;
                let mut last = LAST_INSIDE.lock().unwrap();
                if inside {
                    *last = Some(Instant::now());
                } else if last.map(|t| t.elapsed().as_millis() > 2200).unwrap_or(true) {
                    *last = None;
                    drop(last);
                    layout(hwnd, false);
                    return LRESULT(0);
                }
            }

            unsafe { let _ = InvalidateRect(hwnd, None, false); }
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe { paint(hwnd); }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xffff) as u16 as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as u16 as i16 as i32;
            let x = if EXPANDED.load(Ordering::Relaxed) {
                x - EXPANDED_SHOULDER
            } else {
                x
            };
            if !HOVER_TRACKING.swap(true, Ordering::Relaxed) {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = unsafe { TrackMouseEvent(&mut tme) };
            }
            if EXPANDED.load(Ordering::Relaxed) {
                MASCOT_HOVER.store(false, Ordering::Relaxed);
            } else {
                // The mascot greets the pointer once, then watches it move.
                if !MASCOT_HOVER.swap(true, Ordering::Relaxed) {
                    start_wave(hwnd, 1400);
                }
                let cx = MASCOT_CX.load(Ordering::Relaxed);
                let nx = ((x - cx) as f32 / 42.0).clamp(-1.0, 1.0);
                let ny = ((y - 15) as f32 / 15.0).clamp(-1.0, 1.0);
                let next = (
                    (nx * 1.8 * 10.0).round() as i32,
                    (ny * 1.2 * 10.0).round() as i32,
                );
                let changed = MASCOT_LOOK
                    .lock()
                    .map(|mut l| {
                        if *l != next {
                            *l = next;
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);
                if changed {
                    unsafe { let _ = InvalidateRect(hwnd, None, false); }
                }
            }
            let id = hotspot(x, y);
            if id != HOVER_ID.load(Ordering::Relaxed) {
                HOVER_ID.store(id, Ordering::Relaxed);
                unsafe { let _ = InvalidateRect(hwnd, None, false); }
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            HOVER_TRACKING.store(false, Ordering::Relaxed);
            let was_hover = MASCOT_HOVER.swap(false, Ordering::Relaxed);
            if let Ok(mut l) = MASCOT_LOOK.lock() {
                *l = (0, 0);
            }
            let had_hotspot = HOVER_ID.swap(0, Ordering::Relaxed) != 0;
            if was_hover || had_hotspot {
                unsafe { let _ = InvalidateRect(hwnd, None, false); }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as u16 as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as u16 as i16 as i32;
            let x = if EXPANDED.load(Ordering::Relaxed) {
                x - EXPANDED_SHOULDER
            } else {
                x
            };
            click(hwnd, x, y);
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            if EXPANDED.load(Ordering::Relaxed) {
                layout(hwnd, false);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            if EXPANDED.load(Ordering::Relaxed) {
                let delta = (wparam.0 >> 16) as u16 as i16;
                let page = PAGE.load(Ordering::Relaxed);
                let next = if delta < 0 {
                    (page + 1) % PAGE_COUNT
                } else if page == 0 {
                    PAGE_COUNT - 1
                } else {
                    page - 1
                };
                PAGE.store(next, Ordering::Relaxed);
                HOVER_ID.store(0, Ordering::Relaxed);
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            crate::shelf::handle_drop_files(wparam.0 as isize);
            PAGE.store(PAGE_SHELF, Ordering::Relaxed);
            HOVER_ID.store(0, Ordering::Relaxed);
            if !EXPANDED.load(Ordering::Relaxed) {
                layout(hwnd, true);
            } else {
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            match wparam.0 as u32 {
                x if x == windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE.0 as u32 => {
                    crate::media::MediaSession::play()
                }
                x if x == windows::Win32::UI::Input::KeyboardAndMouse::VK_RIGHT.0 as u32 => {
                    crate::media::MediaSession::next()
                }
                x if x == windows::Win32::UI::Input::KeyboardAndMouse::VK_LEFT.0 as u32 => {
                    crate::media::MediaSession::prev()
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0); }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
