//! Windows snap support — `WndProc` subclass for borderless windows.
//!
//! Adds `WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION` back to a
//! borderless window, hides the OS title bar via `DwmExtendFrameIntoClientArea`, and
//! installs a subclass that handles `WM_NCCALCSIZE` and `WM_NCHITTEST` to enable
//! Aero Snap (drag-to-edge, Win+Arrow, snap layouts).
//!
//! This is the standard approach used by Chrome, `WezTerm`, and Windows Terminal.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LRESULT, RECT};
use windows_sys::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, IsZoomed,
    SetWindowLongPtrW, SetWindowPos,
    GWL_STYLE,
    HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT,
    HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
    NCCALCSIZE_PARAMS,
    SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYFRAME,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    WM_DPICHANGED, WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST,
    WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
};

const SUBCLASS_ID: usize = 0xBEEF;

struct SnapData {
    resize_border: i32,
    caption_height: i32,
    client_rects: Mutex<Vec<[i32; 4]>>,
    /// When true, `WM_DPICHANGED` is suppressed to prevent resize oscillation
    /// during manual window positioning (tab tear-off drag).
    dragging: AtomicBool,
    /// DPI from the most recent `WM_DPICHANGED` message.  Since we eat
    /// `WM_DPICHANGED` (don't pass it to `DefSubclassProc`), winit never fires
    /// `ScaleFactorChanged`.  The app reads this in `handle_resize` to update
    /// `self.scale_factor`.  0 means no DPI change has been received yet.
    last_dpi: AtomicU32,
}

/// Global map from HWND (as usize) → `SnapData` pointer so `set_client_rects` can
/// find it. Raw pointers don't implement `Hash`, so we store HWND as `usize`.
static SNAP_PTRS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

fn snap_ptrs() -> &'static Mutex<HashMap<usize, usize>> {
    SNAP_PTRS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_x_lparam(lp: isize) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let v = (lp & 0xFFFF) as i16;
    i32::from(v)
}

fn get_y_lparam(lp: isize) -> i32 {
    #[allow(clippy::cast_possible_truncation)]
    let v = ((lp >> 16) & 0xFFFF) as i16;
    i32::from(v)
}

/// Extract HWND from a winit Window.
fn hwnd_from_window(window: &winit::window::Window) -> Option<HWND> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}

/// Install snap support on a borderless window.
///
/// Adds `WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION` so that
/// Windows recognizes the window for Aero Snap, hides the OS title bar via DWM,
/// and installs a `WndProc` subclass for `WM_NCCALCSIZE` and `WM_NCHITTEST`.
#[allow(unsafe_code)]
pub fn enable_snap(window: &winit::window::Window, resize_border: i32, caption_height: i32) {
    let Some(hwnd) = hwnd_from_window(window) else {
        return;
    };

    // SAFETY: All calls are standard Win32 API functions operating on a valid HWND
    // obtained from winit. The SnapData pointer is heap-allocated and will be freed
    // in the WM_NCDESTROY handler.
    unsafe {
        // 1. Add snap-enabling styles back to the borderless window
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let snap_bits = (WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION) as isize;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style | snap_bits);

        // Force Windows to re-evaluate the frame after style change
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0, 0, 0, 0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );

        // 2. Hide OS title bar — 1px top margin keeps DWM shadow + snap preview
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 1,
            cyBottomHeight: 0,
        };
        DwmExtendFrameIntoClientArea(hwnd, &raw const margins);

        // 3. Install `WndProc` subclass
        let data = Box::new(SnapData {
            resize_border,
            caption_height,
            client_rects: Mutex::new(Vec::new()),
            dragging: AtomicBool::new(false),
            last_dpi: AtomicU32::new(0),
        });
        let data_ptr = Box::into_raw(data);
        SetWindowSubclass(
            hwnd,
            Some(subclass_proc),
            SUBCLASS_ID,
            data_ptr as usize,
        );

        // Register pointer so set_client_rects can find it
        if let Ok(mut map) = snap_ptrs().lock() {
            map.insert(hwnd as usize, data_ptr as usize);
        }
    }
}

/// Update the interactive regions that should receive mouse clicks (`HTCLIENT`)
/// rather than being treated as caption/drag area (`HTCAPTION`).
///
/// Each rect is `[left, top, right, bottom]` in client coordinates.
/// Call this whenever the tab bar layout changes (resize, tab add/remove).
pub fn set_client_rects(window: &winit::window::Window, rects: Vec<[i32; 4]>) {
    let Some(hwnd) = hwnd_from_window(window) else {
        return;
    };
    let ptr = {
        let map = match snap_ptrs().lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        match map.get(&(hwnd as usize)) {
            Some(&p) => p,
            None => return,
        }
    };
    // SAFETY: The pointer was created by Box::into_raw in enable_snap and remains
    // valid until WM_NCDESTROY. We only access the Mutex-protected field.
    #[allow(unsafe_code)]
    let data = unsafe { &*(ptr as *const SnapData) };
    if let Ok(mut rects_lock) = data.client_rects.lock() {
        *rects_lock = rects;
    }
}

/// Suppress `WM_DPICHANGED` during manual window positioning (tab tear-off drag).
///
/// When `dragging` is true, the subclass eats `WM_DPICHANGED` to prevent winit from
/// resizing the window at per-monitor DPI boundaries (which causes oscillation).
/// Call with `false` when the drag ends so normal DPI handling resumes.
pub fn set_dragging(window: &winit::window::Window, dragging: bool) {
    let Some(hwnd) = hwnd_from_window(window) else {
        return;
    };
    let ptr = {
        let map = match snap_ptrs().lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        match map.get(&(hwnd as usize)) {
            Some(&p) => p,
            None => return,
        }
    };
    // SAFETY: The pointer was created by Box::into_raw in enable_snap and remains
    // valid until WM_NCDESTROY. AtomicBool is safe to access from any thread.
    #[allow(unsafe_code)]
    let data = unsafe { &*(ptr as *const SnapData) };
    data.dragging.store(dragging, Ordering::Relaxed);
}

/// Read the DPI stored by the last `WM_DPICHANGED` message.
///
/// Returns the scale factor (DPI / 96.0), or `None` if no DPI change has been
/// received since `enable_snap` was called.
pub fn get_current_dpi(window: &winit::window::Window) -> Option<f64> {
    let hwnd = hwnd_from_window(window)?;
    let ptr = {
        let map = snap_ptrs().lock().ok()?;
        *map.get(&(hwnd as usize))?
    };
    // SAFETY: The pointer was created by Box::into_raw in enable_snap and remains
    // valid until WM_NCDESTROY. AtomicU32 is safe to access from any thread.
    #[allow(unsafe_code)]
    let data = unsafe { &*(ptr as *const SnapData) };
    let dpi = data.last_dpi.load(Ordering::Relaxed);
    if dpi == 0 {
        None
    } else {
        Some(dpi as f64 / 96.0)
    }
}

/// `WndProc` subclass callback — handles `WM_NCCALCSIZE`, `WM_NCHITTEST`, `WM_NCDESTROY`.
///
/// # Safety
///
/// Called by Windows as a subclass procedure. `ref_data` must be a valid pointer to
/// a `SnapData` allocated by `enable_snap`.
#[allow(unsafe_code)]
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _uid: usize,
    ref_data: usize,
) -> LRESULT {
    // SAFETY: All operations within use valid Win32 API calls on a valid HWND.
    // The ref_data pointer was allocated by enable_snap and remains valid until
    // WM_NCDESTROY frees it.
    unsafe {
        match msg {
            WM_NCCALCSIZE => {
                if wparam == 1 {
                    // wparam=TRUE: lparam points to NCCALCSIZE_PARAMS.
                    // Return 0 so the entire window is client area (no OS frame).
                    // When maximized, inset by frame thickness to prevent overflow.
                    if IsZoomed(hwnd) != 0 {
                        let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                        let frame_x = GetSystemMetrics(SM_CXFRAME)
                            + GetSystemMetrics(SM_CXPADDEDBORDER);
                        let frame_y = GetSystemMetrics(SM_CYFRAME)
                            + GetSystemMetrics(SM_CXPADDEDBORDER);
                        params.rgrc[0].left += frame_x;
                        params.rgrc[0].top += frame_y;
                        params.rgrc[0].right -= frame_x;
                        params.rgrc[0].bottom -= frame_y;
                    }
                    return 0;
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }

            WM_NCHITTEST => {
                let data = &*(ref_data as *const SnapData);

                // Cursor position in screen coordinates
                let cursor_x = get_x_lparam(lparam);
                let cursor_y = get_y_lparam(lparam);

                // Window rect in screen coordinates
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                GetWindowRect(hwnd, &raw mut rect);

                // Convert to client-relative coordinates
                let x = cursor_x - rect.left;
                let y = cursor_y - rect.top;
                let w = rect.right - rect.left;
                let h = rect.bottom - rect.top;

                let border = data.resize_border;

                // Resize edges (not when maximized — no resize border needed)
                if IsZoomed(hwnd) == 0 {
                    let on_left = x < border;
                    let on_right = x >= w - border;
                    let on_top = y < border;
                    let on_bottom = y >= h - border;

                    if on_top && on_left {
                        return HTTOPLEFT as LRESULT;
                    }
                    if on_top && on_right {
                        return HTTOPRIGHT as LRESULT;
                    }
                    if on_bottom && on_left {
                        return HTBOTTOMLEFT as LRESULT;
                    }
                    if on_bottom && on_right {
                        return HTBOTTOMRIGHT as LRESULT;
                    }
                    if on_left {
                        return HTLEFT as LRESULT;
                    }
                    if on_right {
                        return HTRIGHT as LRESULT;
                    }
                    if on_top {
                        return HTTOP as LRESULT;
                    }
                    if on_bottom {
                        return HTBOTTOM as LRESULT;
                    }
                }

                // Caption area (tab bar height) — enables OS drag-to-snap
                if y < data.caption_height {
                    // Check interactive regions (tabs, buttons, controls)
                    if let Ok(rects) = data.client_rects.lock() {
                        for r in rects.iter() {
                            if x >= r[0] && y >= r[1] && x < r[2] && y < r[3] {
                                return HTCLIENT as LRESULT;
                            }
                        }
                    }
                    return HTCAPTION as LRESULT;
                }

                HTCLIENT as LRESULT
            }

            WM_DPICHANGED => {
                let data = &*(ref_data as *const SnapData);

                if data.dragging.load(Ordering::Relaxed) {
                    // During tab tear-off drag, suppress all DPI handling to
                    // prevent resize oscillation at per-monitor boundaries.
                    return 0;
                }

                // Store the new DPI so the app can pick it up in handle_resize.
                // HIWORD(wParam) = new Y-axis DPI (X and Y are always equal).
                let new_dpi = ((wparam >> 16) & 0xFFFF) as u32;
                data.last_dpi.store(new_dpi, Ordering::Relaxed);

                // Apply the suggested rect from Windows (lParam → RECT*).
                // Windows calculates this rect to prevent DPI oscillation.
                // We handle it ourselves instead of letting winit do its own
                // (potentially incorrect) resize that causes oscillation at
                // per-monitor DPI boundaries.
                let suggested = &*(lparam as *const RECT);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                0
            }

            WM_NCDESTROY => {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                // Remove from global map
                if let Ok(mut map) = snap_ptrs().lock() {
                    map.remove(&(hwnd as usize));
                }
                // Free the SnapData
                drop(Box::from_raw(ref_data as *mut SnapData));
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }

            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}
