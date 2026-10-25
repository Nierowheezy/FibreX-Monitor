use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime};

/// The dashboard window, from `tauri.conf.json`.
const MAIN_LABEL: &str = "main";

/// Brings the dashboard forward.
///
/// A menu-bar app keeps this window alive while it is hidden, so showing it is
/// the normal case rather than creating anything.
///
/// The app also becomes a regular macOS app while the dashboard is on screen.
/// This is what lets the menu bar and the Dock go away in fullscreen.
///
/// An accessory app has no menu bar of its own, so macOS keeps showing the
/// menu bar of whichever regular app was in front, and the app cannot own a
/// fullscreen space. That is why the menu bar survived a fullscreen attempt:
/// the app was asking macOS for a space it was not allowed to have. A regular
/// app can have one, and in it the menu bar and the Dock are hidden until the
/// pointer reaches the top of the screen, which is the behaviour that was
/// asked for.
pub fn show_dashboard<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_LABEL)
        .ok_or_else(|| "the dashboard window is missing".to_string())?;

    // The panel floats above ordinary windows, so it would sit on top of the
    // dashboard that is about to open.
    crate::tray::hide_panel(app);

    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;

    Ok(())
}

/// Puts the app back to being a menu-bar app.
///
/// Called when the dashboard is hidden. The resting state of this app is a
/// menu-bar item with no Dock icon and no application menu, which is the whole
/// point of it being a menu-bar app.
pub fn hide_dashboard<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

/// Lets the menu-bar panel raise the dashboard. Idempotent: calling it while the
/// dashboard is already visible just focuses it.
#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_dashboard(&app)
}

/// How the window is filling the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FullscreenMode {
    /// A normal window.
    #[default]
    Windowed,
    /// The native macOS fullscreen, in its own space. The menu bar and the Dock
    /// are out of the way and come back on hover, and the traffic lights stay.
    Native,
    /// The pre-Lion fullscreen: the window covers the screen and the
    /// application's presentation options hide the menu bar and the Dock.
    ///
    /// This is the one a menu-bar app can use. macOS does not give an
    /// accessory app a fullscreen space, so the native path does not engage and
    /// the menu bar stays exactly where it was.
    Simple,
}

/// What the window ended up doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullscreenState {
    pub fullscreen: bool,
    pub mode: FullscreenMode,
}

/// What the app knows about its own fullscreen state.
///
/// The window's library cannot be asked. It records a fullscreen request as
/// fullscreen the moment the request is made, whether or not macOS honoured
/// it, so its answer is a claim about the request rather than about the
/// window. The app keeps its own record instead, and reconciles it with the
/// window's actual size.
#[derive(Default)]
pub struct FullscreenTracker {
    mode: Mutex<FullscreenMode>,
}

impl FullscreenTracker {
    /// The mode the app last put the window into.
    pub fn mode(&self) -> FullscreenMode {
        *self.mode.lock().expect("fullscreen tracker poisoned")
    }

    /// Records the mode the window is in.
    pub fn set(&self, mode: FullscreenMode) {
        *self.mode.lock().expect("fullscreen tracker poisoned") = mode;
    }
}

/// How long the native fullscreen transition is given before it is treated as
/// not having happened.
///
/// macOS animates the move into the fullscreen space, and the window does not
/// cover the screen until the animation lands.
const NATIVE_TRANSITION_MS: u64 = 2_000;

/// How often the window is measured while waiting for the transition.
const POLL_INTERVAL_MS: u64 = 80;

/// How long the library is given to give up its claim of being fullscreen.
///
/// The library believes it is fullscreen from the moment it is asked, and only
/// corrects itself when macOS reports the attempt failed. The two fullscreen
/// paths are mutually exclusive, so the correction has to land before the
/// other one is tried.
const RESET_BUDGET_MS: u64 = 2_000;

/// How long to wait after the library has given up its claim, before the other
/// path is tried.
///
/// The correction arrives as a notification, which is handled after the request
/// that caused it. Asking the library in the same breath as the correction
/// would be asking too early.
const SETTLE_MS: u64 = 400;

/// Enters or leaves fullscreen, and reports what actually happened.
///
/// The native path is tried first because it is the better of the two: the
/// traffic lights stay, the notch is handled, and the window gets its own
/// space. It does not engage for a menu-bar app, so when it does not, the
/// simple path is used instead. The simple path is how fullscreen worked
/// before macOS Lion: it sets the application's presentation options to hide
/// the menu bar and the Dock and sizes the window to the screen, which is the
/// behaviour that was asked for.
#[tauri::command]
pub async fn toggle_fullscreen(
    app: AppHandle,
    tracker: tauri::State<'_, FullscreenTracker>,
) -> Result<FullscreenState, String> {
    let window = app
        .get_webview_window(MAIN_LABEL)
        .ok_or_else(|| "the dashboard window is missing".to_string())?;

    // A fullscreen space belongs to a regular app, so the app becomes one for
    // as long as the dashboard is on screen.
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    toggle(&window, &tracker).await
}

/// Enters or leaves fullscreen, and reports what actually happened.
///
/// The native path is tried first because it is the better of the two: the
/// traffic lights stay, the notch is handled, and the window gets its own
/// space. It does not engage for a menu-bar app, so when it does not, the
/// simple path is used instead. The simple path is how fullscreen worked
/// before macOS Lion: it sets the application's presentation options to hide
/// the menu bar and the Dock and sizes the window to the screen, which is the
/// behaviour that was asked for.
pub async fn toggle<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    tracker: &FullscreenTracker,
) -> Result<FullscreenState, String> {
    // What the app believes, reconciled with what the window actually is.
    let current = current_mode(window, tracker.mode());

    if current != FullscreenMode::Windowed {
        leave_fullscreen(window)?;
        tracker.set(FullscreenMode::Windowed);
        eprintln!("fullscreen: left the {current:?} mode");
        return Ok(FullscreenState {
            fullscreen: false,
            mode: FullscreenMode::Windowed,
        });
    }

    // The size the window had before the attempt, so the attempt can be judged
    // by whether the window actually moved.
    let before = window.outer_size().ok();

    // The native path first.
    //
    // A regular app can own a fullscreen space, and in that space the menu bar
    // and the Dock are hidden until the pointer reaches the top of the screen.
    // An accessory app cannot, which is why the native path is tried at all
    // rather than assumed.
    window
        .set_fullscreen(true)
        .map_err(|error| error.to_string())?;

    if wait_for_full_screen(window, before).await {
        tracker.set(FullscreenMode::Native);
        eprintln!("fullscreen: native, the window covers the screen");
        return Ok(FullscreenState {
            fullscreen: true,
            mode: FullscreenMode::Native,
        });
    }

    // The native path did not engage. This app runs as an accessory, so macOS
    // gives it no fullscreen space. Clear the attempt, wait for the library to
    // notice, then use the path that does work.
    let _ = window.set_fullscreen(false);
    wait_for_release(window).await;
    tokio::time::sleep(std::time::Duration::from_millis(SETTLE_MS)).await;

    window
        .set_simple_fullscreen(true)
        .map_err(|error| error.to_string())?;

    // The claim is checked, not assumed. A command that says it made the
    // window fullscreen when it did not is worse than one that says it could
    // not.
    if !covers_the_screen(window, None) {
        tracker.set(FullscreenMode::Windowed);
        eprintln!("fullscreen: neither mode engaged, the window is unchanged");
        return Ok(FullscreenState {
            fullscreen: false,
            mode: FullscreenMode::Windowed,
        });
    }

    tracker.set(FullscreenMode::Simple);
    eprintln!("fullscreen: simple, the window covers the screen and the menu bar is hidden");
    Ok(FullscreenState {
        fullscreen: true,
        mode: FullscreenMode::Simple,
    })
}

/// What the window is doing now, reconciled with the app's own record.
///
/// The two disagree in one direction only. The library claims a fullscreen that
/// never happened, so the library is never believed on its own. The app's
/// record is believed, and checked against the window's actual size: a window
/// that was in the simple mode and no longer covers the screen has been left.
///
/// The window's own answer is used in the other direction. The green button
/// enters the native fullscreen without going through this app at all, and the
/// library knows about that, so a window the library calls fullscreen is in the
/// native fullscreen even if the app never asked for it.
fn current_mode<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    tracked: FullscreenMode,
) -> FullscreenMode {
    match tracked {
        FullscreenMode::Windowed => {
            if window.is_fullscreen().unwrap_or(false) {
                FullscreenMode::Native
            } else {
                FullscreenMode::Windowed
            }
        }
        FullscreenMode::Native => {
            if window.is_fullscreen().unwrap_or(false) {
                FullscreenMode::Native
            } else {
                FullscreenMode::Windowed
            }
        }
        FullscreenMode::Simple => {
            if covers_the_screen(window, None) {
                FullscreenMode::Simple
            } else {
                FullscreenMode::Windowed
            }
        }
    }
}

/// Leaves whichever fullscreen the window is in.
///
/// Both have to be tried, because the two are mutually exclusive: the library
/// ignores the native request while the simple one is active, and the simple
/// one refuses while the native one is active.
fn leave_fullscreen<R: Runtime>(window: &tauri::WebviewWindow<R>) -> Result<(), String> {
    // The native exit first, so a window in the native space leaves it.
    let _ = window.set_fullscreen(false);
    // Then the simple exit, so a window in the simple mode gives the screen
    // back and restores the title bar it removed.
    let _ = window.set_simple_fullscreen(false);
    Ok(())
}

/// Waits for the window to fill its screen, and says whether it did.
///
/// The window's own frame is the evidence, not the library's opinion. The
/// library records the request as fullscreen the moment it is made, so asking
/// it would only confirm that the request was sent.
async fn wait_for_full_screen<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    before: Option<tauri::PhysicalSize<u32>>,
) -> bool {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(NATIVE_TRANSITION_MS);

    while std::time::Instant::now() < deadline {
        if covers_the_screen(window, before) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
    }

    false
}

/// Waits for the library to give up its claim of being fullscreen.
///
/// It only corrects itself once macOS reports that the attempt failed, and the
/// other fullscreen path refuses to run while it still believes it is
/// fullscreen. So the correction has to be waited for rather than assumed.
async fn wait_for_release<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(RESET_BUDGET_MS);

    while std::time::Instant::now() < deadline {
        if !window.is_fullscreen().unwrap_or(false) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

/// Whether the window now covers its whole screen.
///
/// `before` is the size it had before the attempt, and it is what makes this a
/// real test: a window that already filled the screen and then did not change
/// has not entered a fullscreen space, it was simply already large.
fn covers_the_screen<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    before: Option<tauri::PhysicalSize<u32>>,
) -> bool {
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        return false;
    };
    let Ok(size) = window.outer_size() else {
        return false;
    };

    let screen = monitor.size();
    let fills = size.width + 8 >= screen.width && size.height + 8 >= screen.height;

    if !fills {
        return false;
    }

    // A window that was already that size has not moved, so it has not entered
    // a fullscreen space either.
    match before {
        Some(previous) => previous.width != size.width || previous.height != size.height,
        None => true,
    }
}

/// Leaves fullscreen without toggling.
///
/// A separate command because Escape has to leave fullscreen even when the
/// window is already fullscreen, and a toggle would take it the other way.
#[tauri::command]
pub fn leave_fullscreen_mode(
    app: AppHandle,
    tracker: tauri::State<'_, FullscreenTracker>,
) -> Result<FullscreenState, String> {
    let window = app
        .get_webview_window(MAIN_LABEL)
        .ok_or_else(|| "the dashboard window is missing".to_string())?;

    leave_fullscreen(&window)?;
    tracker.set(FullscreenMode::Windowed);

    Ok(FullscreenState {
        fullscreen: false,
        mode: FullscreenMode::Windowed,
    })
}
