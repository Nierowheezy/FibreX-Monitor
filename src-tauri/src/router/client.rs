//! The read-only HTTP client for the router.
//!
//! Three facts from the live device shape every line here, and each one cost a
//! debugging session to find:
//!
//! 1. The UI is served over **HTTPS on port 80** with a self-signed certificate.
//!    A plain HTTP request returns a stub that redirects, and port 443 is closed.
//! 2. Login and authenticated reads use **two different token endpoints**. Using
//!    the login token for a data request returns `403` on every page.
//! 3. Tokens are single-use and arrive with a UTF-8 BOM that must be stripped.
//!
//! The client is blocking, and is meant to be driven from a dedicated thread. It
//! performs reads only: no `set.cgi`, no configuration writes, no session reuse
//! across a credential change.

use std::time::Duration;

use ureq::Agent;

use super::parse::{self, wan_address_label};
use super::{RouterCredentials, RouterError, RouterErrorKind, RouterSnapshot, RouterSourceStatus};
use crate::models::overview::{RouterHealth, RouterNetwork, WanConnection, WanCounters};
use crate::router::router_reading;

/// How long one request may take. Comfortably longer than a healthy local
/// round trip, because the router is a small embedded device that can be slow
/// while it is busy, and short enough that a wrong address does not hang the
/// reader.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(6);

/// The page carrying device, optical and system figures.
pub const DEVICE_INFO_PATH: &str = "/html/ssmp/deviceinfo/deviceinfo.asp";

/// The page carrying the WAN connection object.
pub const WAN_CACHE_PATH: &str = "/html/bbsp/common/wan_list_cache_wan.asp";

/// The page carrying the router's traffic counters.
///
/// A different page from the connection one, and a different object. The
/// connection page carries metadata; this carries the byte and packet totals.
/// The IP variant of the same call returns an empty array on this firmware, so
/// the PPP counters are the usable source.
pub const WAN_STATS_PATH: &str = "/html/bbsp/common/get_wan_list_pppwanstat.asp";

/// The page carrying the router's Wi-Fi radios.
pub const WLAN_INFO_PATH: &str = "/html/bbsp/common/get_wan_list_wlaninfo.asp";

/// The page carrying the WAN interface's access type.
pub const WAN_ACCESS_TYPE_PATH: &str = "/html/bbsp/common/get_wan_list_wanaccesstype.asp";

/// The page carrying the router's IPv6 state.
///
/// Read with a token in the query string rather than in the body: this page
/// answers a plain request with a "waiting" stub, and the router's own
/// JavaScript fetches it the same way.
pub const IPV6_STATE_PATH: &str = "/html/bbsp/common/wan_list_cache_wanipv6.asp";

/// The page carrying the router's dynamic WAN settings, among them whether it
/// says it supports IPv6.
pub const WAN_DYNAMIC_PATH: &str = "/html/bbsp/common/getWanDynamicData.asp";

/// The token endpoint used for the sign-in request.
const PRE_LOGIN_TOKEN_PATH: &str = "/html/ssmp/common/getRandString.asp";

/// The token endpoint used for every request **after** sign-in.
///
/// This is the detail that matters most. The two paths are not interchangeable,
/// and mixing them up produces a `403` from every authenticated page.
const LOGIN_TOKEN_PATH: &str = "/html/ssmp/common/GetRandToken.asp";

/// The sign-in endpoint.
const LOGIN_PATH: &str = "/login.cgi";

/// Wraps whichever error the HTTP layer produced, without ever including the
/// request body or a header.
///
/// The rule this enforces: a failure message may describe the failure and may
/// name the host, and must never carry a credential, a token or a response body.
fn describe(error: &ureq::Error, host: &str) -> RouterError {
    match error {
        ureq::Error::StatusCode(code) => {
            // 401 and 403 are the router refusing the session, which is a
            // credentials problem and not a network one.
            if *code == 401 || *code == 403 {
                RouterError::new(
                    RouterErrorKind::AuthRejected,
                    format!("The router at {host} refused the sign-in."),
                )
            } else {
                RouterError::new(
                    RouterErrorKind::UnexpectedResponse,
                    format!("The router at {host} answered with status {code}."),
                )
            }
        }
        ureq::Error::Timeout(_) => RouterError::new(
            RouterErrorKind::Timeout,
            format!("The router at {host} did not answer in time."),
        ),
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed => RouterError::new(
            RouterErrorKind::Unreachable,
            format!("Could not reach the router at {host}."),
        ),
        _ => RouterError::new(
            RouterErrorKind::Unreachable,
            format!("Could not read from the router at {host}."),
        ),
    }
}

/// The agent, built once.
///
/// The certificate is not verified because the router serves a self-signed one
/// on an address that no public root store covers. Accepting it is scoped to
/// this agent, which only ever talks to the configured router address; the app's
/// other network use goes through Tauri and is unaffected.
fn agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .disable_verification(true)
                .build(),
        )
        .build()
        .into()
}

/// A signed-in conversation with the router.
///
/// Constructed per read rather than held open across reads. Within one read the
/// session cookie is carried, because the router requires it: `/login.cgi` sets
/// `CookieHttps` and every authenticated page returns a 602-byte stub without
/// it. The cookie is held in memory for the length of one read and then dropped,
/// so no session survives a password change or a restart.
pub struct RouterSession {
    credentials: RouterCredentials,
    agent: Agent,
    /// The cookie set by `/login.cgi`, or `None` before sign-in.
    ///
    /// `RefCell` rather than `&mut self` because `read_snapshot` takes `&self`:
    /// the cookie is session state, not part of the read's identity, and a
    /// signature that demanded a mutable borrow would force every caller to hold
    /// a `&mut` for the whole read just to store one header.
    cookie: std::cell::RefCell<Option<String>>,
}

impl RouterSession {
    pub fn new(credentials: RouterCredentials) -> Self {
        Self {
            credentials,
            agent: agent(),
            cookie: std::cell::RefCell::new(None),
        }
    }

    /// The router's address, for error messages.
    fn host(&self) -> &str {
        &self.credentials.host
    }

    /// The stored cookie, for the requests that must carry it.
    ///
    /// An empty string rather than `None` when there is no cookie, so a caller
    /// can pass it to a header builder without a second branch.
    fn cookie(&self) -> String {
        self.cookie.borrow().clone().unwrap_or_default()
    }

    /// Reads the cookie out of a response, if the router set one.
    ///
    /// Only the name and value are kept: the attributes on the header
    /// (`Path`, `Secure`, `HttpOnly`) describe browser behaviour, and this client
    /// always sends the cookie with the same origin anyway.
    fn capture_cookie(&self, headers: &ureq::http::HeaderMap) {
        let Some(header) = headers.get("set-cookie") else {
            return;
        };
        let Ok(raw) = header.to_str() else {
            return;
        };
        let pair = raw.split(';').next().unwrap_or("").trim();
        if pair.is_empty() {
            return;
        }
        *self.cookie.borrow_mut() = Some(pair.to_string());
    }

    /// One token from the given endpoint, BOM stripped.
    ///
    /// The BOM is the second thing that cost a debugging session: the firmware
    /// prefixes the token with `\ufeff`, and sending that back produces an
    /// authentication failure that looks like a wrong password.
    fn token(&self, path: &str) -> Result<String, RouterError> {
        let url = format!("{}{path}", self.credentials.base_url());
        let response = self
            .agent
            .get(&url)
            .header("Cookie", self.cookie())
            .call()
            .map_err(|error| describe(&error, self.host()))?;

        // A token endpoint may rotate the session cookie, so it is read back
        // before the body is consumed.
        self.capture_cookie(response.headers());

        let body = response.into_body().read_to_string().map_err(|_| {
            RouterError::new(
                RouterErrorKind::UnexpectedResponse,
                format!(
                    "The router at {} sent a token that could not be read.",
                    self.host()
                ),
            )
        })?;

        let cleaned = body.replace('\u{feff}', "");
        let trimmed = cleaned.trim().to_string();

        if trimmed.is_empty() {
            return Err(RouterError::new(
                RouterErrorKind::UnexpectedResponse,
                format!("The router at {} sent an empty token.", self.host()),
            ));
        }

        Ok(trimmed)
    }

    /// Signs in, then confirms the sign-in by reading a page that requires it.
    ///
    /// The confirmation is the point. The login endpoint answers with a page
    /// either way, so a status code alone cannot distinguish success from a
    /// refused password. Reading an authenticated page is what proves the
    /// session, and it is also the first real read, so it is not wasted work.
    pub fn sign_in(&self) -> Result<(), RouterError> {
        let token = self.token(PRE_LOGIN_TOKEN_PATH)?;
        let url = format!("{}{LOGIN_PATH}", self.credentials.base_url());

        // The password is base64 encoded and not hashed, as the live login page
        // does. It is encoded here, used once, and never logged.
        let encoded = base64_encode(self.credentials.password.as_bytes());

        let response = self
            .agent
            .post(&url)
            .send_form([
                ("UserName", self.credentials.username.as_str()),
                ("PassWord", encoded.as_str()),
                ("Language", "english"),
                ("x.X_HW_Token", token.as_str()),
            ])
            .map_err(|error| describe(&error, self.host()))?;

        // The session cookie, read before the body is consumed. This is the
        // third thing that cost a debugging session: the router does set a
        // cookie on a successful sign-in, and every authenticated page returns a
        // 602-byte stub without it. The token alone is not enough.
        self.capture_cookie(response.headers());

        let body = response.into_body().read_to_string().map_err(|_| {
            RouterError::new(
                RouterErrorKind::UnexpectedResponse,
                format!(
                    "The router at {} sent a sign-in page that could not be read.",
                    self.host()
                ),
            )
        })?;

        // The observed success page redirects to `/`. A refused sign-in returns
        // the login page with an error string instead.
        if body.contains("top.location.replace") {
            return Ok(());
        }

        Err(RouterError::new(
            RouterErrorKind::AuthRejected,
            format!(
                "The router at {} did not accept the configured username and password.",
                self.host()
            ),
        ))
    }

    /// One authenticated POST with a fresh token, returning the page body.
    fn post_page(&self, path: &str) -> Result<String, RouterError> {
        let token = self.token(LOGIN_TOKEN_PATH)?;
        let url = format!("{}{path}", self.credentials.base_url());

        let response = self
            .agent
            .post(&url)
            .header("Cookie", self.cookie())
            .send_form([("x.X_HW_Token", token.as_str())])
            .map_err(|error| describe(&error, self.host()))?;

        response.into_body().read_to_string().map_err(|_| {
            RouterError::new(
                RouterErrorKind::UnexpectedResponse,
                format!(
                    "The router at {} sent a page that could not be read.",
                    self.host()
                ),
            )
        })
    }

    /// One authenticated GET with a fresh token in the query string.
    ///
    /// Some of the router's pages answer this way and not the other. A plain
    /// GET without a token, or a POST with the token in the body, gets back a
    /// "waiting" stub that looks like a page but carries nothing.
    fn get_page(&self, path: &str) -> Result<String, RouterError> {
        let token = self.token(LOGIN_TOKEN_PATH)?;
        let separator = if path.contains('?') { '&' } else { '?' };
        let url = format!(
            "{}{path}{separator}x.X_HW_Token={}",
            self.credentials.base_url(),
            token
        );

        let response = self
            .agent
            .get(&url)
            .header("Cookie", self.cookie())
            .call()
            .map_err(|error| describe(&error, self.host()))?;

        response.into_body().read_to_string().map_err(|_| {
            RouterError::new(
                RouterErrorKind::UnexpectedResponse,
                format!(
                    "The router at {} sent a page that could not be read.",
                    self.host()
                ),
            )
        })
    }

    /// Signs in and reads everything this app understands.
    ///
    /// Each page is read independently, so one failing endpoint leaves the other
    /// half usable and both outcomes are reported. A read that produced nothing
    /// from either page is returned as an error rather than as an empty success,
    /// because those are different facts.
    pub fn read_snapshot(&self, read_at: i64) -> Result<RouterSnapshot, RouterError> {
        self.sign_in()?;

        let mut sources = Vec::new();
        let device_page = self.collect(DEVICE_INFO_PATH, "Device information", &mut sources);
        let wan_page = self.collect(WAN_CACHE_PATH, "WAN connection", &mut sources);
        let stats_page = self.collect(WAN_STATS_PATH, "WAN traffic counters", &mut sources);
        let wlan_page = self.collect(WLAN_INFO_PATH, "Wi-Fi radios", &mut sources);
        let access_page = self.collect(WAN_ACCESS_TYPE_PATH, "WAN access type", &mut sources);
        let ipv6_page = self.collect_get(IPV6_STATE_PATH, "IPv6 state", &mut sources);
        let dynamic_page = self.collect_get(WAN_DYNAMIC_PATH, "WAN settings", &mut sources);

        if device_page.is_none()
            && wan_page.is_none()
            && stats_page.is_none()
            && wlan_page.is_none()
            && access_page.is_none()
            && ipv6_page.is_none()
            && dynamic_page.is_none()
        {
            // Both pages failed. The first failure is the most informative.
            let error = sources
                .iter()
                .find_map(|source| source.error.clone())
                .unwrap_or_else(|| {
                    RouterError::new(
                        RouterErrorKind::UnexpectedResponse,
                        "The router answered but reported nothing this app can read.".to_string(),
                    )
                });
            return Err(error);
        }

        let wan = wan_page
            .as_deref()
            .map(parse_wan_connection)
            .unwrap_or_else(WanConnection::unavailable);
        let health = device_page
            .as_deref()
            .map(|page| parse_router_health(page, read_at))
            .unwrap_or_else(RouterHealth::unavailable);

        // The counters are only carried when every one of them parsed. A
        // half-read set of totals cannot produce a rate, and a rate derived
        // from one counter and a missing one would be a number about nothing.
        let counters = stats_page.as_deref().and_then(|page| {
            let fields = parse::parse_wan_stats(page);
            match (
                fields.bytes_sent,
                fields.bytes_received,
                fields.packets_sent,
                fields.packets_received,
            ) {
                (
                    Some(bytes_sent),
                    Some(bytes_received),
                    Some(packets_sent),
                    Some(packets_received),
                ) => Some(WanCounters {
                    bytes_sent,
                    bytes_received,
                    packets_sent,
                    packets_received,
                    measured_at: read_at,
                }),
                _ => None,
            }
        });

        // The router's own view of the network around it. Each part is read
        // from the page that carries it, and a page that did not answer simply
        // leaves its part empty.
        let (ipv6_status, ipv6_prefixes) = ipv6_page
            .as_deref()
            .map(parse::parse_ipv6)
            .unwrap_or((None, Vec::new()));

        let network = RouterNetwork {
            wifi_radios: wlan_page
                .as_deref()
                .map(parse::parse_wifi_radios)
                .unwrap_or_default(),
            wan_access_type: access_page
                .as_deref()
                .and_then(parse::parse_wan_access_type),
            ipv6_status,
            supports_ipv6: dynamic_page.as_deref().and_then(parse::parse_supports_ipv6),
            ipv6_prefixes,
        };

        Ok(RouterSnapshot {
            read_at,
            auth: (&self.credentials).into(),
            wan,
            health,
            counters,
            network,
            sources,
        })
    }

    /// Reads one page, recording its outcome either way.
    fn collect(
        &self,
        path: &str,
        name: &str,
        sources: &mut Vec<RouterSourceStatus>,
    ) -> Option<String> {
        match self.post_page(path) {
            Ok(body) => {
                sources.push(RouterSourceStatus::ok(name, path));
                Some(body)
            }
            Err(error) => {
                sources.push(RouterSourceStatus::failed(name, path, error));
                None
            }
        }
    }

    /// Reads one page that the router answers with a token in the query string.
    fn collect_get(
        &self,
        path: &str,
        name: &str,
        sources: &mut Vec<RouterSourceStatus>,
    ) -> Option<String> {
        match self.get_page(path) {
            Ok(body) => {
                sources.push(RouterSourceStatus::ok(name, path));
                Some(body)
            }
            Err(error) => {
                sources.push(RouterSourceStatus::failed(name, path, error));
                None
            }
        }
    }
}

/// Top-level read used by the background task.
pub fn read_router(
    credentials: &RouterCredentials,
    read_at: i64,
) -> Result<RouterSnapshot, RouterError> {
    if credentials.password.is_empty() || credentials.username.is_empty() {
        return Err(RouterError::new(
            RouterErrorKind::NoCredentials,
            "No router credentials are configured.".to_string(),
        ));
    }

    RouterSession::new(credentials.clone()).read_snapshot(read_at)
}

/// The WAN connection fields out of the WAN cache page.
pub fn parse_wan_connection(page: &str) -> WanConnection {
    let fields = parse::parse_wan_ppp(page);

    let address_kind = fields
        .ip_address
        .as_deref()
        .map(parse::classify_wan_address)
        .map(wan_address_label)
        .map(str::to_string);

    WanConnection {
        status: fields.connection_status,
        ip_address: fields.ip_address,
        address_kind,
        gateway: fields.gateway,
        dns_servers: fields.dns,
        mac: fields.mac,
        mtu: fields.mtu,
        bras: fields.bras,
        access_type: fields.access_type,
        service_list: fields.service_list,
        uptime_seconds: fields.uptime_seconds,
    }
}

/// The router's own health figures out of the device information page.
///
/// The reported flags are read as well as the values, because they let the
/// screen state whether a figure is missing because this router does not report
/// it or because the read failed. Those are different sentences.
pub fn parse_router_health(page: &str, read_at: i64) -> RouterHealth {
    let optical = parse::parse_optical_rx_dbm(page);
    let reports_utilisation = parse::flag_enabled(page, "showCPUnMemUsed");

    // CPU and memory are only read where the firmware says the section is on.
    // Where the flag is false the fields can hold stale or placeholder text, and
    // reading them anyway would be showing a figure the router does not stand
    // behind.
    let utilisation_enabled = reports_utilisation.unwrap_or(false);
    let cpu = utilisation_enabled
        .then(|| parse::percent_var(page, "cpuUsed"))
        .flatten();
    let memory = utilisation_enabled
        .then(|| parse::percent_var(page, "memUsed"))
        .flatten();

    RouterHealth {
        optical_rx_dbm: router_reading(optical, read_at),
        cpu_percent: router_reading(cpu, read_at),
        memory_percent: router_reading(memory, read_at),
        pon_mode: parse::string_var(page, "ontPonMode"),
        // The optical flag is reported as false when the monitor section is off,
        // which is how this firmware says it has no optical figures to give.
        reports_optical: parse::flag_enabled(page, "showMonitorData").map(|enabled| !enabled),
        reports_utilisation,
    }
}

/// Base64, standard alphabet with padding.
///
/// Hand-written rather than pulled in as a dependency, because this is the only
/// place the app needs it and the encoding is fixed: the router expects the
/// standard alphabet with padding, and nothing here is a URL-safe variant.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (first << 16) | (second << 8) | third;

        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::overview::ValueState;

    const DEVICEINFO: &str = r#"
      var showMonitorData = '0';
      var showCPUnMemUsed = '1';
      var opticInfo = '\x2d18\x2e79';
      var cpuUsed = '30%';
      var memUsed = '67%';
      var flashUsed = '%';
      var temp1 = 0;
      var temp2 = 0;
      var ontPonMode = 'gpon';
    "#;

    const WAN_CACHE: &str = r#"function() {
  obj.PPPWanList = new Array(new WanPPP("a","0","0","AlwaysOn","34\x3a66\x3a79\x3aDD\x3a14\x3aD3","Connected","ERROR\x5fNONE","","NCE\x5fHOME\x5fUPublic\x5fIP\x5f457","1","","","Connected","IP\x5fRouted","10\x2e54\x2e23\x2e105","10\x2e54\x2e0\x2e1","1","0","102\x2e88\x2e158\x2e7\x2c197\x2e210\x2e211\x2e1","FN718140","redacted","AlwaysOn","4294967295","457","4294967295","0","0","INTERNET","","0","180","1","1","0","0","Specified","0","1492","T0180\x5fIKY\x5fBRAS\x5fNE40\x2dX8A\x5f1","DetectBidirectionally","7818","0","","18218","0","","","","","0","0","0","0","0","","0","0",""),null);
}"#;

    #[test]
    fn base64_matches_the_standard_encoding() {
        // Known vectors, including the two padding lengths.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_password_is_encoded_in_a_form_the_router_accepts() {
        // The live firmware expects the standard alphabet with padding, not a
        // URL-safe variant.
        //
        // The value here is a stand-in with the same shape as a real credential,
        // not the credential itself: a password must never appear in tracked
        // source, and a test is source like any other.
        assert_eq!(base64_encode(b"Example1"), "RXhhbXBsZTE=");
    }

    // ---------------------------------------------------------------------
    // WAN connection from the page.
    // ---------------------------------------------------------------------

    #[test]
    fn the_live_wan_page_produces_the_observed_connection() {
        let wan = parse_wan_connection(WAN_CACHE);

        assert_eq!(wan.status.as_deref(), Some("Connected"));
        assert_eq!(wan.ip_address.as_deref(), Some("10.54.23.105"));
        assert_eq!(
            wan.address_kind.as_deref(),
            Some("private"),
            "the observed address is RFC1918 private space, so it must not be called public"
        );
        assert_eq!(wan.gateway.as_deref(), Some("10.54.0.1"));
        assert_eq!(wan.mtu, Some(1492));
        assert_eq!(wan.bras.as_deref(), Some("T0180_IKY_BRAS_NE40-X8A_1"));
        assert_eq!(wan.dns_servers.len(), 2);
    }

    #[test]
    fn a_wan_page_that_reports_nothing_yields_an_empty_connection() {
        let wan = parse_wan_connection("<html>no data</html>");
        assert_eq!(
            wan.status, None,
            "a missing status is not a disconnected one"
        );
        assert_eq!(wan.ip_address, None);
        assert_eq!(wan.address_kind, None);
        assert_eq!(wan.gateway, None);
        assert!(wan.dns_servers.is_empty());
        assert_eq!(wan.mtu, None);
    }

    // ---------------------------------------------------------------------
    // Router health from the page.
    // ---------------------------------------------------------------------

    #[test]
    fn the_live_device_page_produces_the_observed_health_values() {
        let health = parse_router_health(DEVICEINFO, 1_700_000_000_000);

        assert_eq!(health.optical_rx_dbm.value, Some(-18.79));
        assert_eq!(health.optical_rx_dbm.state, ValueState::Measured);
        assert_eq!(health.cpu_percent.value, Some(30.0));
        assert_eq!(health.memory_percent.value, Some(67.0));
        assert_eq!(health.pon_mode.as_deref(), Some("gpon"));
        assert_eq!(health.reports_utilisation, Some(true));
        // The monitor section is off on this profile, which is how the firmware
        // says it has no optical figures to give beyond the receive power.
        assert_eq!(health.reports_optical, Some(true));
    }

    #[test]
    fn utilisation_is_not_read_when_the_firmware_switches_it_off() {
        let page = r#"
          var showCPUnMemUsed = '0';
          var cpuUsed = '30%';
          var memUsed = '67%';
        "#;

        let health = parse_router_health(page, 1);

        assert_eq!(
            health.cpu_percent.value, None,
            "a switched-off section must not be shown as a measurement"
        );
        assert_eq!(health.cpu_percent.state, ValueState::Unavailable);
        assert_eq!(health.memory_percent.value, None);
        assert_eq!(health.reports_utilisation, Some(false));
    }

    #[test]
    fn an_absent_temperature_never_becomes_a_reading() {
        // The live firmware reports temp1 and temp2 as bare zero. They are not
        // carried on the wire at all, which is the strongest form of "not
        // reported": there is no field a caller could render as 0 degrees.
        let health = parse_router_health(DEVICEINFO, 1);
        let json = serde_json::to_value(&health).expect("health must serialise");

        assert!(
            json.get("temperature").is_none(),
            "no temperature field may exist while no sensor reports one"
        );
        assert!(json.get("temp1").is_none());
    }

    #[test]
    fn a_health_read_with_no_page_marks_every_figure_unavailable() {
        let health = parse_router_health("<html></html>", 1);

        assert_eq!(health.optical_rx_dbm.value, None);
        assert_eq!(health.optical_rx_dbm.state, ValueState::Unavailable);
        assert_eq!(health.cpu_percent.value, None);
        assert_eq!(health.memory_percent.value, None);
        assert_eq!(health.pon_mode, None);
        assert_eq!(health.reports_utilisation, None);
    }

    #[test]
    fn an_unread_health_reading_carries_no_timestamp() {
        let health = RouterHealth::unavailable();

        for metric in [
            &health.optical_rx_dbm,
            &health.cpu_percent,
            &health.memory_percent,
        ] {
            assert_eq!(metric.value, None);
            assert_eq!(metric.state, ValueState::Unavailable);
            assert_eq!(
                metric.measured_at, None,
                "an unavailable figure has no measurement time to report"
            );
        }
    }

    /// The rule that protects the whole feature: a credential must never appear
    /// in a serialised snapshot.
    #[test]
    fn a_snapshot_never_serialises_the_password() {
        let credentials = RouterCredentials {
            host: "192.168.100.1".to_string(),
            port: 80,
            username: "root".to_string(),
            password: "a-secret-that-must-not-appear".to_string(),
        };

        let summary: super::super::RouterAuthSummary = (&credentials).into();
        let json = serde_json::to_string(&summary).expect("summary must serialise");

        assert!(!json.contains("a-secret-that-must-not-appear"));
        assert!(json.contains("\"hasPassword\":true"));
    }

    // ---------------------------------------------------------------------
    // The session cookie.
    // ---------------------------------------------------------------------

    /// Builds a header map holding one `set-cookie` value.
    fn headers_with_cookie(value: &str) -> ureq::http::HeaderMap {
        let mut headers = ureq::http::HeaderMap::new();
        headers.insert("set-cookie", value.parse().expect("valid header value"));
        headers
    }

    fn session() -> RouterSession {
        RouterSession::new(RouterCredentials {
            host: "192.168.100.1".to_string(),
            port: 80,
            username: "root".to_string(),
            password: "a-password".to_string(),
        })
    }

    #[test]
    fn the_session_starts_with_no_cookie() {
        assert_eq!(session().cookie(), "");
    }

    #[test]
    fn the_cookie_value_is_kept_and_its_attributes_dropped() {
        let session = session();
        session.capture_cookie(&headers_with_cookie(
            "CookieHttps=abc123; Path=/; Secure; HttpOnly",
        ));

        assert_eq!(
            session.cookie(),
            "CookieHttps=abc123",
            "only the name and value travel back, since this client always sends it to the same origin"
        );
    }

    #[test]
    fn a_cookie_is_rotated_by_a_later_response() {
        let session = session();
        session.capture_cookie(&headers_with_cookie("CookieHttps=first"));
        session.capture_cookie(&headers_with_cookie("CookieHttps=second"));

        assert_eq!(session.cookie(), "CookieHttps=second");
    }

    #[test]
    fn a_response_without_a_cookie_leaves_the_stored_one_alone() {
        let session = session();
        session.capture_cookie(&headers_with_cookie("CookieHttps=kept"));
        session.capture_cookie(&ureq::http::HeaderMap::new());

        assert_eq!(
            session.cookie(),
            "CookieHttps=kept",
            "a page that sets no cookie must not clear the session"
        );
    }

    #[test]
    fn an_empty_set_cookie_header_stores_nothing() {
        let session = session();
        session.capture_cookie(&headers_with_cookie("   "));

        assert_eq!(session.cookie(), "");
    }

    /// A live read of the real router.
    ///
    /// Ignored by default, because it needs the router on the local network and
    /// credentials in the environment. Every other test in this module runs
    /// against captured page text, and only a test that talks to the device can
    /// catch a protocol change. Run it deliberately:
    ///
    /// ```text
    /// cd src-tauri
    /// FIBREX_ROUTER_USERNAME=... FIBREX_ROUTER_PASSWORD=... \
    ///   cargo test a_live_read -- --ignored --nocapture
    /// ```
    ///
    /// It asserts the shape of the read and never writes anything to the router.
    #[test]
    #[ignore = "needs the router on the local network and credentials in the environment"]
    fn a_live_read_returns_the_observed_fields() {
        let credentials = match crate::router::credentials::from_process_environment() {
            Some(credentials) => credentials,
            None => {
                eprintln!("skipped: no router credentials in the environment");
                return;
            }
        };

        eprintln!("reading {}:{}", credentials.host, credentials.port);

        let snapshot = match read_router(&credentials, 0) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("live read failed: {:?} {}", error.kind, error.message),
        };

        for source in &snapshot.sources {
            eprintln!(
                "  {} {} {}",
                if source.ok { "ok    " } else { "FAILED" },
                source.path,
                source
                    .error
                    .as_ref()
                    .map(|error| error.message.as_str())
                    .unwrap_or("")
            );
        }
        eprintln!("wan: {:#?}", snapshot.wan);
        eprintln!("health: {:#?}", snapshot.health);
        eprintln!("network: {:#?}", snapshot.network);

        // The read must not claim a credential it does not have.
        assert!(snapshot.auth.has_password);
        assert_eq!(
            snapshot.auth.username.as_deref(),
            Some(credentials.username.as_str())
        );

        // The WAN page is the one that always answered on the observed firmware.
        assert!(
            snapshot
                .sources
                .iter()
                .any(|source| source.path.contains("wan_list_cache_wan")),
            "the WAN page must be attempted"
        );

        // A reported address must be classified, and an RFC1918 one may never be
        // shown as public.
        if let Some(address) = snapshot.wan.ip_address.as_deref() {
            let kind = snapshot.wan.address_kind.as_deref();
            eprintln!("address {address} classified as {kind:?}");
            assert!(
                kind.is_some(),
                "a reported address must carry a classification"
            );
            if address.starts_with("10.") || address.starts_with("192.168.") {
                assert_eq!(
                    kind,
                    Some("private"),
                    "an RFC1918 address must not be called public"
                );
            }
        }

        // An optical reading is either absent or a plausible negative dBm, never
        // a stand-in zero.
        match snapshot.health.optical_rx_dbm.value {
            Some(dbm) => {
                assert!(
                    (-40.0..=10.0).contains(&dbm),
                    "an optical reading outside the physical window is a parsing accident: {dbm}"
                );
                assert_ne!(dbm, 0.0, "zero is the firmware's absent value");
                assert_eq!(snapshot.health.optical_rx_dbm.state, ValueState::Measured);
            }
            None => {
                assert_eq!(
                    snapshot.health.optical_rx_dbm.state,
                    ValueState::Unavailable
                );
            }
        }
    }
}
