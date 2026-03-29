//! Direct CGEventTap implementation that avoids rdev's TSMGetInputSourceProperty crash on macOS 26.
//!
//! rdev 0.5 calls TSMGetInputSourceProperty on a background thread to translate keycodes to
//! strings. macOS 26 added a strict assertion that this HIToolbox API must run on the main
//! dispatch queue, causing a SIGTRAP crash. Since we only need virtual keycodes (not string
//! translations), we bypass rdev entirely for grab/listen and map CGKeyCode → key ID directly.

use cocoa::base::{id, nil};
use cocoa::foundation::NSAutoreleasePool;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, EventField};
use std::os::raw::c_void;

// ---------------------------------------------------------------------------
// FFI declarations (same as rdev uses internally)
// ---------------------------------------------------------------------------

type CFMachPortRef = *const c_void;
type CFIndex = u64;
type CFAllocatorRef = id;
type CFRunLoopSourceRef = id;
type CFRunLoopRef = id;
type CFRunLoopMode = id;
type CGEventTapProxy = id;

const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;

#[repr(u32)]
enum CGEventTapOption {
    Default = 0,
    ListenOnly = 1,
}

/// Mask for keyboard events only (KeyDown + KeyUp + FlagsChanged).
const KEYBOARD_EVENT_MASK: u64 = (1 << CGEventType::KeyDown as u64)
    | (1 << CGEventType::KeyUp as u64)
    | (1 << CGEventType::FlagsChanged as u64);

type RawCallback = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    _type: CGEventType,
    cg_event: CGEvent,
    user_info: *mut c_void,
) -> CGEvent;

extern "C" {
    fn CGEventTapCreate(
        tap: CGEventTapLocation,
        place: u32,
        options: CGEventTapOption,
        eventsOfInterest: u64,
        callback: RawCallback,
        user_info: id,
    ) -> CFMachPortRef;
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        tap: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFRunLoopMode);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);

    static kCFRunLoopCommonModes: CFRunLoopMode;
}

// ---------------------------------------------------------------------------
// Key event types produced by our tap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum KeyAction {
    Press(String),   // key ID like "KeyA", "ControlLeft"
    Release(String), // key ID
}

// ---------------------------------------------------------------------------
// CGKeyCode → key ID mapping (pure lookup, no TSM/HIToolbox calls)
// ---------------------------------------------------------------------------

fn keycode_to_id(code: u16) -> String {
    match code {
        0 => "KeyA".into(),
        1 => "KeyS".into(),
        2 => "KeyD".into(),
        3 => "KeyF".into(),
        4 => "KeyH".into(),
        5 => "KeyG".into(),
        6 => "KeyZ".into(),
        7 => "KeyX".into(),
        8 => "KeyC".into(),
        9 => "KeyV".into(),
        11 => "KeyB".into(),
        12 => "KeyQ".into(),
        13 => "KeyW".into(),
        14 => "KeyE".into(),
        15 => "KeyR".into(),
        16 => "KeyY".into(),
        17 => "KeyT".into(),
        18 => "Digit1".into(),
        19 => "Digit2".into(),
        20 => "Digit3".into(),
        21 => "Digit4".into(),
        22 => "Digit6".into(),
        23 => "Digit5".into(),
        24 => "Equal".into(),
        25 => "Digit9".into(),
        26 => "Digit7".into(),
        27 => "Minus".into(),
        28 => "Digit8".into(),
        29 => "Digit0".into(),
        30 => "BracketRight".into(),
        31 => "KeyO".into(),
        32 => "KeyU".into(),
        33 => "BracketLeft".into(),
        34 => "KeyI".into(),
        35 => "KeyP".into(),
        36 => "Enter".into(),
        37 => "KeyL".into(),
        38 => "KeyJ".into(),
        39 => "Quote".into(),
        40 => "KeyK".into(),
        41 => "Semicolon".into(),
        42 => "Backslash".into(),
        43 => "Comma".into(),
        44 => "Slash".into(),
        45 => "KeyN".into(),
        46 => "KeyM".into(),
        47 => "Period".into(),
        48 => "Tab".into(),
        49 => "Space".into(),
        50 => "Backquote".into(),
        51 => "Backspace".into(),
        53 => "Escape".into(),
        54 => "MetaRight".into(),
        55 => "MetaLeft".into(),
        56 => "ShiftLeft".into(),
        57 => "CapsLock".into(),
        58 => "Alt".into(),
        59 => "ControlLeft".into(),
        60 => "ShiftRight".into(),
        61 => "AltGr".into(),
        62 => "ControlRight".into(),
        63 => "Fn".into(),
        96 => "F5".into(),
        97 => "F6".into(),
        98 => "F7".into(),
        99 => "F3".into(),
        100 => "F8".into(),
        101 => "F9".into(),
        103 => "F11".into(),
        109 => "F10".into(),
        111 => "F12".into(),
        118 => "F4".into(),
        120 => "F2".into(),
        122 => "F1".into(),
        123 => "ArrowLeft".into(),
        124 => "ArrowRight".into(),
        125 => "ArrowDown".into(),
        126 => "ArrowUp".into(),
        // Numpad
        65 => "NumpadDecimal".into(),
        67 => "NumpadMultiply".into(),
        69 => "NumpadAdd".into(),
        71 => "NumLock".into(),
        75 => "NumpadDivide".into(),
        76 => "NumpadEnter".into(),
        78 => "NumpadSubtract".into(),
        82 => "Numpad0".into(),
        83 => "Numpad1".into(),
        84 => "Numpad2".into(),
        85 => "Numpad3".into(),
        86 => "Numpad4".into(),
        87 => "Numpad5".into(),
        88 => "Numpad6".into(),
        89 => "Numpad7".into(),
        91 => "Numpad8".into(),
        92 => "Numpad9".into(),
        other => format!("Unknown_{other}"),
    }
}

// ---------------------------------------------------------------------------
// Global callback storage
// ---------------------------------------------------------------------------

static mut GRAB_CALLBACK: Option<Box<dyn FnMut(KeyAction) -> bool>> = None;
static mut LISTEN_CALLBACK: Option<Box<dyn FnMut(KeyAction)>> = None;
static mut LAST_FLAGS: CGEventFlags = CGEventFlags::CGEventFlagNull;
static mut CURRENT_RUN_LOOP: Option<CFRunLoopRef> = None;

// ---------------------------------------------------------------------------
// grab() — event tap that can suppress keys (requires Accessibility)
// ---------------------------------------------------------------------------

unsafe extern "C" fn grab_raw_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    cg_event: CGEvent,
    _user_info: *mut c_void,
) -> CGEvent {
    if let Some(action) = convert_event(event_type, &cg_event) {
        if let Some(callback) = &mut GRAB_CALLBACK {
            if !callback(action) {
                cg_event.set_type(CGEventType::Null);
            }
        }
    }
    cg_event
}

/// Start an event tap that can suppress keyboard events.
/// `callback` receives a KeyAction and returns `true` to pass through, `false` to suppress.
/// Blocks until `stop_run_loop()` is called.
pub fn grab<F>(callback: F) -> Result<(), String>
where
    F: FnMut(KeyAction) -> bool + 'static,
{
    unsafe {
        GRAB_CALLBACK = Some(Box::new(callback));
        let _pool = NSAutoreleasePool::new(nil);
        let tap = CGEventTapCreate(
            CGEventTapLocation::HID,
            K_CG_HEAD_INSERT_EVENT_TAP,
            CGEventTapOption::Default,
            KEYBOARD_EVENT_MASK,
            grab_raw_callback,
            nil,
        );
        if tap.is_null() {
            return Err("CGEventTapCreate failed — Accessibility permission required".into());
        }
        let source = CFMachPortCreateRunLoopSource(nil, tap, 0);
        if source.is_null() {
            return Err("CFMachPortCreateRunLoopSource failed".into());
        }
        let run_loop = CFRunLoopGetCurrent();
        CURRENT_RUN_LOOP = Some(run_loop);
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
        CURRENT_RUN_LOOP = None;
        GRAB_CALLBACK = None;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// listen() — observe-only event tap (no suppression)
// ---------------------------------------------------------------------------

unsafe extern "C" fn listen_raw_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    cg_event: CGEvent,
    _user_info: *mut c_void,
) -> CGEvent {
    if let Some(action) = convert_event(event_type, &cg_event) {
        if let Some(callback) = &mut LISTEN_CALLBACK {
            callback(action);
        }
    }
    cg_event
}

/// Start an observe-only event tap. Blocks until `stop_run_loop()` is called.
pub fn listen<F>(callback: F) -> Result<(), String>
where
    F: FnMut(KeyAction) + 'static,
{
    unsafe {
        LISTEN_CALLBACK = Some(Box::new(callback));
        let _pool = NSAutoreleasePool::new(nil);
        let tap = CGEventTapCreate(
            CGEventTapLocation::HID,
            K_CG_HEAD_INSERT_EVENT_TAP,
            CGEventTapOption::ListenOnly,
            KEYBOARD_EVENT_MASK,
            listen_raw_callback,
            nil,
        );
        if tap.is_null() {
            return Err("CGEventTapCreate failed (listen)".into());
        }
        let source = CFMachPortCreateRunLoopSource(nil, tap, 0);
        if source.is_null() {
            return Err("CFMachPortCreateRunLoopSource failed".into());
        }
        let run_loop = CFRunLoopGetCurrent();
        CURRENT_RUN_LOOP = Some(run_loop);
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
        CURRENT_RUN_LOOP = None;
        LISTEN_CALLBACK = None;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// stop_run_loop() — break out of CFRunLoopRun from another thread
// ---------------------------------------------------------------------------

pub fn stop_run_loop() {
    unsafe {
        if let Some(rl) = CURRENT_RUN_LOOP {
            CFRunLoopStop(rl);
        }
    }
}

// ---------------------------------------------------------------------------
// convert_event — CGEvent → KeyAction without any TSM calls
// ---------------------------------------------------------------------------

unsafe fn convert_event(event_type: CGEventType, cg_event: &CGEvent) -> Option<KeyAction> {
    match event_type {
        CGEventType::KeyDown => {
            let code = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
            Some(KeyAction::Press(keycode_to_id(code as u16)))
        }
        CGEventType::KeyUp => {
            let code = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
            Some(KeyAction::Release(keycode_to_id(code as u16)))
        }
        CGEventType::FlagsChanged => {
            let code = cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
            let flags = cg_event.get_flags();
            let action = if flags < LAST_FLAGS {
                KeyAction::Release(keycode_to_id(code as u16))
            } else {
                KeyAction::Press(keycode_to_id(code as u16))
            };
            LAST_FLAGS = flags;
            Some(action)
        }
        _ => None,
    }
}
