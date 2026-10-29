//! Runs the Rust application directly on the machine's native backend.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() -> std::process::ExitCode {
    // The elevated half of the OS-service install is this same executable (`core::sidecar::privileged`): answer it
    // before any window or logger exists.
    if let Some(code) = nsh_app_easytierui::core::sidecar::privileged::dispatch(std::env::args_os())
    {
        return std::process::ExitCode::from(code);
    }
    std::process::Termination::report(run())
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn run() -> impl std::process::Termination {
    env_logger::init();
    platform::run_app(
        platform::AppSource::Rust(root()),
        platform::AppOptions {
            title: "EasyTierUI".to_owned(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "com.example.easytierui".to_owned(),
            // Wide enough for the sidebar layout (DESIGN.md: one column below about 640 pt).
            size: (960, 640),
            ..Default::default()
        },
    )
}

/// The root with the app's theme (`nsh.json` `"theme"`), as `nsh run` mounts it.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn root() -> platform::Root {
    let root = platform::Root::of::<nsh_app_easytierui::App>();
    match nsh_base::Theme::from_json(include_str!("../theme.json")) {
        Ok(theme) => root.tokens(theme.into_tokens()),
        Err(error) => {
            log::error!("theme.json: {error}; using the default theme");
            root
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Run this binary on macOS, Windows or Linux; use the TSX entry with nsh run for iOS and Android."
);
