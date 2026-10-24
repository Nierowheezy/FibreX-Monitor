import { invoke } from "@tauri-apps/api/core";

import type { RouterReport } from "../../types/overview";
import type {
  RouterConnectionReport,
  RouterConnectionSave,
} from "../../types/router";

/**
 * The only place the router command is called from.
 *
 * One command rather than several, because a screen always wants the whole
 * picture at once: the snapshot if there is one, why there is not one, and
 * whether a router is configured at all. Several calls would let a screen
 * render a stale combination of those three.
 */
export function getRouterReport(): Promise<RouterReport> {
  return invoke<RouterReport>("get_router_report");
}

/**
 * The connection the user edits in Settings.
 *
 * `-get` is read-only and safe on any screen; `-save` and `-clear` change what
 * the background reader uses, and are called only from the settings card's
 * explicit actions.
 */
export function getRouterConnection(): Promise<RouterConnectionReport> {
  return invoke<RouterConnectionReport>("router_connection_get");
}

export function saveRouterConnection(
  connection: RouterConnectionSave,
): Promise<RouterConnectionReport> {
  // The fields are passed named rather than as the interface itself, because
  // the IPC layer wants a plain record and the contract stays explicit here.
  return invoke<RouterConnectionReport>("router_connection_save", {
    host: connection.host,
    port: connection.port,
    username: connection.username,
    password: connection.password ?? null,
  });
}

export function clearRouterConnection(): Promise<RouterConnectionReport> {
  return invoke<RouterConnectionReport>("router_connection_clear");
}
