/**
 * Whether this code is running inside the Tauri shell.
 *
 * The dashboard and the panel are both plain web pages, and a developer will
 * open either in a browser while working on styling. Tauri injects
 * `window.__TAURI_INTERNALS__` only in its own webview, so it is the honest test
 * for "is there a Rust backend behind this page".
 *
 * The distinction matters beyond convenience. Without the check, every `invoke`
 * call in a browser throws `Cannot read properties of undefined (reading
 * 'invoke')`, which reads like a broken app rather than a page opened in the
 * wrong place, and the throw is deep enough that it can unmount the whole tree.
 * The app is local-first and always runs in Tauri when shipped, so this costs
 * nothing in the packaged build.
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * The message shown when the interface is open outside the desktop shell.
 *
 * It names the actual situation and the actual fix, because "the backend did not
 * answer" would suggest a fault in a backend that is working perfectly well.
 */
export const NO_BACKEND_MESSAGE =
  "This screen needs the desktop app. Open it with `npm run tauri dev`, or the installed FibreX Monitor, so the Rust monitoring backend is available.";
