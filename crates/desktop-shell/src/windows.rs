//! Windows: NSH's window is a top-level `NshWindow` on NSH's UI thread (`nsh-ui`), while the app runs on its engine
//! thread. A Win32 subclass of the window answers `WM_CLOSE` by hiding it while the app keeps to the tray. Work from
//! other threads is queued and the window is sent a message for it; a `WH_GETMESSAGE` hook on the UI thread runs the
//! queue as that thread takes the message (a subclass and the tray icon can only be made on the window's own thread).
//! Windows always has a notification area.
#![expect(
    unsafe_code,
    reason = "Win32 has no safe binding: finding, subclassing, showing and posting to NSH's window are FFI calls; the \
              user's decision, kept to this module"
)]

use std::{
    cell::Cell,
    collections::VecDeque,
    ffi::c_void,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Threading::GetCurrentProcessId,
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                ASFW_ANY, AllowSetForegroundWindow, CallNextHookEx, EnumWindows, GW_OWNER,
                GetClassNameW, GetWindow, GetWindowThreadProcessId, HC_ACTION, IsIconic, MSG,
                PM_REMOVE, PostMessageW, SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow,
                SetWindowsHookExW, ShowWindow, WH_GETMESSAGE, WM_APP, WM_CLOSE, WM_NCDESTROY,
                WM_NULL,
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
/// The message that has the UI thread run [`QUEUE`].
const WM_SHELL_RUN: u32 = WM_APP + 0x45;

/// Work for the UI thread.
type Job = Box<dyn FnOnce() + Send>;

/// NSH's window as an integer (an `HWND` is a pointer, not `Send`); 0 until found.
static WINDOW: AtomicIsize = AtomicIsize::new(0);
/// The app is quitting: `WM_CLOSE` passes through (set from any thread before the close is posted).
static QUITTING: AtomicBool = AtomicBool::new(false);
/// Work waiting for the UI thread, in the order asked.
static QUEUE: Mutex<VecDeque<Job>> = Mutex::new(VecDeque::new());
/// The `WH_GETMESSAGE` hook on the UI thread as an integer; 0 until set. Kept for the life of the process.
static HOOK: Mutex<isize> = Mutex::new(0);

thread_local! {
    /// UI thread: the subclass is on the window.
    static HOOKED: Cell<bool> = const { Cell::new(false) };
}

fn hwnd() -> Option<HWND> {
    match WINDOW.load(Ordering::Acquire) {
        0 => None,
        raw => Some(HWND(raw as *mut c_void)),
    }
}

/// NSH's top-level window in this process, on whichever thread: class `NshWindow`, no owner (popups have one).
fn find_window() -> Option<HWND> {
    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> BOOL {
        let mut process = 0_u32;
        // SAFETY: `hwnd` is a live window the enumeration passed; the out pointer is a local.
        let _thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut process)) };
        // SAFETY: a plain call.
        if process != unsafe { GetCurrentProcessId() } {
            return BOOL(1);
        }
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
    let _stopped_early = unsafe { EnumWindows(Some(visit), LPARAM(&raw mut found as isize)) };
    found
}

/// The hook on the UI thread: runs the queue when the thread takes [`WM_SHELL_RUN`] off its message queue.
unsafe extern "system" fn get_message(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taken = i32::try_from(HC_ACTION).is_ok_and(|action| code == action)
        && usize::try_from(PM_REMOVE.0).is_ok_and(|remove| wparam.0 == remove);
    if taken {
        // SAFETY: for `HC_ACTION`, `lparam` points at the `MSG` just taken, which the hook may change.
        let msg = unsafe { &mut *(lparam.0 as *mut MSG) };
        if msg.message == WM_SHELL_RUN {
            // NSH's window procedure never sees it.
            msg.message = WM_NULL;
            run_queue();
        }
    }
    // SAFETY: the next hook in the chain, with the arguments as received.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Runs the queued work, one job at a time (a job may queue more).
fn run_queue() {
    loop {
        let job = QUEUE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front();
        let Some(job) = job else {
            return;
        };
        job();
    }
}

/// Hooks the window's thread once, so posted work reaches it.
fn hook_thread(window: HWND) -> Result<(), ShellError> {
    let mut hook = HOOK.lock().unwrap_or_else(PoisonError::into_inner);
    if *hook != 0 {
        return Ok(());
    }
    // SAFETY: `window` is NSH's live window; a null process out pointer is allowed.
    let thread = unsafe { GetWindowThreadProcessId(window, None) };
    if thread == 0 {
        return Err(ShellError::NoWindow);
    }
    // SAFETY: a thread hook on a thread of this process needs no module; `get_message` has HOOKPROC's signature and
    // lives as long as the process.
    let installed = unsafe { SetWindowsHookExW(WH_GETMESSAGE, Some(get_message), None, thread) }
        .map_err(|error| {
            ShellError::Tray(format!("the window's thread cannot be reached: {error}"))
        })?;
    *hook = installed.0 as isize;
    Ok(())
}

/// The subclass: hides on `WM_CLOSE` while the app keeps to the tray.
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

/// NSH makes the window before it loads the app, which then runs on another thread; a headless run has none.
pub(crate) fn has_window() -> bool {
    match find_window() {
        Some(found) => {
            WINDOW.store(found.0 as isize, Ordering::Release);
            true
        }
        None => false,
    }
}

/// Queues `f` for the UI thread and tells the window: it runs when NSH's message loop takes the message.
pub(crate) fn run_on_ui(f: impl FnOnce() + Send + 'static) {
    let Some(window) = hwnd().or_else(find_window) else {
        log::warn!("no window to run on");
        return;
    };
    WINDOW.store(window.0 as isize, Ordering::Release);
    if let Err(error) = hook_thread(window) {
        log::error!("{error}");
        return;
    }
    QUEUE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push_back(Box::new(f));
    post(WM_SHELL_RUN);
}

/// UI thread: subclasses NSH's window.
pub(crate) fn hook_window() -> Result<(), ShellError> {
    if HOOKED.get() {
        return Ok(());
    }
    let window = hwnd().or_else(find_window).ok_or(ShellError::NoWindow)?;
    WINDOW.store(window.0 as isize, Ordering::Release);
    // SAFETY: `window` is NSH's live window on this (its own) thread; `subclass` has SUBCLASSPROC's signature.
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

/// Runs `action` on the UI thread, from any thread. Quitting posts the close itself, so it works on a window this
/// crate never subclassed (a second instance that leaves at once).
pub(crate) fn request(action: Action) {
    match action {
        Action::Show | Action::Hide => run_on_ui(move || shell::perform(action)),
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

/// Only macOS takes the app out of its Dock while hidden: nothing to restore here.
pub(crate) fn tray_gone() {}
