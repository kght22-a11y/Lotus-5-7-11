//! Lotus 5-7-11 Sovereign Workspace — Win32 shell
//!
//! All application logic lives in `lotus_core::App`. This file is the whole of
//! what Windows contributes:
//!
//!   1. a window and a message loop,
//!   2. translation of Win32 messages into `lotus_core::Event`,
//!   3. blitting `App::render()` to the window (`SetDIBitsToDevice`),
//!   4. an implementation of `lotus_core::Host` (Win32 clipboard, `std::fs` files).
//!
//! A shell for another platform (X11, a framebuffer, BareMetal OS) supplies the
//! same four things and nothing else.

#![windows_subsystem = "windows"]

mod win32;

use core::ffi::c_void;
use std::ptr;
use win32::*;

use lotus_core::{App, Event, Host, Key, Mods, PointerButton};

const GWLP_USERDATA: i32 = -21;
const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12; // Alt

// Keys and messages beyond what win32.rs declares.
const VK_TAB: usize = 0x09;
const VK_PRIOR: usize = 0x21; // Page Up
const VK_NEXT: usize = 0x22; // Page Down
const VK_END: usize = 0x23;
const VK_HOME: usize = 0x24;
const VK_F1: usize = 0x70;
const VK_F12: usize = 0x7B;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_MOUSEHWHEEL: u32 = 0x020E;
const MK_LBUTTON: usize = 0x0001;
const MK_SHIFT: usize = 0x0004;
/// One wheel notch is 120 units; a notch scrolls three rows.
const WHEEL_DELTA: i32 = 120;
const ROWS_PER_NOTCH: i32 = 3;

// While the left button is held the window keeps receiving mouse moves, even
// outside it, so a drag selection can run past the window edge.
#[link(name = "user32")]
extern "system" {
    fn SetCapture(hwnd: HWND) -> HWND;
    fn ReleaseCapture() -> i32;
}

/// The OS services `App` asks for, answered by Windows.
struct Win32Host {
    hwnd: HWND,
}

impl Host for Win32Host {
    fn clipboard_get(&mut self) -> Option<String> {
        Win32Clipboard::get_text(self.hwnd)
    }

    fn clipboard_set(&mut self, text: &str) {
        Win32Clipboard::set_text(self.hwnd, text);
    }

    fn read_file(&mut self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    fn write_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        std::fs::write(path, bytes).map_err(|e| e.to_string())
    }

    fn request_quit(&mut self) {
        unsafe { PostQuitMessage(0) };
    }
}

unsafe fn current_mods() -> Mods {
    Mods {
        ctrl: (GetKeyState(VK_CONTROL) as i16) < 0,
        shift: (GetKeyState(VK_SHIFT) as i16) < 0,
        alt: (GetKeyState(VK_MENU) as i16) < 0,
    }
}

/// Virtual-key code to `Key`.
fn translate_key(vk: usize, mods: Mods) -> Option<Key> {
    match vk {
        VK_UP => Some(Key::Up),
        VK_DOWN => Some(Key::Down),
        VK_LEFT => Some(Key::Left),
        VK_RIGHT => Some(Key::Right),
        VK_RETURN => Some(Key::Enter),
        VK_ESCAPE => Some(Key::Escape),
        VK_BACK => Some(Key::Backspace),
        VK_DELETE => Some(Key::Delete),
        VK_HOME => Some(Key::Home),
        VK_END => Some(Key::End),
        VK_PRIOR => Some(Key::PageUp),
        VK_NEXT => Some(Key::PageDown),
        VK_TAB => Some(Key::Tab),
        VK_F1..=VK_F12 => Some(Key::F((vk - VK_F1 + 1) as u8)),
        // Letters and digits only count as chords (Ctrl held); plain typing arrives via WM_CHAR.
        0x41..=0x5A if mods.ctrl => Some(Key::Char((vk as u8).to_ascii_lowercase() as char)),
        0x30..=0x39 if mods.ctrl => Some(Key::Char(vk as u8 as char)),
        _ => None,
    }
}

fn pointer_pos(lparam: isize) -> (f32, f32) {
    let x = (lparam & 0xFFFF) as i16 as f32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as f32;
    (x, y)
}

/// Wheel rotation carried in the high word of `wparam`, as signed units.
fn wheel_delta(wparam: usize) -> i32 {
    i32::from(((wparam >> 16) & 0xFFFF) as u16 as i16)
}

unsafe fn paint(hwnd: HWND, app: &mut App) {
    let mut ps: PAINTSTRUCT = core::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);

    let frame = app.render();
    let width = frame.width as i32;
    let height = frame.height as i32;

    let mut bmi: BITMAPINFO = core::mem::zeroed();
    bmi.bmiHeader.biSize = core::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = width;
    bmi.bmiHeader.biHeight = -height; // top-down DIB
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    SetDIBitsToDevice(
        hdc,
        0,
        0,
        width as u32,
        height as u32,
        0,
        0,
        0,
        height as u32,
        frame.pixels.as_ptr() as *const c_void,
        &bmi,
        DIB_RGB_COLORS,
    );

    EndPaint(hwnd, &ps);
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    let app_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if app_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let app = &mut *app_ptr;
    let mut host = Win32Host { hwnd };

    let event = match msg {
        WM_PAINT => {
            paint(hwnd, app);
            return 0;
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            return 0;
        }
        WM_SIZE => Some(Event::Resize {
            width: (lparam & 0xFFFF) as usize,
            height: ((lparam >> 16) & 0xFFFF) as usize,
        }),
        WM_LBUTTONDOWN => {
            SetCapture(hwnd);
            let (x, y) = pointer_pos(lparam);
            Some(Event::PointerDown { x, y, button: PointerButton::Left })
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let (x, y) = pointer_pos(lparam);
            Some(Event::PointerUp { x, y, button: PointerButton::Left })
        }
        WM_RBUTTONUP => {
            let (x, y) = pointer_pos(lparam);
            Some(Event::PointerUp { x, y, button: PointerButton::Right })
        }
        WM_RBUTTONDOWN => {
            let (x, y) = pointer_pos(lparam);
            Some(Event::PointerDown { x, y, button: PointerButton::Right })
        }
        WM_MOUSEMOVE => {
            let (x, y) = pointer_pos(lparam);
            Some(Event::PointerMove { x, y, left_down: wparam & MK_LBUTTON != 0 })
        }
        WM_MOUSEWHEEL => {
            // Wheel up is positive; scrolling up is fewer rows. Shift+wheel scrolls sideways.
            let amount = -(wheel_delta(wparam) * ROWS_PER_NOTCH) / WHEEL_DELTA;
            if wparam & MK_SHIFT != 0 {
                Some(Event::Scroll { cols: amount, rows: 0 })
            } else {
                Some(Event::Scroll { cols: 0, rows: amount })
            }
        }
        WM_MOUSEHWHEEL => {
            let amount = (wheel_delta(wparam) * ROWS_PER_NOTCH) / WHEEL_DELTA;
            Some(Event::Scroll { cols: amount, rows: 0 })
        }
        WM_KEYDOWN => {
            let mods = current_mods();
            translate_key(wparam, mods).map(|key| Event::Key { key, mods })
        }
        WM_CHAR => {
            // Ctrl+letter also produces a WM_CHAR control code; chords are handled via WM_KEYDOWN.
            let ch = char::from_u32(wparam as u32).unwrap_or('\0');
            if current_mods().ctrl { None } else { Some(Event::Text(ch)) }
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    };

    if let Some(event) = event {
        app.handle_event(&mut host, event);
    }
    if app.take_dirty() {
        InvalidateRect(hwnd, ptr::null(), 0);
    }
    0
}

fn main() {
    let file_path = std::env::args().nth(1);

    let class_name = to_wide_chars("Lotus5711WindowClass");
    let window_title = to_wide_chars("Lotus 5-7-11 Sovereign Workspace");

    unsafe {
        let h_instance = GetModuleHandleW(ptr::null());

        let mut wc: WNDCLASSEXW = core::mem::zeroed();
        wc.cbSize = core::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_HREDRAW | CS_VREDRAW | CS_OWNDC;
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = h_instance;
        wc.hCursor = LoadCursorW(ptr::null_mut(), 32512 as *const u16); // IDC_ARROW
        wc.lpszClassName = class_name.as_ptr();

        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            100,
            850,
            620,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            eprintln!("Failed to create Win32 window.");
            return;
        }

        let mut client_rect: RECT = core::mem::zeroed();
        GetClientRect(hwnd, &mut client_rect);
        let client_w = (client_rect.right - client_rect.left) as usize;
        let client_h = (client_rect.bottom - client_rect.top) as usize;

        let mut host = Win32Host { hwnd };
        let app = Box::new(App::new(&mut host, client_w, client_h, file_path));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app) as isize);

        ShowWindow(hwnd, 1);
        UpdateWindow(hwnd);

        let mut msg: MSG = core::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Free the app state on exit.
        let app_raw = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut App;
        if !app_raw.is_null() {
            drop(Box::from_raw(app_raw));
        }
    }
}
