//! Legacy names are accepted ONLY at the persistence boundary.
use crate::hotkey::{Binding, Key};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
pub fn migrate(value: &mut serde_json::Value) {
    if value.get("holdBinding").is_some() || value.get("toggleBinding").is_some() {
        return;
    }
    let mut hold = crate::hotkey::default_hold();
    let mut toggle = crate::hotkey::default_toggle();
    if let Some(old) = value.get("hotkey").and_then(|v| v.as_str()) {
        let binding = if old.is_empty() {
            Some(None)
        } else {
            old.split('+')
                .map(|s| {
                    let s = s.trim();
                    if s == "NumpadEnter" {
                        return Some(Key {
                            scan: 0x1c,
                            extended: true,
                            vk: VK_RETURN,
                        });
                    }
                    legacy_vk(s).map(Key::from_vk)
                })
                .collect::<Option<Vec<_>>>()
                .map(|keys| Some(Binding::new(keys, true)))
        };
        if binding.is_none() {
            log::warn!("Unrecognized legacy hotkey; restoring the default binding");
        }
        if let Some(binding) = binding {
            if value.get("hotkeyMode").and_then(|v| v.as_str()) == Some("tap") {
                toggle = binding;
            } else {
                hold = binding;
            }
        }
    }
    if hold
        .as_ref()
        .zip(toggle.as_ref())
        .is_some_and(|(a, b)| a.same_keys(b))
    {
        toggle = None;
    }
    value["holdBinding"] = serde_json::to_value(hold).unwrap();
    value["toggleBinding"] = serde_json::to_value(toggle).unwrap();
    if let Some(obj) = value.as_object_mut() {
        obj.remove("hotkey");
        obj.remove("hotkeyMode");
    }
}
fn legacy_vk(id: &str) -> Option<u16> {
    // Accept the old VK spellings at migration only, never during capture/matching.
    if let Some(name) = id.strip_prefix("VK_") {
        let vk = match name {
            "LCONTROL" => VK_LCONTROL,
            "RCONTROL" => VK_RCONTROL,
            "LSHIFT" => VK_LSHIFT,
            "RSHIFT" => VK_RSHIFT,
            "LMENU" => VK_LMENU,
            "RMENU" => VK_RMENU,
            "LWIN" => VK_LWIN,
            "RWIN" => VK_RWIN,
            "SPACE" => VK_SPACE,
            "RETURN" => VK_RETURN,
            "TAB" => VK_TAB,
            "ESCAPE" => VK_ESCAPE,
            "DELETE" => VK_DELETE,
            "MEDIA_PLAY_PAUSE" => VK_MEDIA_PLAY_PAUSE,
            "MEDIA_NEXT_TRACK" => VK_MEDIA_NEXT_TRACK,
            "MEDIA_PREV_TRACK" => VK_MEDIA_PREV_TRACK,
            "MEDIA_STOP" => VK_MEDIA_STOP,
            "VOLUME_UP" => VK_VOLUME_UP,
            "VOLUME_DOWN" => VK_VOLUME_DOWN,
            "VOLUME_MUTE" => VK_VOLUME_MUTE,
            "OEM_1" => VK_OEM_1,
            "OEM_2" => VK_OEM_2,
            "OEM_3" => VK_OEM_3,
            "OEM_4" => VK_OEM_4,
            "OEM_5" => VK_OEM_5,
            "OEM_6" => VK_OEM_6,
            "OEM_7" => VK_OEM_7,
            "OEM_102" => VK_OEM_102,
            _ => {
                return legacy_vk(name).or_else(|| {
                    u16::from_str_radix(name, 16)
                        .ok()
                        .filter(|v| *v > 0 && *v <= 255)
                })
            }
        };
        return Some(vk);
    }
    let vk: u16 = match id {
        "Ctrl" | "Control" | "CmdOrCtrl" | "ControlLeft" => VK_LCONTROL,
        "ControlRight" => VK_RCONTROL,
        "Shift" | "ShiftLeft" => VK_LSHIFT,
        "ShiftRight" => VK_RSHIFT,
        "Option" | "AltLeft" | "Alt" => VK_LMENU,
        "AltRight" | "AltGr" => VK_RMENU,
        "Win" | "Cmd" | "Command" | "Super" | "Meta" | "MetaLeft" => VK_LWIN,
        "MetaRight" => VK_RWIN,
        "Space" => VK_SPACE,
        "Return" | "Enter" => VK_RETURN,
        "Tab" => VK_TAB,
        "Esc" | "Escape" => VK_ESCAPE,
        "Backspace" => VK_BACK,
        "Delete" => VK_DELETE,
        "Insert" => VK_INSERT,
        "Home" => VK_HOME,
        "End" => VK_END,
        "PageUp" => VK_PRIOR,
        "PageDown" => VK_NEXT,
        "Up" | "UpArrow" | "ArrowUp" => VK_UP,
        "Down" | "DownArrow" | "ArrowDown" => VK_DOWN,
        "Left" | "LeftArrow" | "ArrowLeft" => VK_LEFT,
        "Right" | "RightArrow" | "ArrowRight" => VK_RIGHT,
        "CapsLock" => VK_CAPITAL,
        "F1" => VK_F1,
        "F2" => VK_F2,
        "F3" => VK_F3,
        "F4" => VK_F4,
        "F5" => VK_F5,
        "F6" => VK_F6,
        "F7" => VK_F7,
        "F8" => VK_F8,
        "F9" => VK_F9,
        "F10" => VK_F10,
        "F11" => VK_F11,
        "F12" => VK_F12,
        "BackQuote" | "Backquote" => VK_OEM_3,
        "Minus" => VK_OEM_MINUS,
        "Equal" => VK_OEM_PLUS,
        "LeftBracket" | "BracketLeft" => VK_OEM_4,
        "RightBracket" | "BracketRight" => VK_OEM_6,
        "SemiColon" | "Semicolon" => VK_OEM_1,
        "Quote" => VK_OEM_7,
        "BackSlash" | "Backslash" => VK_OEM_5,
        "Comma" => VK_OEM_COMMA,
        "Dot" | "Period" => VK_OEM_PERIOD,
        "Slash" => VK_OEM_2,
        "IntlBackslash" => VK_OEM_102,
        "NumLock" => VK_NUMLOCK,
        "ScrollLock" => VK_SCROLL,
        "PrintScreen" => VK_SNAPSHOT,
        "Pause" => VK_PAUSE,
        "Numpad0" => VK_NUMPAD0,
        "Numpad1" => VK_NUMPAD1,
        "Numpad2" => VK_NUMPAD2,
        "Numpad3" => VK_NUMPAD3,
        "Numpad4" => VK_NUMPAD4,
        "Numpad5" => VK_NUMPAD5,
        "Numpad6" => VK_NUMPAD6,
        "Numpad7" => VK_NUMPAD7,
        "Numpad8" => VK_NUMPAD8,
        "Numpad9" => VK_NUMPAD9,
        "NumpadAdd" => VK_ADD,
        "NumpadSubtract" => VK_SUBTRACT,
        "NumpadMultiply" => VK_MULTIPLY,
        "NumpadDivide" => VK_DIVIDE,
        "NumpadDecimal" => VK_DECIMAL,
        s if s.starts_with("Key") && s.len() == 4 => {
            let c = s.as_bytes()[3];
            if c.is_ascii_uppercase() {
                c as u16
            } else {
                return None;
            }
        }
        s if s.starts_with("Digit") && s.len() == 6 => {
            let c = s.as_bytes()[5];
            if c.is_ascii_digit() {
                c as u16
            } else {
                return None;
            }
        }
        s if s.starts_with('F') => {
            let n = s[1..].parse::<u16>().ok()?;
            if !(1..=24).contains(&n) {
                return None;
            }
            VK_F1 + n - 1
        }
        s if s.starts_with("Unknown_") => s[8..].parse::<u16>().ok()?,
        s if s.starts_with("VK_") => u16::from_str_radix(&s[3..], 16).ok()?,
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => {
            s.to_ascii_uppercase().as_bytes()[0] as u16
        }
        _ => return None,
    };
    Some(vk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn migrated(mut v: serde_json::Value) -> (Option<Binding>, Option<Binding>) {
        migrate(&mut v);
        (
            serde_json::from_value(v["holdBinding"].clone()).unwrap(),
            serde_json::from_value(v["toggleBinding"].clone()).unwrap(),
        )
    }
    #[test]
    fn defaults_and_explicit_disabled_survive() {
        let (hold, toggle) = migrated(json!({}));
        assert_eq!(hold, crate::hotkey::default_hold());
        assert_eq!(toggle, crate::hotkey::default_toggle());
        let (hold, toggle) = migrated(json!({"holdBinding":null,"toggleBinding":null}));
        assert!(hold.is_none() && toggle.is_none());
    }
    #[test]
    fn legacy_hold_and_toggle_keep_their_mode() {
        let (hold, _) = migrated(json!({"hotkey":"ControlRight","hotkeyMode":"hold"}));
        assert_eq!(hold.unwrap().keys, vec![Key::from_vk(VK_RCONTROL)]);
        let (hold, toggle) = migrated(json!({"hotkey":"Ctrl+Shift+F24","hotkeyMode":"tap"}));
        assert_eq!(hold, crate::hotkey::default_hold());
        assert_eq!(toggle.unwrap().keys.len(), 3);
    }
    #[test]
    fn migration_is_idempotent_and_preserves_suppression() {
        let mut v = json!({"hotkey":"KeyA"});
        migrate(&mut v);
        let once = v.clone();
        migrate(&mut v);
        assert_eq!(once, v);
        v["holdBinding"]["suppress"] = json!(false);
        migrate(&mut v);
        assert_eq!(v["holdBinding"]["suppress"], false);
    }
    #[test]
    fn unknown_names_fall_back_and_numpad_enter_keeps_extended_flag() {
        let (hold, _) = migrated(json!({"hotkey":"Fn"}));
        assert_eq!(hold, crate::hotkey::default_hold());
        let (hold, _) = migrated(json!({"hotkey":"NumpadEnter"}));
        assert!(hold.unwrap().keys[0].extended);
        let (hold, _) = migrated(json!({"hotkey":""}));
        assert!(hold.is_none());
    }
    #[test]
    fn duplicate_default_toggle_is_disabled() {
        let (_, toggle) = migrated(json!({"hotkey":"ControlLeft+MetaLeft"}));
        assert!(toggle.is_none());
    }
}

#[cfg(test)]
mod vk_migration_tests {
    use super::*;
    #[test]
    fn vk_names_and_extended_scan_codes_migrate() {
        let mut v = serde_json::json!({"hotkey":"VK_RCONTROL+VK_F24"});
        migrate(&mut v);
        let b: Binding = serde_json::from_value(v["holdBinding"].clone()).unwrap();
        assert!(b.keys.contains(&Key::from_vk(VK_RCONTROL)));
        assert!(b.keys.contains(&Key::from_vk(VK_F24)));
        assert!(
            b.keys
                .iter()
                .find(|k| k.vk == VK_RCONTROL)
                .unwrap()
                .extended
        );
        assert_eq!(legacy_vk("VK_MEDIA_PLAY_PAUSE"), Some(VK_MEDIA_PLAY_PAUSE));
        assert_eq!(legacy_vk("VK_A3"), Some(VK_RCONTROL));
    }
}
