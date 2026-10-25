//! Physical bindings and pure matching. All live input state belongs to hotkey_input.
use serde::{Deserialize, Serialize};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Key {
    pub scan: u16,
    pub extended: bool,
    pub vk: u16,
}
// VK is metadata, except for scan-less keys (media keys, mouse side buttons).
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.scan == other.scan
            && self.extended == other.extended
            && (self.scan != 0 || self.vk == other.vk)
    }
}
impl Eq for Key {}
impl Key {
    pub fn from_vk(vk: u16) -> Self {
        if vk == VK_PAUSE {
            return Self {
                scan: 0x45,
                extended: false,
                vk,
            };
        }
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
        Self {
            scan: (scan & 0xff) as u16,
            extended: scan & 0xff00 != 0,
            vk,
        }
    }
    pub fn modifier(self) -> bool {
        matches!(
            self.vk,
            VK_LCONTROL
                | VK_RCONTROL
                | VK_LSHIFT
                | VK_RSHIFT
                | VK_LMENU
                | VK_RMENU
                | VK_LWIN
                | VK_RWIN
        )
    }
    pub fn label(self) -> String {
        if self.scan == 0 && matches!(self.vk, VK_XBUTTON1 | VK_XBUTTON2) {
            return format!("Mouse {}", if self.vk == VK_XBUTTON1 { 4 } else { 5 });
        }
        let mut buf = [0u16; 128];
        let param = ((self.scan as i32) << 16) | ((self.extended as i32) << 24);
        let len = unsafe { GetKeyNameTextW(param, buf.as_mut_ptr(), buf.len() as i32) };
        let name = if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            format!("VK {:02X}", self.vk)
        };
        // GetKeyNameText does not distinguish both Shift keys on every layout.
        match self.vk {
            VK_LCONTROL | VK_LSHIFT | VK_LMENU | VK_LWIN => {
                format!("Left {}", name.strip_prefix("Left ").unwrap_or(&name))
            }
            VK_RCONTROL | VK_RSHIFT | VK_RMENU | VK_RWIN => {
                format!("Right {}", name.strip_prefix("Right ").unwrap_or(&name))
            }
            _ => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub keys: Vec<Key>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub suppress: bool,
}
impl Binding {
    pub fn new(mut keys: Vec<Key>, suppress: bool) -> Self {
        keys.sort_by_key(|k| (!k.modifier(), k.vk, k.scan, k.extended));
        keys.dedup();
        let label = keys
            .iter()
            .map(|k| k.label())
            .collect::<Vec<_>>()
            .join(" + ");
        let suppress = suppress && !keys.iter().all(|k| k.modifier());
        Self {
            keys,
            label,
            suppress,
        }
    }
    pub fn same_keys(&self, other: &Self) -> bool {
        self.keys.len() == other.keys.len() && self.keys.iter().all(|k| other.keys.contains(k))
    }
    pub fn suppresses(&self) -> bool {
        self.suppress && self.keys.iter().any(|k| !k.modifier())
    }
}
pub fn matches(binding: &Binding, held: &[Key]) -> bool {
    !binding.keys.is_empty()
        && binding.keys.iter().all(|k| held.contains(k))
        && held
            .iter()
            .filter(|k| k.modifier())
            .all(|k| binding.keys.contains(k))
}
pub fn default_hold() -> Option<Binding> {
    Some(Binding::new(vec![Key::from_vk(VK_RCONTROL)], false))
}
pub fn default_toggle() -> Option<Binding> {
    Some(Binding::new(
        vec![Key::from_vk(VK_LCONTROL), Key::from_vk(VK_LWIN)],
        false,
    ))
}
pub fn default_cancel() -> Option<Binding> {
    Some(Binding::new(vec![Key::from_vk(VK_ESCAPE)], false))
}

/// Validates one binding against the reserved system shortcuts and against the
/// other bindings (hold, toggle and cancel must all differ).
pub fn conflict(binding: &Binding, others: &[Option<Binding>]) -> Result<(), String> {
    if binding
        .keys
        .iter()
        .any(|k| k.vk == 0 || k.vk > 255 || k.scan > 255)
    {
        return Err("Invalid physical key".into());
    }
    if binding.keys.is_empty() || binding.keys.len() > 16 {
        return Err("Choose between 1 and 16 keys".into());
    }
    if others.iter().flatten().any(|b| b.same_keys(binding)) {
        return Err("Already assigned to another shortcut".into());
    }
    let has = |vk| binding.keys.iter().any(|k| k.vk == vk);
    let win = has(VK_LWIN) || has(VK_RWIN);
    let ctrl = has(VK_LCONTROL) || has(VK_RCONTROL);
    let alt = has(VK_LMENU) || has(VK_RMENU);
    let shift = has(VK_LSHIFT) || has(VK_RSHIFT);
    if (win
        && [
            0x4c, 0x44, 0x45, 0x52, 0x49, VK_TAB, VK_SPACE, VK_UP, VK_DOWN, VK_LEFT, VK_RIGHT,
        ]
        .iter()
        .any(|v| has(*v)))
        || (alt && (has(VK_TAB) || has(VK_ESCAPE) || has(VK_F4)))
        || (ctrl && alt && has(VK_DELETE))
        || (ctrl && has(VK_ESCAPE))
        || (ctrl && shift && has(VK_ESCAPE))
    {
        return Err("This combination is a reserved system shortcut".into());
    }
    Ok(())
}
/// Validates `[hold, toggle, cancel]` together: each must be valid on its own
/// and none may equal another. Esc alone is allowed; Esc combinations with
/// Ctrl or Alt stay reserved.
pub fn validate_all(bindings: &[Option<Binding>; 3]) -> Result<(), String> {
    for (i, binding) in bindings.iter().enumerate() {
        if let Some(b) = binding {
            let others: Vec<Option<Binding>> = bindings
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, o)| o.clone())
                .collect();
            conflict(b, &others)?;
        }
    }
    Ok(())
}
pub use crate::hotkey_input::{cancel_record_binding, record_binding, start, stop, update};

#[cfg(test)]
mod tests {
    use super::*;
    fn binding(vks: &[u16]) -> Binding {
        Binding::new(vks.iter().copied().map(Key::from_vk).collect(), true)
    }
    #[test]
    fn extra_non_modifiers_do_not_block_but_other_modifiers_do() {
        let b = binding(&[VK_RCONTROL]);
        assert!(matches(
            &b,
            &[Key::from_vk(VK_RCONTROL), Key::from_vk(0x41)]
        ));
        assert!(!matches(
            &b,
            &[Key::from_vk(VK_RCONTROL), Key::from_vk(VK_LSHIFT)]
        ));
        assert!(!matches(&b, &[Key::from_vk(VK_LCONTROL)]));
        assert!(!matches(&b, &[]));
    }
    #[test]
    fn combinations_require_every_key_independent_of_order() {
        let b = binding(&[VK_LCONTROL, VK_LWIN]);
        assert!(matches(
            &b,
            &[
                Key::from_vk(VK_LWIN),
                Key::from_vk(0x41),
                Key::from_vk(VK_LCONTROL)
            ]
        ));
        assert!(!matches(&b, &[Key::from_vk(VK_LCONTROL)]));
        assert!(!matches(&Binding::new(vec![], true), &[]));
    }
    #[test]
    fn vk_is_not_keyboard_identity_and_extended_is() {
        let a = Key {
            scan: 30,
            extended: false,
            vk: 0x41,
        };
        assert_eq!(a, Key { vk: 0x51, ..a });
        assert_ne!(
            a,
            Key {
                extended: true,
                ..a
            }
        );
        assert_ne!(
            Key {
                scan: 0,
                vk: VK_XBUTTON1,
                ..a
            },
            Key {
                scan: 0,
                vk: VK_XBUTTON2,
                ..a
            }
        );
    }
    #[test]
    fn modifier_only_never_suppresses_and_labels_are_side_specific() {
        let left = binding(&[VK_LCONTROL]);
        let right = binding(&[VK_RCONTROL]);
        assert!(!left.suppresses());
        assert!(!right.suppresses());
        assert_ne!(left.label, right.label);
        assert!(!left.label.is_empty());
        assert!(binding(&[VK_LCONTROL, VK_SPACE]).suppresses());
    }
    #[test]
    fn mouse_labels_and_extended_keys() {
        assert_eq!(
            Key {
                scan: 0,
                extended: false,
                vk: VK_XBUTTON1
            }
            .label(),
            "Mouse 4"
        );
        assert_eq!(
            Key {
                scan: 0,
                extended: false,
                vk: VK_XBUTTON2
            }
            .label(),
            "Mouse 5"
        );
        let enter = Key::from_vk(VK_RETURN);
        assert_ne!(
            enter,
            Key {
                extended: true,
                ..enter
            }
        );
    }
    #[test]
    fn uncommon_keys_match_without_name_tables() {
        for vk in [VK_F13, VK_F24, VK_MEDIA_PLAY_PAUSE, VK_OEM_102] {
            let b = binding(&[vk]);
            assert!(matches(&b, &b.keys));
            assert!(!b.label.is_empty());
        }
    }
    #[test]
    fn conflicts_are_order_independent_and_system_shortcuts_are_rejected() {
        let b = binding(&[VK_LCONTROL, VK_SPACE]);
        assert!(conflict(&binding(&[VK_SPACE, VK_LCONTROL]), &[Some(b)]).is_err());
        for keys in [
            vec![VK_LWIN, 0x4c],
            vec![VK_RWIN, 0x44],
            vec![VK_LCONTROL, VK_RMENU, VK_DELETE],
            vec![VK_RMENU, VK_TAB],
        ] {
            assert!(conflict(&binding(&keys), &[]).is_err());
        }
        assert!(conflict(&binding(&[VK_RCONTROL]), &[None]).is_ok());
    }
    #[test]
    fn cancel_must_differ_from_hold_and_toggle_and_may_be_plain_escape() {
        let hold = default_hold();
        let toggle = default_toggle();
        let ok = |c: Option<Binding>| validate_all(&[hold.clone(), toggle.clone(), c]).is_ok();
        assert!(ok(default_cancel()));
        assert!(ok(None));
        assert!(ok(Some(binding(&[VK_F8]))));
        assert!(ok(Some(binding(&[VK_LCONTROL, 0x51]))));
        // Equal to hold or toggle, in any key order.
        assert!(!ok(hold.clone()));
        assert!(!ok(Some(binding(&[VK_LWIN, VK_LCONTROL]))));
        let f8 = Some(binding(&[VK_F8]));
        assert!(validate_all(&[f8.clone(), None, f8.clone()]).is_err());
        assert!(validate_all(&[None, f8.clone(), f8.clone()]).is_err());
        assert!(validate_all(&[None, None, f8]).is_ok());
        // Reserved Esc combinations stay rejected for cancel too.
        for keys in [
            vec![VK_LCONTROL, VK_ESCAPE],
            vec![VK_LMENU, VK_ESCAPE],
            vec![VK_LCONTROL, VK_LSHIFT, VK_ESCAPE],
        ] {
            assert!(!ok(Some(binding(&keys))));
        }
    }
}
