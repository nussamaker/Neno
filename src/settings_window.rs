use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, DeleteObject, EndPaint, InvalidateRect, PAINTSTRUCT, DT_LEFT, DT_RIGHT, HDC,
    HGDIOBJ,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRect, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    GetSystemMetrics, PostQuitMessage, RegisterClassExW, SetForegroundWindow, ShowWindow,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, SM_CXSCREEN, SW_HIDE, SW_SHOW, WM_CLOSE,
    WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_PAINT, WNDCLASSEXW,
    WS_CAPTION, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_SYSMENU, WINDOW_STYLE,
};

use crate::ui;

const W: i32 = 360;
const H: i32 = 270;
const WM_MOUSELEAVE: u32 = 0x02A3;
const STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0);

static SETTINGS_VISIBLE: AtomicBool = AtomicBool::new(false);
#[allow(static_mut_refs)]
static mut SETTINGS_HWND: Option<HWND> = None;
static HOVER: AtomicUsize = AtomicUsize::new(0);
static HOVER_TRACKING: AtomicBool = AtomicBool::new(false);

fn settings_hwnd() -> Option<HWND> {
    unsafe { SETTINGS_HWND }
}

pub fn create_settings() -> Result<HWND, windows::core::Error> {
    unsafe {
        let class_name: Vec<u16> = "NenoSettings\0".encode_utf16().collect();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_proc),
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

        let mut rc = RECT { left: 0, top: 0, right: W, bottom: H };
        let _ = AdjustWindowRect(&mut rc, STYLE, false);
        let outer_w = rc.right - rc.left;
        let outer_h = rc.bottom - rc.top;
        let sw = GetSystemMetrics(SM_CXSCREEN);

        let title: Vec<u16> = "Neno Settings\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            STYLE,
            (sw - outer_w) / 2,
            160,
            outer_w,
            outer_h,
            None,
            None,
            wc.hInstance,
            None,
        )?;
        ui::dark_frame(hwnd);
        SETTINGS_HWND = Some(hwnd);
        Ok(hwnd)
    }
}

pub fn toggle_settings() {
    if let Some(hwnd) = settings_hwnd() {
        let now = !SETTINGS_VISIBLE.load(Ordering::SeqCst);
        SETTINGS_VISIBLE.store(now, Ordering::SeqCst);
        unsafe {
            if now {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            } else {
                let _ = ShowWindow(hwnd, SW_HIDE);
                HOVER.store(0, Ordering::Relaxed);
                HOVER_TRACKING.store(false, Ordering::Relaxed);
            }
        }
    }
}

fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
    RECT { left: l, top: t, right: r, bottom: b }
}

fn hover_target(y: i32) -> usize {
    if (68..108).contains(&y) {
        1
    } else if (116..156).contains(&y) {
        2
    } else if (164..204).contains(&y) {
        3
    } else {
        0
    }
}

unsafe fn row(
    hdc: HDC,
    top: i32,
    label: &str,
    value: &str,
    pill_bg: u32,
    pill_fg: u32,
    hot: bool,
    f_body: windows::Win32::Graphics::Gdi::HFONT,
    f_value: windows::Win32::Graphics::Gdi::HFONT,
) {
    ui::round(hdc, 14, top, W - 14, top + 40, 12, if hot { ui::ROW_HOVER } else { ui::CARD });
    ui::text(hdc, f_body, ui::TEXT, label, rect(28, top + 2, 220, top + 38), DT_LEFT);
    let pill_w = 84;
    let pill_x = W - 14 - 14 - pill_w;
    ui::pill(hdc, pill_x, top + 8, pill_w, 24, pill_bg, pill_fg, value, f_value);
}

unsafe fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    ui::paint_bg(hdc, W, H);

    let f_title = ui::font(16, 600);
    let f_body = ui::font(12, 400);
    let f_tiny = ui::font(10, 400);
    let f_value = ui::font(11, 600);
    let hover = HOVER.load(Ordering::Relaxed);

    ui::text(hdc, f_title, ui::TEXT, "Settings", rect(18, 12, 200, 36), DT_LEFT);
    ui::text(hdc, f_tiny, ui::DIM, concat!("Neno ", env!("CARGO_PKG_VERSION")), rect(240, 14, W - 18, 32), DT_RIGHT);
    ui::text(hdc, f_tiny, ui::DIM, "GENERAL", rect(18, 46, 140, 64), DT_LEFT);

    let water = crate::water::WaterReminder::load();
    row(
        hdc,
        68,
        "Water goal",
        &format!("{:.1} L", water.goal_liters),
        0x00303033,
        ui::TEXT,
        hover == 1,
        f_body,
        f_value,
    );

    let autostart = crate::autostart::is_autostart();
    row(
        hdc,
        116,
        "Start with Windows",
        if autostart { "ON" } else { "OFF" },
        if autostart { ui::ACCENT } else { ui::FAINT },
        if autostart { ui::ACCENT_FG } else { ui::DIM },
        hover == 2,
        f_body,
        f_value,
    );

    let weather = crate::settings::Settings::load()
        .get("weather_enabled")
        .map(|v| v.trim() != "false")
        .unwrap_or(true);
    row(
        hdc,
        164,
        "Weather",
        if weather { "ON" } else { "OFF" },
        if weather { ui::ACCENT } else { ui::FAINT },
        if weather { ui::ACCENT_FG } else { ui::DIM },
        hover == 3,
        f_body,
        f_value,
    );

    ui::text(
        hdc,
        f_tiny,
        ui::DIM,
        "Click a row to change  ·  Esc to close",
        rect(18, 220, W - 18, 238),
        DT_LEFT,
    );
    ui::text(
        hdc,
        f_tiny,
        ui::DIM,
        "Changes are saved automatically",
        rect(18, 238, W - 18, 256),
        DT_LEFT,
    );

    for f in [f_title, f_body, f_tiny, f_value] {
        let _ = DeleteObject(HGDIOBJ(f.0));
    }
    let _ = EndPaint(hwnd, &ps);
}

fn repaint(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            unsafe { paint(hwnd); }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            if !HOVER_TRACKING.swap(true, Ordering::Relaxed) {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = unsafe { TrackMouseEvent(&mut tme) };
            }
            let id = hover_target(y);
            if id != HOVER.load(Ordering::Relaxed) {
                HOVER.store(id, Ordering::Relaxed);
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            HOVER_TRACKING.store(false, Ordering::Relaxed);
            if HOVER.swap(0, Ordering::Relaxed) != 0 {
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            if (68..108).contains(&y) {
                let mut water = crate::water::WaterReminder::load();
                water.goal_liters = match water.goal_liters {
                    1.5 => 2.0,
                    2.0 => 2.5,
                    2.5 => 3.0,
                    _ => 1.5,
                };
                water.save();
                let mut settings = crate::settings::Settings::load();
                settings.set("water_goal", &water.goal_liters.to_string());
                settings.save();
            } else if (116..156).contains(&y) {
                let enabled = !crate::autostart::is_autostart();
                if let Ok(exe) = std::env::current_exe() {
                    crate::autostart::set_autostart(enabled, &exe);
                }
            } else if (164..204).contains(&y) {
                let mut settings = crate::settings::Settings::load();
                let currently_off = settings
                    .get("weather_enabled")
                    .map(|v| v.trim() == "false")
                    .unwrap_or(false);
                settings.set("weather_enabled", if currently_off { "true" } else { "false" });
                settings.save();
            }
            repaint(hwnd);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u32
                == windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0 as u32
            {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    SETTINGS_VISIBLE.store(false, Ordering::SeqCst);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
                SETTINGS_VISIBLE.store(false, Ordering::SeqCst);
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
