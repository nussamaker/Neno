//! Small GDI helpers shared by the panel and settings windows.
//!
//! The notch paints itself with its own copy of these primitives; this module
//! keeps the utility windows in the same visual language (dark card, Segoe UI,
//! soft rounded shapes, green accent).

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWMWINDOWATTRIBUTE,
};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, FillRect, GetStockObject,
    LineTo, MoveToEx, RoundRect, SelectObject, SetBkMode, SetTextColor, CLEARTYPE_QUALITY,
    DEFAULT_CHARSET, DRAW_TEXT_FORMAT, DT_CENTER, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, HDC,
    HFONT, HGDIOBJ, NULL_PEN, PS_SOLID, TRANSPARENT,
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
        // Windows may tint the caption with the user's accent colour; force a
        // black frame so the utility windows match the notch.
        let black: u32 = 0;
        let white: u32 = 0x00FFFFFF;
        for (attr, color) in [
            (DWMWA_CAPTION_COLOR, black),
            (DWMWA_TEXT_COLOR, white),
            (DWMWA_BORDER_COLOR, black),
        ] {
            let _ = DwmSetWindowAttribute(
                hwnd,
                attr,
                &color as *const u32 as *const core::ffi::c_void,
                4,
            );
        }
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

/// Paint the flat black window background.
pub unsafe fn paint_bg(hdc: HDC, w: i32, h: i32) {
    fill(hdc, 0, 0, w, h, BG);
}
