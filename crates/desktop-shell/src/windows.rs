//! Windows: NSH's window is a top-level `NshWindow` on the UI thread. A Win32 subclass of it answers `WM_CLOSE` by
//! hiding the window while the app keeps to the tray, and handles the shell's own messages; requests from other
//! threads are posted to the window. Windows always has a notification area.
#![expect(
    unsafe_code,
    reason = "Win32 has no safe binding: finding, subclassing, showing and posting to NSH's window are FFI calls; the \
              user's decision, kept to this module"
)]

use std::{
    cell::Cell,
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicIsize, Ordering},
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                ASFW_ANY, AllowSetForegroundWindow, EnumThreadWindows, GW_OWNER, GetClassNameW,
                GetWindow, IsIconic, PostMessageW, SW_HIDE, SW_RESTORE, SW_SHOW,
                SetForegroundWindow, ShowWindow, WM_APP, WM_CLOSE, WM_NCDESTROY,
            },
        },
    },
    core::BOOL,
};

use crate::{
    ShellError,
    shell::{self, Action},
};

/// NSH's window class.
const NSH_CLASS: &str = "NshWindow";
/// The subclass's id ("ETUI").
const SUBCLASS_ID: usize = 0x4554_5549;
/// The shell's messages to the window.
const WM_SHELL_SHOW: u32 = WM_APP + 0x45;
const WM_SHELL_HIDE: u32 = WM_APP + 0x46;

/// NSH's window as an integer (an `HWND` is a pointer, not `Send`); 0 until found.
static WINDOW: AtomicIsize = AtomicIsize::new(0);
/// The app is quitting: `WM_CLOSE` passes through (set from any thread before the close is posted).
static QUITTING: AtomicBool = AtomicBool::new(false);

thread_local! {
    static HOOKED: Cell<bool> = const { Cell::new(false) };
}

fn hwnd() -> Option<HWND> {
    match WINDOW.load(Ordering::Acquire) {
        0 => None,
        raw => Some(HWND(raw as *mut c_void)),
    }
}

/// NSH's top-level window on this thread: class `NshWindow`, no owner (popups have one).
fn find_window() -> Option<HWND> {
    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> BOOL {
        let mut class = [0_u16; 32];
        // SAFETY: `hwnd` is a live window the enumeration passed; the buffer is ours.
        let len = unsafe { GetClassNameW(hwnd, &mut class) };
        let name = usize::try_from(len)
            .ok()
            .and_then(|len| class.get(..len))
            .map(String::from_utf16_lossy);
        // SAFETY: as above; an error means "no owner".
        let owned = unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok();
        if name.as_deref() == Some(NSH_CLASS) && !owned {
            // SAFETY: `found` is the address of `find_window`'s `Option<HWND>`, alive for the whole enumeration.
            unsafe { *(found.0 as *mut Option<HWND>) = Some(hwnd) };
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut found: Option<HWND> = None;
    // SAFETY: `visit` only writes through the pointer to `found`, which outlives the call; the enumeration's result
    // only says whether it stopped early.
    let _stopped_early = unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(visit),
            LPARAM(&raw mut found as isize),
        )
    };
    found
}

/// The subclass: hides on `WM_CLOSE` while the app keeps to the tray, shows and hides on the shell's messages.
unsafe extern "system" fn subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_CLOSE if shell::keeps_in_tray() && !QUITTING.load(Ordering::Acquire) => {
            // SAFETY: `hwnd` is the window this procedure runs for.
            let _was_visible = unsafe { ShowWindow(hwnd, SW_HIDE) };
            return LRESULT(0);
        }
        WM_SHELL_SHOW => {
            shell::perform(Action::Show);
            return LRESULT(0);
        }
        WM_SHELL_HIDE => {
            shell::perform(Action::Hide);
            return LRESULT(0);
        }
        WM_NCDESTROY => {
            HOOKED.set(false);
            WINDOW.store(0, Ordering::Release);
            // SAFETY: removing this subclass from its own window, as its last message allows.
            let _removed = unsafe { RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID) };
        }
        _ => {}
    }
    // SAFETY: the next procedure in the chain, with the message as received.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

pub(crate) fn tray_host_known() -> Option<bool> {
    Some(true)
}

pub(crate) fn probe_tray_host() -> bool {
    true
}

/// NSH makes the window before it loads the app, on this (the UI) thread; a headless run has none.
pub(crate) fn has_window() -> bool {
    match find_window() {
        Some(found) => {
            WINDOW.store(found.0 as isize, Ordering::Release);
            true
        }
        None => false,
    }
}

/// Starting hidden is a hide posted behind NSH's first show ([`request`]); nothing to do before.
pub(crate) fn conceal() {}

pub(crate) fn reveal() {}

/// The UI thread can attach and make the tray at once.
pub(crate) fn defer(f: impl FnOnce() + Send + 'static) {
    f();
}

pub(crate) fn hook_window() -> Result<(), ShellError> {
    if HOOKED.get() {
        return Ok(());
    }
    let window = hwnd().or_else(find_window).ok_or(ShellError::NoWindow)?;
    WINDOW.store(window.0 as isize, Ordering::Release);
    // SAFETY: `window` is NSH's live window on this thread; `subclass` has SUBCLASSPROC's signature.
    let hooked = unsafe { SetWindowSubclass(window, Some(subclass), SUBCLASS_ID, 0) };
    if !hooked.as_bool() {
        return Err(ShellError::NoWindow);
    }
    HOOKED.set(true);
    Ok(())
}

pub(crate) fn show_window() {
    let Some(window) = hwnd() else {
        log::warn!("no window to show");
        return;
    };
    // SAFETY: plain calls on NSH's live window, from its thread.
    let (_was_visible, _foreground) = unsafe {
        let command = if IsIconic(window).as_bool() {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        (ShowWindow(window, command), SetForegroundWindow(window))
    };
}

pub(crate) fn hide_window() {
    if let Some(window) = hwnd() {
        // SAFETY: a plain call on NSH's live window, from its thread.
        let _was_visible = unsafe { ShowWindow(window, SW_HIDE) };
    }
}

/// Closes the window for real: NSH destroys it and its run loop ends.
pub(crate) fn quit() {
    QUITTING.store(true, Ordering::Release);
    post(WM_CLOSE);
}

/// Posts `action` to the window, from any thread. Quitting posts the close itself, so it works on a window this
/// crate never subclassed (a second instance that leaves at once).
pub(crate) fn request(action: Action) {
    match action {
        Action::Show => post(WM_SHELL_SHOW),
        Action::Hide => post(WM_SHELL_HIDE),
        Action::Quit => quit(),
    }
}

fn post(msg: u32) {
    let Some(window) = hwnd() else {
        log::warn!("no window to post to");
        return;
    };
    // SAFETY: posting is allowed from any thread; a window destroyed meanwhile makes the call fail, which is logged.
    if let Err(error) = unsafe { PostMessageW(Some(window), msg, WPARAM(0), LPARAM(0)) } {
        log::warn!("the window did not take the request: {error}");
    }
}

/// A second instance lets the first bring its window to the front (Windows only lets the foreground process grant
/// that).
pub(crate) fn allow_foreground() {
    // SAFETY: a plain call.
    if let Err(error) = unsafe { AllowSetForegroundWindow(ASFW_ANY) } {
        log::debug!("the first instance may not come to the front: {error}");
    }
}
