//! macOS: NSH's window has NSH's delegate (`NshAppDelegate`, also the app's delegate), whose `windowShouldClose:`
//! stops the engine. A stand-in delegate takes its place and answers `windowShouldClose:` itself while the app keeps
//! to the tray (the window orders out, the close is refused); everything else is forwarded to NSH's delegate. Requests
//! from other threads go through the main dispatch queue, which also waits for AppKit's run loop that status items
//! need. Clicking the Dock icon reopens the window through NSH's own reopen handler.
#![expect(
    unsafe_code,
    reason = "a window delegate is an Objective-C class (`define_class!`) whose `init` is a message send; the user's \
              decision, kept to this module"
)]

use std::cell::RefCell;

use dispatch2::DispatchQueue;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_app_kit::{NSApplication, NSWindow, NSWindowDelegate};
use objc2_foundation::NSNotification;

use crate::{
    ShellError,
    shell::{self, Action},
};

/// NSH's own delegate, which the stand-in forwards to.
struct Ivars {
    inner: Retained<ProtocolObject<dyn NSWindowDelegate>>,
}

define_class!(
    /// Answers `windowShouldClose:` while the app keeps to the tray; forwards to NSH's delegate otherwise.
    // SAFETY: NSObject may be subclassed; the class adds no `dealloc`, and its ivars are a retained object.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "EasyTierUITrayWindowDelegate"]
    #[ivars = Ivars]
    struct TrayDelegate;

    // SAFETY: NSObjectProtocol asks nothing beyond NSObject.
    unsafe impl NSObjectProtocol for TrayDelegate {}

    // SAFETY: both methods have NSWindowDelegate's signatures.
    unsafe impl NSWindowDelegate for TrayDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, window: &NSWindow) -> Bool {
            if shell::keeps_in_tray() {
                window.orderOut(None);
                return Bool::NO;
            }
            let inner = &self.ivars().inner;
            Bool::new(
                !inner.respondsToSelector(sel!(windowShouldClose:))
                    || inner.windowShouldClose(window),
            )
        }

        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, notification: &NSNotification) {
            let inner = &self.ivars().inner;
            if inner.respondsToSelector(sel!(windowWillClose:)) {
                inner.windowWillClose(notification);
            }
        }
    }
);

thread_local! {
    static WINDOW: RefCell<Option<Retained<NSWindow>>> = const { RefCell::new(None) };
    /// The stand-in, kept for the life of the process: it may be running (inside the window's close) when the shell
    /// is dropped, so it is never released early.
    static DELEGATE: RefCell<Option<Retained<TrayDelegate>>> = const { RefCell::new(None) };
}

/// NSH's window: the one hooked, else the first window with a delegate (NSH's overlays have none).
fn window(mtm: MainThreadMarker) -> Option<Retained<NSWindow>> {
    if let Some(window) = WINDOW.with_borrow(Clone::clone) {
        return Some(window);
    }
    NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .find(|window| window.delegate().is_some())
}

pub(crate) fn tray_host_known() -> Option<bool> {
    Some(true)
}

pub(crate) fn probe_tray_host() -> bool {
    true
}

/// NSH's runner sets the app's delegate before it makes the window; a headless run has none (or is off the main
/// thread).
pub(crate) fn has_window() -> bool {
    MainThreadMarker::new()
        .is_some_and(|mtm| NSApplication::sharedApplication(mtm).delegate().is_some())
}

/// Starting hidden: the window is made transparent before NSH orders it front, so it never flashes; it is ordered out
/// (and made opaque again) once the run loop runs.
pub(crate) fn conceal() {
    if let Some(window) = MainThreadMarker::new().and_then(window) {
        window.setAlphaValue(0.0);
    }
}

pub(crate) fn reveal() {
    if let Some(window) = MainThreadMarker::new().and_then(window) {
        window.setAlphaValue(1.0);
    }
}

/// Runs `f` on the main queue: after NSH's runner has started AppKit's run loop.
pub(crate) fn defer(f: impl FnOnce() + Send + 'static) {
    DispatchQueue::main().exec_async(f);
}

pub(crate) fn hook_window() -> Result<(), ShellError> {
    let mtm = MainThreadMarker::new().ok_or(ShellError::NoWindow)?;
    if DELEGATE.with_borrow(Option::is_some) {
        return Ok(());
    }
    let window = window(mtm).ok_or(ShellError::NoWindow)?;
    let inner = window.delegate().ok_or(ShellError::NoWindow)?;
    let delegate = TrayDelegate::alloc(mtm).set_ivars(Ivars { inner });
    // SAFETY: NSObject's designated initializer, sent once to the object just allocated.
    let delegate: Retained<TrayDelegate> = unsafe { msg_send![super(delegate), init] };
    window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    WINDOW.set(Some(window));
    DELEGATE.set(Some(delegate));
    Ok(())
}

pub(crate) fn show_window() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(window) = window(mtm) else {
        log::warn!("no window to show");
        return;
    };
    window.setAlphaValue(1.0);
    if window.isMiniaturized() {
        window.deminiaturize(None);
    }
    // Activating first: ordering a window front in an inactive app leaves it behind the active one.
    NSApplication::sharedApplication(mtm).activate();
    window.makeKeyAndOrderFront(None);
}

pub(crate) fn hide_window() {
    if let Some(window) = MainThreadMarker::new().and_then(window) {
        window.orderOut(None);
        window.setAlphaValue(1.0);
    }
}

/// NSH's `applicationShouldTerminate:` closes the window and stops the engine.
pub(crate) fn quit() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}

/// Runs `action` on the main thread, from any thread.
pub(crate) fn request(action: Action) {
    DispatchQueue::main().exec_async(move || shell::perform(action));
}

/// macOS lets an app activate itself when it is asked to: nothing to grant.
pub(crate) fn allow_foreground() {}
