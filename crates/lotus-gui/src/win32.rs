//! Raw Zero-Dependency Win32 Window & GDI DIB Surface Harness for Lotus 5-7-11.
//!
//! Direct C-ABI FFI linking against standard Windows user32.dll and gdi32.dll.
//! Eliminates bloated third-party windowing crates (winit/softbuffer) while providing
//! native 60fps subpixel vector blitting, mouse events, and clipboard access.

#![allow(non_snake_case, dead_code)]

use core::ffi::c_void;
use std::ptr;

pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type HICON = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HGLOBAL = *mut c_void;

pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
pub const WS_VISIBLE: u32 = 0x10000000;
pub const CS_HREDRAW: u32 = 0x0002;
pub const CS_VREDRAW: u32 = 0x0001;
pub const CS_OWNDC: u32 = 0x0020;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_CHAR: u32 = 0x0102;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_RBUTTONDOWN: u32 = 0x0204;

pub const VK_BACK: usize = 0x08;
pub const VK_RETURN: usize = 0x0D;
pub const VK_ESCAPE: usize = 0x1B;
pub const VK_LEFT: usize = 0x25;
pub const VK_UP: usize = 0x26;
pub const VK_RIGHT: usize = 0x27;
pub const VK_DOWN: usize = 0x28;
pub const VK_DELETE: usize = 0x2E;

pub const DIB_RGB_COLORS: u32 = 0;
pub const SRCCOPY: u32 = 0x00CC0020;
pub const BI_RGB: u32 = 0;
pub const CF_UNICODETEXT: u32 = 13;
pub const GMEM_MOVEABLE: u32 = 0x0002;

#[repr(C)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: usize,
    pub lParam: isize,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: i32,
    pub rcPaint: RECT,
    pub fRestore: i32,
    pub fIncUpdate: i32,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: Option<unsafe extern "system" fn(HWND, u32, usize, isize) -> isize>,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HICON,
}

#[link(name = "user32")]
extern "system" {
    pub fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
    pub fn RegisterClassExW(lpWndClass: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: *mut c_void,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> i32;
    pub fn UpdateWindow(hWnd: HWND) -> i32;
    pub fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) -> isize;
    pub fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    pub fn TranslateMessage(lpMsg: *const MSG) -> i32;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: i32) -> i32;
    pub fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> i32;
    pub fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> i32;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    pub fn GetKeyState(nVirtKey: i32) -> i16;
    pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
    pub fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> isize;

    // Clipboard
    pub fn OpenClipboard(hWndNewOwner: HWND) -> i32;
    pub fn CloseClipboard() -> i32;
    pub fn EmptyClipboard() -> i32;
    pub fn SetClipboardData(uFormat: u32, hMem: *mut c_void) -> *mut c_void;
    pub fn GetClipboardData(uFormat: u32) -> *mut c_void;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn SetDIBitsToDevice(
        hdc: HDC,
        xDest: i32,
        yDest: i32,
        w: u32,
        h: u32,
        xSrc: i32,
        ySrc: i32,
        StartScan: u32,
        cLines: u32,
        lpvBits: *const c_void,
        lpbmi: *const BITMAPINFO,
        ColorUse: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> HGLOBAL;
    pub fn GlobalLock(hMem: HGLOBAL) -> *mut c_void;
    pub fn GlobalUnlock(hMem: HGLOBAL) -> i32;
}

pub fn to_wide_chars(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

/// Native Windows clipboard helper (zero crates).
pub struct Win32Clipboard;

impl Win32Clipboard {
    pub fn get_text(hwnd: HWND) -> Option<String> {
        unsafe {
            if OpenClipboard(hwnd) == 0 {
                return None;
            }
            let h_data = GetClipboardData(CF_UNICODETEXT);
            if h_data.is_null() {
                CloseClipboard();
                return None;
            }
            let ptr_u16 = GlobalLock(h_data) as *const u16;
            if ptr_u16.is_null() {
                CloseClipboard();
                return None;
            }

            let mut len = 0;
            while *ptr_u16.add(len) != 0 {
                len += 1;
            }
            let slice = core::slice::from_raw_parts(ptr_u16, len);
            let text = String::from_utf16_lossy(slice);

            GlobalUnlock(h_data);
            CloseClipboard();
            Some(text)
        }
    }

    pub fn set_text(hwnd: HWND, text: &str) -> bool {
        unsafe {
            if OpenClipboard(hwnd) == 0 {
                return false;
            }
            EmptyClipboard();

            let wide = to_wide_chars(text);
            let byte_count = wide.len() * 2;
            let h_mem = GlobalAlloc(GMEM_MOVEABLE, byte_count);
            if h_mem.is_null() {
                CloseClipboard();
                return false;
            }

            let dest = GlobalLock(h_mem) as *mut u16;
            if !dest.is_null() {
                ptr::copy_nonoverlapping(wide.as_ptr(), dest, wide.len());
                GlobalUnlock(h_mem);
                SetClipboardData(CF_UNICODETEXT, h_mem);
            }

            CloseClipboard();
            true
        }
    }
}
