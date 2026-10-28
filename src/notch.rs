use std::sync::Mutex;

use std::sync::OnceLock;
static NOTCH_TEXT: Mutex<String> = Mutex::new(String::new());
static THEME_DARK: Mutex<bool> = Mutex::new(true);
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
                ShowWindow(hwnd, SW_SHOW);
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

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget, ID2D1SolidColorBrush,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_RENDER_TARGET_PROPERTIES, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, InvalidateRect, LineTo, MoveToEx, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor, TextOutW, TRANSPARENT, PS_SOLID};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassExW, SetTimer, TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, WM_DESTROY, WM_DROPFILES,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_ACCEPTFILES, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::PCWSTR;

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
            // Position at top-center of primary screen.
            let sw = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN) };
            let x = (sw - 420) / 2;
            let y = 0; // top of screen
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_ACCEPTFILES,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                x, y, 420, 90,
                None, None, wc.hInstance, None,
            )?;
            unsafe {
                use windows::Win32::UI::Shell::DragAcceptFiles;
                DragAcceptFiles(hwnd, true);
                // 1-second timer for notch text refresh.
                SetTimer(hwnd, 1, 1000, None);
                // Water reminder every 30 minutes.
                SetTimer(hwnd, 2, 1800000, None);
            }
            set_notch_hwnd(hwnd);
            Ok(Self { hwnd })
        }
    }

    pub fn show(&self) {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::ShowWindow;
            use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
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
                // Water reminder: update notch text and reset timer.
                crate::notch::set_text("DRINK WATER! Click notch for panel");
                return LRESULT(0);
            }
            if let Some(focus_mutex) = FOCUS.get() {
                if let Ok(mut f) = focus_mutex.lock() {
                    let _ = f.tick();
                    let media = MEDIA_TITLE.lock().map(|m| m.clone()).unwrap_or_default();
                    let lyrics = LYRICS.lock().map(|l| l.clone()).unwrap_or_default();
                    // Sample system monitor.
                    if let Some(sm) = SYSMON_INSTANCE.get() {
                        if let Ok(mut m) = sm.lock() {
                            m.sample_if_stale(std::time::Duration::from_secs(2));
                            let s = format!("CPU {:.0}% RAM {:.0}%", m.cpu_usage, m.ram_usage);
                            if let Ok(mut sys) = SYSMON.lock() {
                                *sys = s;
                            }
                            // Push to history (cap 60).
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
                    let weather = WEATHER.lock().map(|w| w.clone()).unwrap_or_default();
                    let sysmon = SYSMON.lock().map(|s| s.clone()).unwrap_or_default();
                    let bt = BT_COUNT.lock().map(|c| *c).unwrap_or(0);
                    let notif = NOTIF_COUNT.lock().map(|c| *c).unwrap_or(0);
                    let lyric_snip = if lyrics.is_empty() {
                        String::new()
                    } else {
                        let s: String = lyrics.chars().take(20).collect();
                        format!(" | {}", s)
                    };
                    let weather_snip = if weather.is_empty() { String::new() } else { format!(" | {}", weather) };
                    let sysmon_snip = if sysmon.is_empty() { String::new() } else { format!(" | {}", sysmon) };
                    let text = if media.is_empty() {
                        format!("Focus {}s | BT {} | Notif {}{}{}{}", f.remaining().as_secs(), bt, notif, lyric_snip, weather_snip, sysmon_snip)
                    } else {
                        format!("{} | Focus {}s | BT {} | Notif {}{}{}{}", media, f.remaining().as_secs(), bt, notif, lyric_snip, weather_snip, sysmon_snip)
                    };
                    if let Ok(mut t) = NOTCH_TEXT.lock() {
                        *t = text;
                    }
                }
            }
            unsafe { let _ = InvalidateRect(hwnd, None, false); }
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                // GDI fallback rendering (reliable)
                let dark = is_dark_theme();
                let bg_color = if dark { 0x00141414u32 } else { 0x00E0E0E0u32 };
                let text_color = if dark { 0x00FFFFFFu32 } else { 0x00000000u32 };
                let mut rc = RECT { left: 0, top: 0, right: 420, bottom: 90 };
                let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(bg_color));
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(brush.0));
                SetTextColor(hdc, windows::Win32::Foundation::COLORREF(text_color));
                // border
                let pen = CreatePen(PS_SOLID, 1, windows::Win32::Foundation::COLORREF(0x00333333));
                let old = SelectObject(hdc, windows::Win32::Graphics::Gdi::HGDIOBJ(pen.0));
                let _ = windows::Win32::Graphics::Gdi::Rectangle(hdc, 0, 0, 419, 89);
                SelectObject(hdc, old);
                let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(pen.0));
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));
                let text = get_text();
                let display = if text.is_empty() { "Neno Notch".to_string() } else { text };
                let v: Vec<u16> = display.encode_utf16().collect();
                TextOutW(hdc, 16, 8, &v);

                // Draw CPU/RAM history graph (right side, small)
                if let Ok(cpu_hist) = CPU_HISTORY.lock() {
                    if let Ok(ram_hist) = RAM_HISTORY.lock() {
                        let graph_x = 260i32;
                        let graph_y = 10i32;
                        let graph_w = 150i32;
                        let graph_h = 70i32;
                        // CPU line (green)
                        let cpu_brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x0000FF00));
                        let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(cpu_brush.0));
                        // Draw CPU as green line
                        if cpu_hist.len() > 1 {
                            let pen = CreatePen(PS_SOLID, 1, windows::Win32::Foundation::COLORREF(0x0000FF00));
                            let old = SelectObject(hdc, windows::Win32::Graphics::Gdi::HGDIOBJ(pen.0));
                            let step = graph_w as f32 / (cpu_hist.len() as f32 - 1.0).max(1.0);
                            let first = cpu_hist[0];
                            let _ = MoveToEx(hdc, graph_x, graph_y + graph_h - (first / 100.0 * graph_h as f32) as i32, None);
                            for (i, &val) in cpu_hist.iter().enumerate().skip(1) {
                                let px = graph_x + (i as f32 * step) as i32;
                                let py = graph_y + graph_h - (val / 100.0 * graph_h as f32) as i32;
                                let _ = LineTo(hdc, px, py);
                            }
                            SelectObject(hdc, old);
                            let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(pen.0));
                        }
                        // Draw RAM as blue line
                        if ram_hist.len() > 1 {
                            let pen = CreatePen(PS_SOLID, 1, windows::Win32::Foundation::COLORREF(0x00FF0000));
                            let old = SelectObject(hdc, windows::Win32::Graphics::Gdi::HGDIOBJ(pen.0));
                            let step = graph_w as f32 / (ram_hist.len() as f32 - 1.0).max(1.0);
                            let first = ram_hist[0];
                            let _ = MoveToEx(hdc, graph_x, graph_y + graph_h - (first / 100.0 * graph_h as f32) as i32, None);
                            for (i, &val) in ram_hist.iter().enumerate().skip(1) {
                                let px = graph_x + (i as f32 * step) as i32;
                                let py = graph_y + graph_h - (val / 100.0 * graph_h as f32) as i32;
                                let _ = LineTo(hdc, px, py);
                            }
                            SelectObject(hdc, old);
                            let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(pen.0));
                        }
                    }
                }
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            crate::shelf::handle_drop_files(wparam.0 as isize);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            crate::panel::toggle_panel();
            LRESULT(0)
        }
        WM_KEYDOWN => {
            match wparam.0 as u32 {
                x if x == windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE.0 as u32 => crate::media::MediaSession::play(),
                x if x == windows::Win32::UI::Input::KeyboardAndMouse::VK_RIGHT.0 as u32 => crate::media::MediaSession::next(),
                x if x == windows::Win32::UI::Input::KeyboardAndMouse::VK_LEFT.0 as u32 => crate::media::MediaSession::prev(),
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

#[allow(dead_code)]
fn render_d2d(hwnd: HWND) -> Result<(), windows::core::Error> {
    unsafe {
        let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let props = D2D1_RENDER_TARGET_PROPERTIES::default();
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U { width: 420, height: 90 },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let rt: ID2D1HwndRenderTarget =
            factory.CreateHwndRenderTarget(&props as *const _, &hwnd_props as *const _)?;
        rt.BeginDraw();
        let bg = D2D1_COLOR_F { r: 0.1, g: 0.1, b: 0.1, a: 0.95 };
        rt.Clear(Some(&bg));
        let rect = D2D_RECT_F { left: 8.0, top: 8.0, right: 412.0, bottom: 82.0 };
        let color = D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let brush: ID2D1SolidColorBrush = rt.CreateSolidColorBrush(&color as *const _, None)?;
        rt.DrawRectangle(&rect, &brush, 1.0, None);

        let dw_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let family: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
        let locale: Vec<u16> = "en-us\0".encode_utf16().collect();
        let tf: IDWriteTextFormat = dw_factory.CreateTextFormat(
            PCWSTR(family.as_ptr()),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            18.0,
            PCWSTR(locale.as_ptr()),
        )?;
        let txt = get_text();
        let display = if txt.is_empty() { "Neno — notch".to_string() } else { txt };
        let text: Vec<u16> = display.encode_utf16().collect();
        let text_rect = D2D_RECT_F { left: 16.0, top: 20.0, right: 400.0, bottom: 70.0 };
        let _ = rt.DrawText(
            &text,
            &tf,
            &text_rect as *const _,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        let _ = rt.EndDraw(None, None);
    }
    Ok(())
}
