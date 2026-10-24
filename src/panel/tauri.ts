/**
 * The panel's window helpers.
 *
 * `isTauri` is re-exported from the shared environment module rather than
 * defined here, so the dashboard and the panel can never disagree about what
 * "there is a backend" means.
 */
export { isTauri } from "../lib/tauri/environment";
