use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor, TextOutW, PAINTSTRUCT, TRANSPARENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassExW,
    ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, SW_HIDE, SW_SHOW, WM_CHAR, WM_CLOSE,
    WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_PAINT, WM_RBUTTONDOWN, WNDCLASSEXW, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW,
};
use windows::core::PCWSTR;

#[allow(static_mut_refs)]
static PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);
#[allow(static_mut_refs)]
static mut PANEL_HWND: Option<HWND> = None;
static INPUT: Mutex<String> = Mutex::new(String::new());

fn panel_hwnd() -> Option<HWND> {
    unsafe { PANEL_HWND }
}

fn set_panel_hwnd(hwnd: HWND) {
    unsafe { PANEL_HWND = Some(hwnd); }
}

pub fn create_panel() -> Result<HWND, windows::core::Error> {
    unsafe {
        let class_name: Vec<u16> = "NenoPanel\0".encode_utf16().collect();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(panel_proc),
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

        let title: Vec<u16> = "Neno Panel\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            100, 120, 340, 500,
            None, None, wc.hInstance, None,
        )?;
        set_panel_hwnd(hwnd);
        Ok(hwnd)
    }
}

pub fn toggle_panel() {
    if let Some(hwnd) = panel_hwnd() {
        let now = !PANEL_VISIBLE.load(Ordering::SeqCst);
        PANEL_VISIBLE.store(now, Ordering::SeqCst);
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::ShowWindow;
            if now {
                ShowWindow(hwnd, SW_SHOW);
            } else {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

pub fn show_panel() {
    if let Some(hwnd) = panel_hwnd() {
        PANEL_VISIBLE.store(true, Ordering::SeqCst);
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOW};
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
    }
}

unsafe extern "system" fn panel_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT { left: 0, top: 0, right: 340, bottom: 500 };
                let brush = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00202020));
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(windows::Win32::Graphics::Gdi::HGDIOBJ(brush.0));
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, windows::Win32::Foundation::COLORREF(0x00FFFFFF));

                let mut y = 10i32;
                let header: Vec<u16> = "Neno - To-Do & Water".encode_utf16().collect();
                TextOutW(hdc, 10, y, &header);
                y += 24;

                let todo = crate::todo::TodoList::load();
                for item in &todo.items {
                    let prefix = if item.done { "[x] " } else { "[ ] " };
                    let line = format!("{}{}", prefix, item.text);
                    let v: Vec<u16> = line.encode_utf16().collect();
                    TextOutW(hdc, 10, y, &v);
                    y += 18;
                }
                if todo.items.is_empty() {
                    let empty: Vec<u16> = "No items - type below, Enter to add".encode_utf16().collect();
                    TextOutW(hdc, 10, y, &empty);
                    y += 18;
                }

                y += 12;
                let water = crate::water::WaterReminder::load();
                let wline = format!("Water: {}/{} ml", water.consumed_ml, (water.goal_liters * 1000.0) as u32);
                let wv: Vec<u16> = wline.encode_utf16().collect();
                TextOutW(hdc, 10, y, &wv);
                y += 18;
                let prog = format!("Progress: {:.0}%", water.progress() * 100.0);
                let pv: Vec<u16> = prog.encode_utf16().collect();
                TextOutW(hdc, 10, y, &pv);
                y += 18;
                // Drink button hint
                let hint: Vec<u16> = "Click here to drink 250ml".encode_utf16().collect();
                TextOutW(hdc, 10, y, &hint);

                y += 24;
                let input = INPUT.lock().map(|s| s.clone()).unwrap_or_default();
                let prompt = format!("> {}", input);
                let iv: Vec<u16> = prompt.encode_utf16().collect();
                TextOutW(hdc, 10, y, &iv);

                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_CHAR => {
            let ch = (wparam.0 as u32) as u16;
            if ch == 13 {
                let text = INPUT.lock().map(|s| s.clone()).unwrap_or_default();
                if !text.trim().is_empty() {
                    let mut todo = crate::todo::TodoList::load();
                    todo.add(text.trim().to_string());
                    todo.save();
                    if let Ok(mut inp) = INPUT.lock() { inp.clear(); }
                }
            } else if ch == 8 {
                if let Ok(mut inp) = INPUT.lock() { inp.pop(); }
            } else if ch >= 32 {
                if let Some(c) = char::from_u32(ch as u32) {
                    if let Ok(mut inp) = INPUT.lock() { inp.push(c); }
                }
            }
            unsafe {
                use windows::Win32::Graphics::Gdi::InvalidateRect;
                InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
            if y >= 34 {
                let todo = crate::todo::TodoList::load();
                let idx = ((y - 34) / 18) as usize;
                if idx < todo.items.len() {
                    let mut todo = crate::todo::TodoList::load();
                    todo.toggle(idx);
                    todo.save();
                }
            }
            // Check if click is on "drink" button area (y between water section)
            // Simple: if y > 200, refill water
            if y > 200 && y < 230 {
                let mut water = crate::water::WaterReminder::load();
                water.refill(250);
            }
            unsafe {
                use windows::Win32::Graphics::Gdi::InvalidateRect;
                InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
            if y >= 34 {
                let todo = crate::todo::TodoList::load();
                let idx = ((y - 34) / 18) as usize;
                if idx < todo.items.len() {
                    let mut todo = crate::todo::TodoList::load();
                    todo.remove(idx);
                    todo.save();
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
                PANEL_VISIBLE.store(false, Ordering::SeqCst);
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
