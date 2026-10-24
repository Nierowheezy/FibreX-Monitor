//! The commands the interface calls to read router data.

use std::sync::Arc;

use tauri::State;

use crate::router::app_config::{self, ConfigDir, KeyringStore, RouterConnectionReport};
use crate::router::state::{RouterReport, RouterState};

/// Everything the router integration currently knows.
///
/// One command rather than several, because the interface always wants the whole
/// picture at once: the snapshot if there is one, why there is not if there is
/// not, and whether the router is even configured. Three separate calls would let
/// a screen render a stale combination of them.
#[tauri::command]
pub fn get_router_report(state: State<'_, Arc<RouterState>>) -> RouterReport {
    state.report()
}

/// What the settings card needs: which source is in charge and whether reads
/// will happen, without ever returning the password itself.
#[tauri::command]
pub fn router_connection_get(dir: State<'_, ConfigDir>) -> Result<RouterConnectionReport, String> {
    Ok(app_config::current_report(dir.0.as_deref(), &KeyringStore))
}

/// Applies a save from the settings card.
///
/// `<password>` empty means keep the stored one, so retyping the password is
/// never required to change the address; erasing the connection is a separate,
/// explicit action.
#[tauri::command]
pub fn router_connection_save(
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    dir: State<'_, ConfigDir>,
    router: State<'_, Arc<RouterState>>,
) -> Result<RouterConnectionReport, String> {
    app_config::save_connection(
        dir.0.as_deref(),
        &KeyringStore,
        &router,
        &host,
        port,
        &username,
        password.as_deref(),
    )
}

/// Erases the stored settings and password and disarms the reader.
///
/// A deliberate, separate action rather than a side effect of editing, so a
/// mistyped address can never cost someone their password.
#[tauri::command]
pub fn router_connection_clear(
    dir: State<'_, ConfigDir>,
    router: State<'_, Arc<RouterState>>,
) -> Result<RouterConnectionReport, String> {
    app_config::clear_connection(dir.0.as_deref(), &KeyringStore, &router)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::overview::{RouterHealth, RouterNetwork, WanConnection};
    use crate::router::{
        RouterAuthSummary, RouterError, RouterErrorKind, RouterSnapshot, RouterSourceStatus,
    };

    fn snapshot() -> RouterSnapshot {
        RouterSnapshot {
            read_at: 1_700_000_000_000,
            auth: RouterAuthSummary {
                host: "192.168.100.1".to_string(),
                port: 80,
                username: Some("root".to_string()),
                has_password: true,
            },
            wan: WanConnection::unavailable(),
            health: RouterHealth::unavailable(),
            counters: None,
            network: RouterNetwork::default(),
            sources: vec![RouterSourceStatus::ok("WAN connection", "/wan")],
        }
    }

    /// Registration and serialisation are the two things a unit call cannot
    /// prove, so this drives the command through the real IPC boundary.
    #[test]
    fn the_command_answers_over_ipc_with_the_contract() {
        use tauri::ipc::CallbackFn;
        use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
        use tauri::webview::InvokeRequest;

        let state = Arc::new(RouterState::new());

        let app = mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![get_router_report])
            .build(mock_context(noop_assets()))
            .expect("mock app must build");

        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview must build");

        let call = || {
            let url = "tauri://localhost".parse().expect("valid invoke url");
            get_ipc_response(
                &webview,
                InvokeRequest {
                    cmd: "get_router_report".into(),
                    callback: CallbackFn(0),
                    error: CallbackFn(1),
                    url,
                    body: tauri::ipc::InvokeBody::default(),
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            )
            .expect("command must succeed")
            .deserialize::<serde_json::Value>()
            .expect("response must be JSON")
        };

        // Nothing configured and nothing attempted.
        let empty = call();
        assert_eq!(empty["configured"], serde_json::json!(false));
        assert_eq!(empty["attempted"], serde_json::json!(false));
        assert_eq!(empty["snapshot"], serde_json::json!(null));
        assert_eq!(empty["error"], serde_json::json!(null));

        // Then a real read.
        state.record_success(snapshot());
        let filled = call();
        assert_eq!(filled["configured"], serde_json::json!(false));
        assert_eq!(filled["attempted"], serde_json::json!(true));
        assert_eq!(
            filled["snapshot"]["readAt"],
            serde_json::json!(1_700_000_000_000_i64)
        );
        assert_eq!(
            filled["snapshot"]["auth"]["hasPassword"],
            serde_json::json!(true)
        );

        // Then a failure on top of it, which must not discard the snapshot.
        state.record_failure(RouterError::new(RouterErrorKind::Timeout, "no answer"));
        let failed = call();
        assert_eq!(failed["error"]["kind"], serde_json::json!("timeout"));
        assert_eq!(
            failed["snapshot"]["readAt"],
            serde_json::json!(1_700_000_000_000_i64),
            "a later failure must not throw away a real reading"
        );
    }
}
