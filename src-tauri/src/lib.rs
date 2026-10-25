// The commands are public so the window behaviour can be exercised on a real
// window rather than only through the interface. The fullscreen logic in
// particular cannot be tested with a mock runtime, because whether macOS gives
// an accessory app a fullscreen space is a question about macOS.
pub mod commands;
mod models;
mod monitoring;
mod router;
pub mod speedtest;
mod tray;

use std::sync::Arc;

use tauri::Manager;

use monitoring::config::default_settings;
use monitoring::probe::TcpNetwork;
use monitoring::runner::{interval_for, spawn};
use monitoring::state::MonitorState;
use monitoring::store::{Store, default_path};
use router::app_config;
use router::state::RouterState;

/// The history database, as managed state.
///
/// A newtype because `Option<Arc<Store>>` and `Arc<Store>` are different
/// managed types to Tauri and a bare `Option` cannot be distinguished from
/// "not managed at all". Wrapping makes "history is switched off" an explicit,
/// readable state rather than an absent one.
pub struct HistoryStore(pub Option<Arc<Store>>);

impl HistoryStore {
    pub fn get(&self) -> Option<&Arc<Store>> {
        self.0.as_ref()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(MonitorState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .setup(move |app| {
            // A menu-bar app has no Dock presence. Without this, macOS shows a
            // Dock icon and an application menu, which makes the app look and
            // behave like the windowed app it is replacing.
            //
            // The policy is set before the tray is installed because the panel
            // is created there, and macOS only gives a window working Space
            // behaviour when the app is already a menu-bar app at that moment.
            // `tray::create_panel` explains what goes wrong otherwise. The
            // dashboard is hidden on launch (see `tauri.conf.json`) so nothing
            // is on screen while this happens, and it switches the app back to
            // a regular app once the panel exists.
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::install(app.handle())?;

            // The probe loop starts here rather than on first request, so the
            // dashboard has a reading waiting for it instead of an empty first
            // paint it then has to correct.
            let settings = default_settings();

            // History is opened before the loop so the very first probe is
            // stored. A failure here is reported and then ignored: the app has
            // to keep monitoring, because refusing to start over an unwritable
            // database would trade the feature that matters for one that does
            // not.
            let store = match app.path().app_data_dir() {
                Ok(directory) => {
                    let path = default_path(&directory);
                    match Store::open(&path) {
                        Ok(store) => Some(Arc::new(store)),
                        Err(error) => {
                            eprintln!("history disabled, could not open {path:?}: {error}");
                            None
                        }
                    }
                }
                Err(error) => {
                    eprintln!("history disabled, no app data directory: {error}");
                    None
                }
            };
            app.manage(HistoryStore(store.clone()));

            // Tauri's setup hook runs on the main thread, outside the async
            // runtime. Spawning a task there panics with "no reactor running"
            // and takes the whole app down, so the runtime context is entered
            // for as long as the spawn takes. The monitoring engine stays plain
            // tokio and does not need to know about Tauri.
            let monitor_handle = {
                let runtime = tauri::async_runtime::handle();
                let _entered = runtime.inner().enter();
                spawn(
                    Arc::new(TcpNetwork),
                    settings.clone(),
                    state.clone(),
                    interval_for(&settings),
                    store,
                )
            };
            // Managed so the handle outlives setup: dropping it would stop the
            // loop immediately.
            app.manage(monitor_handle);

            // The router reader. Credentials resolve in a fixed order: a
            // complete pair in the process environment wins (a developer
            // running it from a shell keeps override power), then what the user
            // saved in the app's own settings card — non-secrets in a JSON file
            // under the app's settings directory, the password in the
            // Keychain. Without either, the connectivity monitoring runs
            // exactly as before and only the router sections go quiet. The
            // reader runs on its own thread because the reads are blocking
            // requests to a small device on a much slower cadence than a probe.
            let config_dir = app.path().app_config_dir().ok();
            if let Some(dir) = config_dir.as_ref() {
                // Created before anything reads it, so the first save cannot
                // fail on a missing directory.
                let _ = std::fs::create_dir_all(dir);
            }
            app.manage(app_config::ConfigDir(config_dir.clone()));

            let router_state = Arc::new(RouterState::new());
            let stored = config_dir.as_deref().and_then(app_config::read_config);
            let resolution = app_config::resolve_full(
                |name| std::env::var(name).ok(),
                stored,
                &app_config::KeyringStore,
            );
            match resolution.credentials {
                Some(credentials) => {
                    // The host and user name are safe to name in a log line; the
                    // password is not, and is never touched here.
                    eprintln!(
                        "router integration enabled for {}:{} ({:?})",
                        credentials.host,
                        credentials.port,
                        resolution.report.source
                    );
                    router_state.set_credentials(Some(credentials));
                }
                None => {
                    eprintln!(
                        "router integration disabled, no credentials in the environment or the app settings"
                    );
                    router::reader::record_unconfigured(&router_state);
                }
            }
            app.manage(router_state.clone());
            app.manage(router::reader::spawn(router_state));

            // The app's own record of its fullscreen state. The window's
            // library reports a fullscreen request as a fullscreen whether or
            // not macOS honoured it, so the app keeps its own record and
            // checks it against the window's real size.
            app.manage(commands::window::FullscreenTracker::default());

            // The green button is the app's fullscreen control, so it is
            // pointed at the window's fullscreen action rather than left to
            // zoom. AppKit wants this on the main thread.
            if let Some(window) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                let _ = handle.run_on_main_thread(move || {
                    if let Err(error) = commands::window_controls::make_green_button_fullscreen(&window) {
                        eprintln!("the green button keeps its default behaviour: {error}");
                    }
                });
            }

            // The dashboard opens on launch, as it did when its window was
            // declared visible. It opens last: the panel has to be created
            // while the app is accessory, and showing the dashboard switches
            // the app back to a regular app for as long as it is on screen.
            if let Err(error) = commands::window::show_dashboard(app.handle()) {
                eprintln!("the dashboard did not open at launch: {error}");
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Closing a window must not end a menu-bar app. The process
                // outlives every window and only Quit stops it, because an app
                // that hides everything and refuses to exit cannot be killed
                // from the menu bar.
                api.prevent_close();
                let _ = window.hide();

                // With the dashboard gone the app goes back to being a menu-bar
                // app: no Dock icon and no application menu. It becomes a
                // regular app again the moment the dashboard is shown, which is
                // what lets a fullscreen hide the menu bar and the Dock.
                commands::window::hide_dashboard(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview::get_connection_overview,
            commands::overview::get_latency_series,
            commands::overview::set_monitoring_paused,
            commands::overview::is_monitoring_paused,
            commands::history::get_incidents,
            commands::history::get_history_info,
            commands::history::get_stored_series,
            commands::history::get_uptime,
            commands::window::show_main_window,
            commands::window::toggle_fullscreen,
            commands::window::leave_fullscreen_mode,
            commands::speedtest::run_speed_test,
            commands::router::get_router_report,
            commands::router::router_connection_get,
            commands::router::router_connection_save,
            commands::router::router_connection_clear
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
