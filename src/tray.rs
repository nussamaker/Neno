use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, AppendMenuW, TrackPopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, GetMessageW, GetSystemMetrics, PostQuitMessage, RegisterClassExW,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, HICON, MSG, SM_CXSMICON, SM_CYSMICON,
    TPM_LEFTALIGN, TPM_BOTTOMALIGN, WM_COMMAND, WM_DESTROY, WM_RBUTTONUP, WM_USER, WNDCLASSEXW,
    WS_OVERLAPPED, MF_STRING,
};
use windows::core::PCWSTR;

/// The character icon embedded in the executable by build.rs.
///
/// Ask for the notification area's actual small-icon size so Windows uses the
/// hand-tuned frame for the current DPI (16 at 100%, 20 at 125%, 24 at 150%,
/// 32 at 200%) instead of downscaling the 32 px frame.
fn resource_icon() -> Option<HICON> {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR};
    unsafe {
        let module = GetModuleHandleW(None).ok()?;
        let cx = GetSystemMetrics(SM_CXSMICON).max(16);
        let cy = GetSystemMetrics(SM_CYSMICON).max(16);
        let handle = LoadImageW(
            HINSTANCE(module.0),
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            cx,
            cy,
            LR_DEFAULTCOLOR,
        )
        .or_else(|_| {
            LoadImageW(
                HINSTANCE(module.0),
                PCWSTR(1 as *const u16),
                IMAGE_ICON,
                32,
                32,
                LR_DEFAULTCOLOR,
            )
        })
        .ok()?;
        if handle.0.is_null() {
            None
        } else {
            Some(HICON(handle.0))
        }
    }
}

/// Switch the process to dark Win32 menus (undocumented uxtheme ordinals 135/136,
/// used by Win32 apps for years). Falls back silently to the system look.
fn enable_dark_menus() {
    use windows::core::PCSTR;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    unsafe {
        let name: Vec<u16> = "uxtheme.dll\0".encode_utf16().collect();
        if let Ok(uxtheme) = LoadLibraryW(PCWSTR(name.as_ptr())) {
            if let Some(proc) = GetProcAddress(uxtheme, PCSTR(135usize as *const u8)) {
                let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 =
                    std::mem::transmute(proc);
                let _ = set_preferred_app_mode(2); // ForceDark
            }
            if let Some(proc) = GetProcAddress(uxtheme, PCSTR(136usize as *const u8)) {
                let flush_menu_themes: unsafe extern "system" fn() = std::mem::transmute(proc);
                flush_menu_themes();
            }
        }
    }
}

const TRAY_MSG: u32 = WM_USER + 1;
const CMD_TOGGLE_PANEL: usize = 1001;
const CMD_TOGGLE_NOTCH: usize = 1002;
const CMD_SETTINGS: usize = 1003;
const CMD_QUIT: usize = 1004;

pub struct TrayIcon {
    pub hwnd: HWND,
}

impl TrayIcon {
    pub fn create() -> Result<Self, windows::core::Error> {
        unsafe {
            enable_dark_menus();
            let class_name: Vec<u16> = "NenoTray\0".encode_utf16().collect();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(tray_proc),
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

            let hwnd = CreateWindowExW(
                Default::default(),
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                WS_OVERLAPPED,
                0, 0, 0, 0,
                None, None, wc.hInstance, None,
            )?;

            use windows::Win32::UI::WindowsAndMessaging::LoadIconW;
            use windows::Win32::UI::WindowsAndMessaging::IDI_APPLICATION;
            // Prefer the embedded cat character; fall back to the system icon.
            let hicon = resource_icon()
                .unwrap_or_else(|| LoadIconW(None, IDI_APPLICATION).unwrap_or_default());
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: TRAY_MSG,
                hIcon: hicon,
                ..Default::default()
            };
            let tip: Vec<u16> = "Neno".encode_utf16().chain(std::iter::once(0)).collect();
            let len = tip.len().min(nid.szTip.len());
            nid.szTip[..len].copy_from_slice(&tip[..len]);
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            Ok(Self { hwnd })
        }
    }

    pub fn remove(&self) {
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.remove();
    }
}

unsafe extern "system" fn tray_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        TRAY_MSG => {
            let event = lparam.0 as u32;
            if event == 0x0202 {
                // WM_LBUTTONUP -> toggle panel
                crate::panel::toggle_panel();
            }
            if event == 0x0203 {
                // WM_LBUTTONDBLCLK -> toggle notch
                crate::notch::toggle_notch();
            }
            if event == 0x0205 {
                // WM_RBUTTONUP -> show context menu
                unsafe {
                    let menu = CreatePopupMenu().unwrap_or_default();
                    let toggle_text: Vec<u16> = "Toggle Panel\0".encode_utf16().collect();
                    let notch_text: Vec<u16> = "Toggle Notch\0".encode_utf16().collect();
                    let settings_text: Vec<u16> = "Settings\0".encode_utf16().collect();
                    let quit_text: Vec<u16> = "Quit\0".encode_utf16().collect();
                    let _ = AppendMenuW(menu, MF_STRING, CMD_TOGGLE_PANEL, PCWSTR(toggle_text.as_ptr()));
                    let _ = AppendMenuW(menu, MF_STRING, CMD_TOGGLE_NOTCH, PCWSTR(notch_text.as_ptr()));
                    let _ = AppendMenuW(menu, MF_STRING, CMD_SETTINGS, PCWSTR(settings_text.as_ptr()));
                    let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, PCWSTR(quit_text.as_ptr()));
                    // Get cursor position
                    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
                    use windows::Win32::Foundation::POINT;
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, None);
                    let _ = DestroyMenu(menu);
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = (wparam.0 & 0xFFFF) as usize;
            match cmd {
                CMD_TOGGLE_PANEL => crate::panel::toggle_panel(),
                CMD_TOGGLE_NOTCH => crate::notch::toggle_notch(),
                CMD_SETTINGS => crate::settings_window::toggle_settings(),
                CMD_QUIT => std::process::exit(0),
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
