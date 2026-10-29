//! The tray icon and its menu (show the window, quit), through tray-icon. Its events arrive on the UI thread on macOS
//! and Windows and on ksni's D-Bus thread on Linux; each becomes a request the platform posts to the UI thread.

use std::sync::Once;

use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::{
    ShellError, platform,
    shell::{Action, TrayIcons, TrayLabels},
};

const TRAY_ID: &str = "easytierui.tray";
const SHOW_ID: &str = "easytierui.tray.show";
const QUIT_ID: &str = "easytierui.tray.quit";

/// The tray icon with its menu's items (kept to reword them).
pub(crate) struct Tray {
    icon: TrayIcon,
    show: MenuItem,
    quit: MenuItem,
}

impl Tray {
    /// Shows the tray icon.
    pub(crate) fn new(icons: TrayIcons, labels: &TrayLabels) -> Result<Self, ShellError> {
        route_events();
        let show = MenuItem::with_id(SHOW_ID, menu_text(&labels.show), true, None);
        let quit = MenuItem::with_id(QUIT_ID, menu_text(&labels.quit), true, None);
        let menu = Menu::new();
        let separator = PredefinedMenuItem::separator();
        for item in [&show as &dyn tray_icon::menu::IsMenuItem, &separator, &quit] {
            menu.append(item)
                .map_err(|error| ShellError::Tray(error.to_string()))?;
        }
        let builder = TrayIconBuilder::new()
            .with_id(TRAY_ID)
            .with_menu(Box::new(menu))
            .with_tooltip(&labels.tooltip);
        // The menu bar draws a template image in the menu bar's own color; elsewhere a click shows the window and
        // the menu opens on a right click, as Windows' and Linux's trays do.
        #[cfg(target_os = "macos")]
        let builder = builder
            .with_icon_templated(decode(icons.template)?)
            .with_menu_on_left_click(true);
        #[cfg(not(target_os = "macos"))]
        let builder = builder
            .with_icon(decode(icons.color)?)
            .with_menu_on_left_click(false);
        let icon = builder
            .build()
            .map_err(|error| ShellError::Tray(error.to_string()))?;
        Ok(Self { icon, show, quit })
    }

    /// Rewords the menu and the tooltip.
    pub(crate) fn relabel(&self, labels: &TrayLabels) {
        self.show.set_text(menu_text(&labels.show));
        self.quit.set_text(menu_text(&labels.quit));
        if let Err(error) = self.icon.set_tooltip(Some(&labels.tooltip)) {
            log::warn!("the tray's tooltip was not changed: {error}");
        }
    }
}

/// `label` as a menu item's text: `&` marks a mnemonic there, so a literal one is doubled.
fn menu_text(label: &str) -> String {
    label.replace('&', "&&")
}

/// Sends the tray's and its menu's events to the UI thread (once per process: the handlers are global).
fn route_events() {
    static ROUTED: Once = Once::new();
    ROUTED.call_once(|| {
        MenuEvent::set_event_handler(Some(|event: MenuEvent| {
            let id: &str = event.id.as_ref();
            match id {
                SHOW_ID => platform::request(Action::Show),
                QUIT_ID => platform::request(Action::Quit),
                _ => {}
            }
        }));
        TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
            if opens_window(&event) {
                platform::request(Action::Show);
            }
        }));
    });
}

/// A primary click on the icon shows the window, except on macOS, where a click opens the menu as status items do.
fn opens_window(event: &TrayIconEvent) -> bool {
    if cfg!(target_os = "macos") {
        return false;
    }
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } | TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        }
    )
}

/// The image of an 8-bit RGBA PNG.
fn decode(bytes: &[u8]) -> Result<Icon, ShellError> {
    let bad = |what: &dyn std::fmt::Display| ShellError::Icon(what.to_string());
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|error| bad(&error))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| bad(&"too large"))?;
    let mut rgba = vec![0; size];
    let frame = reader.next_frame(&mut rgba).map_err(|error| bad(&error))?;
    if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
        return Err(bad(&format!(
            "{:?} at {:?} bits",
            frame.color_type, frame.bit_depth
        )));
    }
    rgba.truncate(frame.buffer_size());
    Icon::from_rgba(rgba, frame.width, frame.height).map_err(|error| bad(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_icons_decode() {
        for bytes in [
            include_bytes!("../../../brand/tray-template.png").as_slice(),
            include_bytes!("../../../brand/tray.png").as_slice(),
        ] {
            decode(bytes).unwrap();
        }
    }

    #[test]
    fn an_icon_without_alpha_is_refused() {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 2, 2);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0_u8; 12]).unwrap();
        }
        assert!(matches!(decode(&out), Err(ShellError::Icon(_))));
    }

    #[test]
    fn a_literal_ampersand_is_not_a_mnemonic() {
        assert_eq!(menu_text("Show & hide"), "Show && hide");
    }
}
