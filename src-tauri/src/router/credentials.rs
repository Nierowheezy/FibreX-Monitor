//! Where the router credentials come from.
//!
//! They come from the process environment and nowhere else. There is deliberately
//! no compiled-in default, no settings-screen field that writes a password to
//! disk, and no fallback to a guessed pair: the app must never carry a credential
//! in its source, and a build with none configured must read the router not at
//! all rather than try a plausible default.
//!
//! The variable names are documented in `.env.example`. `.env` itself is
//! git-ignored, and the app only reads the values at startup.

use super::RouterCredentials;

/// The environment variable holding the router address.
pub const HOST_VAR: &str = "FIBREX_ROUTER_HOST";

/// The environment variable holding the router port.
pub const PORT_VAR: &str = "FIBREX_ROUTER_PORT";

/// The environment variable holding the sign-in user name.
pub const USERNAME_VAR: &str = "FIBREX_ROUTER_USERNAME";

/// The environment variable holding the sign-in password.
pub const PASSWORD_VAR: &str = "FIBREX_ROUTER_PASSWORD";

/// The port the observed firmware serves its UI on over HTTPS.
///
/// Port 443 is closed on the live device and the real UI answers on 80, which is
/// why the default is not the conventional HTTPS port.
pub const DEFAULT_PORT: u16 = 80;

/// The address the observed router uses.
pub const DEFAULT_HOST: &str = "192.168.100.1";

/// Reads whatever the environment provides, or `None` when the router is not
/// configured.
///
/// `None` rather than a partial `Some`: half a credential pair is not a
/// configuration, and returning it would produce an authentication failure that
/// reads like a wrong password instead of an unset variable. A port that does not
/// parse falls back to the default rather than failing the whole configuration,
/// because the address and the credentials are the parts that matter.
pub fn from_environment<F>(lookup: F) -> Option<RouterCredentials>
where
    F: Fn(&str) -> Option<String>,
{
    let read = |name: &str| -> Option<String> {
        lookup(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };

    let username = read(USERNAME_VAR)?;
    let password = read(PASSWORD_VAR)?;

    let host = read(HOST_VAR).unwrap_or_else(|| DEFAULT_HOST.to_string());
    let port = read(PORT_VAR)
        .and_then(|value| value.parse::<u16>().ok())
        // A port of zero is not a port. Falling back keeps a typo in an optional
        // variable from disabling the whole integration.
        .filter(|port| *port != 0)
        .unwrap_or(DEFAULT_PORT);

    Some(RouterCredentials {
        host,
        port,
        username,
        password,
    })
}

/// Reads the real process environment.
pub fn from_process_environment() -> Option<RouterCredentials> {
    from_environment(|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    #[test]
    fn a_complete_environment_produces_credentials() {
        let credentials = from_environment(env(&[
            (USERNAME_VAR, "root"),
            (PASSWORD_VAR, "a-password"),
            (HOST_VAR, "192.168.1.1"),
            (PORT_VAR, "8443"),
        ]))
        .expect("complete credentials must be read");

        assert_eq!(credentials.host, "192.168.1.1");
        assert_eq!(credentials.port, 8443);
        assert_eq!(credentials.username, "root");
        assert_eq!(credentials.password, "a-password");
    }

    #[test]
    fn the_address_falls_back_to_the_observed_default() {
        let credentials =
            from_environment(env(&[(USERNAME_VAR, "root"), (PASSWORD_VAR, "a-password")]))
                .expect("credentials must be read without an explicit host");

        assert_eq!(credentials.host, DEFAULT_HOST);
        assert_eq!(credentials.port, DEFAULT_PORT);
    }

    #[test]
    fn a_username_without_a_password_is_not_a_configuration() {
        assert!(
            from_environment(env(&[(USERNAME_VAR, "root")])).is_none(),
            "half a credential pair must not produce a session that then fails as a wrong password"
        );
    }

    #[test]
    fn a_password_without_a_username_is_not_a_configuration() {
        assert!(from_environment(env(&[(PASSWORD_VAR, "a-password")])).is_none());
    }

    #[test]
    fn an_empty_environment_configures_nothing() {
        assert!(from_environment(env(&[])).is_none());
    }

    #[test]
    fn a_blank_value_counts_as_unset() {
        assert!(
            from_environment(env(&[(USERNAME_VAR, "root"), (PASSWORD_VAR, "   "),])).is_none(),
            "whitespace is not a password"
        );
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_from_a_real_value() {
        let credentials = from_environment(env(&[
            (USERNAME_VAR, "  root  "),
            (PASSWORD_VAR, " a-password "),
        ]))
        .expect("padded values must still configure the router");

        assert_eq!(credentials.username, "root");
        assert_eq!(credentials.password, "a-password");
    }

    #[test]
    fn an_unparseable_port_falls_back_rather_than_disabling_the_router() {
        let credentials = from_environment(env(&[
            (USERNAME_VAR, "root"),
            (PASSWORD_VAR, "a-password"),
            (PORT_VAR, "not-a-port"),
        ]))
        .expect("a bad optional port must not disable the integration");

        assert_eq!(credentials.port, DEFAULT_PORT);
    }

    #[test]
    fn a_zero_port_falls_back() {
        let credentials = from_environment(env(&[
            (USERNAME_VAR, "root"),
            (PASSWORD_VAR, "a-password"),
            (PORT_VAR, "0"),
        ]))
        .expect("port zero is not a usable port");

        assert_eq!(credentials.port, DEFAULT_PORT);
    }

    #[test]
    fn a_missing_host_but_present_port_keeps_the_port() {
        let credentials = from_environment(env(&[
            (USERNAME_VAR, "root"),
            (PASSWORD_VAR, "a-password"),
            (PORT_VAR, "8443"),
        ]))
        .expect("credentials must be read");

        assert_eq!(credentials.host, DEFAULT_HOST);
        assert_eq!(credentials.port, 8443);
    }
}
