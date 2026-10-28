use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, DeleteObject, EndPaint, InvalidateRect, PAINTSTRUCT, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_RIGHT, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRect, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    GetSystemMetrics, PostQuitMessage, RegisterClassExW, SetForegroundWindow, ShowWindow,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, SM_CXSCREEN, SW_HIDE, SW_SHOW, WM_CHAR,
    WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONDOWN,
    WNDCLASSEXW, WS_CAPTION, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_SYSMENU,
    WINDOW_STYLE,
};

use crate::ui;

const W: i32 = 340;
const H: i32 = 500;
const ROW_TOP: i32 = 42;
const ROW_H: i32 = 24;
const ROW_MAX: usize = 10;
const STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0);

static PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);
#[allow(static_mut_refs)]
static mut PANEL_HWND: Option<HWND> = None;
static INPUT: Mutex<String> = Mutex::new(String::new());
static SCROLL: AtomicUsize = AtomicUsize::new(0);

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

        let mut rc = RECT { left: 0, top: 0, right: W, bottom: H };
        let _ = AdjustWindowRect(&mut rc, STYLE, false);
        let outer_w = rc.right - rc.left;
        let outer_h = rc.bottom - rc.top;
        let sw = GetSystemMetrics(SM_CXSCREEN);

        let title: Vec<u16> = "Neno Panel\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            STYLE,
            (sw - outer_w) / 2,
            148,
            outer_w,
            outer_h,
            None,
            None,
            wc.hInstance,
            None,
        )?;
        ui::dark_frame(hwnd);
        set_panel_hwnd(hwnd);
        Ok(hwnd)
    }
}

pub fn toggle_panel() {
    if let Some(hwnd) = panel_hwnd() {
        let now = !PANEL_VISIBLE.load(Ordering::SeqCst);
        PANEL_VISIBLE.store(now, Ordering::SeqCst);
        unsafe {
            if now {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            } else {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

pub fn show_panel() {
    if let Some(hwnd) = panel_hwnd() {
        PANEL_VISIBLE.store(true, Ordering::SeqCst);
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

fn task_index(y: i32) -> Option<usize> {
    if y < ROW_TOP || y >= ROW_TOP + ROW_H * ROW_MAX as i32 {
        return None;
    }
    let visible = ((y - ROW_TOP) / ROW_H) as usize;
    let len = crate::todo::TodoList::load().items.len();
    let max_scroll = len.saturating_sub(ROW_MAX);
    let scroll = SCROLL.load(Ordering::Relaxed).min(max_scroll);
    let idx = scroll + visible;
    if idx < len { Some(idx) } else { None }
}

fn repaint(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    ui::fill(hdc, 0, 0, W, H, ui::BG);

    let f_title = ui::font(16, 600);
    let f_body = ui::font(12, 400);
    let f_small = ui::font(11, 400);
    let f_tiny = ui::font(10, 400);
    let f_value = ui::font(13, 600);
    let f_btn = ui::font(11, 600);

    let todo = crate::todo::TodoList::load();
    let open = todo.items.iter().filter(|i| !i.done).count();

    ui::text(hdc, f_title, ui::TEXT, "To-Do", rect(16, 10, 160, 34), DT_LEFT);
    let count = format!("{} open", open);
    ui::text(hdc, f_tiny, ui::DIM, &count, rect(180, 12, W - 16, 32), DT_RIGHT);

    let max_scroll = todo.items.len().saturating_sub(ROW_MAX);
    let scroll = SCROLL.load(Ordering::Relaxed).min(max_scroll);
    if todo.items.is_empty() {
        ui::text(
            hdc,
            f_body,
            ui::DIM,
            "No tasks yet.",
            rect(16, 140, W - 16, 164),
            DT_CENTER,
        );
        ui::text(
            hdc,
            f_small,
            ui::DIM,
            "Add one below and press Enter.",
            rect(16, 164, W - 16, 186),
            DT_CENTER,
        );
    } else {
        for (vi, item) in todo.items.iter().skip(scroll).take(ROW_MAX).enumerate() {
            let y = ROW_TOP + vi as i32 * ROW_H;
            ui::checkbox(hdc, 16, y + 5, item.done, ui::GREEN);
            let color = if item.done { ui::DONE } else { ui::TEXT };
            ui::text(
                hdc,
                f_body,
                color,
                &item.text,
                rect(38, y + 2, W - 18, y + 22),
                DT_LEFT | DT_END_ELLIPSIS,
            );
        }
        if todo.items.len() > scroll + ROW_MAX {
            let more = format!("+{} more", todo.items.len() - scroll - ROW_MAX);
            ui::text(hdc, f_tiny, ui::DIM, &more, rect(38, 282, W - 16, 296), DT_LEFT);
        }
    }

    ui::fill(hdc, 16, 294, W - 16, 295, 0x002A2A2D);

    let water = crate::water::WaterReminder::load();
    ui::text(hdc, f_value, ui::TEXT, "Water", rect(16, 300, 120, 324), DT_LEFT);
    let value = format!(
        "{:.2} / {:.1} L",
        water.consumed_ml as f32 / 1000.0,
        water.goal_liters
    );
    ui::text(hdc, f_small, ui::DIM, &value, rect(150, 302, W - 16, 324), DT_RIGHT);
    ui::progress(hdc, 16, 328, W - 16, water.progress(), ui::GREEN, ui::TRACK);
    let pct = format!("{:.0}% of daily goal", water.progress() * 100.0);
    ui::text(hdc, f_tiny, ui::DIM, &pct, rect(16, 338, 240, 356), DT_LEFT);
    if water.snoozed {
        ui::text(
            hdc,
            f_tiny,
            ui::DIM,
            "snoozed",
            rect(W - 100, 338, W - 16, 356),
            DT_RIGHT,
        );
    }

    ui::pill(hdc, 16, 360, 112, 28, ui::GREEN, 0x00000000, "Drank +250", f_btn);
    ui::pill(hdc, 136, 360, 86, 28, ui::CARD, ui::TEXT, "Snooze", f_btn);
    ui::pill(hdc, 230, 360, 94, 28, ui::CARD, ui::TEXT, "Reset", f_btn);

    ui::fill(hdc, 16, 400, W - 16, 401, 0x002A2A2D);

    ui::text(hdc, f_tiny, ui::DIM, "Add a task", rect(16, 406, 200, 424), DT_LEFT);
    ui::outline(hdc, 16, 424, W - 16, 454, 10, ui::FAINT, ui::CARD);
    let input = INPUT.lock().map(|s| s.clone()).unwrap_or_default();
    if input.is_empty() {
        ui::text(
            hdc,
            f_small,
            ui::DIM,
            "Type here and press Enter",
            rect(28, 426, W - 28, 452),
            DT_LEFT,
        );
    } else {
        let line = format!("{}|", input);
        ui::text(
            hdc,
            f_small,
            ui::TEXT,
            &line,
            rect(28, 426, W - 28, 452),
            DT_LEFT | DT_END_ELLIPSIS,
        );
    }

    ui::text(
        hdc,
        f_tiny,
        ui::DIM,
        "Click a task to toggle  ·  right-click to remove",
        rect(16, 462, W - 16, 480),
        DT_LEFT,
    );

    for f in [f_title, f_body, f_small, f_tiny, f_value, f_btn] {
        let _ = DeleteObject(HGDIOBJ(f.0));
    }
    let _ = EndPaint(hwnd, &ps);
}

fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
    RECT { left: l, top: t, right: r, bottom: b }
}

unsafe extern "system" fn panel_proc(
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
        WM_CHAR => {
            let ch = (wparam.0 as u32) as u16;
            if ch == 13 {
                let text = INPUT.lock().map(|s| s.clone()).unwrap_or_default();
                if !text.trim().is_empty() {
                    let mut todo = crate::todo::TodoList::load();
                    todo.add(text.trim().to_string());
                    todo.save();
                    if let Ok(mut inp) = INPUT.lock() {
                        inp.clear();
                    }
                    let max_scroll = todo.items.len().saturating_sub(ROW_MAX);
                    SCROLL.store(max_scroll, Ordering::Relaxed);
                }
            } else if ch == 8 {
                if let Ok(mut inp) = INPUT.lock() {
                    inp.pop();
                }
            } else if ch >= 32 {
                if let Some(c) = char::from_u32(ch as u32) {
                    if let Ok(mut inp) = INPUT.lock() {
                        inp.push(c);
                    }
                }
            }
            repaint(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as u16 as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            if let Some(idx) = task_index(y) {
                let mut todo = crate::todo::TodoList::load();
                todo.toggle(idx);
                todo.save();
                repaint(hwnd);
                return LRESULT(0);
            }
            if (360..388).contains(&y) {
                let mut water = crate::water::WaterReminder::load();
                if (16..128).contains(&x) {
                    water.refill(250);
                } else if (136..222).contains(&x) {
                    water.snooze();
                } else if (230..324).contains(&x) {
                    water.consumed_ml = 0;
                    water.snoozed = false;
                    water.save();
                }
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            if let Some(idx) = task_index(y) {
                let mut todo = crate::todo::TodoList::load();
                todo.remove(idx);
                todo.save();
                let max_scroll = todo.items.len().saturating_sub(ROW_MAX);
                let scroll = SCROLL.load(Ordering::Relaxed).min(max_scroll);
                SCROLL.store(scroll, Ordering::Relaxed);
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = (wparam.0 >> 16) as u16 as i16;
            let len = crate::todo::TodoList::load().items.len();
            let max_scroll = len.saturating_sub(ROW_MAX);
            let mut scroll = SCROLL.load(Ordering::Relaxed);
            if delta < 0 {
                scroll = (scroll + 1).min(max_scroll);
            } else {
                scroll = scroll.saturating_sub(1);
            }
            SCROLL.store(scroll, Ordering::Relaxed);
            repaint(hwnd);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u32
                == windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0 as u32
            {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    PANEL_VISIBLE.store(false, Ordering::SeqCst);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
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
