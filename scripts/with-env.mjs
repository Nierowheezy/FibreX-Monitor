#!/usr/bin/env node
/**
 * Runs a command with a local `.env` loaded into its environment.
 *
 * The Rust backend reads the router credentials from its own process
 * environment and nowhere else, so a plain `npm run tauri dev` would find none.
 * This wrapper is what bridges the two, and it lives in the launcher rather than
 * in the app: the shipped binary contains no code that reads a credential file.
 *
 * Only the keys a router configuration needs are passed through. Everything else
 * in `.env` is ignored, so a future variable cannot leak into the app just by
 * being written down.
 *
 * A missing `.env` is not an error. The app runs without a router exactly as it
 * does when the router is unreachable, which is a supported state.
 */

import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const ENV_PATH = resolve(ROOT, ".env");

/** The only keys carried from the file into the app. */
const KEYS = [
  "FIBREX_ROUTER_HOST",
  "FIBREX_ROUTER_PORT",
  "FIBREX_ROUTER_USERNAME",
  "FIBREX_ROUTER_PASSWORD",
];

/**
 * Parses a dotenv file into the four keys above.
 *
 * Deliberately a small parser rather than a dependency: the format used here is
 * one `KEY=value` per line, so nothing more is needed, and a dependency for four
 * lines would be the larger risk.
 */
function readEnvFile(path) {
  const values = {};

  for (const rawLine of readFileSync(path, "utf8").split("\n")) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) continue;

    const separator = line.indexOf("=");
    if (separator === -1) continue;

    const key = line.slice(0, separator).trim();
    if (!KEYS.includes(key)) continue;

    let value = line.slice(separator + 1).trim();

    // Strip one layer of matching quotes, so a value that needs surrounding
    // whitespace can be written quoted.
    if (
      value.length >= 2 &&
      ((value.startsWith("'") && value.endsWith("'")) ||
        (value.startsWith('"') && value.endsWith('"')))
    ) {
      value = value.slice(1, -1);
    }

    // Trim an unquoted value's trailing spaces. The `.env` in this repository
    // has a trailing space after a value, and a password read with it would fail
    // sign-in in a way that looks like a wrong password.
    values[key] = value.trim();
  }

  return values;
}

const [command, ...args] = process.argv.slice(2);
if (command === undefined) {
  console.error("usage: node scripts/with-env.mjs <command> [args...]");
  process.exit(2);
}

const fromFile = existsSync(ENV_PATH) ? readEnvFile(ENV_PATH) : {};

// A real environment variable always wins over the file, so a one-off override
// on the command line needs no edit to `.env`.
const environment = { ...process.env };
for (const key of KEYS) {
  if (environment[key] === undefined && fromFile[key] !== undefined) {
    environment[key] = fromFile[key];
  }
}

const configured = KEYS.filter((key) => environment[key] !== undefined).length;
console.log(
  existsSync(ENV_PATH)
    ? `loaded .env, ${configured} router variable(s) present`
    : "no .env found, running without router credentials",
);

const child = spawn(command, args, {
  stdio: "inherit",
  env: environment,
  cwd: ROOT,
});

child.on("exit", (code, signal) => {
  if (signal !== null) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
