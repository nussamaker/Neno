use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, AppendMenuW, TrackPopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassExW, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, MSG, TPM_LEFTALIGN, TPM_BOTTOMALIGN, WM_COMMAND, WM_DESTROY, WM_RBUTTONUP,
    WM_USER, WNDCLASSEXW, WS_OVERLAPPED, MF_STRING,
};
use windows::core::PCWSTR;

const TRAY_MSG: u32 = WM_USER + 1;
const CMD_TOGGLE_PANEL: usize = 1001;
const CMD_QUIT: usize = 1002;

pub struct TrayIcon {
    pub hwnd: HWND,
}

impl TrayIcon {
    pub fn create() -> Result<Self, windows::core::Error> {
        unsafe {
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
            let hicon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
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
            if event == 0x0205 {
                // WM_RBUTTONUP -> show context menu
                unsafe {
                    let menu = CreatePopupMenu().unwrap_or_default();
                    let toggle_text: Vec<u16> = "Toggle Panel\0".encode_utf16().collect();
                    let quit_text: Vec<u16> = "Quit\0".encode_utf16().collect();
                    let _ = AppendMenuW(menu, MF_STRING, CMD_TOGGLE_PANEL, PCWSTR(toggle_text.as_ptr()));
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
