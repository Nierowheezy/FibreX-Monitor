//! The macOS window's own controls.
//!
//! The green traffic-light button is macOS's fullscreen control, and it is the
//! one a person reaches for. Left alone it zooms the window, which fills the
//! screen but leaves the menu bar exactly where it was, so it would be a
//! button that looks like a fullscreen button and is not one.
//!
//! Pointing it at the window's own `toggleFullScreen:` action makes it do what
//! it looks like it does. The window already implements that method, so
//! nothing has to be reimplemented and no new class is needed: the button
//! simply sends its action to the window.

use std::ffi::CStr;

/// The name of the method the button is pointed at.
///
/// `toggleFullScreen:` is AppKit's own method on `NSWindow`, and it takes the
/// sender as its argument, which is exactly the shape a button action has.
const FULLSCREEN_ACTION: &CStr = c"toggleFullScreen:";

/// Makes the green button enter and leave fullscreen instead of zooming.
///
/// Called once, when the window exists. Failing is not fatal: the button then
/// keeps macOS's default behaviour, which is a zoom, and the app's own
/// fullscreen command still works.
pub fn make_green_button_fullscreen<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<(), String> {
    let pointer = window.ns_window().map_err(|error| error.to_string())?;

    if pointer.is_null() {
        return Err("the window has no native handle".to_string());
    }

    // SAFETY: the pointer comes from the window's own native handle, and
    // `NSWindow` is the type that handle refers to. The window outlives this
    // call, so the reference is only read here and never stored.
    let ns_window = unsafe { &*(pointer as *const objc2_app_kit::NSWindow) };

    let Some(green) = ns_window.standardWindowButton(objc2_app_kit::NSWindowButton::ZoomButton)
    else {
        return Err("the window has no green button".to_string());
    };

    let action = objc2::runtime::Sel::register(FULLSCREEN_ACTION);

    // SAFETY: the target is the window itself, which lives at least as long
    // as the button, and the action is AppKit's own method on that window. The
    // sender the button passes is the button, which is what the method
    // expects.
    unsafe {
        green.setTarget(Some(ns_window));
        green.setAction(Some(action));
    }

    Ok(())
}
