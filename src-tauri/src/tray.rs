//! The menu-bar icon, its menu, and the floating panel it toggles.
//!
//! Everything the tray needs lives here rather than in `lib.rs`, which stays a
//! thin wiring layer.

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, LogicalPosition, Manager, Position, Rect, Runtime, Size, WebviewWindow, WindowEvent,
};

const TRAY_ICON: &[u8] = include_bytes!("../assets/tray-icon.png");

const TRAY_ID: &str = "fibrex-tray";

/// The floating panel. The window is declared in `tauri.conf.json` under this
/// label, and the capability grants this label too.
const PANEL_LABEL: &str = "panel";

const OPEN_DASHBOARD_ID: &str = "open-dashboard";
const QUIT_ID: &str = "quit";

/// Creates the menu-bar icon, its menu, and the panel's hide-on-focus-loss rule.
///
/// The icon is a macOS template image: black pixels with an alpha channel, which
/// macOS inverts for a dark menu bar. It is deliberately neutral rather than
/// status-coloured, because no measurement exists yet and a coloured dot would
/// claim something the app cannot support.
pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open_dashboard =
        MenuItem::with_id(app, OPEN_DASHBOARD_ID, "Open dashboard", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit FibreX Monitor", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open_dashboard, &separator, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_ICON)?)
        .icon_as_template(true)
        .tooltip("FibreX Monitor")
        .menu(&menu)
        // The left click toggles the panel, so it must not also open the menu.
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_event)
        .build(app)?;

    hide_panel_when_it_loses_focus(app);

    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        // The tray menu and the panel button raise the dashboard the same way,
        // so there is one implementation to keep correct.
        OPEN_DASHBOARD_ID => {
            let _ = crate::commands::window::show_dashboard(app);
        }
        QUIT_ID => app.exit(0),
        _ => {}
    }
}

fn handle_tray_event<R: Runtime>(tray: &tauri::tray::TrayIcon<R>, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        rect,
        ..
    } = event
    {
        toggle_panel(tray.app_handle(), Some(&rect));
    }
}

fn toggle_panel<R: Runtime>(app: &AppHandle<R>, tray_rect: Option<&Rect>) {
    let Some(panel) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };

    match panel.is_visible() {
        Ok(true) => {
            let _ = panel.hide();
        }
        _ => {
            if let Some(rect) = tray_rect {
                place_under_tray(&panel, rect);
            }
            let _ = panel.show();
            let _ = panel.set_focus();
        }
    }
}

/// Reduces a Tauri position to one pair of numbers.
///
/// The tray rectangle arrives in physical pixels on macOS, but Tauri models
/// position and size as enums that may also be logical. The placement maths only
/// cares about the numbers, and the panel's position setter wants logical units.
fn position_of(value: Position) -> (f64, f64) {
    match value {
        Position::Physical(position) => (position.x as f64, position.y as f64),
        Position::Logical(position) => (position.x, position.y),
    }
}

fn size_of(value: Size) -> (f64, f64) {
    match value {
        Size::Physical(size) => (size.width as f64, size.height as f64),
        Size::Logical(size) => (size.width, size.height),
    }
}

/// Centres the panel under the menu-bar icon rather than dropping it in a
/// screen corner, and keeps it fully on screen: a panel half off the edge is
/// worse than one that is slightly off-centre.
fn place_under_tray<R: Runtime>(panel: &WebviewWindow<R>, tray_rect: &Rect) {
    let (Ok(scale), Ok(size)) = (panel.scale_factor(), panel.outer_size()) else {
        return;
    };

    let (tray_x, tray_y) = position_of(tray_rect.position);
    let (tray_width, tray_height) = size_of(tray_rect.size);

    let panel_width = size.width as f64;
    let mut x = (tray_x + tray_width / 2.0) / scale - panel_width / (2.0 * scale);
    let below_tray = (tray_y + tray_height) / scale;

    if let Ok(Some(monitor)) = panel.current_monitor() {
        // The monitor reports physical pixels directly, so it needs none of the
        // enum handling the tray rectangle does.
        let monitor_size = monitor.size();
        let monitor_position = monitor.position();
        let left = monitor_position.x as f64 / scale;
        let usable = monitor_size.width as f64 / scale - panel_width / scale;
        x = x.clamp(left, left + usable);
    }

    let _ = panel.set_position(LogicalPosition::new(x, below_tray));
}

/// Hides the panel when it loses focus, so clicking anywhere else dismisses it
/// the way a menu-bar popover is expected to behave.
fn hide_panel_when_it_loses_focus<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();

    if let Some(panel) = app.get_webview_window(PANEL_LABEL) {
        panel.on_window_event(move |event| {
            if matches!(event, WindowEvent::Focused(false))
                && let Some(panel) = app.get_webview_window(PANEL_LABEL)
            {
                let _ = panel.hide();
            }
        });
    }
}
