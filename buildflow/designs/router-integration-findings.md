# Router integration findings: Huawei OptiXstar HG8145X7-10

Captured by probing the live router read-only. Every claim here was observed, not
assumed. This is the reference for implementing the router features.

## Identity, confirmed

- Model: `HG8145X7-10` (the login page sets `var ProductName = 'HG8145X7-10'`);
  the page title is the same string.
- Carrier profile: `CfgMode = 'MTN'`.
- Address: `192.168.100.1`.
- PON mode: `gpon`.
- Firmware UI build tag: `49bd6ca47519dc0553184798`.

## The transport, and why it is not obvious

Three facts together caused every early request to fail:

1. **The router serves its UI over HTTPS on port 80.** Plain HTTP returns a stub
   page whose `LoadFrame()` redirects to `https://<host>:80/`. Port 443 is
   closed. The certificate is self-signed, so any client must accept it.
2. **There are two different token endpoints, and they are not interchangeable.**

| Purpose | Endpoint |
|---|---|
| Token for the login POST | `/html/ssmp/common/getRandString.asp` |
| Token for every authenticated request after login | `/html/ssmp/common/GetRandToken.asp` |

Using the login token for data requests returns `403` on every page. This is the
single most important detail for any implementation.

The token is returned with a UTF-8 BOM (`\ufeff`) that must be stripped before
use. Tokens are single-use: fetch a fresh one per request.

## Login

`POST /login.cgi`, form-encoded:

| Field | Value |
|---|---|
| `UserName` | the configured username |
| `PassWord` | the password, base64 encoded (not hashed) |
| `Language` | `english` |
| `x.X_HW_Token` | from `getRandString.asp` |

Success is a page containing `top.location.replace('/')`. Failure returns the
login page with an error string.

**A session cookie is set, and every authenticated request must carry it.** This
was wrong in the first pass of this document and cost a full debugging session to
find. `/login.cgi` returns `Set-Cookie: CookieHttps=...`, and any authenticated
page read without that cookie returns a 602-byte stub with status `200` rather
than real data. Because the stub is a 200, its absence looks like a parse failure
rather than an authentication failure.

So authentication is **two things at once**: the per-request token *and* the
session cookie. Both are required, and neither alone is enough:

| Request | Token required | Cookie required |
|---|---|---|
| `getRandString.asp` (pre-login) | no | no |
| `login.cgi` | yes, from `getRandString.asp` | no, it sets it |
| `GetRandToken.asp` (post-login) | no | yes, or `403` |
| any data page | yes, from `GetRandToken.asp` | yes, or a 200 stub |

## WAN address classification

The observed WAN address is `10.54.23.105`. The first pass of this document
called it "carrier-grade NAT range", which is **wrong**: `100.64.0.0/10` is the
carrier-grade NAT range, and `10.54.23.105` is not in it. It is RFC1918 private
space, in `10.0.0.0/8`.

The distinction still matters for a reader, for the same underlying reason: a
real MTN consumer WAN would normally be handed a publicly routable address, so a
private one means the router sits behind the carrier's own NAT as well as doing
its own. The app must present it as a private address and never as a public one.

## Verified readable data

### Device and system

`POST /html/ssmp/deviceinfo/deviceinfo.asp` (57 KB, status 200).

Observed variable values:

| Variable | Observed | Meaning |
|---|---|---|
| `opticInfo` | `-18.79` | Optical receive power, dBm. A real fibre measurement. |
| `cpuUsed` | `30%` | Router CPU utilisation |
| `memUsed` | `67%` | Router memory utilisation |
| `flashUsed` | `%` | Empty on this firmware; not usable |
| `temp1`, `temp2` | `0` | Not reported by this firmware. Must not be shown as a real temperature. |
| `ontPonMode` | `gpon` | PON mode |
| `showMonitorData` | `0` | Monitor data section is disabled on this profile |
| `showCPUnMemUsed` | `1` | CPU and memory are enabled |

### WAN connection

`POST /html/bbsp/common/wan_list_cache_wan.asp` (829 bytes, status 200) returns
a `WanPPP` object. Observed fields:

| Field | Observed | Meaning |
|---|---|---|
| connection status | `Connected` | WAN is up |
| IP address | `10.54.23.105` | RFC1918 private space, **not** a public address and not the `100.64/10` CGNAT range |
| gateway | `10.54.0.1` | |
| DNS | `102.88.158.7`, `197.210.211.1` | MTN resolvers |
| WAN MAC | `34:66:79:DD:14:D3` | |
| MTU | `1492` | Consistent with PPPoE |
| BRAS | `T0180_IKY_BRAS_NE40-X8A_1` | MTN access server identity |
| connection type | `IP_Routed`, `AlwaysOn` | |

Also readable, status 200: `/html/bbsp/common/wan_list.asp` (74 KB),
`wan_list_info.asp` (46 KB), `wan_check.asp` (64 KB), `/index.asp` (119 KB).

## What was NOT found

- **No device or client list.** The router's own "User Device Information" page
  is `html/bbsp/userdevinfo/userdevinfo1.asp`, and its data source is
  `html/bbsp/userdevinfo/getuserdevinfo.asp`. That call answers `"NONE"`,
  because the router builds the device table lazily from an ARP scan and the
  file does not exist until that scan has run. The page also reads a file state
  and refuses to show anything until it is `Completed`. So the endpoint is
  known, but the data behind it is not available on demand.
- **No optical transmit power.** The router's own dashboard calls
  `/html/ssmp/common/getOpticTxRx.asp` for a `TxPower` value, but that path
  answers 404 on this firmware, so the transmit figure the router asks for does
  not exist here. Receive power does, and is read from `opticInfo`.
- **No temperature, voltage, or bias current.** `temp1`/`temp2` are `0` and the
  optical page exposes only RX power.

## WAN traffic counters: found

The byte counters live behind a page the connection page does not link to:

`POST /html/bbsp/common/get_wan_list_pppwanstat.asp`

which returns:

```js
function() {
  return new Array(new WaninfoStats("InternetGatewayDevice.WANDevice.1.WANConnectionDevice.1.WANPPPConnection.1.Stats","3665038882","3918353917","4298497","4769122","4298496","4758642","0","0","1","10480"),null);
}
```

The constructor is
`WaninfoStats(domain, BytesSent, BytesReceived, PacketsSent, PacketsReceived,
UnicastSent, UnicastReceived, MulticastSent, MulticastReceived,
BroadcastSent, BroadcastReceived, ...)`, so the positions are 1, 2, 3 and 4.

The counters were confirmed to advance between two reads four seconds apart:
`BytesSent` went 3,665,793,940 to 3,665,815,859 and `BytesReceived` went
3,919,609,559 to 3,920,002,037.

The IP variant, `/html/bbsp/common/get_wan_list_ipwanstat.asp`, returns
`new Array(null)` on this firmware, so the PPP counters are the usable source.
The router's own `wan_list.asp` copies these into the connection object through
a `WanStatsLoader`, which is how the location was found.

## More pages that do answer

| Page | How it answers | What it gives |
|---|---|---|
| `get_wan_list_wlaninfo.asp` | POST, token in the body | The Wi-Fi radios: `stWlanInfo(domain, name, X_HW_ServiceEnable, enable, bindenable)`. Two on this router, `ath0` and `ath4`, both on. |
| `get_wan_list_wanaccesstype.asp` | POST, token in the body | `AccessType(domain, type)`. This router reports `GPON`. |
| `wan_list_cache_wanipv6.asp` | **GET**, token in the query string | The IPv6 state. `IPv6WanInfo(domain, Type, ConnectionStatus, ...)`; the observed connection status is `Invalid`. |
| `getWanDynamicData.asp` | **GET**, token in the query string | The WAN settings, among them `SupportIPv6 = "1"`. |

**The last two only answer a GET with the token in the query string.** A POST
with the token in the body, or a GET without a token, gets back the 602-byte
"waiting" stub, which looks like a page and carries nothing. This is the same
shape of trap as the session cookie, and it is worth knowing about before
spending an afternoon on it.

## Standing rules for implementing this

- Read-only. No `set.cgi` calls, no configuration writes, no session hijacking.
- Credentials never hardcoded and never logged. They come from the environment.
- The app must keep working when none of this is reachable.
- A field that returns `0`, an empty string, or a placeholder is **unavailable**,
  never a measured zero. `temp1 = 0` is not a temperature of zero degrees.
- Optical and temperature figures must remain optional and must never gate
  anything else.

## Probing further, reproducibly

The observations above were made with a scripted probe, kept in the repository:

    node scripts/with-env.mjs node scripts/probe-router.mjs <path> [<path> ...]

It signs in with the same two-token and session-cookie protocol, then asks each
named page both ways (token in the body and token in the query string) and
prints how each answers. It is read-only: no `set.cgi`, no configuration
writes, and no credential is ever printed. Each observed claim above can be
re-checked by naming its page, for example:

    node scripts/with-env.mjs node scripts/probe-router.mjs \
      /html/bbsp/common/get_wan_list_pppwanstat.asp \
      /html/ssmp/common/getOpticTxRx.asp

Probing needs a machine on the router's LAN. From any other network the router
does not answer: connections to `192.168.100.1` time out, and the app logs the
read failure and carries on, which is the degradation the standing rules ask
for, observed live.
