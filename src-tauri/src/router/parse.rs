//! Reading values out of the router's own pages.
//!
//! The router does not offer a JSON API. Each page is HTML with a handful of
//! inline `var name = value;` assignments, and the values are the real data. This
//! module pulls those assignments out.
//!
//! Two habits matter throughout:
//!
//! - A field the firmware leaves at `0` or an empty string is treated as **not
//!   reported**, not as a reading of zero. The observed firmware sets `temp1` and
//!   `temp2` to `0` while the optical page exposes no temperature at all, so
//!   `0` here means "no sensor", and showing it as 0 degrees would be a
//!   fabrication.
//! - A value is only returned when it parses as the type it claims to be. A
//!   malformed figure becomes `None`, which the caller renders as unavailable.

/// The `var NAME = VALUE;` shape, scoped to one name.
///
/// Written as a scan rather than a regex because the values contain quotes,
/// escapes and semicolons of their own, and a regex over HTML attributes is a
/// well known way to get this subtly wrong.
fn assignment<'a>(page: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("var {name}");
    let mut search_from = 0;

    while let Some(offset) = page[search_from..].find(&needle) {
        let start = search_from + offset;
        search_from = start + needle.len();

        // The match must be the whole identifier, so `var temp1` is not found
        // while looking for `var temp` and `var monitorDataArray` is not found
        // while looking for `var monitorData`.
        let after = &page[start + needle.len()..];
        let Some(rest) = after.trim_start().strip_prefix('=') else {
            // `var temp1;` or another declaration with no value. Keep scanning:
            // an assignment to the same name may follow.
            continue;
        };

        return Some(rest.trim_start());
    }

    None
}

/// A `var NAME = VALUE;` assignment, up to the terminating semicolon.
fn raw_value<'a>(page: &'a str, name: &str) -> Option<&'a str> {
    let rest = assignment(page, name)?;

    // Scanning to the first semicolon is correct for these pages: the firmware
    // does not put semicolons inside a scalar value. A quoted string is handled
    // below, so a semicolon inside quotes never reaches here.
    let end = rest.find(';')?;
    Some(rest[..end].trim())
}

/// A single-quoted or double-quoted string literal, with the firmware's hex
/// escapes resolved.
///
/// The pages escape non-alphanumeric characters as `\xNN`, so an address is
/// written `10\x2e54\x2e23\x2e105`. Leaving that encoded would put a literal
/// backslash-x sequence in the interface.
pub fn string_var(page: &str, name: &str) -> Option<String> {
    let raw = raw_value(page, name)?;
    let unquoted = raw
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            raw.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })?;

    Some(decode_hex_escapes(unquoted))
}

/// Resolves `\xNN` escapes and common backslash escapes.
///
/// Unknown escapes are dropped rather than passed through, so a stray sequence
/// cannot smuggle a control character into a rendered value.
pub fn decode_hex_escapes(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }

        match chars.next() {
            Some('x') => {
                let digits: String = chars.clone().take(2).collect();
                if digits.len() == 2
                    && let Ok(code) = u8::from_str_radix(&digits, 16)
                {
                    // Non-ASCII byte escapes are not produced by these pages,
                    // so a high byte is treated as a plain code point rather
                    // than being guessed at as UTF-8.
                    out.push(code as char);
                    chars.next();
                    chars.next();
                    continue;
                }
                // Not a valid escape: keep the text rather than eating it, so a
                // malformed sequence is visible instead of silently vanishing.
                out.push_str("\\x");
                out.push_str(&digits);
                for _ in 0..digits.chars().count() {
                    chars.next();
                }
            }
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('"') => out.push('"'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }

    out
}

/// A numeric assignment such as `var opticInfo = '\x2d18\x2e79';`.
///
/// The firmware quotes numbers, sometimes escapes the sign and decimal point,
/// and sometimes leaves them bare. All three forms are handled, and anything
/// that does not parse becomes `None`.
pub fn number_var(page: &str, name: &str) -> Option<f64> {
    let raw = raw_value(page, name)?;
    let text = raw
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            raw.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .map(decode_hex_escapes)
        .unwrap_or_else(|| raw.to_string());

    let trimmed = text.trim();

    // An empty string is the firmware's way of saying there is no value, which
    // is not a reading of zero.
    if trimmed.is_empty() {
        return None;
    }

    trimmed.parse::<f64>().ok()
}

/// A percentage such as `var cpuUsed = '30%';`, returned without the sign.
///
/// A figure that is present but not a number, or that is present with no digits
/// at all as `flashUsed` is on this firmware, is not a reading.
pub fn percent_var(page: &str, name: &str) -> Option<f64> {
    let raw = raw_value(page, name)?;
    let text = raw
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            raw.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .map(decode_hex_escapes)
        .unwrap_or_else(|| raw.to_string());

    let digits: String = text
        .trim()
        .trim_end_matches('%')
        .trim()
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == '.')
        .collect();

    if digits.is_empty() {
        return None;
    }

    let value = digits.parse::<f64>().ok()?;
    // The firmware reports an unused percentage as the empty string, which is
    // caught above. A literal 0 percent is a real reading where the field is
    // known to be enabled, so it is not filtered here; the caller decides
    // whether the field is enabled at all.
    Some(value)
}

/// The fields read from one `WanPPP(...)` constructor call.
///
/// The WAN page returns JavaScript that constructs a `WanPPP` object with
/// positional arguments. Reading by position is brittle, so the arguments are
/// parsed once into named fields and everything downstream uses the names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WanPppFields {
    pub connection_status: Option<String>,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub mac: Option<String>,
    pub mtu: Option<u32>,
    pub bras: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub service_list: Option<String>,
    pub access_type: Option<String>,
}

/// Positional indexes of the fields this app reads, as observed on the
/// `HG8145X7-10` with the MTN profile.
///
/// Named constants rather than bare numbers at the call site, and asserted
/// against a real captured response in the tests, because an off-by-one here
/// silently swaps two figures and no type system notices.
///
/// The numbers are positions in the 58-argument `WanPPP(...)` call the live
/// router returns. Two independent checks pin them: the argument-count test
/// fails if the constructor's arity changes, and the field test fails if a
/// position moves. Reading by position is brittle by nature, which is exactly
/// why both tests exist.
mod wan_index {
    /// The router's own word for the connection state, `Connected` on the live
    /// device. Position 5, before the second copy at 12.
    pub const CONNECTION_STATUS: usize = 5;
    pub const SERVICE_LIST: usize = 27;
    pub const ACCESS_TYPE: usize = 13;
    pub const IP_ADDRESS: usize = 14;
    pub const GATEWAY: usize = 15;
    /// A comma-separated list, joined inside one argument.
    pub const DNS: usize = 18;
    pub const MAC: usize = 4;
    /// How long the connection has been up, in seconds.
    pub const UPTIME: usize = 23;
    pub const MTU: usize = 37;
    pub const BRAS: usize = 38;
}

/// Splits the arguments of one constructor call.
///
/// The generic form, so a second object on the same firmware can be read the
/// same way. The name is the constructor's, and the call must be `new <name>(`.
pub fn constructor_arguments(page: &str, name: &str) -> Option<Vec<String>> {
    let needle = format!("new {name}(");
    let start = page.find(&needle)?;
    let rest = &page[start + needle.len()..];

    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut chars = rest.chars();

    while let Some(character) = chars.next() {
        match in_quote {
            Some(quote) => {
                if character == '\\' {
                    // Keep the escape intact: hex decoding happens once the
                    // whole argument is assembled, so a `\x2c` cannot be mistaken
                    // for a separator.
                    current.push(character);
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                    continue;
                }
                if character == quote {
                    in_quote = None;
                    continue;
                }
                current.push(character);
            }
            None => match character {
                '\'' | '"' => in_quote = Some(character),
                ',' => {
                    arguments.push(current.trim().to_string());
                    current.clear();
                }
                ')' => {
                    arguments.push(current.trim().to_string());
                    return Some(arguments);
                }
                _ => current.push(character),
            },
        }
    }

    // Unterminated call: no complete argument list to read.
    None
}

/// Splits the arguments of one `WanPPP(...)` call.
///
/// A split on commas is not enough: the values contain `\x2c` escapes precisely
/// so a comma cannot appear inside a quoted string. After hex decoding there is
/// still no bare comma inside a value, so splitting on commas outside quotes is
/// correct and the quote handling is what makes it safe.
pub fn wan_ppp_arguments(page: &str) -> Option<Vec<String>> {
    constructor_arguments(page, "WanPPP")
}

/// Reads the `WanPPP` fields out of a WAN cache page.
///
/// Anything the firmware leaves empty stays `None`. A missing status is not a
/// status of "disconnected", and an empty address is not a null address.
pub fn parse_wan_ppp(page: &str) -> WanPppFields {
    let Some(arguments) = wan_ppp_arguments(page) else {
        return WanPppFields::default();
    };

    let at = |index: usize| -> Option<String> {
        let value = decode_hex_escapes(arguments.get(index)?);
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    };

    let dns = at(wan_index::DNS)
        .map(|joined| {
            joined
                .split(',')
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    WanPppFields {
        connection_status: at(wan_index::CONNECTION_STATUS),
        ip_address: at(wan_index::IP_ADDRESS),
        gateway: at(wan_index::GATEWAY),
        dns,
        mac: at(wan_index::MAC),
        mtu: at(wan_index::MTU).and_then(|value| value.parse::<u32>().ok()),
        bras: at(wan_index::BRAS),
        uptime_seconds: at(wan_index::UPTIME).and_then(|value| value.parse::<u64>().ok()),
        service_list: at(wan_index::SERVICE_LIST),
        access_type: at(wan_index::ACCESS_TYPE),
    }
}

/// The fields read from one `WaninfoStats(...)` constructor call.
///
/// The router's traffic counters. This is a different object from `WanPPP`:
/// the connection page carries metadata, and this one carries the byte and
/// packet totals. They come from different endpoints and are read separately.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WanStatsFields {
    pub bytes_sent: Option<u64>,
    pub bytes_received: Option<u64>,
    pub packets_sent: Option<u64>,
    pub packets_received: Option<u64>,
}

/// Positional indexes of the counter fields, as observed on the
/// `HG8145X7-10` with the MTN profile.
///
/// The constructor is
/// `WaninfoStats(domain, BytesSent, BytesReceived, PacketsSent,
/// PacketsReceived, UnicastSent, UnicastReceived, MulticastSent,
/// MulticastReceived, BroadcastSent, BroadcastReceived, ...)`, and the domain
/// is index 0.
///
/// Read by position, so the same two checks the WAN connection has apply: the
/// argument-count test fails if the arity changes and the field test fails if a
/// position moves.
mod wan_stats_index {
    pub const BYTES_SENT: usize = 1;
    pub const BYTES_RECEIVED: usize = 2;
    pub const PACKETS_SENT: usize = 3;
    pub const PACKETS_RECEIVED: usize = 4;
}

/// Reads the traffic counters out of the WAN statistics page.
///
/// A counter the firmware leaves empty is not a counter of zero, so an empty
/// argument stays `None`.
pub fn parse_wan_stats(page: &str) -> WanStatsFields {
    let Some(arguments) = constructor_arguments(page, "WaninfoStats") else {
        return WanStatsFields::default();
    };

    let at = |index: usize| -> Option<u64> {
        let value = decode_hex_escapes(arguments.get(index)?);
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        trimmed.parse::<u64>().ok()
    };

    WanStatsFields {
        bytes_sent: at(wan_stats_index::BYTES_SENT),
        bytes_received: at(wan_stats_index::BYTES_RECEIVED),
        packets_sent: at(wan_stats_index::PACKETS_SENT),
        packets_received: at(wan_stats_index::PACKETS_RECEIVED),
    }
}

/// The router's Wi-Fi radios, from the `stWlanInfo` list.
///
/// The constructor is `stWlanInfo(domain, name, X_HW_ServiceEnable, enable,
/// bindenable)`, and the observed firmware passes four arguments, so the last
/// one is absent rather than false.
pub fn parse_wifi_radios(page: &str) -> Vec<crate::models::overview::WifiRadio> {
    let mut radios = Vec::new();

    for arguments in constructor_calls(page, "stWlanInfo") {
        let at = |index: usize| -> Option<String> {
            let value = decode_hex_escapes(arguments.get(index)?);
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        };

        let Some(name) = at(1) else { continue };

        radios.push(crate::models::overview::WifiRadio {
            name,
            enabled: at(3).as_deref() == Some("1"),
            bound: at(4).as_deref() == Some("1"),
        });
    }

    radios
}

/// The access type of the WAN interface, from the `AccessType` list.
///
/// The constructor is `AccessType(domain, type)`, and the observed router
/// reports `GPON`, which is the router's own word for how the line reaches it.
pub fn parse_wan_access_type(page: &str) -> Option<String> {
    let arguments = constructor_calls(page, "AccessType").into_iter().next()?;
    let value = decode_hex_escapes(arguments.get(1)?);
    let trimmed = value.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// The router's IPv6 state, from the IPv6 WAN list and the prefix list.
///
/// The constructor is `IPv6WanInfo(domain, Type, ConnectionStatus,
/// L2EncapType, MACAddress, Vlan, Pri, ...)`, and the connection status is the
/// router's own word for it. The prefixes are the ones delegated to the
/// router, and the observed firmware delegates an empty one, which is reported
/// as no prefixes rather than as a prefix with nothing in it.
pub fn parse_ipv6(page: &str) -> (Option<String>, Vec<String>) {
    let status = constructor_calls(page, "IPv6WanInfo")
        .into_iter()
        .next()
        .and_then(|arguments| {
            let value = decode_hex_escapes(arguments.get(2)?);
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        });

    let prefixes = constructor_calls(page, "IPv6PrefixInfo")
        .into_iter()
        .filter_map(|arguments| {
            let value = decode_hex_escapes(arguments.get(2)?);
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .collect();

    (status, prefixes)
}

/// Whether the router says it supports IPv6.
pub fn parse_supports_ipv6(page: &str) -> Option<bool> {
    let value = string_var(page, "SupportIPv6")?;
    match value.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

/// Every set of arguments passed to a constructor, in the order they appear.
///
/// The pages construct their objects inside `new Array(...)`, so the objects
/// are found by their constructor name rather than by a regular expression over
/// the whole page.
fn constructor_calls(page: &str, name: &str) -> Vec<Vec<String>> {
    let needle = format!("new {name}(");
    let mut calls = Vec::new();
    let mut search_from = 0;

    while let Some(offset) = page[search_from..].find(&needle) {
        let start = search_from + offset;
        search_from = start + needle.len();

        let rest = &page[search_from..];
        let mut arguments = Vec::new();
        let mut current = String::new();
        let mut in_quote: Option<char> = None;
        // The constructor's own opening parenthesis was part of the needle, so
        // the depth starts at one and a closing parenthesis at one ends the call.
        let mut depth = 1usize;
        let mut chars = rest.chars().peekable();

        while let Some(character) = chars.next() {
            match in_quote {
                Some(quote) => {
                    if character == '\\' {
                        current.push(character);
                        if let Some(next) = chars.next() {
                            current.push(next);
                        }
                        continue;
                    }
                    if character == quote {
                        in_quote = None;
                        continue;
                    }
                    current.push(character);
                }
                None => match character {
                    '\'' | '"' => in_quote = Some(character),
                    '(' => {
                        depth += 1;
                        current.push(character);
                    }
                    ')' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            arguments.push(current.trim().to_string());
                            break;
                        }
                        current.push(character);
                    }
                    ',' if depth == 1 => {
                        arguments.push(current.trim().to_string());
                        current.clear();
                    }
                    _ => current.push(character),
                },
            }
        }

        if !arguments.is_empty() {
            calls.push(arguments);
        }
    }

    calls
}

/// The optical receive power in dBm, or `None` when the firmware reports none.///
/// The observed router reports `opticInfo` as a negative number. A value that is
/// present but outside any physically plausible window is discarded, because a
/// figure a reader would act on must not be a parsing accident.
pub fn parse_optical_rx_dbm(page: &str) -> Option<f64> {
    let value = number_var(page, "opticInfo")?;

    // A real GPON receive power sits well inside these bounds; the range is wide
    // on purpose so it only ever rejects the impossible, not the merely unusual.
    if !(-40.0..=10.0).contains(&value) {
        return None;
    }

    // The firmware uses 0 as "no value" for its optical fields.
    if value == 0.0 {
        return None;
    }

    Some(value)
}

/// Categorises a WAN address so the interface can say what it actually is.
///
/// The observed connection reports `10.54.23.105`, which is RFC1918 private
/// space rather than a publicly routable address. Calling that a "public IP"
/// would be wrong, and the distinction is exactly the sort of thing a person
/// debugging remote access needs to see stated rather than guessed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WanAddressKind {
    /// A publicly routable address.
    Public,
    /// Inside `100.64.0.0/10`, the carrier-grade NAT range.
    CarrierNat,
    /// RFC1918 private space: `10/8`, `172.16/12` or `192.168/16`. On a WAN
    /// interface this means the carrier is NATing too, which is worth showing.
    Private,
    /// A loopback or link-local address, which is never a real WAN address.
    Local,
    /// The text was present but is not an IPv4 address.
    NotAddress,
}

/// Classifies a WAN address. Only IPv4 is handled, because that is what the
/// observed firmware reports and guessing at a v6 form would be inventing data.
pub fn classify_wan_address(address: &str) -> WanAddressKind {
    let octets: Vec<u8> = match address
        .split('.')
        .map(|part| part.parse::<u8>())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(octets) if octets.len() == 4 => octets,
        _ => return WanAddressKind::NotAddress,
    };

    let [a, b, ..] = octets[..] else {
        return WanAddressKind::NotAddress;
    };

    if a == 127 || (a == 169 && b == 254) {
        return WanAddressKind::Local;
    }
    if a == 100 && (64..=127).contains(&b) {
        return WanAddressKind::CarrierNat;
    }
    if a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) {
        return WanAddressKind::Private;
    }

    WanAddressKind::Public
}

/// A label for the address kind, used on the wire so the interface does not
/// repeat the classification rule.
pub fn wan_address_label(kind: WanAddressKind) -> &'static str {
    match kind {
        WanAddressKind::Public => "public",
        WanAddressKind::CarrierNat => "carrierNat",
        WanAddressKind::Private => "private",
        WanAddressKind::Local => "local",
        WanAddressKind::NotAddress => "unknown",
    }
}

/// Which of the router's own sections reported a feature as switched on.
///
/// The firmware ships one build for many carriers and turns sections off with
/// flags. Reading the flag lets the screen say "this router does not report
/// optical figures" instead of "we could not read the optical figures", which
/// are different sentences and only one of them is true.
pub fn flag_enabled(page: &str, name: &str) -> Option<bool> {
    let raw = raw_value(page, name)?;
    let text = raw.trim_matches(|character| character == '\'' || character == '"');

    match text {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The observed deviceinfo page shape, reduced to the assignments this app
    /// reads. Values are exactly as the live router returned them.
    const DEVICEINFO: &str = r#"
<script>
  var showMonitorData = '0';
  var showCPUnMemUsed = '1';
  var opticInfo = '\x2d18\x2e79';
  var cpuUsed = '30%';
  var memUsed = '67%';
  var flashUsed = '%';
  var temp1 = 0;
  var temp2 = 0;
  var ontPonMode = 'gpon';
</script>
"#;

    /// The observed `wan_list_cache_wan.asp` response, verbatim apart from the
    /// line break, which was added for length.
    const WAN_CACHE: &str = r#"function() {
  var obj = {};
  obj.PPPWanList = new Array(new WanPPP("InternetGatewayDevice.WANDevice.1.WANConnectionDevice.1.WANPPPConnection.1","0","0","AlwaysOn","34\x3a66\x3a79\x3aDD\x3a14\x3aD3","Connected","ERROR\x5fNONE","","NCE\x5fHOME\x5fUPublic\x5fIP\x5f457","1","","","Connected","IP\x5fRouted","10\x2e54\x2e23\x2e105","10\x2e54\x2e0\x2e1","1","0","102\x2e88\x2e158\x2e7\x2c197\x2e210\x2e211\x2e1","FN718140","redacted","AlwaysOn","4294967295","457","4294967295","0","0","INTERNET","","0","180","1","1","0","0","Specified","0","1492","T0180\x5fIKY\x5fBRAS\x5fNE40\x2dX8A\x5f1","DetectBidirectionally","7818","0","","18218","0","","","","","0","0","0","0","0","","0","0",""),null);
  return obj;
}"#;

    /// The observed `get_wan_list_pppwanstat.asp` response, verbatim.
    ///
    /// The counters are the router's own, taken from the live device. They are
    /// what a second reading is compared against to produce a rate.
    const WAN_STATS: &str = r#"function() {
  return new Array(new WaninfoStats("InternetGatewayDevice.WANDevice.1.WANConnectionDevice.1.WANPPPConnection.1.Stats","3665038882","3918353917","4298497","4769122","4298496","4758642","0","0","1","10480"),null);
}"#;

    /// The observed `get_wan_list_wlaninfo.asp` response, verbatim.
    ///
    /// Two radios, both switched on, and the last argument absent, which is
    /// why the bound flag reads false rather than the router saying it.
    const WLAN_INFO: &str = r#"function() { return new Array(new stWlanInfo("InternetGatewayDevice\x2eLANDevice\x2e1\x2eWLANConfiguration\x2e1","ath0","1","1"),new stWlanInfo("InternetGatewayDevice\x2eLANDevice\x2e1\x2eWLANConfiguration\x2e5","ath4","1","1"),null); }"#;

    /// The observed `get_wan_list_wanaccesstype.asp` response, verbatim.
    const ACCESS_TYPE: &str = r#"function() { return new Array(new AccessType("InternetGatewayDevice.WANDevice.1.WANCommonInterfaceConfig","GPON"),null); }"#;

    /// The observed `wan_list_cache_wanipv6.asp` response, verbatim.
    const IPV6_STATE: &str = r#"function() { var obj = {}; obj.IPv6AddressList = new Array(null); obj.IPv6PrefixList = new Array(new IPv6PrefixInfo("InternetGatewayDevice.WANDevice.1.X_HW_ShowInterface.1.IPv6Prefix.1","PrefixDelegation","","0","0","0"),null); obj.IPv6WanInfoList = new Array(new IPv6WanInfo("InternetGatewayDevice.WANDevice.1.X_HW_ShowInterface.1","WAN","Invalid","PPPoE","34\x3a66\x3a79\x3aDD\x3a14\x3aD3","457","0","","","","","0"),null); return obj; }"#;

    #[test]
    fn the_live_wifi_page_reads_both_radios() {
        let radios = parse_wifi_radios(WLAN_INFO);

        assert_eq!(radios.len(), 2);
        assert_eq!(radios[0].name, "ath0");
        assert!(radios[0].enabled);
        assert_eq!(radios[1].name, "ath4");
        assert!(radios[1].enabled);
    }

    /// A radio that is switched off is not the same as a radio that is missing.
    #[test]
    fn a_switched_off_radio_is_read_as_off() {
        let page =
            r#"function() { return new Array(new stWlanInfo("domain","ath1","0","0"),null); }"#;

        let radios = parse_wifi_radios(page);

        assert_eq!(radios.len(), 1);
        assert!(!radios[0].enabled, "a radio the router says is off is off");
    }

    #[test]
    fn the_live_access_type_page_reads_gpon() {
        assert_eq!(parse_wan_access_type(ACCESS_TYPE).as_deref(), Some("GPON"));
    }

    #[test]
    fn an_access_type_that_is_not_reported_is_none() {
        assert_eq!(parse_wan_access_type("<html>nothing</html>"), None);
    }

    /// The IPv6 status is the router's own word, so it is passed through.
    #[test]
    fn the_live_ipv6_page_reads_the_rosters_own_word() {
        let (status, prefixes) = parse_ipv6(IPV6_STATE);

        assert_eq!(status.as_deref(), Some("Invalid"));
        assert!(
            prefixes.is_empty(),
            "an empty prefix is no prefix, not a prefix with nothing in it"
        );
    }

    #[test]
    fn the_ipv6_support_flag_is_read_from_the_settings_page() {
        let page = "var SupportIPv6 = \"1\";";

        assert_eq!(parse_supports_ipv6(page), Some(true));
        assert_eq!(parse_supports_ipv6("var SupportIPv6 = \"0\";"), Some(false));
        assert_eq!(parse_supports_ipv6("var Something = \"1\";"), None);
    }

    /// A constructor called with a nested parenthesis must not throw the
    /// reading off, which is what the depth counter is for.
    #[test]
    fn a_nested_parenthesis_does_not_end_the_arguments() {
        let page = r#"new Thing("a","b(c)","d")"#;

        let calls = constructor_calls(page, "Thing");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], vec!["a", "b(c)", "d"]);
    }

    #[test]
    fn the_live_traffic_page_produces_the_observed_counters() {
        let fields = parse_wan_stats(WAN_STATS);

        assert_eq!(fields.bytes_sent, Some(3_665_038_882));
        assert_eq!(fields.bytes_received, Some(3_918_353_917));
        assert_eq!(fields.packets_sent, Some(4_298_497));
        assert_eq!(fields.packets_received, Some(4_769_122));
    }

    /// The constructor's arity is part of the contract. If the firmware changes
    /// it, the positions have to be re-verified against a real response, and
    /// this test is what says so.
    #[test]
    fn the_traffic_constructor_has_the_arity_the_positions_assume() {
        let arguments =
            constructor_arguments(WAN_STATS, "WaninfoStats").expect("the observed page must parse");

        assert_eq!(
            arguments.len(),
            11,
            "the live response carries 11 arguments; a different count means the positions must be re-verified"
        );
    }

    /// A counter the firmware leaves empty is not a counter of zero.
    #[test]
    fn an_empty_counter_is_not_read_as_zero() {
        let page = r#"function() {
  return new Array(new WaninfoStats("domain","","","","","","","","","",""),null);
}"#;

        let fields = parse_wan_stats(page);

        assert_eq!(fields.bytes_sent, None);
        assert_eq!(fields.bytes_received, None);
        assert_eq!(fields.packets_sent, None);
        assert_eq!(fields.packets_received, None);
    }

    /// The IP variant of the same call returns an empty array on this firmware,
    /// which is why the PPP counters are the usable source.
    #[test]
    fn an_empty_array_reads_as_no_counters() {
        let page = "function() {\n  return new Array(null);\n}";

        let fields = parse_wan_stats(page);

        assert_eq!(fields.bytes_sent, None);
        assert_eq!(fields.bytes_received, None);
    }

    /// A page with no constructor at all is not a reading.
    #[test]
    fn a_page_without_the_constructor_reads_as_no_counters() {
        let fields = parse_wan_stats("<html>not found</html>");

        assert_eq!(fields.bytes_sent, None);
        assert_eq!(fields.bytes_received, None);
    }

    /// The connection object and the statistics object are different
    /// constructors on different pages. Reading one for the other must produce
    /// nothing rather than a plausible wrong number.
    #[test]
    fn the_two_wan_objects_do_not_get_confused() {
        let from_connection_page = parse_wan_stats(WAN_CACHE);
        assert_eq!(from_connection_page.bytes_sent, None);
        assert_eq!(from_connection_page.bytes_received, None);

        let from_statistics_page = parse_wan_ppp(WAN_STATS);
        assert_eq!(from_statistics_page.ip_address, None);
    }

    #[test]
    fn a_quoted_number_with_hex_escapes_is_read() {
        assert_eq!(number_var(DEVICEINFO, "opticInfo"), Some(-18.79));
    }

    #[test]
    fn a_percentage_is_read_without_its_sign() {
        assert_eq!(percent_var(DEVICEINFO, "cpuUsed"), Some(30.0));
        assert_eq!(percent_var(DEVICEINFO, "memUsed"), Some(67.0));
    }

    #[test]
    fn an_empty_percentage_is_not_a_reading_of_zero() {
        // The live firmware reports `flashUsed = '%'`. That is an absent value,
        // and rendering it as 0 percent would be the fabrication this app exists
        // to avoid.
        assert_eq!(percent_var(DEVICEINFO, "flashUsed"), None);
    }

    #[test]
    fn a_bare_zero_temperature_is_not_a_temperature_of_zero() {
        // Parsed at the number level so the test documents the raw shape...
        assert_eq!(number_var(DEVICEINFO, "temp1"), Some(0.0));
        // ...and the caller is what treats a bare zero as absent. Asserted here
        // so the two facts cannot drift apart without a test noticing.
        assert_eq!(
            plain_zero_is_absent_temp(number_var(DEVICEINFO, "temp1")),
            None
        );
    }

    /// The rule the optical and temperature fields share: the firmware's `0` is
    /// "no sensor", never a reading.
    fn plain_zero_is_absent_temp(value: Option<f64>) -> Option<f64> {
        value.filter(|reading| *reading != 0.0)
    }

    #[test]
    fn a_missing_variable_is_none_rather_than_zero() {
        assert_eq!(number_var(DEVICEINFO, "noSuchField"), None);
        assert_eq!(percent_var(DEVICEINFO, "noSuchField"), None);
        assert_eq!(string_var(DEVICEINFO, "noSuchField"), None);
    }

    #[test]
    fn a_name_that_is_a_prefix_of_another_is_not_matched() {
        // `var monitorDataArray` must not satisfy a lookup for `monitorData`.
        let page = "var monitorDataArray = 'x';";
        assert_eq!(string_var(page, "monitorData"), None);
    }

    #[test]
    fn the_optical_receive_power_is_read_from_the_escaped_form() {
        assert_eq!(parse_optical_rx_dbm(DEVICEINFO), Some(-18.79));
    }

    #[test]
    fn an_optical_zero_is_unavailable_not_zero_dbm() {
        let page = "var opticInfo = '0';";
        assert_eq!(parse_optical_rx_dbm(page), None);
    }

    #[test]
    fn an_implausible_optical_value_is_discarded() {
        // A positive receive power cannot happen, so it is a parsing accident
        // rather than a measurement.
        let page = "var opticInfo = '99';";
        assert_eq!(parse_optical_rx_dbm(page), None);
    }

    #[test]
    fn a_section_flag_reads_as_a_boolean() {
        assert_eq!(flag_enabled(DEVICEINFO, "showCPUnMemUsed"), Some(true));
        assert_eq!(flag_enabled(DEVICEINFO, "showMonitorData"), Some(false));
        assert_eq!(flag_enabled(DEVICEINFO, "noSuchField"), None);
    }

    #[test]
    fn hex_escapes_are_decoded_to_their_real_characters() {
        assert_eq!(decode_hex_escapes(r"10\x2e54\x2e23"), "10.54.23");
        assert_eq!(decode_hex_escapes(r"T0180\x5fIKY"), "T0180_IKY");
        assert_eq!(decode_hex_escapes(r"NE40\x2dX8A\x5f1"), "NE40-X8A_1");
    }

    // ---------------------------------------------------------------------
    // The WAN page.
    // ---------------------------------------------------------------------

    #[test]
    fn the_wan_arguments_split_on_commas_outside_quotes() {
        let arguments = wan_ppp_arguments(WAN_CACHE).expect("the captured page must parse");
        // One address field holds a comma-joined DNS list, so a naive split
        // would produce more arguments than the constructor takes.
        assert_eq!(arguments.len(), 58, "argument count changed: {arguments:?}");
    }

    #[test]
    fn the_wan_connection_fields_match_the_live_router() {
        let wan = parse_wan_ppp(WAN_CACHE);

        assert_eq!(wan.connection_status.as_deref(), Some("Connected"));
        assert_eq!(wan.ip_address.as_deref(), Some("10.54.23.105"));
        assert_eq!(wan.gateway.as_deref(), Some("10.54.0.1"));
        assert_eq!(wan.mac.as_deref(), Some("34:66:79:DD:14:D3"));
        assert_eq!(wan.mtu, Some(1492));
        assert_eq!(wan.bras.as_deref(), Some("T0180_IKY_BRAS_NE40-X8A_1"));
        assert_eq!(wan.access_type.as_deref(), Some("IP_Routed"));
        assert_eq!(wan.service_list.as_deref(), Some("INTERNET"));
        assert_eq!(
            wan.dns,
            vec!["102.88.158.7".to_string(), "197.210.211.1".to_string()]
        );
    }

    #[test]
    fn a_wan_page_with_no_constructor_yields_no_fields() {
        let wan = parse_wan_ppp("<html><body>nothing here</body></html>");
        assert_eq!(wan, WanPppFields::default());
    }

    #[test]
    fn an_empty_wan_field_is_absent_rather_than_an_empty_string() {
        // The captured page has empty arguments in several positions. They must
        // not surface as empty strings a caller could render as a value.
        let wan = parse_wan_ppp(WAN_CACHE);
        assert_eq!(wan.uptime_seconds, Some(457));
    }

    // ---------------------------------------------------------------------
    // Address classification.
    // ---------------------------------------------------------------------

    #[test]
    fn the_live_wan_address_is_private_carrier_side_space() {
        // The observed WAN address is `10.54.23.105`. That is RFC1918 private
        // space, not the `100.64.0.0/10` carrier-grade NAT range; the two are
        // different ranges and conflating them was a mistake worth pinning down
        // with a test. What matters for a reader is that it is *not public*:
        // a real MTN WAN would normally be a publicly routable address, so a
        // private one here means the router sits behind the carrier's own NAT.
        assert_eq!(
            classify_wan_address("10.54.23.105"),
            WanAddressKind::Private
        );
        assert_ne!(classify_wan_address("10.54.23.105"), WanAddressKind::Public);
    }

    #[test]
    fn the_carrier_nat_range_boundaries_are_exact() {
        assert_eq!(
            classify_wan_address("100.64.0.0"),
            WanAddressKind::CarrierNat
        );
        assert_eq!(
            classify_wan_address("100.127.255.255"),
            WanAddressKind::CarrierNat
        );
        // One below and one above the range are not carrier NAT.
        assert_eq!(classify_wan_address("100.63.0.1"), WanAddressKind::Public);
        assert_eq!(classify_wan_address("100.128.0.1"), WanAddressKind::Public);
    }

    #[test]
    fn private_and_local_addresses_are_told_apart() {
        assert_eq!(classify_wan_address("192.168.1.1"), WanAddressKind::Private);
        assert_eq!(classify_wan_address("10.0.0.1"), WanAddressKind::Private);
        assert_eq!(classify_wan_address("172.16.0.1"), WanAddressKind::Private);
        assert_eq!(classify_wan_address("172.32.0.1"), WanAddressKind::Public);
        assert_eq!(classify_wan_address("127.0.0.1"), WanAddressKind::Local);
        assert_eq!(classify_wan_address("169.254.1.1"), WanAddressKind::Local);
    }

    #[test]
    fn a_public_address_is_recognised() {
        assert_eq!(classify_wan_address("8.8.8.8"), WanAddressKind::Public);
        assert_eq!(classify_wan_address("41.58.1.1"), WanAddressKind::Public);
    }

    #[test]
    fn text_that_is_not_an_address_is_not_classified_as_one() {
        assert_eq!(
            classify_wan_address("not an address"),
            WanAddressKind::NotAddress
        );
        assert_eq!(classify_wan_address(""), WanAddressKind::NotAddress);
        assert_eq!(
            classify_wan_address("999.1.1.1"),
            WanAddressKind::NotAddress
        );
        // An IPv6 address is not guessed at; it is simply not IPv4.
        assert_eq!(
            classify_wan_address("2001:db8::1"),
            WanAddressKind::NotAddress
        );
    }
}
