#!/usr/bin/env node
/**
 * A read-only probe of the router, for settling endpoint questions.
 *
 * This is the tool behind the endpoint observations in
 * `buildflow/designs/router-integration-findings.md`. It signs in with the
 * same two-token and session-cookie protocol the app uses, then asks a list
 * of pages how they answer, recording the status, size and body of each.
 *
 * Everything here is a GET or a read-shaped POST that the router's own web UI
 * makes. Nothing is written, nothing is configured, and no credential is ever
 * printed: they come from the environment through `scripts/with-env.mjs`.
 *
 * Usage, from the repository root:
 *
 *     node scripts/with-env.mjs node scripts/probe-router.mjs
 *
 * The machine must be on the router's LAN for this to work at all.
 */

import https from "node:https";

const host = process.env.FIBREX_ROUTER_HOST || "192.168.100.1";
const port = Number(process.env.FIBREX_ROUTER_PORT || 80);
const username = process.env.FIBREX_ROUTER_USERNAME;
const password = process.env.FIBREX_ROUTER_PASSWORD;

if (!username || !password) {
  console.error("no router credentials in the environment");
  process.exit(1);
}

// The router serves its UI over HTTPS on port 80 with a self-signed
// certificate, so every request must accept it.
const agent = new https.Agent({ rejectUnauthorized: false, keepAlive: true });

/** The session cookie, captured from whatever response sets it. */
let cookie = "";

/**
 * One request.
 *
 * The session is subtle: the cookie is not always granted on the login
 * response itself, so it is picked up from whatever response carries one,
 * which is what a browser does.
 */
async function request(method, path, { tokenInQuery = false, body } = {}) {
  let search = "";
  if (tokenInQuery) {
    const token = await freshToken();
    search = `?x.X_HW_Token=${token}`;
  }

  return new Promise((resolve, reject) => {
    const req = https.request(
      {
        agent,
        host,
        port,
        path: path + search,
        method,
        headers: {
          "User-Agent": "FibreX-probe/1",
          Accept: "*/*",
          ...(cookie ? { Cookie: cookie } : {}),
          ...(body ? { "Content-Type": "application/x-www-form-urlencoded" } : {}),
        },
      },
      (response) => {
        const chunks = [];
        response.on("data", (chunk) => chunks.push(chunk));
        response.on("end", () => {
          captureCookie(response.headers["set-cookie"]);
          resolve({ status: response.statusCode, body: Buffer.concat(chunks) });
        });
      },
    );
    req.on("error", reject);
    req.setTimeout(10_000, () => req.destroy(new Error("timed out")));
    if (body) req.write(body);
    req.end();
  });
}

/** Keeps the session cookie from a `set-cookie` header, if there is one. */
function captureCookie(headers) {
  if (!headers) return;
  const match = headers.join("; ").match(/CookieHttps=[^;]+/);
  if (match) cookie = match[0];
}

/** Reads a single-use token, stripping the BOM the router prefixes it with. */
async function freshToken(path = "/html/ssmp/common/GetRandToken.asp") {
  const response = await request("GET", path);
  return response.body.toString("utf8").replace(/^\uFEFF/, "").trim();
}

/** One authenticated POST with a fresh token in the body. */
async function postWithToken(path, extraForm) {
  const token = await freshToken();
  const form = new URLSearchParams({
    "x.X_HW_Token": token,
    ...(extraForm || {}),
  }).toString();
  return request("POST", path, { body: form });
}

/** One authenticated GET with a fresh token in the query string. */
async function getWithToken(path) {
  return request("GET", path, { tokenInQuery: true });
}

function describe(response) {
  if (!response) return "request failed";
  const text = response.body.toString("utf8");
  if (response.status !== 200) return `${response.status}, ${response.body.length} bytes`;
  const stub = text.startsWith("<!DOCTYPE html") || text.includes("Waiting");
  return `${response.status}, ${response.body.length} bytes${stub ? " (page stub)" : ""}, ${JSON.stringify(text.slice(0, 160).replace(/\ufeff/g, ""))}`;
}

async function main() {
  // Sign in.
  const loginToken = await freshToken("/html/ssmp/common/getRandString.asp");
  const login = await request("POST", "/login.cgi", {
    body: new URLSearchParams({
      UserName: username,
      PassWord: Buffer.from(password).toString("base64"),
      Language: "english",
      "x.X_HW_Token": loginToken,
    }).toString(),
  });

  if (!login.body.toString("utf8").includes("top.location.replace")) {
    console.error("the router did not accept the configured password");
    process.exit(1);
  }
  console.log("signed in. cookie after login:", cookie || "(none)");

  // Complete the session the way the browser does, by loading the root page.
  const root = await request("GET", "/");
  console.log("GET /", describe(root), "cookie now:", cookie || "(none)");

  // The endpoints the findings document asks about are named here, and both
  // request styles are tried for each, because this router's pages differ in
  // which style they accept.
  const candidates = process.argv.slice(2);
  for (const path of candidates.length ? candidates : ["/html/ssmp/deviceinfo/deviceinfo.asp"]) {
    const post = await postWithToken(path).catch(() => null);
    const get = await getWithToken(path).catch(() => null);
    console.log(`${path}\n   POST: ${describe(post)}\n   GET : ${describe(get)}`);

    // When the page answers with real content, print its constructor calls so
    // the field positions can be read straight from the output.
    const bodies = [post, get].filter(
      (response) => response && response.status === 200 && response.body.length > 700,
    );
    for (const response of bodies) {
      const text = response.body.toString("utf8").replace(/\ufeff/g, "");
      const constructors = new Set();
      for (const match of text.matchAll(/function\s*\(\s*\)\s*\{\s*return\s+new\s+(\w+)/g)) {
        constructors.add(match[1]);
      }
      if (constructors.size) console.log("   constructors:", [...constructors].join(", "));
    }
  }
}

main().catch((error) => {
  console.error("probe error:", error);
  process.exit(1);
});
