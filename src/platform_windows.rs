//! Windows snap support — `WndProc` subclass for borderless windows.
//!
//! Adds `WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION` back to a
//! borderless window, hides the OS title bar via `DwmExtendFrameIntoClientArea`, and
//! installs a subclass that handles `WM_NCCALCSIZE` and `WM_NCHITTEST` to enable
//! Aero Snap (drag-to-edge, Win+Arrow, snap layouts).
//!
//! This is the standard approach used by Chrome, `WezTerm`, and Windows Terminal.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LRESULT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_EXTENDED_FRAME_BOUNDS, DwmExtendFrameIntoClientArea, DwmGetWindowAttribute,
};
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, HTBOTTOM,
    HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT,
    HTTOPRIGHT, IsZoomed, NCCALCSIZE_PARAMS, SM_CXFRAME, SM_CXPADDEDBORDER,
    SM_CYFRAME, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOW,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_DPICHANGED, WM_EXITSIZEMOVE, WM_MOVING,
    WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_THICKFRAME,
};

const SUBCLASS_ID: usize = 0xBEEF;

struct SnapData {
    resize_border: i32,
    caption_height: i32,
    client_rects: Mutex<Vec<[i32; 4]>>,
    /// DPI from the most recent `WM_DPICHANGED` message.  Since we eat
    /// `WM_DPICHANGED` (don't pass it to `DefSubclassProc`), winit never fires
    /// `ScaleFactorChanged`.  The app reads this in `handle_resize` to update
    /// `self.scale_factor`.  0 means no DPI change has been received yet.
    last_dpi: AtomicU32,
    /// True when this window is mid-tear-off OS drag (`drag_window()`).
    is_torn_off: AtomicBool,
    /// Set by `WM_EXITSIZEMOVE` when `is_torn_off` is true — signals that the
    /// OS move loop ended and the app should check for merge targets.
    drag_ended: AtomicBool,
    /// Screen cursor X at the moment the OS drag ended.
    end_cursor_x: AtomicI32,
    /// Screen cursor Y at the moment the OS drag ended.
    end_cursor_y: AtomicI32,
    /// Set by `WM_MOVING` when tab bar overlap is detected. Signals that
    /// `merge_proposed_*` contain the proposed window rect at the moment of
    /// overlap (before the window snaps back after `ReleaseCapture`).
    merge_detected: AtomicBool,
    /// Proposed window rect at the moment `WM_MOVING` detected overlap.
    /// This is the rect the OS was about to move the window to — it's the
    /// true position before snap-back invalidates everything.
    merge_proposed_left: AtomicI32,
    merge_proposed_top: AtomicI32,
    merge_proposed_right: AtomicI32,
    merge_proposed_bottom: AtomicI32,
    /// Tab bar rects of other windows for live merge detection during OS drag.
    /// Each entry: `[left, top, right, tab_bar_bottom]` in screen coordinates.
    /// Populated before `drag_window()`, checked in `WM_MOVING`.
    merge_rects: Mutex<Vec<[i32; 4]>>,
}

/// Global map from HWND (as usize) → `SnapData` pointer so `set_client_rects` can
/// find it. Raw pointers don't implement `Hash`, so we store HWND as `usize`.
static SNAP_PTRS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

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
            0,
            0,
            0,
            0,
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
            last_dpi: AtomicU32::new(0),
            is_torn_off: AtomicBool::new(false),
            drag_ended: AtomicBool::new(false),
            end_cursor_x: AtomicI32::new(0),
            end_cursor_y: AtomicI32::new(0),
            merge_detected: AtomicBool::new(false),
            merge_proposed_left: AtomicI32::new(0),
            merge_proposed_top: AtomicI32::new(0),
            merge_proposed_right: AtomicI32::new(0),
            merge_proposed_bottom: AtomicI32::new(0),
            merge_rects: Mutex::new(Vec::new()),
        });
        let data_ptr = Box::into_raw(data);
        SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, data_ptr as usize);

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
    let Some(data) = snap_data_for_window(window) else {
        return;
    };
    if let Ok(mut rects_lock) = data.client_rects.lock() {
        *rects_lock = rects;
    }
}

/// Read the DPI stored by the last `WM_DPICHANGED` message.
///
/// Returns the scale factor (DPI / 96.0), or `None` if no DPI change has been
/// received since `enable_snap` was called.
pub fn get_current_dpi(window: &winit::window::Window) -> Option<f64> {
    let data = snap_data_for_window(window)?;
    let dpi = data.last_dpi.load(Ordering::Relaxed);
    if dpi == 0 {
        None
    } else {
        Some(dpi as f64 / 96.0)
    }
}

/// Mark a window as mid-tear-off OS drag.
///
/// When `torn_off` is true, `WM_EXITSIZEMOVE` will capture the cursor position
/// so the app can check for merge targets after the OS drag loop ends.
pub fn set_torn_off(window: &winit::window::Window, torn_off: bool) {
    let Some(data) = snap_data_for_window(window) else {
        return;
    };
    data.is_torn_off.store(torn_off, Ordering::Relaxed);
    if !torn_off {
        data.drag_ended.store(false, Ordering::Relaxed);
    }
}

/// If the OS drag loop ended for a torn-off window, return the cursor
/// screen position at drag end and clear the flag.
///
/// Returns `None` if no drag-end has been signaled yet.
pub fn take_drag_ended(window: &winit::window::Window) -> Option<(i32, i32)> {
    let data = snap_data_for_window(window)?;
    if data.drag_ended.swap(false, Ordering::Relaxed) {
        data.is_torn_off.store(false, Ordering::Relaxed);
        let x = data.end_cursor_x.load(Ordering::Relaxed);
        let y = data.end_cursor_y.load(Ordering::Relaxed);
        Some((x, y))
    } else {
        None
    }
}

/// If `WM_MOVING` detected tab bar overlap during the OS drag, return the
/// proposed window rect at the moment of detection and clear the flag.
///
/// This rect is the position the OS was about to place the window — captured
/// BEFORE `ReleaseCapture()` ends the move loop. After that, the window snaps
/// back and positions are unreliable.
///
/// Returns `(left, top, right, bottom)` in screen coordinates.
pub fn take_merge_detected(window: &winit::window::Window) -> Option<(i32, i32, i32, i32)> {
    let data = snap_data_for_window(window)?;
    if data.merge_detected.swap(false, Ordering::Relaxed) {
        let l = data.merge_proposed_left.load(Ordering::Relaxed);
        let t = data.merge_proposed_top.load(Ordering::Relaxed);
        let r = data.merge_proposed_right.load(Ordering::Relaxed);
        let b = data.merge_proposed_bottom.load(Ordering::Relaxed);
        Some((l, t, r, b))
    } else {
        None
    }
}

/// Set the merge candidate rects for live overlap detection during OS drag.
///
/// Each rect is `[left, top, right, tab_bar_bottom]` in screen coordinates
/// (DWM visible bounds). Called before `drag_window()` to populate the
/// `WM_MOVING` handler with target tab bar regions.
pub fn set_merge_rects(window: &winit::window::Window, rects: Vec<[i32; 4]>) {
    let Some(data) = snap_data_for_window(window) else {
        return;
    };
    if let Ok(mut lock) = data.merge_rects.lock() {
        *lock = rects;
    }
}

/// Returns the visible frame bounds of a window, excluding the invisible
/// DWM extended frame that `GetWindowRect` / `outer_position()` include.
///
/// Returns `(left, top, right, bottom)` in screen coordinates, or `None`
/// if the query fails.
#[allow(unsafe_code)]
pub fn visible_frame_bounds(window: &winit::window::Window) -> Option<(i32, i32, i32, i32)> {
    let hwnd = hwnd_from_window(window)?;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // DWMWA_EXTENDED_FRAME_BOUNDS is defined as i32 in windows-sys but
    // DwmGetWindowAttribute expects u32.
    #[allow(clippy::cast_sign_loss, reason = "DWMWA constant is always non-negative")]
    let attr = DWMWA_EXTENDED_FRAME_BOUNDS as u32;
    // SAFETY: Standard Win32 API call with valid HWND and properly sized output buffer.
    let hr = unsafe {
        DwmGetWindowAttribute(hwnd, attr, (&raw mut rect).cast(), size_of::<RECT>() as u32)
    };
    if hr == 0 {
        Some((rect.left, rect.top, rect.right, rect.bottom))
    } else {
        None
    }
}

/// Show a window that was hidden via raw Win32 `ShowWindow(SW_HIDE)`.
///
/// Uses raw Win32 `ShowWindow(SW_SHOW)` to bypass winit's internal visibility
/// tracking. Necessary because `WM_MOVING` hides the window directly (for
/// Chrome-style merge), and winit's `set_visible(true)` may not undo that.
#[allow(unsafe_code)]
pub fn show_window(window: &winit::window::Window) {
    if let Some(hwnd) = hwnd_from_window(window) {
        // SAFETY: Standard Win32 API call with valid HWND.
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
        }
    }
}

fn snap_ptrs() -> &'static Mutex<HashMap<usize, usize>> {
    SNAP_PTRS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Look up the `SnapData` pointer for a window.
///
/// # Safety
///
/// The returned reference is valid until `WM_NCDESTROY` fires for the window.
/// Callers must not hold the reference across event loop iterations.
#[allow(unsafe_code)]
fn snap_data_for_window(window: &winit::window::Window) -> Option<&'static SnapData> {
    let hwnd = hwnd_from_window(window)?;
    let ptr = {
        let map = snap_ptrs().lock().ok()?;
        *map.get(&(hwnd as usize))?
    };
    // SAFETY: The pointer was created by Box::into_raw in enable_snap and remains
    // valid until WM_NCDESTROY frees it.
    Some(unsafe { &*(ptr as *const SnapData) })
}

fn get_x_lparam(lp: isize) -> i32 {
    #[allow(clippy::cast_possible_truncation, reason = "LPARAM low/high word extraction is inherently truncating")]
    let v = (lp & 0xFFFF) as i16;
    i32::from(v)
}

fn get_y_lparam(lp: isize) -> i32 {
    #[allow(clippy::cast_possible_truncation, reason = "LPARAM low/high word extraction is inherently truncating")]
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
                        let frame_x =
                            GetSystemMetrics(SM_CXFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                        let frame_y =
                            GetSystemMetrics(SM_CYFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
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

            WM_MOVING => {
                let data = &*(ref_data as *const SnapData);
                if data.is_torn_off.load(Ordering::Relaxed) {
                    // Chrome-style live merge: check if the dragged window's tab
                    // bar overlaps any other window's tab bar. If so, end the OS
                    // move loop immediately — the app will merge on WM_EXITSIZEMOVE.
                    let proposed = &*(lparam as *const RECT);
                    let drag_bar_bot = proposed.top + data.caption_height;
                    if let Ok(rects) = data.merge_rects.lock() {
                        for &[cl, ct, cr, ctb] in rects.iter() {
                            let y_overlap = proposed.top < ctb && drag_bar_bot > ct;
                            let x_overlap = proposed.right > cl && proposed.left < cr;
                            if y_overlap && x_overlap {
                                // Store the proposed rect — after ReleaseCapture
                                // the window snaps back and positions are invalid.
                                data.merge_proposed_left
                                    .store(proposed.left, Ordering::Relaxed);
                                data.merge_proposed_top
                                    .store(proposed.top, Ordering::Relaxed);
                                data.merge_proposed_right
                                    .store(proposed.right, Ordering::Relaxed);
                                data.merge_proposed_bottom
                                    .store(proposed.bottom, Ordering::Relaxed);
                                data.merge_detected.store(true, Ordering::Relaxed);
                                // Hide window before ending loop (Chrome does this
                                // to prevent snap-back visual artifact).
                                ShowWindow(hwnd, SW_HIDE);
                                ReleaseCapture();
                                return DefSubclassProc(hwnd, msg, wparam, lparam);
                            }
                        }
                    }
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }

            WM_EXITSIZEMOVE => {
                use std::io::Write;
                let data = &*(ref_data as *const SnapData);
                let was_torn_off = data.is_torn_off.load(Ordering::Relaxed);
                if was_torn_off {
                    // OS drag loop ended for a torn-off tab — capture cursor
                    // position so the app can check for merge targets.
                    let mut pt = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
                    GetCursorPos(&raw mut pt);
                    data.end_cursor_x.store(pt.x, Ordering::Relaxed);
                    data.end_cursor_y.store(pt.y, Ordering::Relaxed);
                    data.drag_ended.store(true, Ordering::Relaxed);
                }
                // Debug: log WM_EXITSIZEMOVE from WndProc.
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("oriterm_debug.log")
                {
                    let _ = writeln!(
                        f,
                        "WM_EXITSIZEMOVE: is_torn_off={was_torn_off}, hwnd={:#x}",
                        hwnd as usize,
                    );
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
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
