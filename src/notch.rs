//! The notch overlay, styled after the original Nusa app:
//! a compact black pill at the top of the screen that expands into a
//! rounded 420x126 card with five pages (media, focus, water, to-do, shelf).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreatePolygonRgn, CreateSolidBrush, DeleteObject, DrawTextW,
    Ellipse, EndPaint, FillRect, GetStockObject, GetTextExtentPoint32W, InvalidateRect, LineTo,
    MoveToEx, Polygon, RoundRect, SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TextOutW,
    ALTERNATE, CLEARTYPE_QUALITY, DEFAULT_CHARSET, DRAW_TEXT_FORMAT, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, FW_NORMAL, FW_SEMIBOLD,
    HDC, HFONT, HGDIOBJ, NULL_BRUSH, NULL_PEN, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
    GetWindowRect, PostQuitMessage, RegisterClassExW, SetTimer, SetWindowPos, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, MSG, SM_CXSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    WM_DESTROY, WM_DROPFILES, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONDOWN,
    WM_TIMER, WNDCLASSEXW, WS_EX_ACCEPTFILES, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const COLLAPSED_W: i32 = 164;
const COLLAPSED_H: i32 = 30;
const EXPANDED_W: i32 = 420;
const EXPANDED_H: i32 = 140;
const PAGE_COUNT: usize = 5;

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
static THEME_DARK: Mutex<bool> = Mutex::new(true);
static LAST_INSIDE: Mutex<Option<Instant>> = Mutex::new(None);
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

pub fn set_theme(dark: bool) {
    if let Ok(mut t) = THEME_DARK.lock() {
        *t = dark;
    }
}

pub fn is_dark_theme() -> bool {
    THEME_DARK.lock().map(|t| *t).unwrap_or(true)
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
        let (w, h) = if expanded { (EXPANDED_W, EXPANDED_H) } else { (COLLAPSED_W, COLLAPSED_H) };
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
        apply_region(hwnd, w, h, if expanded { 18 } else { 12 });
        EXPANDED.store(expanded, Ordering::Relaxed);
        if !expanded {
            *LAST_INSIDE.lock().unwrap() = None;
        } else {
            *LAST_INSIDE.lock().unwrap() = Some(Instant::now());
        }
        let _ = InvalidateRect(hwnd, None, true);
    }
}

/// Square top edge, rounded bottom corners (like the Nusa notch).
unsafe fn apply_region(hwnd: HWND, w: i32, h: i32, r: i32) {
    let steps = 10;
    let mut pts: Vec<POINT> = Vec::with_capacity(steps * 2 + 4);
    pts.push(POINT { x: 0, y: 0 });
    pts.push(POINT { x: w, y: 0 });
    for i in 0..=steps {
        let a = (i as f32) * (std::f32::consts::FRAC_PI_2 / steps as f32);
        pts.push(POINT {
            x: w - r + (r as f32 * a.cos()) as i32,
            y: h - r + (r as f32 * a.sin()) as i32,
        });
    }
    for i in 0..=steps {
        let a = std::f32::consts::FRAC_PI_2
            + (i as f32) * (std::f32::consts::FRAC_PI_2 / steps as f32);
        pts.push(POINT {
            x: r + (r as f32 * a.cos()) as i32,
            y: h - r + (r as f32 * a.sin()) as i32,
        });
    }
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
    faint: u32,
    green: u32,
    art: u32,
    track: u32,
}

fn palette() -> Palette {
    if is_dark_theme() {
        Palette {
            bg: 0x00000000,
            text: 0x00FFFFFF,
            dim: 0x00938E8E,   // #8E8E93
            faint: 0x003C3A3A, // #3A3A3C
            green: 0x0058D130, // #30D158
            art: 0x001E1C1C,   // #1C1C1E
            track: 0x00303030,
        }
    } else {
        Palette {
            bg: 0x00F7F5F5,    // #F5F5F7
            text: 0x001E1C1C,  // #1C1C1E
            dim: 0x00736E6E,   // #6E6E73
            faint: 0x00DBD6D6, // #D6D6DB
            green: 0x0058D130,
            art: 0x00EDE8E8,   // #E8E8ED
            track: 0x00DCDCDC,
        }
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
        CLEARTYPE_QUALITY.0 as u32,
        FF_DONTCARE.0 as u32,
        PCWSTR(face.as_ptr()),
    )
}

unsafe fn make_font_face(size: i32, weight: u32, face: &str) -> HFONT {
    let name: Vec<u16> = format!("{}\0", face).encode_utf16().collect();
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
        PCWSTR(name.as_ptr()),
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

unsafe fn status_icon(hdc: HDC, cx: i32, cy: i32, color: u32, bolt: u32) {
    ring(hdc, cx, cy, 9, color, 2);
    let pts = [
        POINT { x: cx - 1, y: cy - 7 },
        POINT { x: cx + 4, y: cy - 7 },
        POINT { x: cx + 0, y: cy - 1 },
        POINT { x: cx + 4, y: cy - 1 },
        POINT { x: cx - 3, y: cy + 8 },
        POINT { x: cx - 1, y: cy + 1 },
        POINT { x: cx - 5, y: cy + 1 },
    ];
    filled_poly(hdc, &pts, 0x00FFFFFF);
}

unsafe fn play_triangle(hdc: HDC, cx: i32, cy: i32, color: u32) {
    let pts = [
        POINT { x: cx - 4, y: cy - 7 },
        POINT { x: cx - 4, y: cy + 7 },
        POINT { x: cx + 7, y: cy },
    ];
    filled_poly(hdc, &pts, color);
}

unsafe fn pause_bars(hdc: HDC, cx: i32, cy: i32, color: u32) {
    fill(hdc, cx - 5, cy - 7, cx - 2, cy + 7, color);
    fill(hdc, cx + 2, cy - 7, cx + 5, cy + 7, color);
}

unsafe fn skip_icon(hdc: HDC, cx: i32, cy: i32, color: u32, dir: i32) {
    // dir = -1 previous, +1 next
    if dir < 0 {
        fill(hdc, cx - 7, cy - 7, cx - 5, cy + 7, color);
        let pts = [
            POINT { x: cx + 6, y: cy - 7 },
            POINT { x: cx + 6, y: cy + 7 },
            POINT { x: cx - 5, y: cy },
        ];
        filled_poly(hdc, &pts, color);
    } else {
        fill(hdc, cx + 5, cy - 7, cx + 7, cy + 7, color);
        let pts = [
            POINT { x: cx - 6, y: cy - 7 },
            POINT { x: cx - 6, y: cy + 7 },
            POINT { x: cx + 5, y: cy },
        ];
        filled_poly(hdc, &pts, color);
    }
}

unsafe fn chevron(hdc: HDC, x: i32, y: i32, dir: i32, color: u32) {
    let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    MoveToEx(hdc, x - 3 * dir, y - 6, None);
    let _ = LineTo(hdc, x + 3 * dir, y);
    let _ = LineTo(hdc, x - 3 * dir, y + 6);
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
    let radius = 3;
    let track = CreateSolidBrush(COLORREF(bg));
    let old_brush = SelectObject(hdc, HGDIOBJ(track.0));
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = RoundRect(hdc, l, t, r, t + 5, radius, radius);
    let filled_w = ((r - l) as f32 * pct.clamp(0.0, 1.0)) as i32;
    if filled_w > 2 {
        let fill_brush = CreateSolidBrush(COLORREF(fg));
        SelectObject(hdc, HGDIOBJ(fill_brush.0));
        let _ = RoundRect(hdc, l, t, l + filled_w.max(radius), t + 5, radius, radius);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(fill_brush.0));
    } else {
        SelectObject(hdc, old_brush);
    }
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(HGDIOBJ(track.0));
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
        fill(hdc, 0, 0, COLLAPSED_W, COLLAPSED_H, p.bg);
        let (clock, _) = now_parts();
        let f_clock = make_font(20, FW_SEMIBOLD.0);
        status_icon(hdc, 27, 15, if WATER_ALERT.load(Ordering::Relaxed) { 0x0050A8FF } else { p.green }, p.text);
        draw_text(
            hdc,
            f_clock,
            p.text,
            &clock,
            RECT { left: 0, top: 0, right: COLLAPSED_W, bottom: COLLAPSED_H },
            DT_CENTER,
        );
        let _ = DeleteObject(HGDIOBJ(f_clock.0));
        let _ = EndPaint(hwnd, &ps);
        return;
    }

    fill(hdc, 0, 0, EXPANDED_W, EXPANDED_H, p.bg);
    let f_date = make_font(12, FW_NORMAL.0);
    let f_time = make_font(13, FW_BOLD.0);
    let f_title = make_font(15, FW_SEMIBOLD.0);
    let f_small = make_font(11, FW_NORMAL.0);
    let f_tiny = make_font(10, FW_NORMAL.0);
    let f_big = make_font(34, FW_SEMIBOLD.0);
    let f_mid = make_font(24, FW_SEMIBOLD.0);
    let f_btn = make_font(11, FW_SEMIBOLD.0);
    let f_glyph = make_font_face(26, FW_NORMAL.0, "Segoe UI Symbol");

    let (clock, date) = now_parts();
    let page = PAGE.load(Ordering::Relaxed);

    match page {
        PAGE_MEDIA => paint_media(
            hdc, &p, f_glyph, f_title, f_small, f_tiny, f_date, f_time, &clock, &date,
        ),
        PAGE_FOCUS => paint_focus(hdc, &p, f_big, f_date, f_time, f_small, f_btn, &clock, &date),
        PAGE_WATER => paint_water(hdc, &p, f_mid, f_date, f_time, f_small, f_btn, &clock, &date),
        PAGE_TODO => paint_todo(hdc, &p, f_date, f_time, f_small, f_tiny, &clock, &date),
        _ => paint_shelf(hdc, &p, f_date, f_time, f_title, f_small, &clock, &date),
    }

    paint_dots(hdc, &p, page);
    if page > 0 {
        circle(hdc, 22, 70, 11, p.faint);
        chevron(hdc, 22, 70, -1, p.dim);
    }
    if page + 1 < PAGE_COUNT {
        circle(hdc, EXPANDED_W - 22, 70, 11, p.faint);
        chevron(hdc, EXPANDED_W - 22, 70, 1, p.dim);
    }

    for f in [f_date, f_time, f_title, f_small, f_tiny, f_big, f_mid, f_btn, f_glyph] {
        let _ = DeleteObject(HGDIOBJ(f.0));
    }
    let _ = EndPaint(hwnd, &ps);
}

unsafe fn paint_header(
    hdc: HDC,
    p: &Palette,
    f_date: HFONT,
    f_time: HFONT,
    clock: &str,
    date: &str,
) {
    let date_w = text_width(hdc, f_date, date);
    let time_w = text_width(hdc, f_time, clock);
    let total = 18 + 7 + date_w + 8 + time_w;
    let start = (EXPANDED_W - total) / 2;
    status_icon(hdc, start + 9, 18, p.green, p.text);
    draw_text(
        hdc,
        f_date,
        p.dim,
        date,
        RECT { left: start + 24, top: 6, right: start + 24 + date_w + 20, bottom: 30 },
        DT_LEFT,
    );
    draw_text(
        hdc,
        f_time,
        p.text,
        clock,
        RECT {
            left: start + 24 + date_w + 8,
            top: 6,
            right: start + 24 + date_w + 8 + time_w + 20,
            bottom: 30,
        },
        DT_LEFT,
    );
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_media(
    hdc: HDC,
    p: &Palette,
    f_glyph: HFONT,
    f_title: HFONT,
    f_small: HFONT,
    f_tiny: HFONT,
    f_date: HFONT,
    f_time: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, clock, date);

    // Album art placeholder.
    let art = CreateSolidBrush(COLORREF(p.art));
    let old_brush = SelectObject(hdc, HGDIOBJ(art.0));
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    RoundRect(hdc, 12, 12, 84, 84, 20, 20);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(art.0));
    draw_text(
        hdc,
        f_glyph,
        p.dim,
        "\u{266B}",
        RECT { left: 12, top: 14, right: 84, bottom: 82 },
        DT_CENTER,
    );

    let media = MEDIA_TITLE.lock().map(|m| m.clone()).unwrap_or_default();
    let (title, artist) = match media.split_once(" \u{2014} ") {
        Some((t, a)) => (t.to_string(), a.to_string()),
        None => (media.clone(), String::new()),
    };
    let (title, artist) = if title.is_empty() {
        ("No media".to_string(), "Play something to see it here".to_string())
    } else {
        (title, artist)
    };

    draw_text(
        hdc,
        f_title,
        p.text,
        &title,
        RECT { left: 96, top: 36, right: EXPANDED_W - 46, bottom: 58 },
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
            RECT { left: 96, top: 56, right: EXPANDED_W - 46, bottom: 74 },
            DT_LEFT | DT_END_ELLIPSIS,
        );
    }

    // Progress row.
    let pos = PLAY_POS.load(Ordering::Relaxed);
    let dur = PLAY_LEN.load(Ordering::Relaxed);
    let has = dur > 0;
    let pct = if has { pos as f32 / dur as f32 } else { 0.0 };
    draw_text(
        hdc,
        f_tiny,
        p.dim,
        &mmss(if has { pos } else { 0 }),
        RECT { left: 96, top: 72, right: 132, bottom: 88 },
        DT_LEFT,
    );
    progress_bar(hdc, 132, 77, EXPANDED_W - 86, pct, p.text, p.track);
    draw_text(
        hdc,
        f_tiny,
        p.dim,
        &format!("-{}", mmss(if has { dur.saturating_sub(pos) } else { 0 })),
        RECT { left: EXPANDED_W - 82, top: 72, right: EXPANDED_W - 39, bottom: 88 },
        DT_RIGHT,
    );

    // Controls.
    let cy = 102;
    skip_icon(hdc, 168, cy, if media.is_empty() { p.faint } else { p.dim }, -1);
    if PLAYING.load(Ordering::Relaxed) {
        pause_bars(hdc, 210, cy, p.text);
    } else {
        play_triangle(hdc, 210, cy, p.text);
    }
    skip_icon(hdc, 252, cy, if media.is_empty() { p.faint } else { p.dim }, 1);

    // Lyrics bubble on the left.
    let bubble = CreateSolidBrush(COLORREF(p.faint));
    let ob = SelectObject(hdc, HGDIOBJ(bubble.0));
    let op = SelectObject(hdc, GetStockObject(NULL_PEN));
    RoundRect(hdc, 26, 94, 46, 110, 10, 10);
    SelectObject(hdc, op);
    SelectObject(hdc, ob);
    let _ = DeleteObject(HGDIOBJ(bubble.0));
    line(hdc, 30, 99, 42, 99, p.dim, 1);
    line(hdc, 30, 103, 42, 103, p.dim, 1);
    let pts = [POINT { x: 30, y: 110 }, POINT { x: 30, y: 115 }, POINT { x: 36, y: 110 }];
    filled_poly(hdc, &pts, p.faint);
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
    paint_header(hdc, p, f_date, f_time, clock, date);
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
    button(hdc, EXPANDED_W / 2 - 78, 98, 72, 22, p.green, 0x00000000, start_label, f_btn);
    button(hdc, EXPANDED_W / 2 + 8, 98, 70, 22, p.faint, p.text, "Reset", f_btn);

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
    paint_header(hdc, p, f_date, f_time, clock, date);
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
    progress_bar(hdc, 60, 76, EXPANDED_W - 60, pct, p.green, p.track);
    draw_text(
        hdc,
        f_small,
        p.dim,
        &format!("{:.0}% of daily goal", pct * 100.0),
        RECT { left: 0, top: 82, right: EXPANDED_W, bottom: 96 },
        DT_CENTER,
    );
    button(hdc, EXPANDED_W / 2 - 112, 100, 78, 22, p.green, 0x00000000, "Drank +250", f_btn);
    button(hdc, EXPANDED_W / 2 - 26, 100, 60, 22, p.faint, p.text, "Snooze", f_btn);
    button(hdc, EXPANDED_W / 2 + 42, 100, 60, 22, p.faint, p.text, "Reset", f_btn);
}

unsafe fn paint_todo(
    hdc: HDC,
    p: &Palette,
    f_date: HFONT,
    f_time: HFONT,
    f_small: HFONT,
    f_tiny: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, clock, date);
    let list = crate::todo::TodoList::load();
    let open: Vec<_> = list.items.iter().filter(|i| !i.done).collect();
    if open.is_empty() {
        draw_text(
            hdc,
            f_small,
            p.dim,
            "No tasks. Click to add one.",
            RECT { left: 0, top: 44, right: EXPANDED_W, bottom: 66 },
            DT_CENTER,
        );
    } else {
        for (i, item) in open.iter().take(3).enumerate() {
            let y = 36 + (i as i32) * 24;
            let bx = 28;
            let brush = CreateSolidBrush(COLORREF(p.track));
            let ob = SelectObject(hdc, HGDIOBJ(brush.0));
            let op = SelectObject(hdc, GetStockObject(NULL_PEN));
            RoundRect(hdc, bx, y, bx + 12, y + 12, 6, 6);
            SelectObject(hdc, op);
            SelectObject(hdc, ob);
            let _ = DeleteObject(HGDIOBJ(brush.0));
            draw_text(
                hdc,
                f_small,
                p.text,
                &item.text,
                RECT { left: bx + 20, top: y - 2, right: EXPANDED_W - 20, bottom: y + 14 },
                DT_LEFT | DT_END_ELLIPSIS,
            );
        }
    }
    let hint = if list.items.is_empty() {
        "Open the task panel to get started"
    } else {
        "Click to open the task panel"
    };
    draw_text(
        hdc,
        f_tiny,
        p.dim,
        hint,
        RECT { left: 0, top: 108, right: EXPANDED_W, bottom: 126 },
        DT_CENTER,
    );
}

unsafe fn paint_shelf(
    hdc: HDC,
    p: &Palette,
    f_date: HFONT,
    f_time: HFONT,
    f_title: HFONT,
    f_small: HFONT,
    clock: &str,
    date: &str,
) {
    paint_header(hdc, p, f_date, f_time, clock, date);
    draw_text(
        hdc,
        f_title,
        p.text,
        "Drop files here",
        RECT { left: 0, top: 44, right: EXPANDED_W, bottom: 68 },
        DT_CENTER,
    );
    let shelf = crate::shelf::Shelf::new();
    draw_text(
        hdc,
        f_small,
        p.dim,
        &format!("{} file(s) on the shelf", shelf.files.len()),
        RECT { left: 0, top: 72, right: EXPANDED_W, bottom: 90 },
        DT_CENTER,
    );
    draw_text(
        hdc,
        f_small,
        p.dim,
        "Click to open the shelf folder",
        RECT { left: 0, top: 94, right: EXPANDED_W, bottom: 112 },
        DT_CENTER,
    );
}

fn dot_centers(active: usize) -> Vec<i32> {
    // Active page is a 14px pill, the others are 3px dots (Nusa layout).
    let total = 14 + 6 + 3 + 5 + 3 + 5 + 3 + 5 + 3;
    let mut x = EXPANDED_W / 2 - total / 2;
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
            fill(hdc, cx - 7, 129, cx + 7, 133, p.text);
        } else {
            fill(hdc, cx - 1, 130, cx + 2, 133, 0x00808085);
        }
    }
}

// ------------------------------------------------------------------ input

fn click(hwnd: HWND, x: i32, y: i32) {
    if !EXPANDED.load(Ordering::Relaxed) {
        layout(hwnd, true);
        return;
    }
    let page = PAGE.load(Ordering::Relaxed);

    // Page dots (bottom strip).
    if y >= 122 {
        for (i, cx) in dot_centers(page).iter().enumerate() {
            if (x - cx).abs() <= 7 {
                PAGE.store(i, Ordering::Relaxed);
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
                return;
            }
        }
        return;
    }

    // Side chevrons navigate pages.
    if (x < 38 || x > EXPANDED_W - 38) && (46..=94).contains(&y) {
        let mut next = page;
        if x < 38 && page > 0 {
            next = page - 1;
        } else if x > EXPANDED_W - 38 && page + 1 < PAGE_COUNT {
            next = page + 1;
        }
        if next != page {
            PAGE.store(next, Ordering::Relaxed);
            unsafe { let _ = InvalidateRect(hwnd, None, true); }
        }
        return;
    }

    match page {
        PAGE_MEDIA => {
            // Controls row.
            if (80..=112).contains(&y) {
                if (144..=184).contains(&x) {
                    crate::media::MediaSession::prev();
                } else if (186..=234).contains(&x) {
                    crate::media::MediaSession::toggle();
                } else if (236..=276).contains(&x) {
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
                style: CS_HREDRAW | CS_VREDRAW,
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
            apply_region(hwnd, COLLAPSED_W, COLLAPSED_H, 12);
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
            if wparam.0 == 2 {
                WATER_ALERT.store(true, Ordering::Relaxed);
                set_text("Time to drink");
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
                return LRESULT(0);
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
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as u16 as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as u16 as i16 as i32;
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
                unsafe { let _ = InvalidateRect(hwnd, None, true); }
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            crate::shelf::handle_drop_files(wparam.0 as isize);
            PAGE.store(PAGE_SHELF, Ordering::Relaxed);
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
