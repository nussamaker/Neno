use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor, TextOutW, PAINTSTRUCT, TRANSPARENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassExW,
    ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, SW_HIDE, SW_SHOW, WM_CLOSE, WM_DESTROY,
    WM_LBUTTONDOWN, WM_PAINT, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW,
};
use windows::core::PCWSTR;

static mut SETTINGS_VISIBLE: bool = false;
static mut SETTINGS_HWND: Option<HWND> = None;

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

        let title: Vec<u16> = "Neno Settings\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            200, 200, 320, 260,
            None, None, wc.hInstance, None,
        )?;
        SETTINGS_HWND = Some(hwnd);
        Ok(hwnd)
    }
}

pub fn toggle_settings() {
    unsafe {
        if let Some(hwnd) = SETTINGS_HWND {
            SETTINGS_VISIBLE = !SETTINGS_VISIBLE;
            if SETTINGS_VISIBLE {
                ShowWindow(hwnd, SW_SHOW);
            } else {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

unsafe extern "system" fn settings_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT { left: 0, top: 0, right: 320, bottom: 260 };
                let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00202020));
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(brush.0));
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));

                let mut y = 10i32;
                let header: Vec<u16> = "Settings".encode_utf16().collect();
                TextOutW(hdc, 10, y, &header);
                y += 30;

                let water = crate::water::WaterReminder::load();
                let wline = format!("Water goal: {:.1} L (click to cycle)", water.goal_liters);
                let wv: Vec<u16> = wline.encode_utf16().collect();
                TextOutW(hdc, 10, y, &wv);
                y += 24;

                let auto = if crate::autostart::is_autostart() { "Autostart: ON (click to toggle)" } else { "Autostart: OFF (click to toggle)" };
                let av: Vec<u16> = auto.encode_utf16().collect();
                TextOutW(hdc, 10, y, &av);
                y += 24;

                let hint: Vec<u16> = "Click water goal to cycle 1.5/2.0/2.5/3.0".encode_utf16().collect();
                TextOutW(hdc, 10, y, &hint);

                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
            if y >= 30 && y < 54 {
                // Cycle water goal
                let mut water = crate::water::WaterReminder::load();
                water.goal_liters = match water.goal_liters {
                    1.5 => 2.0,
                    2.0 => 2.5,
                    2.5 => 3.0,
                    _ => 1.5,
                };
                water.save();
            } else if y >= 54 && y < 78 {
                // Toggle autostart
                let enabled = !crate::autostart::is_autostart();
                if let Ok(exe) = std::env::current_exe() {
                    crate::autostart::set_autostart(enabled, &exe);
                }
            }
            unsafe {
                use windows::Win32::Graphics::Gdi::InvalidateRect;
                InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::ShowWindow;
                ShowWindow(hwnd, SW_HIDE);
                SETTINGS_VISIBLE = false;
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
