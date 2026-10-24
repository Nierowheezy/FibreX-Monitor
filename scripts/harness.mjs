// A browser harness for the dashboard.
//
// The app needs a Tauri backend to render, so this serves the built frontend
// with a stub that answers the same commands with realistic values. It exists
// so the layout can be looked at in a browser, and it is a development tool
// only: it is not imported by the app and it never reaches the bundle.
//
// The stub has to match the real IPC contract exactly, so the values here are
// the shapes the Rust commands actually send.

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

// `fileURLToPath` rather than `URL.pathname`, because the pathname is
// percent-encoded and this project has a space in its folder name.
const ROOT = fileURLToPath(new URL("../dist/", import.meta.url));
const PORT = Number(process.env.PORT || 4173);

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".woff2": "font/woff2",
  ".webp": "image/webp",
  ".png": "image/png",
  ".svg": "image/svg+xml",
};

const now = Date.now();

const OVERVIEW = {
  healthState: "connected",
  primaryTarget: {
    name: "Internet",
    host: "1.1.1.1",
    port: 443,
    method: "TCP",
    intervalMs: 2000,
    timeoutMs: 1500,
  },
  monitoringActive: true,
  paused: false,
  lastProbeAt: now - 2000,
  packetLossWindowMs: 300000,
  latency: { value: 14.2, state: "measured", measuredAt: now - 2000 },
  packetLoss: { value: 12, state: "measured", measuredAt: now - 2000 },
  downloadRate: { value: null, state: "unavailable", measuredAt: null },
  uploadRate: { value: null, state: "unavailable", measuredAt: null },
  probeFailureBasis: { failed: 3, total: 25 },
  router: {
    host: "192.168.100.1",
    reachable: true,
    latency: { value: 1.4, state: "measured", measuredAt: now - 2000 },
    checkedAt: now - 2000,
  },
};

const ROUTER_REPORT = {
  snapshot: {
    readAt: now - 5000,
    auth: { host: "192.168.100.1", port: 80, username: "root", hasPassword: true },
    wan: {
      status: "Connected",
      ipAddress: "10.54.23.105",
      addressKind: "private",
      gateway: "10.54.0.1",
      dnsServers: ["102.88.158.7", "197.210.211.1"],
      mac: "34:66:79:DD:14:D3",
      mtu: 1492,
      bras: "T0180_IKY_BRAS_NE40-X8A_1",
      accessType: "IP_Routed",
      serviceList: "INTERNET",
      uptimeSeconds: 457,
    },
    health: {
      opticalRxDbm: { value: -18.83, state: "measured", measuredAt: now - 5000 },
      cpuPercent: { value: 12, state: "measured", measuredAt: now - 5000 },
      memoryPercent: { value: 66, state: "measured", measuredAt: now - 5000 },
      ponMode: "gpon",
      reportsOptical: true,
      reportsUtilisation: true,
    },
    counters: {
      bytesSent: 3665815859,
      bytesReceived: 3920002037,
      packetsSent: 4299846,
      packetsReceived: 4770894,
      measuredAt: now - 5000,
    },
    network: {
      wifiRadios: [
        { name: "ath0", enabled: true, bound: true },
        { name: "ath4", enabled: true, bound: true },
      ],
      wanAccessType: "GPON",
      ipv6Status: "Invalid",
      supportsIpv6: true,
      ipv6Prefixes: [],
    },
    sources: [
      { name: "Device information", path: "/device", ok: true, error: null },
      { name: "WAN connection", path: "/wan", ok: true, error: null },
      { name: "WAN traffic counters", path: "/wanstat", ok: true, error: null },
    ],
  },
  error: null,
  configured: true,
  attempted: true,
  traffic: {
    sentBytesPerSecond: 234000,
    receivedBytesPerSecond: 1240000,
    state: "calculated",
    measuredAt: now - 5000,
    note: null,
  },
};

const INCIDENTS = [
  {
    id: 3,
    targetId: 1,
    startedAt: now - 3600000,
    recoveredAt: now - 3600000 + 252000,
    probeCount: 9,
    routerReachable: true,
  },
  {
    id: 2,
    targetId: 1,
    startedAt: now - 86400000,
    recoveredAt: now - 86400000 + 48000,
    probeCount: 4,
    routerReachable: false,
  },
];

const HISTORY_INFO = {
  enabled: true,
  earliestSampleAt: now - 5400000,
  sampleCount: 2700,
  retentionMs: 14 * 86400000,
};

const UPTIME = {
  percent: 99.2,
  connected: 248,
  total: 250,
  windowStart: now - 5400000,
  windowEnd: now - 2000,
};

/**
 * The stub, injected into the head before the app's own script runs.
 *
 * The series generator is written out as source rather than as data, because
 * the number of points in it depends on the range the interface asks for.
 *
 * A `?click=` parameter clicks a rail button by its title after the app has
 * mounted, so a screen can be looked at directly.
 */
const STUB = `<script>
(function () {
  var DATA = ${JSON.stringify({
    overview: OVERVIEW,
    routerReport: ROUTER_REPORT,
    incidents: INCIDENTS,
    historyInfo: HISTORY_INFO,
    uptime: UPTIME,
    now,
  })};

  function series(sinceMs) {
    var count = 150;
    var points = [];
    var value = 16;
    for (var index = 0; index < count; index += 1) {
      value += Math.sin(index / 7) * 2.2 + (Math.random() - 0.5) * 3;
      points.push({
        at: DATA.now - sinceMs + Math.round((index * sinceMs) / count),
        value: Math.max(4, Math.round(value * 10) / 10),
      });
    }
    points[63].value = null;
    points[64].value = null;
    points[117].value = null;
    return points;
  }

  // The router connection state the stub serves and mutates.
  var routerConnection = {
    source: "appSettings",
    host: "192.168.100.1",
    port: 80,
    username: "admin",
    hasPassword: true,
    willRead: true,
    passwordMissing: false,
  };

  var RESPONSES = {
    get_connection_overview: function () { return DATA.overview; },
    get_latency_series: function () { return series(300000); },
    get_stored_series: function (args) { return series(args && args.sinceMs ? args.sinceMs : 300000); },
    get_history_info: function () { return DATA.historyInfo; },
    get_uptime: function () { return DATA.uptime; },
    get_incidents: function () { return DATA.incidents; },
    get_router_report: function () { return DATA.routerReport; },
    run_speed_test: function () {
      // The progress is published first, then the figures arrive, so the
      // running state of every phase can be seen: the pings count up, then the
      // download rate climbs, then the upload's.
      var phase = "ping";
      var latency = [42.0, 38.4, 36.1];
      var sampleAt = 0;
      var downloadTicks = 0;

      var tick = window.setInterval(function () {
        var payload;
        if (phase === "ping") {
          var sample = latency[sampleAt];
          sampleAt = sampleAt + 1;
          payload = {
            phase: "ping",
            bytesPerSecond: null,
            bytesSoFar: null,
            latencyMs: sample,
          };
          if (sampleAt >= latency.length) {
            phase = "download";
          }
        } else {
          payload = {
            phase: phase,
            bytesPerSecond: phase === "download" ? 6_100_000 : 2_150_000,
            bytesSoFar: 10_485_760,
            latencyMs: null,
          };
          // The download runs for about one and a half seconds of harness
          // time, then the upload takes over, so the running state of both
          // transfer phases can be seen.
          if (phase === "download") {
            downloadTicks = downloadTicks + 1;
            if (downloadTicks >= 6) {
              phase = "upload";
            }
          }
        }
        window.__TAURI_INTERNALS__.emit("speedtest://progress", payload);
      }, 240);

      return new Promise(function (resolve) {
        window.setTimeout(function () {
          window.clearInterval(tick);
          resolve({
            kind: "completed",
            result: {
              latencyMs: 38.4,
              jitterMs: 3.1,
              downloadBytesPerSecond: 6_100_000,
              uploadBytesPerSecond: 2_150_000,
              host: "speed.cloudflare.com",
              finishedAt: DATA.now,
            },
          });
        }, 3400);
      });
    },
    is_monitoring_paused: function () { return false; },
    set_monitoring_paused: function (args) { return Boolean(args && args.paused); },
    // The router connection the settings card edits. The stub keeps whatever
    // was "saved" in this page session, so a save is visibly reflected without
    // pretending to touch a filesystem or a Keychain.
    router_connection_get: function () {
      return routerConnection;
    },
    router_connection_save: function (args) {
      routerConnection = {
        source: "appSettings",
        host: args.host,
        port: args.port,
        username: args.username,
        hasPassword:
          Boolean(args.password && args.password.length > 0) ||
          routerConnection.hasPassword,
        willRead:
          (Boolean(args.password && args.password.length > 0) ||
            routerConnection.hasPassword) &&
          String(args.username || "").trim().length > 0,
        passwordMissing: false,
      };
      return routerConnection;
    },
    router_connection_clear: function () {
      routerConnection = {
        source: null,
        host: null,
        port: null,
        username: null,
        hasPassword: false,
        willRead: false,
        passwordMissing: false,
      };
      return routerConnection;
    },
    is_fullscreen: function () { return { fullscreen: false, mode: "windowed" }; },
    toggle_fullscreen: function () { return { fullscreen: false, mode: "windowed" }; },
    leave_fullscreen_mode: function () { return { fullscreen: false, mode: "windowed" }; },
    show_main_window: function () { return null; },
    // The event plugin. The frontend's real listener command registers its
    // callback, which the harness delivers to by the same registry shape the
    // backend uses: the callback id from transformCallback, delivered as a
    // full event object.
    "plugin:event|listen": function (args) {
      var handlers = events[args.event] || (events[args.event] = []);
      handlers.push(args.handler);
      var unlistenId = unlistenCounter;
      unlistenCounter = unlistenCounter + 1;
      unlistenIndex[unlistenId] = args.event + "|" + args.handler;
      return unlistenId;
    },
    "plugin:event|unlisten": function (args) {
      var key = args.event + "|" + args.eventId;
      var parts = unlistenIndex[key] ? unlistenIndex[key].split("|") : null;
      if (parts) {
        var handlers = events[parts[0]];
        if (handlers) {
          var at = handlers.indexOf(Number(parts[1]));
          if (at >= 0) handlers.splice(at, 1);
        }
        delete unlistenIndex[key];
      }
      return null;
    },
  };

  // The event registry the plugin handlers above work against, and the
  // counter the callback ids come from.
  var events = {};
  var callbackCounter = 1;
  var unlistenCounter = 1000;
  var unlistenIndex = {};

  window.__TAURI_INTERNALS__ = {
    // The window metadata, which the window API reads to work out which window
    // it is talking about.
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main" },
    },
    invoke: function (cmd, args) {
      var handler = RESPONSES[cmd];
      if (!handler) return Promise.reject(new Error("no stub for " + cmd));
      return Promise.resolve(handler(args));
    },
    transformCallback: function (callback, once) {
      // The shape the real runtime keeps: the callback lives on the window
      // under an id, and the backend delivers to it by calling that property
      // with the full event object.
      var id = callbackCounter;
      callbackCounter = callbackCounter + 1;
      var prop = "_" + id;
      window[prop] = function (event) {
        callback(event);
        if (once) delete window[prop];
      };
      return id;
    },
    convertFileSrc: function (path) { return path; },
    unregisterCallback: function () {},
    plugins: {},
    // Delivers an event to the listeners registered through the command
    // above, exactly as the backend would.
    emit: function (event, payload) {
      var handlers = events[event] || [];
      handlers.forEach(function (handlerId) {
        var delivered = window["_" + handlerId];
        if (typeof delivered === "function") {
          delivered({ event: event, id: handlerId, payload: payload });
        }
      });
    },
  };

  var wanted = new URLSearchParams(window.location.search).get("click");
  if (wanted) {
    window.setTimeout(function () {
      var button = document.querySelector('[title="' + wanted + '"]');
      if (button) button.click();
    }, 1200);
  }

  // A "run" parameter presses the primary action, so a screen that needs one
  // press can be looked at in its finished state.
  if (new URLSearchParams(window.location.search).has("run")) {
    window.setTimeout(function () {
      var button = Array.from(document.querySelectorAll("button")).find(
        function (candidate) {
          return /Run test/.test(candidate.textContent || "");
        },
      );
      if (button) button.click();
    }, 2200);
  }

  // A clock format can be set before the app reads it, so the effect of the
  // setting can be seen without clicking through to it.
  var clock = new URLSearchParams(window.location.search).get("clock");
  if (clock) {
    try {
      window.localStorage.setItem("fibrex.clockFormat", clock);
    } catch (error) {
      // Storage may be unavailable. The clock then follows the system, which is
      // still a valid state to look at.
    }
  }

  // A theme can be set the same way, so the appearance modes can be captured
  // rather than guessed at. "system" resolves from the browser's own setting.
  var theme = new URLSearchParams(window.location.search).get("theme");
  if (theme) {
    try {
      window.localStorage.setItem("fibrex.theme.appearance", theme);
    } catch (error) {
      // Same reasoning: the appearance follows the system instead.
    }
  }

  var wanted = new URLSearchParams(window.location.search).get("click");
  if (wanted) {
    window.setTimeout(function () {
      var button = document.querySelector('[title="' + wanted + '"]');
      if (button) button.click();
    }, 1200);
  }

  // Headless Chrome will not make a window narrower than 500 pixels, so a
  // "width" parameter narrows the page itself. The panel really is 340 points
  // wide, and a layout that only works at 500 is a layout that is broken in
  // the app.
  var width = new URLSearchParams(window.location.search).get("width");
  if (width) {
    document.write(
      '<style>html,body{width:' + width + 'px !important;}</style>',
    );
  }

  // A "wide" parameter reports the elements that are wider than the window,
  // which is how a layout that overflows gets found rather than guessed at.
  if (new URLSearchParams(window.location.search).has("wide")) {    window.setTimeout(function () {
      var limit = document.documentElement.clientWidth;
      var wide = [];
      document.querySelectorAll("*").forEach(function (element) {
        var box = element.getBoundingClientRect();
        if (box.width > limit + 1) {
          wide.push(
            element.tagName.toLowerCase() +
              "." +
              String(element.className).split(" ").slice(0, 4).join(".") +
              " = " +
              Math.round(box.width),
          );
        }
      });
      console.log("WIDE " + limit + " :: " + wide.slice(0, 12).join(" | "));
    }, 2000);
  }
})();
</script>`;

async function serve() {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url, `http://localhost:${PORT}`);
    let path = normalize(url.pathname);
    if (path === "/" || path.endsWith("/")) {
      path += "index.html";
    }

    const file = join(ROOT, path);
    try {
      let body = await readFile(file);
      if (file.endsWith(".html")) {
        // The stub has to be in the head, before the module that calls it.
        const text = body.toString("utf8");
        body = Buffer.from(text.replace("</head>", `${STUB}\n</head>`));
      }
      response.writeHead(200, {
        "Content-Type": TYPES[extname(file)] ?? "application/octet-stream",
      });
      response.end(body);
    } catch {
      response.writeHead(404);
      response.end("not found");
    }
  });

  server.listen(PORT, () => {
    console.log(`harness on http://localhost:${PORT}`);
  });
}

void serve();
