use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, AppendMenuW, TrackPopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassExW, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, HICON, MSG, TPM_LEFTALIGN, TPM_BOTTOMALIGN, WM_COMMAND, WM_DESTROY,
    WM_RBUTTONUP, WM_USER, WNDCLASSEXW, WS_OVERLAPPED, MF_STRING,
};
use windows::core::PCWSTR;

/// Paint the Neno mark (dark disc, green ring, white sparkle) into a 32x32 icon.
fn app_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    use windows::Win32::Foundation::{COLORREF, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreatePen, CreateSolidBrush, DeleteDC,
        DeleteObject, Ellipse, FillRect, GetDC, GetStockObject, HGDIOBJ, Polygon, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, NULL_BRUSH, NULL_PEN,
        PS_SOLID,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};

    unsafe {
        let size = 32i32;
        let screen = GetDC(None);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(color) = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = ReleaseDC(None, screen);
            return Default::default();
        };
        let dc = CreateCompatibleDC(screen);

        // Transparent black backdrop, then the dark disc.
        let bg = CreateSolidBrush(COLORREF(0));
        let rc = RECT { left: 0, top: 0, right: size, bottom: size };
        FillRect(dc, &rc, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));

        let old_bitmap = SelectObject(dc, HGDIOBJ(color.0));
        let disc = CreateSolidBrush(COLORREF(0x000A0A0A));
        let old_brush = SelectObject(dc, HGDIOBJ(disc.0));
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        let _ = Ellipse(dc, 1, 1, size - 1, size - 1);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        let _ = DeleteObject(HGDIOBJ(disc.0));

        // Green ring.
        let ring_pen = CreatePen(PS_SOLID, 2, COLORREF(0x0058D130));
        SelectObject(dc, HGDIOBJ(ring_pen.0));
        SelectObject(dc, GetStockObject(NULL_BRUSH));
        let _ = Ellipse(dc, 6, 6, size - 6, size - 6);
        SelectObject(dc, old_brush);
        let _ = DeleteObject(HGDIOBJ(ring_pen.0));

        // White sparkle, matching the notch brand mark.
        let cx = 16;
        let cy = 16;
        let sparkle = [
            windows::Win32::Foundation::POINT { x: cx, y: cy - 8 },
            windows::Win32::Foundation::POINT { x: cx + 2, y: cy - 2 },
            windows::Win32::Foundation::POINT { x: cx + 4, y: cy },
            windows::Win32::Foundation::POINT { x: cx + 2, y: cy + 2 },
            windows::Win32::Foundation::POINT { x: cx, y: cy + 8 },
            windows::Win32::Foundation::POINT { x: cx - 2, y: cy + 2 },
            windows::Win32::Foundation::POINT { x: cx - 4, y: cy },
            windows::Win32::Foundation::POINT { x: cx - 2, y: cy - 2 },
        ];
        let sparkle_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
        let sparkle_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00FFFFFF));
        let ob = SelectObject(dc, HGDIOBJ(sparkle_brush.0));
        let op = SelectObject(dc, HGDIOBJ(sparkle_pen.0));
        let _ = Polygon(dc, &sparkle);
        SelectObject(dc, op);
        SelectObject(dc, ob);
        let _ = DeleteObject(HGDIOBJ(sparkle_pen.0));
        let _ = DeleteObject(HGDIOBJ(sparkle_brush.0));

        SelectObject(dc, old_bitmap);
        let _ = DeleteDC(dc);
        let _ = ReleaseDC(None, screen);

        // Round alpha mask so the icon has soft edges on the taskbar.
        let px = bits as *mut u8;
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 + 0.5 - 16.0;
                let dy = y as f32 + 0.5 - 16.0;
                let d = (dx * dx + dy * dy).sqrt();
                let a = ((15.4 - d).clamp(0.0, 1.0) * 255.0) as u8;
                *px.add(((y * size + x) * 4 + 3) as usize) = a;
            }
        }

        let zero_mask = vec![0u8; 4 * 32];
        let mask = CreateBitmap(size, size, 1, 1, Some(zero_mask.as_ptr() as *const _));
        let info = ICONINFO {
            fIcon: windows::Win32::Foundation::BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        let _ = DeleteObject(HGDIOBJ(mask.0));
        let _ = DeleteObject(HGDIOBJ(color.0));
        icon
    }
}

/// The character icon embedded in the executable by build.rs.
fn resource_icon() -> Option<HICON> {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR};
    unsafe {
        let module = GetModuleHandleW(None).ok()?;
        let handle = LoadImageW(
            HINSTANCE(module.0),
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            32,
            32,
            LR_DEFAULTCOLOR,
        )
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
            // Prefer the embedded cat character; the painted mark is only a fallback.
            let hicon = resource_icon()
                .or_else(|| {
                    let painted = app_icon();
                    (!painted.is_invalid()).then_some(painted)
                })
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
