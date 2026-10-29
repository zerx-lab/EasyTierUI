//! The desktop app's own behavior in Settings, and its tray icon's menu.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field. Templates use `{name}` placeholders filled by
//! [`super::fill`].

/// Strings of this area in one language.
#[derive(Debug)]
pub struct DesktopStrings {
    /// Settings: the section's name.
    pub section: &'static str,
    /// Switch: keep running in the tray after the window closes.
    pub keep_in_tray: &'static str,
    /// What it does.
    pub keep_in_tray_body: &'static str,
    /// This desktop draws no tray icons, so the switch cannot turn on.
    pub no_tray_host: &'static str,
    /// Switch: start the app when the user signs in.
    pub login: &'static str,
    /// What it does.
    pub login_body: &'static str,
    /// The login item could not be changed; `{reason}` is the system's words.
    pub login_failed: &'static str,
    /// Switch: start without showing the window.
    pub quiet_start: &'static str,
    /// What it does.
    pub quiet_start_body: &'static str,
    /// Why the switch is off limits while the app does not keep to the tray.
    pub quiet_start_needs_tray: &'static str,
    /// Phones and the web: these settings belong to the desktop app.
    pub unsupported: &'static str,
    /// The tray icon's tooltip.
    pub tray_tooltip: &'static str,
    /// The tray menu's item that shows the window.
    pub tray_show: &'static str,
    /// The tray menu's item that quits.
    pub tray_quit: &'static str,
}

pub(super) static EN: DesktopStrings = DesktopStrings {
    section: "Desktop",
    keep_in_tray: "Keep running in the tray",
    keep_in_tray_body: "Closing the window leaves EasyTierUI in the tray, and this device's networks stay up. Open or \
                        quit it from the tray icon.",
    no_tray_host: "This desktop shows no tray icons, so closing the window quits EasyTierUI. On GNOME, the \
                   AppIndicator extension adds them.",
    login: "Start at login",
    login_body: "Opens EasyTierUI when you sign in to this computer.",
    login_failed: "Start at login was not changed: {reason}",
    quiet_start: "Start quietly",
    quiet_start_body: "EasyTierUI starts without showing its window, only its tray icon.",
    quiet_start_needs_tray: "Turn on keeping running in the tray first: the tray icon is the way back to the window.",
    unsupported: "Keeping to the tray, starting at login and starting quietly belong to the desktop app.",
    tray_tooltip: "EasyTierUI",
    tray_show: "Show EasyTierUI",
    tray_quit: "Quit EasyTierUI",
};

pub(super) static ZH_CN: DesktopStrings = DesktopStrings {
    section: "桌面",
    keep_in_tray: "托盘常驻",
    keep_in_tray_body: "关闭窗口后 EasyTierUI 留在托盘里，本机网络继续运行；从托盘图标打开或退出。",
    no_tray_host: "这个桌面不显示托盘图标，关闭窗口会退出 EasyTierUI。GNOME 装上 AppIndicator 扩展后即可显示。",
    login: "开机自启",
    login_body: "登录这台电脑后自动打开 EasyTierUI。",
    login_failed: "开机自启没有改成：{reason}",
    quiet_start: "免打扰启动",
    quiet_start_body: "启动时不弹出主窗口，只在托盘里显示图标。",
    quiet_start_needs_tray: "先开启托盘常驻：要靠托盘图标打开窗口。",
    unsupported: "托盘常驻、开机自启和免打扰启动只在桌面版里有。",
    tray_tooltip: "EasyTierUI",
    tray_show: "显示 EasyTierUI",
    tray_quit: "退出 EasyTierUI",
};
