/**
 * Shared vocabulary for connection readings. Mirrors the Rust types in
 * `src-tauri/src/models/overview.rs`; keep both sides in step when changing it.
 */
export type ValueState = "measured" | "calculated" | "stale" | "unavailable";

export type HealthState =
  | "connected"
  | "degraded"
  | "offline"
  | "unknown"
  | "paused";

export interface MetricValue<T> {
  /** Null whenever nothing was measured. Never a stand-in zero. */
  value: T | null;
  state: ValueState;
  /** UTC epoch milliseconds, or null before the first reading. */
  measuredAt: number | null;
}

/**
 * The counts behind the failure figure.
 *
 * Sent from the backend so that no second copy of the counting rule has to
 * exist in TypeScript. The backend counts; this side renders.
 */
export type ProbeFailureBasis = { failed: number; total: number };

/**
 * The local router, probed separately from the internet.
 *
 * `reachable` is a three-state value on purpose: null means no probe has
 * completed, false means one completed and did not connect, true means one
 * connected. Collapsing null and false would make a router that was never
 * checked look like a router that is down.
 */
export interface RouterReachability {
  /** The address that was probed, or null when no router target is configured. */
  host: string | null;
  reachable: boolean | null;
  /** Connect time in milliseconds when the router answered. */
  latency: MetricValue<number>;
  /** When the most recent router probe ran, success or failure. */
  checkedAt: number | null;
}

/**
 * The probe target behind the headline reading, named for display.
 *
 * Sent with the overview so the interface can say what a latency is a
 * measurement of, rather than showing a bare number with no subject.
 */
export interface ProbeTargetSummary {
  name: string;
  host: string;
  port: number;
  /** "TCP" today. A label, not a boolean, so a second method is never mislabelled. */
  method: string;
  intervalMs: number;
  timeoutMs: number;
}

export interface ConnectionOverview {
  healthState: HealthState;
  /** Which target the headline latency belongs to, or null before the loop names one. */
  primaryTarget: ProbeTargetSummary | null;
  monitoringActive: boolean;
  /**
   * Whether the loop is paused.
   *
   * Separate from `monitoringActive` because a paused app is not a stopped one:
   * the loop is alive and will resume, and the readings taken before the pause
   * are still stored.
   */
  paused: boolean;
  lastProbeAt: number | null;
  /** Measurement window the failure figure covers, or null when nothing was measured. */
  packetLossWindowMs: number | null;
  latency: MetricValue<number>;
  packetLoss: MetricValue<number>;
  downloadRate: MetricValue<number>;
  uploadRate: MetricValue<number>;
  /** Null exactly when `packetLoss` is unavailable. */
  probeFailureBasis: ProbeFailureBasis | null;
  router: RouterReachability;
}

/**
 * What kind of address the router's WAN interface holds.
 *
 * `private` on a WAN interface is worth saying out loud: a real MTN connection
 * would normally carry a publicly routable address, so a private one means the
 * carrier is NATing as well as the router. It must never be shown as "public".
 */
export type WanAddressKind =
  | "public"
  | "carrierNat"
  | "private"
  | "local"
  | "unknown";

/**
 * The WAN connection as the router itself reports it.
 *
 * Mirrors `WanConnection` in `src-tauri/src/models/overview.rs`. Every field is
 * separately nullable because the router's reporting is patchy: it can name its
 * address while reporting no MTU, and a screen that required all of them would
 * show nothing at all.
 */
export interface WanConnection {
  /** The router's own word, such as "Connected". Null when it reported none. */
  status: string | null;
  ipAddress: string | null;
  /** Null when there is no address to classify. */
  addressKind: WanAddressKind | null;
  gateway: string | null;
  dnsServers: string[];
  mac: string | null;
  mtu: number | null;
  /** The access server the carrier terminates the session on, where reported. */
  bras: string | null;
  accessType: string | null;
  serviceList: string | null;
  /** How long the connection has been up, in seconds. */
  uptimeSeconds: number | null;
}

/**
 * What the router says about its own health, where it says anything.
 *
 * Deliberately narrow, and matches the Rust type of the same name. There is no
 * temperature, voltage or bias current field, because the observed firmware does
 * not report them: the fields simply do not exist here, so no component can
 * render a zero temperature by accident.
 */
export interface RouterHealth {
  opticalRxDbm: MetricValue<number>;
  cpuPercent: MetricValue<number>;
  memoryPercent: MetricValue<number>;
  ponMode: string | null;
  /** False means this firmware does not report an optical figure at all. */
  reportsOptical: boolean | null;
  reportsUtilisation: boolean | null;
}

/**
 * The router's cumulative traffic counters.
 *
 * Totals since the WAN session began, not rates. The two are different facts
 * and the interface labels them differently.
 */
export interface WanCounters {
  bytesSent: number;
  bytesReceived: number;
  packetsSent: number;
  packetsReceived: number;
  /** UTC epoch milliseconds of the read. */
  measuredAt: number;
}

/**
 * A throughput rate derived from two counter readings.
 *
 * A rate is the app's own arithmetic on two of the router's totals, so its
 * state is `calculated` rather than `measured`. `note` says why there is no
 * rate, because "waiting for a second reading" and "the router reset the
 * counter" are different facts that need different actions.
 */
export interface TrafficRate {
  /** Bytes per second towards the internet, or null when no rate could be derived. */
  sentBytesPerSecond: number | null;
  /** Bytes per second from the internet, or null when no rate could be derived. */
  receivedBytesPerSecond: number | null;
  state: ValueState;
  /** UTC epoch milliseconds of the newer of the two readings. */
  measuredAt: number | null;
  note: string | null;
}

/** One of the router's Wi-Fi radios. */
export interface WifiRadio {
  /** The radio's own name, such as "ath0". */
  name: string;
  /** Whether the radio is switched on. */
  enabled: boolean;
  /** Whether the radio is bound to a service. */
  bound: boolean;
}

/**
 * The router's own view of the network around it.
 *
 * Everything here is something the router reported. Where the router reports
 * that a thing is not present, that is the absence of the thing rather than a
 * zero: the IPv6 status on the observed connection is the router's own word
 * "Invalid", and it is passed through rather than interpreted.
 */
export interface RouterNetwork {
  wifiRadios: WifiRadio[];
  /** The access type of the WAN interface, such as "GPON". */
  wanAccessType: string | null;
  /** The router's word for the IPv6 connection state, such as "Invalid". */
  ipv6Status: string | null;
  /** Whether the router says it supports IPv6. */
  supportsIpv6: boolean | null;
  /** The IPv6 prefixes delegated to the router. */
  ipv6Prefixes: string[];
}

/** Whether one router page was read, and if not, why. */
export interface RouterSourceStatus {
  name: string;
  path: string;
  ok: boolean;
  error: RouterError | null;
}

export type RouterErrorKind =
  | "noCredentials"
  | "unreachable"
  | "authRejected"
  | "unexpectedResponse"
  | "timeout";

/** Why a router read did not produce data. Never carries a credential. */
export interface RouterError {
  kind: RouterErrorKind;
  message: string;
}

/**
 * One successful router read.
 *
 * `auth` names the address and user, and reports only whether a password is
 * configured. The password itself never crosses this boundary.
 */
export interface RouterSnapshot {
  /** UTC epoch milliseconds of the read that produced this. */
  readAt: number;
  auth: {
    host: string;
    port: number;
    username: string | null;
    hasPassword: boolean;
  };
  wan: WanConnection;
  health: RouterHealth;
  /** The router's traffic counters, when the statistics page answered. */
  counters: WanCounters | null;
  /** The router's own view of the network around it. */
  network: RouterNetwork;
  /** Every page that was attempted, so the screen can be specific about a gap. */
  sources: RouterSourceStatus[];
}

/**
 * Everything the router integration currently knows.
 *
 * The three fields answer three different questions: is there a reading, why is
 * there not one, and is a router even configured. Collapsing them would make an
 * unconfigured app look like a broken one.
 */
export interface RouterReport {
  /** Null until a read has succeeded. Kept across a later failure. */
  snapshot: RouterSnapshot | null;
  /** The most recent failure, or null when the last read succeeded. */
  error: RouterError | null;
  configured: boolean;
  /** False until a read has been attempted, so "waiting" is not shown as "failed". */
  attempted: boolean;
  /**
   * The throughput rate, when two counter readings have produced one.
   *
   * Null until there have been two readings, because one total cannot produce
   * a rate.
   */
  traffic: TrafficRate | null;
}
