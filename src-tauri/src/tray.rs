//! The menu-bar icon, its menu, and the floating panel it toggles.
//!
//! Everything the tray needs lives here rather than in `lib.rs`, which stays a
//! thin wiring layer.

#[cfg(not(target_os = "macos"))]
use tauri::WindowEvent;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, LogicalPosition, Manager, Position, Rect, Runtime, Size, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

const TRAY_ICON: &[u8] = include_bytes!("../assets/tray-icon.png");

const TRAY_ID: &str = "fibrex-tray";

/// The floating panel.
///
/// The window is created in [`create_panel`] rather than declared in
/// `tauri.conf.json`; the capability grants this label either way.
const PANEL_LABEL: &str = "panel";

const OPEN_DASHBOARD_ID: &str = "open-dashboard";
const QUIT_ID: &str = "quit";

/// Creates the menu-bar icon, its menu, and its panel.
///
/// The icon is a macOS template image: black pixels with an alpha channel, which
/// macOS inverts for a dark menu bar. It is deliberately neutral rather than
/// status-coloured, because no measurement exists yet and a coloured dot would
/// claim something the app cannot support.
///
/// The app is expected to already be a menu-bar app when this is called. The
/// panel is created here, so that is a requirement rather than a detail: see
/// [`create_panel`].
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

    let panel = create_panel(app)?;
    make_panel_float_across_spaces(&panel);

    // macOS dismisses the panel on a click in another app. The blur rule is
    // only for the platforms without that monitor: on macOS the app can lose
    // activation a second or two after showing the panel without anyone having
    // clicked anything, and a blur rule treats that as a dismissal.
    #[cfg(target_os = "macos")]
    hide_panel_when_another_app_is_clicked(app);

    #[cfg(not(target_os = "macos"))]
    hide_panel_when_it_loses_focus(app);

    Ok(())
}

/// Creates the floating panel.
///
/// The panel is created here, not declared in `tauri.conf.json`, and that
/// placement is the fix for the panel appearing only on the Space the dashboard
/// was last on. Tauri creates configured windows before the setup hook runs,
/// under the default regular-app policy. macOS decides a window's Space
/// behaviour when the window is created, and a window created for a regular app
/// ignores `canJoinAllSpaces` for the rest of its life: it stays on the Space it
/// was born on, so clicking the menu-bar icon from another app's Space showed
/// nothing.
///
/// A window created after the app has become a menu-bar app (an accessory app,
/// as far as macOS is concerned) lands on the active Space and keeps following
/// it. That was measured on this macOS version before the change, including the
/// round trip back to a regular app while the dashboard is open. The setup hook
/// therefore hides the dashboard, switches the policy to accessory, calls
/// [`install`], and only then shows the dashboard again.
fn create_panel<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    if let Some(panel) = app.get_webview_window(PANEL_LABEL) {
        return Ok(panel);
    }

    WebviewWindowBuilder::new(app, PANEL_LABEL, WebviewUrl::App("panel.html".into()))
        .title("FibreX Monitor panel")
        .inner_size(340.0, 520.0)
        .visible(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(true)
        .visible_on_all_workspaces(true)
        // The panel is normally opened from the menu bar while another app is
        // frontmost, so the first click has to land on the control under the
        // pointer instead of being spent activating the window.
        .accept_first_mouse(true)
        .build()
}

/// Hides the panel when the person clicks in another app.
///
/// This is macOS's dismissal path. The obvious alternative, hiding on the
/// window's blur, is not safe here: an accessory app can be given activation
/// and then have it taken back a second or two later, with no input from
/// anyone, and a blur rule reads that as a click-away and removes the panel
/// while it is being read. A global monitor sees mouse-downs delivered to
/// *other* applications, which is exactly the event that should dismiss a
/// menu-bar popover. Events delivered to this app, including the tray icon and
/// the panel itself, are not reported to a global monitor, so clicking the icon
/// to close the panel still reaches the tray.
#[cfg(target_os = "macos")]
fn hide_panel_when_another_app_is_clicked<R: Runtime>(app: &AppHandle<R>) {
    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask};
    use std::ptr::NonNull;

    let handle = app.clone();
    let block = RcBlock::new(move |_event: NonNull<NSEvent>| {
        let app = handle.clone();
        let _ = app.clone().run_on_main_thread(move || {
            let Some(panel) = app.get_webview_window(PANEL_LABEL) else {
                return;
            };
            if panel.is_visible().unwrap_or(false) {
                let _ = panel.hide();
            }
        });
    });

    // Installing a global event monitor is an ordinary AppKit call, and the
    // block only captures an app handle. AppKit copies the block, so the local
    // `RcBlock` may be dropped afterwards. The returned token is what keeps the
    // monitor installed, so it is deliberately not dropped: the app installs
    // exactly one monitor and lives as long as the process.
    let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown,
        &block,
    );
    std::mem::forget(monitor);
}

/// Hides the panel if it is open.
///
/// Used when the dashboard is raised: the panel floats above ordinary windows,
/// so leaving it open would put it on top of the dashboard it just opened.
pub fn hide_panel<R: Runtime>(app: &AppHandle<R>) {
    if let Some(panel) = app.get_webview_window(PANEL_LABEL) {
        let _ = panel.hide();
    }
}

/// Makes the panel behave like a menu-bar popover rather than an ordinary
/// window.
///
/// Without this the panel belongs to whichever Space the app happened to be on,
/// so clicking the menu-bar icon while another app is frontmost looks like
/// nothing happening: the panel is shown in its own Space, behind the app being
/// looked at. `canJoinAllSpaces` puts it on every Space, and the floating level
/// Tauri already set keeps it above the app underneath.
///
/// This is a second line of defence. A window only accepts `canJoinAllSpaces`
/// if it is created while the app is a menu-bar app, which is why the panel is
/// built in [`create_panel`]; applying the behaviour again here keeps it true
/// across later policy changes, including the ones that happen while the
/// dashboard is open.
///
/// `fullScreenAuxiliary` is the one behaviour Tauri does not expose, so it is
/// added through AppKit. Without it a panel opened while another app is in
/// fullscreen stays behind that app's fullscreen window instead of over it.
fn make_panel_float_across_spaces<R: Runtime>(panel: &WebviewWindow<R>) {
    let _ = panel.set_visible_on_all_workspaces(true);
    let _ = panel.set_always_on_top(true);

    #[cfg(target_os = "macos")]
    {
        let Ok(pointer) = panel.ns_window() else {
            return;
        };
        if pointer.is_null() {
            return;
        }

        // SAFETY: the pointer comes from the window's own native handle, and
        // `NSWindow` is the type that handle refers to. The window outlives
        // this call, so the reference is only read here and never stored.
        let ns_window = unsafe { &*(pointer as *const objc2_app_kit::NSWindow) };
        // Reading and writing the collection behaviour are ordinary AppKit
        // property accesses on a live window, and this runs on the main thread.
        let behavior = ns_window.collectionBehavior()
            | objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
            | objc2_app_kit::NSWindowCollectionBehavior::FullScreenAuxiliary;
        ns_window.setCollectionBehavior(behavior);
    }
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
            // Everything happens on the main thread: the AppKit call that
            // pins the panel to every Space is a UI-thread operation, and the
            // tray event is not guaranteed to arrive on that thread.
            let handle = app.clone();
            let rect = tray_rect.copied();
            let _ = app.run_on_main_thread(move || show_panel(&handle, rect));
        }
    }
}

/// Shows the panel over whatever is on screen, then makes the app active.
///
/// Called on the main thread only.
fn show_panel<R: Runtime>(app: &AppHandle<R>, tray_rect: Option<Rect>) {
    let Some(panel) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };

    // Applied on every show, so a policy change between openings cannot leave
    // the panel tied to the Space it was last on.
    make_panel_float_across_spaces(&panel);
    if let Some(rect) = tray_rect.as_ref() {
        place_under_tray(&panel, rect);
    }

    let _ = panel.show();
    let _ = panel.set_focus();
    activate_app();
}

/// Makes this app the active app.
///
/// Clicking a menu-bar icon does not activate an accessory app, so a panel
/// shown from that click belongs to an inactive app: macOS gives it focus for a
/// moment and then may hand activation back to the app that had it. Asking for
/// activation is still worth it, because the panel is keyboard-interactive
/// (Escape closes it, the buttons take focus), and it does convince macOS in
/// most cases. Dismissal does not depend on it either way: that is the global
/// click monitor's job.
///
/// `activate` is the cooperative replacement for `activateIgnoringOtherApps`,
/// and it is the call macOS 14 and later expect. It has to come after the
/// window is on screen: activating an app with no visible window is what macOS
/// declines.
#[cfg(target_os = "macos")]
fn activate_app() {
    if let Some(mtm) = objc2::MainThreadMarker::new() {
        objc2_app_kit::NSApplication::sharedApplication(mtm).activate();
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_app() {}

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

/// The monitor whose frame contains a physical point, if any.
///
/// The window's own monitor is not the right answer here: the panel can be
/// shown from any Space and any display, so where it *last was* says nothing
/// about where the menu-bar icon is now.
fn monitor_at<R: Runtime>(panel: &WebviewWindow<R>, x: f64, y: f64) -> Option<tauri::Monitor> {
    panel
        .available_monitors()
        .ok()?
        .into_iter()
        .find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            x >= position.x as f64
                && x < (position.x as f64 + size.width as f64)
                && y >= position.y as f64
                && y < (position.y as f64 + size.height as f64)
        })
}

/// Centres the panel under the menu-bar icon rather than dropping it in a
/// screen corner, and keeps it fully on screen: a panel half off the edge is
/// worse than one that is slightly off-centre.
fn place_under_tray<R: Runtime>(panel: &WebviewWindow<R>, tray_rect: &Rect) {
    let Ok(size) = panel.outer_size() else {
        return;
    };

    let (tray_x, tray_y) = position_of(tray_rect.position);
    let (tray_width, tray_height) = size_of(tray_rect.size);
    let center_x = tray_x + tray_width / 2.0;
    let below_tray_physical = tray_y + tray_height;

    // The icon's own monitor decides the scale, not the panel's: on a mixed
    // Retina/non-Retina setup the two are not the same number.
    let monitor = monitor_at(panel, center_x, below_tray_physical);
    let scale = match monitor.as_ref() {
        Some(monitor) => monitor.scale_factor(),
        None => panel.scale_factor().unwrap_or(1.0),
    };

    let panel_width = size.width as f64;
    let mut x = center_x / scale - panel_width / (2.0 * scale);

    if let Some(monitor) = monitor.as_ref() {
        // The monitor reports physical pixels directly, so it needs none of the
        // enum handling the tray rectangle does.
        let monitor_size = monitor.size();
        let monitor_position = monitor.position();
        let left = monitor_position.x as f64 / scale;
        let usable = monitor_size.width as f64 / scale - panel_width / scale;
        x = x.clamp(left, left + usable);
    }

    let _ = panel.set_position(LogicalPosition::new(x, below_tray_physical / scale));
}

/// Hides the panel when it loses focus, so clicking anywhere else dismisses it
/// the way a menu-bar popover is expected to behave.
///
/// Only used on platforms without the global click monitor that macOS installs.
/// On macOS the app can lose activation a second or two after the panel is
/// shown without anyone having clicked anything, and this rule reads that as a
/// dismissal and takes the panel away while it is being read.
#[cfg(not(target_os = "macos"))]
fn hide_panel_when_it_loses_focus<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();

    if let Some(panel) = app.get_webview_window(PANEL_LABEL) {
        panel.on_window_event(move |event| {
            if !matches!(event, WindowEvent::Focused(false)) {
                return;
            }
            if let Some(panel) = app.get_webview_window(PANEL_LABEL) {
                let _ = panel.hide();
            }
        });
    }
}
