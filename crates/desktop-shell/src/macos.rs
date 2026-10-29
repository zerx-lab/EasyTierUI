//! macOS: NSH's window has NSH's delegate (`NshAppDelegate`, also the app's delegate), whose `windowShouldClose:`
//! stops the engine. A stand-in delegate takes its place and answers `windowShouldClose:` itself while the app keeps
//! to the tray (the window orders out, the close is refused); everything else is forwarded to NSH's delegate. Work
//! from other threads (the app's engine thread, the tray's and the instance listener's) goes through the main dispatch
//! queue, which also waits for AppKit's run loop that status items need. While the window is hidden to the tray the
//! app leaves the Dock (the accessory activation policy) and returns to it when the window shows; without a tray icon
//! it stays in the Dock, whose icon reopens the window through NSH's own reopen handler.
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
    runtime::{AnyClass, Bool, NSObject, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSWindow, NSWindowDelegate};
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
                leave_dock(window.mtm());
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

/// Hidden to the tray: the Dock icon goes, the tray icon is the way back.
fn leave_dock(mtm: MainThreadMarker) {
    set_policy(mtm, NSApplicationActivationPolicy::Accessory);
}

fn set_policy(mtm: MainThreadMarker, policy: NSApplicationActivationPolicy) {
    let app = NSApplication::sharedApplication(mtm);
    if app.activationPolicy() != policy && !app.setActivationPolicy(policy) {
        log::warn!("the activation policy did not change to {policy:?}");
    }
}

pub(crate) fn tray_host_known() -> Option<bool> {
    Some(true)
}

pub(crate) fn probe_tray_host() -> bool {
    true
}

/// NSH's runner registers its app delegate's class (and sets the delegate) before it makes the window and loads the
/// app; a headless run never does. The Objective-C runtime answers from any thread.
pub(crate) fn has_window() -> bool {
    AnyClass::get(c"NshAppDelegate").is_some()
}

/// Runs `f` on the main queue: on AppKit's thread, once NSH's runner has started its run loop.
pub(crate) fn run_on_ui(f: impl FnOnce() + Send + 'static) {
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
    if window.isMiniaturized() {
        window.deminiaturize(None);
    }
    // Back in the Dock before activating: an accessory app gets no menu bar.
    set_policy(mtm, NSApplicationActivationPolicy::Regular);
    // Activating first: ordering a window front in an inactive app leaves it behind the active one.
    NSApplication::sharedApplication(mtm).activate();
    window.makeKeyAndOrderFront(None);
}

pub(crate) fn hide_window() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if let Some(window) = window(mtm) {
        window.orderOut(None);
        leave_dock(mtm);
    }
}

/// Without a tray icon the Dock icon is the only way back to a hidden window: the app returns to the Dock.
pub(crate) fn tray_gone() {
    if let Some(mtm) = MainThreadMarker::new() {
        set_policy(mtm, NSApplicationActivationPolicy::Regular);
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
    run_on_ui(move || shell::perform(action));
}

/// macOS lets an app activate itself when it is asked to: nothing to grant.
pub(crate) fn allow_foreground() {}
