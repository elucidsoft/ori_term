//! Windows snap support — `WndProc` subclass for borderless windows.
//!
//! Adds `WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION` back to a
//! borderless window, hides the OS title bar via `DwmExtendFrameIntoClientArea`, and
//! installs a subclass that handles `WM_NCCALCSIZE` and `WM_NCHITTEST` to enable
//! Aero Snap (drag-to-edge, Win+Arrow, snap layouts).
//!
//! This is the standard approach used by Chrome, `WezTerm`, and Windows Terminal.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
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

/// Configuration for an OS drag session, passed to `begin_os_drag()`.
pub struct OsDragConfig {
    /// Cursor-to-window-origin offset at the moment the drag started.
    /// `WM_MOVING` corrects the proposed rect every frame: `pos = cursor - grab_offset`.
    pub grab_offset: (i32, i32),
    /// Tab bar zones of other windows in screen coordinates.
    /// Each entry: `[left, top, right, tab_bar_bottom]`.
    /// Cursor-based merge: if `GetCursorPos()` falls within a zone, merge is triggered.
    pub merge_rects: Vec<[i32; 4]>,
    /// Number of `WM_MOVING` frames to skip merge detection after tear-off.
    /// Position correction still runs on every frame.
    pub skip_count: i32,
}

/// Result of an OS drag session, consumed by `take_os_drag_result()`.
pub enum OsDragResult {
    /// OS drag ended normally (user released mouse). Cursor at end position.
    DragEnded { cursor: (i32, i32) },
    /// `WM_MOVING` detected cursor in a merge target's tab bar zone.
    /// Window was hidden + `ReleaseCapture` called. Cursor captured at detection.
    MergeDetected { cursor: (i32, i32) },
}

/// Mutable state for an active OS drag session, stored behind `Mutex` in `SnapData`.
///
/// Created by `begin_os_drag()`, consumed by `WM_MOVING`/`WM_EXITSIZEMOVE` handlers,
/// and read by `take_os_drag_result()`.
struct OsDragState {
    grab_offset: (i32, i32),
    merge_rects: Vec<[i32; 4]>,
    skip_remaining: i32,
    result: Option<OsDragResult>,
}

struct SnapData {
    resize_border: i32,
    caption_height: i32,
    client_rects: Mutex<Vec<[i32; 4]>>,
    /// DPI from the most recent `WM_DPICHANGED` message.  Since we eat
    /// `WM_DPICHANGED` (don't pass it to `DefSubclassProc`), winit never fires
    /// `ScaleFactorChanged`.  The app reads this in `handle_resize` to update
    /// `self.scale_factor`.  0 means no DPI change has been received yet.
    last_dpi: AtomicU32,
    /// Active OS drag session state. `Some` while `drag_window()` is in progress,
    /// `None` otherwise. Replaces the previous 13 atomics.
    os_drag: Mutex<Option<OsDragState>>,
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
            os_drag: Mutex::new(None),
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

/// Begin an OS drag session for tab tear-off or single-tab window drag.
///
/// Stores `OsDragState` so the `WM_MOVING` handler can correct window position
/// and detect cursor-based merges. Call this before `drag_window()`.
pub fn begin_os_drag(window: &winit::window::Window, config: OsDragConfig) {
    let Some(data) = snap_data_for_window(window) else {
        return;
    };
    if let Ok(mut lock) = data.os_drag.lock() {
        *lock = Some(OsDragState {
            grab_offset: config.grab_offset,
            merge_rects: config.merge_rects,
            skip_remaining: config.skip_count,
            result: None,
        });
    }
}

/// If an OS drag session completed (either normal end or merge detection),
/// return the result and clear the drag state.
///
/// Returns `None` if no drag session is active or it hasn't ended yet.
pub fn take_os_drag_result(window: &winit::window::Window) -> Option<OsDragResult> {
    let data = snap_data_for_window(window)?;
    let mut lock = data.os_drag.lock().ok()?;
    let state = lock.as_mut()?;
    let result = state.result.take()?;
    // Drag session is complete — clear the entire state.
    *lock = None;
    Some(result)
}

/// Get the current screen cursor position via Win32 `GetCursorPos`.
#[allow(unsafe_code)]
pub fn cursor_screen_pos() -> (i32, i32) {
    let mut pt = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    // SAFETY: Standard Win32 API call with valid output pointer.
    unsafe {
        GetCursorPos(&raw mut pt);
    }
    (pt.x, pt.y)
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
                if let Ok(mut lock) = data.os_drag.lock() {
                    if let Some(state) = lock.as_mut() {
                        let proposed = &mut *(lparam as *mut RECT);
                        let w = proposed.right - proposed.left;
                        let h = proposed.bottom - proposed.top;

                        // 1. Always correct position (Chrome pattern).
                        // winit's drag_window() doesn't accept a drag_offset,
                        // so the OS picks up whatever offset exists. We
                        // compensate by correcting the proposed rect every
                        // frame: window origin = cursor - grab_offset.
                        let mut pt =
                            windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
                        GetCursorPos(&raw mut pt);
                        let (gx, gy) = state.grab_offset;
                        proposed.left = pt.x - gx;
                        proposed.top = pt.y - gy;
                        proposed.right = proposed.left + w;
                        proposed.bottom = proposed.top + h;

                        // 2. Skip merge check only (position still corrected).
                        if state.skip_remaining > 0 {
                            state.skip_remaining -= 1;
                            return DefSubclassProc(hwnd, msg, wparam, lparam);
                        }

                        // 3. Cursor-based merge (Chrome's DoesTabStripContain).
                        // Check if the cursor falls within any target tab bar zone.
                        for &[cl, ct, cr, ctb] in &state.merge_rects {
                            if pt.x >= cl && pt.x < cr && pt.y >= ct && pt.y < ctb {
                                state.result = Some(OsDragResult::MergeDetected {
                                    cursor: (pt.x, pt.y),
                                });
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
                let data = &*(ref_data as *const SnapData);
                if let Ok(mut lock) = data.os_drag.lock() {
                    if let Some(state) = lock.as_mut() {
                        // Only store DragEnded if WM_MOVING didn't already set a
                        // MergeDetected result.
                        if state.result.is_none() {
                            let mut pt =
                                windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
                            GetCursorPos(&raw mut pt);
                            state.result = Some(OsDragResult::DragEnded {
                                cursor: (pt.x, pt.y),
                            });
                        }
                    }
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
