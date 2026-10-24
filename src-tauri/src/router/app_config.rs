//! The router connection the user can edit inside the app.
//!
//! Before this module the router credentials existed only in the process
//! environment, which is right for a development machine but unusable for
//! someone who just downloaded the app: they cannot be expected to launch a
//! binary with exported variables. So the address, port, and user name are
//! stored in a small JSON file inside the app's own directory, and the password
//! is stored in the macOS Keychain through `keyring` — never in that file,
//! never in the source, and never in a SQLite table that someone else's
//! process could open.
//!
//! The environment still wins when it is present. A developer running the
//! binary from a shell keeps full control without having to clear the stored
//! settings first, and the interface is told which source is in charge so it
//! can say so rather than pretending the form is the only truth.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::RouterCredentials;
use super::credentials;
use super::state::RouterState;

/// The Keychain entries live under this service, so the item is named after the
/// app and is trivially inspectable in Keychain Access on purpose. Secrecy
/// through obscurity is not the mechanism: the Keychain's own access control is.
pub const PASSWORD_SERVICE: &str = "FibreX Monitor";
/// One account per purpose, so a future second secret does not fight this one.
pub const PASSWORD_ACCOUNT: &str = "router";

/// The stored file name, inside the directory managed as [`ConfigDir`].
pub const CONFIG_FILE_NAME: &str = "router.json";

/// The directory the stored settings live in, managed so commands can reach it.
///
/// `None` when the platform could not provide its usual per-app settings
/// directory; the settings card then says so rather than failing silently.
#[derive(Clone, Debug)]
pub struct ConfigDir(pub Option<PathBuf>);

/// The non-secret router settings, as they are stored and served.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterAppConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
}

impl Default for RouterAppConfig {
    fn default() -> Self {
        Self {
            host: "192.168.100.1".to_string(),
            port: 80,
            username: String::new(),
        }
    }
}

/// Where the effective credentials came from, as the interface should name it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RouterCredentialSource {
    /// A complete pair in the process environment wins over stored settings.
    Environment,
    /// What the user saved in the app.
    AppSettings,
}

/// How the router reads stand right now, as the settings card and the report
/// command serve it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterConnectionReport {
    /// Which source is in charge, or `None` when nothing is configured at all.
    pub source: Option<RouterCredentialSource>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    /// Whether a password is available, without ever carrying it.
    pub has_password: bool,
    /// Whether router reads will actually be attempted. A stored address with
    /// no password is saved but will not read, and the interface must say so.
    pub will_read: bool,
    /// A stored configuration exists but the password was never saved.
    pub password_missing: bool,
}

/// What a password lookup, write or erase can fail with, without pretending it
/// succeeded.
#[derive(Debug, Clone)]
pub struct PasswordStoreError(pub String);

/// The secret storage, abstracted so a test never has to touch the real
/// Keychain and a non-macOS build does not pretend it has one.
pub trait PasswordStore {
    fn get(&self) -> Result<Option<String>, PasswordStoreError>;
    fn set(&self, password: &str) -> Result<(), PasswordStoreError>;
    fn clear(&self) -> Result<(), PasswordStoreError>;
}

#[cfg(target_os = "macos")]
mod keychain {
    use keyring;

    use super::{PASSWORD_ACCOUNT, PASSWORD_SERVICE, PasswordStore, PasswordStoreError};

    /// The real macOS Keychain.
    ///
    /// A missing item is the normal state of a fresh install, so it maps to
    /// `Ok(None)` rather than an error; anything else is honest failure.
    pub struct KeyringStore;

    impl PasswordStore for KeyringStore {
        fn get(&self) -> Result<Option<String>, PasswordStoreError> {
            let entry = keyring::Entry::new(PASSWORD_SERVICE, PASSWORD_ACCOUNT)
                .map_err(|error| PasswordStoreError(error.to_string()))?;
            match entry.get_password() {
                Ok(password) if password.is_empty() => Ok(None),
                Ok(password) => Ok(Some(password)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(PasswordStoreError(error.to_string())),
            }
        }

        fn set(&self, password: &str) -> Result<(), PasswordStoreError> {
            let entry = keyring::Entry::new(PASSWORD_SERVICE, PASSWORD_ACCOUNT)
                .map_err(|error| PasswordStoreError(error.to_string()))?;
            entry
                .set_password(password)
                .map_err(|error| PasswordStoreError(error.to_string()))
        }

        fn clear(&self) -> Result<(), PasswordStoreError> {
            let entry = keyring::Entry::new(PASSWORD_SERVICE, PASSWORD_ACCOUNT)
                .map_err(|error| PasswordStoreError(error.to_string()))?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                // Erasing something that is not there reached the same state.
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(PasswordStoreError(error.to_string())),
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use keychain::KeyringStore;

#[cfg(not(target_os = "macos"))]
mod fallback {
    use std::sync::Mutex;

    use super::{PASSWORD_ACCOUNT, PASSWORD_SERVICE, PasswordStore, PasswordStoreError};

    /// Where a secret goes on a platform with no managed Keychain integration.
    ///
    /// Honest about being weaker: the build targets macOS, and this only keeps
    /// a non-macOS build from lying about a Keychain it never had.
    #[derive(Default)]
    pub struct KeyringStore {
        held: Mutex<Option<(String, String)>>,
    }

    impl PasswordStore for KeyringStore {
        fn get(&self) -> Result<Option<String>, PasswordStoreError> {
            let held = self.held.lock().expect("the lock cannot be poisoned");
            Ok(held
                .as_ref()
                .filter(|(service, account)| {
                    service == PASSWORD_SERVICE && account == PASSWORD_ACCOUNT
                })
                .map(|(_, password)| password.clone()))
        }

        fn set(&self, password: &str) -> Result<(), PasswordStoreError> {
            let mut held = self.held.lock().expect("the lock cannot be poisoned");
            *held = Some((
                PASSWORD_SERVICE.to_string(),
                PASSWORD_ACCOUNT.to_string(),
                password.to_string(),
            ));
            Ok(())
        }

        fn clear(&self) -> Result<(), PasswordStoreError> {
            let mut held = self.held.lock().expect("the lock cannot be poisoned");
            *held = None;
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub use fallback::KeyringStore;

/// Reads the saved non-secret settings, or `None` when they are absent or
/// unreadable.
///
/// A truncated or hand-edited file is treated as absent rather than as an
/// error that blocks the app: the settings card simply shows defaults.
pub fn read_config(dir: &Path) -> Option<RouterAppConfig> {
    let text = fs::read_to_string(dir.join(CONFIG_FILE_NAME)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes the saved non-secret settings, creating the directory first.
pub fn write_config(dir: &Path, config: &RouterAppConfig) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let text = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(dir.join(CONFIG_FILE_NAME), text).map_err(|error| error.to_string())
}

/// Removes the saved non-secret settings. Absent already is success.
pub fn delete_config(dir: &Path) {
    let _ = fs::remove_file(dir.join(CONFIG_FILE_NAME));
}

/// Resolves which credentials the router reads should use.
///
/// Order is deliberate: a complete environment pair wins (a developer's shell
/// overrides saved settings), then a stored configuration with a stored
/// password reads, then a stored configuration without one reports itself as
/// saved-but-not-reading, and nothing is simply unconfigured.
pub fn resolve(
    lookup: impl Fn(&str) -> Option<String>,
    stored: Option<RouterAppConfig>,
    store: &dyn PasswordStore,
) -> RouterConnectionReport {
    let mut report = RouterConnectionReport {
        source: None,
        host: None,
        port: None,
        username: None,
        has_password: false,
        will_read: false,
        password_missing: false,
    };

    if let Some(saved) = stored.as_ref() {
        report.host = Some(saved.host.clone());
        report.port = Some(saved.port);
        report.username = Some(saved.username.clone());
        report.source = Some(RouterCredentialSource::AppSettings);
        match store.get() {
            Ok(Some(password)) if !password.is_empty() => {
                report.has_password = true;
            }
            _ => {
                report.password_missing = true;
            }
        }
    }

    if credentials::from_environment(&lookup).is_some() {
        report.source = Some(RouterCredentialSource::Environment);
    }

    // What the reader needs, in the same precedence the report describes.
    if report.source == Some(RouterCredentialSource::Environment) {
        report.will_read = true;
        return report;
    }

    if let (Some(saved), true) = (stored.as_ref(), report.has_password) {
        report.will_read = !saved.username.trim().is_empty();
    }

    report
}

/// A saved configuration as it will be applied, or why nothing can be applied.
///
/// Returned together with the report so the command applies exactly what the
/// resolution rules chose, and no command re-implements the rules on its own.
#[derive(Debug, Clone)]
pub struct Resolution {
    pub report: RouterConnectionReport,
    pub credentials: Option<RouterCredentials>,
}

/// The same resolution, plus the credentials the reader should use.
///
/// The report alone cannot carry the password, so this is the only type that
/// hands one along, and it is built in exactly one function.
pub fn resolve_full(
    lookup: impl Fn(&str) -> Option<String>,
    stored: Option<RouterAppConfig>,
    store: &dyn PasswordStore,
) -> Resolution {
    let report = resolve(&lookup, stored.clone(), store);

    let credentials = if report.will_read {
        match report.source {
            Some(RouterCredentialSource::Environment) => credentials::from_environment(&lookup),
            Some(RouterCredentialSource::AppSettings) => stored.and_then(|saved| {
                store.get().ok().flatten().and_then(|password| {
                    let username = saved.username.trim().to_string();
                    if username.is_empty() || password.is_empty() {
                        None
                    } else {
                        Some(RouterCredentials {
                            host: saved.host,
                            port: saved.port,
                            username,
                            password,
                        })
                    }
                })
            }),
            None => None,
        }
    } else {
        None
    };

    Resolution {
        report,
        credentials,
    }
}

/// Applies a save from the settings card: validates, writes, and leaves the
/// reader in exactly the state the resolution says it should be in.
pub fn save_connection(
    dir: Option<&Path>,
    store: &dyn PasswordStore,
    router_state: &RouterState,
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
) -> Result<RouterConnectionReport, String> {
    let host = host.trim();
    let username = username.trim();
    if host.is_empty() {
        return Err("The router address is required.".to_string());
    }
    if !(1..=65_535).contains(&port) {
        return Err("The port must be between 1 and 65535.".to_string());
    }
    if username.is_empty() {
        return Err("The user name is required.".to_string());
    }

    let config = RouterAppConfig {
        host: host.to_string(),
        port,
        username: username.to_string(),
    };

    if let Some(dir) = dir {
        write_config(dir, &config)?;
    }

    // A present password overwrites, an absent or empty one leaves the stored
    // one alone, so re-saving the address without retyping the password is the
    // ordinary way round.
    if let Some(password) = password.filter(|entered| !entered.is_empty()) {
        store
            .set(password)
            .map_err(|error| format!("The password could not be stored: {}", error.0))?;
    }

    let config_dir = dir.map(|dir| dir.to_path_buf());
    let stored = config_dir.as_deref().and_then(read_config);
    let resolution = resolve_full(|name| std::env::var(name).ok(), stored, store);
    let called_out = resolution.credentials.is_some();

    router_state.set_credentials(resolution.credentials);
    if !called_out {
        super::reader::record_unconfigured(router_state);
    }

    Ok(resolution.report)
}

/// Applies a clear from the settings card: the stored settings and password are
/// erased, and the reader drops back to not configured.
pub fn clear_connection(
    dir: Option<&Path>,
    store: &dyn PasswordStore,
    router_state: &RouterState,
) -> Result<RouterConnectionReport, String> {
    if let Some(dir) = dir {
        delete_config(dir);
    }
    store
        .clear()
        .map_err(|error| format!("The stored password could not be removed: {}", error.0))?;

    router_state.set_credentials(None);
    super::reader::record_unconfigured(router_state);

    Ok(current_report(
        dir.map(|dir| dir.to_path_buf()).as_deref(),
        store,
    ))
}

/// Reads the current connection report without touching anything.
pub fn current_report(dir: Option<&Path>, store: &dyn PasswordStore) -> RouterConnectionReport {
    let stored = dir.and_then(read_config);
    resolve_full(|name| std::env::var(name).ok(), stored, store).report
}

#[cfg(test)]
mod tests {
    use super::credentials::{HOST_VAR, PASSWORD_VAR, USERNAME_VAR};
    use super::*;

    /// An in-memory store, so no test ever touches a real Keychain.
    struct FakeStore(std::cell::RefCell<Option<String>>);

    impl FakeStore {
        fn with_password(value: &str) -> Self {
            Self(std::cell::RefCell::new(Some(value.to_string())))
        }
    }

    impl Default for FakeStore {
        fn default() -> Self {
            Self(std::cell::RefCell::new(None))
        }
    }

    impl PasswordStore for FakeStore {
        fn get(&self) -> Result<Option<String>, PasswordStoreError> {
            Ok(self.0.borrow().clone())
        }

        fn set(&self, password: &str) -> Result<(), PasswordStoreError> {
            *self.0.borrow_mut() = Some(password.to_string());
            Ok(())
        }

        fn clear(&self) -> Result<(), PasswordStoreError> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    fn save_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fibrex-monitor-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn stored_config() -> RouterAppConfig {
        RouterAppConfig {
            host: "192.168.100.1".to_string(),
            port: 80,
            username: "admin".to_string(),
        }
    }

    fn env_map(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        use std::collections::HashMap;
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    #[test]
    fn an_empty_configuration_is_unconfigured_and_honest_about_it() {
        let store = FakeStore::default();

        let report = resolve(env_map(&[]), None, &store);

        assert_eq!(report.source, None);
        assert!(!report.will_read);
        assert!(!report.password_missing);
    }

    #[test]
    fn a_saved_setting_with_a_password_will_read() {
        let store = FakeStore::with_password("a-password");

        let report = resolve(env_map(&[]), Some(stored_config()), &store);

        assert_eq!(report.source, Some(RouterCredentialSource::AppSettings));
        assert!(report.has_password);
        assert!(report.will_read);
    }

    #[test]
    fn a_saved_setting_without_a_password_is_saved_but_not_reading() {
        let store = FakeStore::default();

        let report = resolve(env_map(&[]), Some(stored_config()), &store);

        assert_eq!(report.source, Some(RouterCredentialSource::AppSettings));
        assert!(!report.has_password);
        assert!(!report.will_read);
        assert!(report.password_missing);
    }

    #[test]
    fn the_environment_overrides_a_saved_setting() {
        let store = FakeStore::with_password("a-password");
        let lookup = env_map(&[
            (USERNAME_VAR, "env-admin"),
            (PASSWORD_VAR, "env-password"),
            (HOST_VAR, "192.168.1.1"),
        ]);

        let report = resolve(lookup, Some(stored_config()), &store);

        assert_eq!(report.source, Some(RouterCredentialSource::Environment));
        // The card still shows what is saved, but marked as overridden.
        assert_eq!(report.host.as_deref(), Some("192.168.100.1"));
        assert!(report.will_read);
    }

    #[test]
    fn the_environment_pair_must_be_complete_to_apply() {
        let store = FakeStore::with_password("a-password");
        let lookup = env_map(&[(USERNAME_VAR, "env-admin"), (HOST_VAR, "192.168.1.1")]);

        let report = resolve(lookup, Some(stored_config()), &store);

        assert_eq!(report.source, Some(RouterCredentialSource::AppSettings));
        assert!(report.will_read);
    }

    #[test]
    fn the_resolution_hands_the_reader_the_saved_credentials() {
        let store = FakeStore::with_password("a-password");

        let resolution = resolve_full(env_map(&[]), Some(stored_config()), &store);

        let credentials = resolution.credentials.expect("a complete save reads");
        assert_eq!(credentials.host, "192.168.100.1");
        assert_eq!(credentials.port, 80);
        assert_eq!(credentials.username, "admin");
        assert_eq!(credentials.password, "a-password");
    }

    #[test]
    fn an_environment_resolution_hands_the_reader_the_environment_credentials() {
        let store = FakeStore::default();
        let lookup = env_map(&[(USERNAME_VAR, "env-admin"), (PASSWORD_VAR, "env-password")]);

        let resolution = resolve_full(lookup, None, &store);

        let credentials = resolution.credentials.expect("environment wins");
        assert_eq!(credentials.username, "env-admin");
        assert_eq!(credentials.password, "env-password");
    }

    #[test]
    fn a_save_writes_the_file_and_arms_the_reader() {
        let dir = save_dir();
        let store = FakeStore::default();
        let state = RouterState::new();

        let report = save_connection(
            Some(&dir),
            &store,
            &state,
            "192.168.100.1",
            80,
            "admin",
            Some("a-password"),
        )
        .expect("a valid save must succeed");

        assert!(report.will_read);
        assert_eq!(
            read_config(&dir),
            Some(stored_config()),
            "the saved file is exactly what was entered"
        );
        assert!(
            state.credentials().is_some(),
            "the reader must pick the new connection up without a restart"
        );
    }

    #[test]
    fn a_save_that_omits_the_password_keeps_the_stored_one() {
        let dir = save_dir();
        let store = FakeStore::with_password("kept");
        let state = RouterState::new();

        save_connection(
            Some(&dir),
            &store,
            &state,
            "192.168.100.1",
            80,
            "admin",
            None,
        )
        .expect("a save without a password must succeed");

        let credentials = state.credentials().expect("the password was kept");
        assert_eq!(credentials.password, "kept");
    }

    #[test]
    fn a_save_is_rejected_when_the_address_or_user_name_is_empty() {
        let store = FakeStore::default();
        let state = RouterState::new();

        let no_host = save_connection(None, &store, &state, "  ", 80, "admin", None);
        let no_user = save_connection(None, &store, &state, "192.168.100.1", 80, "  ", None);

        assert!(no_host.is_err() && no_user.is_err());
        assert!(
            state.credentials().is_none(),
            "a rejected save must not have changed what the reader uses"
        );
    }

    #[test]
    fn a_clear_erases_both_the_file_and_the_reader_credentials() {
        let dir = save_dir();
        let store = FakeStore::with_password("a-password");
        let state = RouterState::new();
        save_connection(
            Some(&dir),
            &store,
            &state,
            "192.168.100.1",
            80,
            "admin",
            None,
        )
        .expect("the setup save must succeed");

        let report = clear_connection(Some(&dir), &store, &state).expect("a clear must succeed");

        assert_eq!(report.source, None);
        assert!(read_config(&dir).is_none());
        assert!(state.credentials().is_none());
    }

    #[test]
    fn a_current_report_reflects_the_stored_setting_without_applying_anything() {
        let dir = save_dir();
        let store = FakeStore::with_password("a-password");
        write_config(&dir, &stored_config()).expect("the setup write must succeed");

        let report = current_report(Some(&dir), &store);

        assert_eq!(report.source, Some(RouterCredentialSource::AppSettings));
        assert!(report.will_read);
    }

    #[test]
    fn a_corrupt_saved_file_reads_as_absent() {
        let dir = save_dir();
        std::fs::create_dir_all(&dir).expect("the temp dir must exist");
        std::fs::write(dir.join(CONFIG_FILE_NAME), "{ not json").expect("written");

        assert_eq!(read_config(&dir), None);
    }
}
