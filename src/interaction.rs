use crate::state::{OverlayState, SessionTags};
use crate::window_info::WindowInfo;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_1, VK_2, VK_3, VK_4, VK_5, VK_6, VK_7, VK_8, VK_9, VK_A, VK_CONTROL,
    VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_SPACE, VK_Z,
};
// VK_0 is used only in tests
#[cfg(test)]
use windows::Win32::UI::Input::KeyboardAndMouse::VK_0;

/// Result of processing a WM_HOTKEY event.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum HotkeyAction {
    /// Overlay should be activated (was Hidden).
    Activate,
    /// Overlay should be dismissed (was Active or FadingIn).
    Dismiss,
    /// No action (was FadingOut — already dismissing).
    None,
}

/// Handle a WM_HOTKEY event (activation shortcut re-pressed).
/// Returns the action to take based on current overlay state.
#[allow(dead_code)]
pub fn handle_hotkey_event(state: &OverlayState) -> HotkeyAction {
    match state {
        OverlayState::Hidden => HotkeyAction::Activate,
        OverlayState::FadingIn | OverlayState::Active { .. } => HotkeyAction::Dismiss,
        OverlayState::FadingOut { .. } => HotkeyAction::None,
        OverlayState::LabelMode { .. } => HotkeyAction::None, // Label mode uses its own hotkey
    }
}

/// Handle a WM_ACTIVATE WA_INACTIVE event (overlay lost focus, e.g. alt-tab).
/// Returns true if the overlay should be dismissed.
#[allow(dead_code)]
pub fn handle_focus_lost(state: &OverlayState) -> bool {
    matches!(
        state,
        OverlayState::Active { .. } | OverlayState::FadingIn | OverlayState::LabelMode { .. }
    )
}

/// Result of processing a key event.
#[derive(Debug, Clone)]
pub enum KeyAction {
    /// No action (key was ignored or no-op).
    None,
    /// A window was selected (index). Triggers overlay redraw.
    Select(usize),
    /// Begin switching to the given HWND (triggers fade-out).
    SwitchTo(HWND),
    /// Dismiss the overlay without switching (triggers fade-out, then restore previous focus).
    Dismiss,
    /// Tag was assigned to the selected window. Triggers persistence and redraw.
    TagAssigned { number: u8, hwnd: HWND },
    /// Move the given HWND to the center of the monitor at `monitor_index`,
    /// then switch focus to it and dismiss the overlay.
    MoveToMonitor { hwnd: HWND, monitor_index: usize },
}

/// Handle a WM_KEYDOWN event while the overlay is active.
/// Returns the action to take.
/// When `direct_switch` is true, pressing a letter key immediately switches
/// to the window instead of selecting it first.
/// `monitor_count` bounds the Shift+1..9 "move to monitor" digit range.
pub fn handle_key_down(
    vk_code: u32,
    state: &OverlayState,
    windows: &[WindowInfo],
    tags: &mut SessionTags,
    direct_switch: bool,
    monitor_count: usize,
) -> KeyAction {
    // Use GetAsyncKeyState (physical key state) instead of GetKeyState
    // because the low-level keyboard hook swallows all keystrokes before the
    // message queue processes them, so GetKeyState never sees Ctrl/Shift as pressed.
    let ctrl_held = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 };
    let shift_held = unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) < 0 };

    handle_key_down_with_modifiers(
        vk_code,
        state,
        windows,
        tags,
        direct_switch,
        monitor_count,
        ctrl_held,
        shift_held,
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_key_down_with_modifiers(
    vk_code: u32,
    state: &OverlayState,
    windows: &[WindowInfo],
    tags: &mut SessionTags,
    direct_switch: bool,
    monitor_count: usize,
    ctrl_held: bool,
    shift_held: bool,
) -> KeyAction {
    match state {
        OverlayState::FadingOut { .. } => {
            // No input accepted during fade-out
            return KeyAction::None;
        }
        OverlayState::Hidden => {
            return KeyAction::None;
        }
        _ => {}
    }

    // Number keys (1-9)
    if let Some(num) = vk_to_digit(vk_code) {
        if ctrl_held {
            // Ctrl+Number: assign tag to selected window
            if let OverlayState::Active {
                selected: Some(idx),
            } = state
            {
                if let Some(window) = windows.get(*idx) {
                    // Clear any previous holder of this tag
                    tags.assign(num, window.hwnd);
                    tracing::info!("Tag {} assigned to {:?}", num, window.hwnd);
                    return KeyAction::TagAssigned {
                        number: num,
                        hwnd: window.hwnd,
                    };
                }
            }
        } else if shift_held {
            // Shift+Number: move selected window to the center of monitor `num`.
            if let OverlayState::Active {
                selected: Some(idx),
            } = state
            {
                let monitor_index = (num - 1) as usize;
                if monitor_index < monitor_count {
                    if let Some(window) = windows.get(*idx) {
                        return KeyAction::MoveToMonitor {
                            hwnd: window.hwnd,
                            monitor_index,
                        };
                    }
                }
            }
        } else {
            // Number key alone: switch to tagged window
            if let Some(tagged_hwnd) = tags.get(num) {
                if unsafe {
                    windows::Win32::UI::WindowsAndMessaging::IsWindow(tagged_hwnd).as_bool()
                } {
                    return KeyAction::SwitchTo(tagged_hwnd);
                } else {
                    tags.remove_by_hwnd(tagged_hwnd);
                }
            }
        }
        return KeyAction::None;
    }

    // Letter keys (a-z) — select or switch to a window (or switch directly in label mode)
    // When Ctrl is held, always select (never direct-switch) so the user
    // can Ctrl+Letter to select, then Ctrl+Number to assign a quick-list tag.
    // When Shift is held, always select (never direct-switch) so Shift+1..9 can
    // move the selected window without accidentally switching to it first.
    if let Some(letter) = vk_to_letter(vk_code) {
        if let Some(idx) = crate::letter_assignment::find_by_letter(windows, letter) {
            if direct_switch && !ctrl_held && !shift_held {
                if let Some(window) = windows.get(idx) {
                    return KeyAction::SwitchTo(window.hwnd);
                }
            }
            // In label mode, pressing a letter directly switches to that window
            if matches!(state, OverlayState::LabelMode { .. }) {
                if let Some(window) = windows.get(idx) {
                    return KeyAction::SwitchTo(window.hwnd);
                }
            }
            // In overlay mode, pressing a letter selects the window
            return KeyAction::Select(idx);
        }
        // Unassigned letter: no-op
        return KeyAction::None;
    }

    // Enter or Space: confirm switch
    if vk_code == VK_RETURN.0 as u32 || vk_code == VK_SPACE.0 as u32 {
        match state {
            OverlayState::Active {
                selected: Some(idx),
            }
            | OverlayState::LabelMode {
                selected: Some(idx),
            } => {
                if let Some(window) = windows.get(*idx) {
                    return KeyAction::SwitchTo(window.hwnd);
                }
            }
            _ => {}
        }
        return KeyAction::None;
    }

    // Escape: dismiss overlay
    if vk_code == VK_ESCAPE.0 as u32 {
        return KeyAction::Dismiss;
    }

    KeyAction::None
}

/// Convert a virtual key code to a digit 1-9 (None for 0 or non-digit).
fn vk_to_digit(vk: u32) -> Option<u8> {
    match vk {
        x if x == VK_1.0 as u32 => Some(1),
        x if x == VK_2.0 as u32 => Some(2),
        x if x == VK_3.0 as u32 => Some(3),
        x if x == VK_4.0 as u32 => Some(4),
        x if x == VK_5.0 as u32 => Some(5),
        x if x == VK_6.0 as u32 => Some(6),
        x if x == VK_7.0 as u32 => Some(7),
        x if x == VK_8.0 as u32 => Some(8),
        x if x == VK_9.0 as u32 => Some(9),
        _ => None,
    }
}

/// Convert a virtual key code to a lowercase letter.
fn vk_to_letter(vk: u32) -> Option<char> {
    // VK_A through VK_Z are 0x41–0x5A
    if vk >= VK_A.0 as u32 && vk <= VK_Z.0 as u32 {
        let c = (b'a' + (vk - VK_A.0 as u32) as u8) as char;
        Some(c)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SessionTags;
    use crate::window_info::WindowInfo;

    fn hwnd(n: isize) -> HWND {
        HWND(n as *mut _)
    }

    fn make_window_info(hwnd_n: isize, letter: char) -> WindowInfo {
        let mut w = WindowInfo::new(hwnd(hwnd_n), format!("Window {}", hwnd_n), false, 0);
        w.letter = Some(letter);
        w
    }

    fn active_state(sel: Option<usize>) -> OverlayState {
        OverlayState::Active { selected: sel }
    }

    // Default monitor count for tests that don't exercise Shift+Number moves.
    const DEFAULT_MONITOR_COUNT: usize = 9;

    fn handle_key_down_test(
        vk_code: u32,
        state: &OverlayState,
        windows: &[WindowInfo],
        tags: &mut SessionTags,
        direct_switch: bool,
        ctrl_held: bool,
    ) -> KeyAction {
        handle_key_down_test_full(
            vk_code,
            state,
            windows,
            tags,
            direct_switch,
            ctrl_held,
            false,
            DEFAULT_MONITOR_COUNT,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_key_down_test_full(
        vk_code: u32,
        state: &OverlayState,
        windows: &[WindowInfo],
        tags: &mut SessionTags,
        direct_switch: bool,
        ctrl_held: bool,
        shift_held: bool,
        monitor_count: usize,
    ) -> KeyAction {
        super::handle_key_down_with_modifiers(
            vk_code,
            state,
            windows,
            tags,
            direct_switch,
            monitor_count,
            ctrl_held,
            shift_held,
        )
    }

    #[test]
    fn test_letter_key_selects_window() {
        let windows = vec![make_window_info(1, 'a'), make_window_info(2, 's')];
        let mut tags = SessionTags::new();
        let state = active_state(None);

        // Press 'A' (VK_A = 0x41)
        let action = handle_key_down_test(VK_A.0 as u32, &state, &windows, &mut tags, false, false);
        assert!(matches!(action, KeyAction::Select(0)));
    }

    #[test]
    fn test_escape_dismisses() {
        let windows: Vec<WindowInfo> = vec![];
        let mut tags = SessionTags::new();
        let state = active_state(None);
        let action = handle_key_down_test(
            VK_ESCAPE.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
        );
        assert!(matches!(action, KeyAction::Dismiss));
    }

    #[test]
    fn test_enter_with_no_selection_is_noop() {
        let windows = vec![make_window_info(1, 'a')];
        let mut tags = SessionTags::new();
        let state = active_state(None);
        let action = handle_key_down_test(
            VK_RETURN.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
        );
        assert!(matches!(action, KeyAction::None));
    }

    #[test]
    fn test_enter_with_selection_switches() {
        let h = hwnd(42);
        let mut w = WindowInfo::new(h, "Test".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![w];
        let mut tags = SessionTags::new();
        let state = active_state(Some(0));
        let action = handle_key_down_test(
            VK_RETURN.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
        );
        assert!(matches!(action, KeyAction::SwitchTo(_)));
    }

    #[test]
    fn test_fading_out_ignores_input() {
        let windows: Vec<WindowInfo> = vec![];
        let mut tags = SessionTags::new();
        let state = OverlayState::FadingOut {
            switch_target: None,
        };
        let action = handle_key_down_test(
            VK_ESCAPE.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
        );
        assert!(matches!(action, KeyAction::None));
    }

    #[test]
    fn test_vk_to_letter_range() {
        // A-Z maps to a-z
        for (offset, expected) in ('a'..='z').enumerate() {
            let vk = VK_A.0 as u32 + offset as u32;
            assert_eq!(super::vk_to_letter(vk), Some(expected));
        }
    }

    #[test]
    fn test_vk_to_digit_range() {
        assert_eq!(super::vk_to_digit(VK_1.0 as u32), Some(1));
        assert_eq!(super::vk_to_digit(VK_9.0 as u32), Some(9));
        assert_eq!(super::vk_to_digit(VK_0.0 as u32), None);
    }

    // --- TC-4.4: Space key behaves identically to Enter for confirm-switch ---
    #[test]
    fn test_space_key_confirms_switch() {
        let h = hwnd(42);
        let mut w = WindowInfo::new(h, "Test".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![w];
        let mut tags = SessionTags::new();
        let state = active_state(Some(0));
        let action =
            handle_key_down_test(VK_SPACE.0 as u32, &state, &windows, &mut tags, false, false);
        assert!(
            matches!(action, KeyAction::SwitchTo(_)),
            "Space key with selection should trigger SwitchTo, got {:?}",
            action
        );
    }

    // --- TC-4.6: Unassigned letter key is a no-op ---
    #[test]
    fn test_unassigned_letter_is_noop() {
        // Snapshot only has window with letter 'a'. Press 'z' which is unassigned.
        let windows = vec![make_window_info(1, 'a')];
        let mut tags = SessionTags::new();
        let state = active_state(None);
        let action = handle_key_down_test(VK_Z.0 as u32, &state, &windows, &mut tags, false, false);
        assert!(
            matches!(action, KeyAction::None),
            "Unassigned letter should produce None, got {:?}",
            action
        );
    }

    // --- TC-4.8: Re-press of activation hotkey in Active state dismisses ---
    #[test]
    fn test_hotkey_event_in_active_state_dismisses() {
        let state = active_state(None);
        let action = handle_hotkey_event(&state);
        assert_eq!(
            action,
            HotkeyAction::Dismiss,
            "WM_HOTKEY in Active state should produce Dismiss"
        );
    }

    // --- TC-4.9: Hotkey press in FadingIn state also dismisses ---
    #[test]
    fn test_hotkey_event_in_fading_in_dismisses() {
        let state = OverlayState::FadingIn;
        let action = handle_hotkey_event(&state);
        assert_eq!(
            action,
            HotkeyAction::Dismiss,
            "WM_HOTKEY in FadingIn state should produce Dismiss"
        );
    }

    // --- TC-4.11: Alt+Tab focus loss (WM_ACTIVATE WA_INACTIVE) triggers dismiss ---
    #[test]
    fn test_focus_lost_in_active_state_triggers_dismiss() {
        let state = active_state(None);
        assert!(
            handle_focus_lost(&state),
            "Focus lost in Active state should trigger dismiss"
        );
    }

    #[test]
    fn test_focus_lost_in_fading_in_triggers_dismiss() {
        let state = OverlayState::FadingIn;
        assert!(
            handle_focus_lost(&state),
            "Focus lost in FadingIn state should trigger dismiss"
        );
    }

    #[test]
    fn test_focus_lost_in_hidden_does_not_dismiss() {
        let state = OverlayState::Hidden;
        assert!(
            !handle_focus_lost(&state),
            "Focus lost in Hidden state should not trigger dismiss"
        );
    }

    // --- TC-4.15: Ctrl+Number assigns tag to selected window ---
    #[test]
    fn test_ctrl_number_assigns_tag() {
        let h = hwnd(55);
        let mut w = WindowInfo::new(h, "Tagged Window".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![make_window_info(10, 'b'), make_window_info(11, 'c'), w];
        let mut tags = SessionTags::new();
        let state = active_state(Some(2));

        let action = handle_key_down_test(VK_1.0 as u32, &state, &windows, &mut tags, false, true);
        assert!(matches!(action, KeyAction::TagAssigned { number: 1, hwnd } if hwnd == h));
        assert_eq!(
            tags.get(1),
            Some(h),
            "Tag 1 should point to the selected window's HWND"
        );

        let action_no_ctrl = handle_key_down_test(
            VK_1.0 as u32,
            &state,
            &windows,
            &mut SessionTags::new(),
            false,
            false,
        );
        // With an empty tags store and no Ctrl, pressing 1 should produce None (no tagged window).
        assert!(
            matches!(action_no_ctrl, KeyAction::None),
            "Number key with no tag assigned should produce None, got {:?}",
            action_no_ctrl
        );
    }

    // --- TC-4.16: Ctrl+Number with no selection is a no-op ---
    #[test]
    fn test_ctrl_number_no_selection_is_noop() {
        let windows = vec![make_window_info(1, 'a')];
        let mut tags = SessionTags::new();
        let state = active_state(None);
        let action = handle_key_down_test(VK_1.0 as u32, &state, &windows, &mut tags, false, true);
        assert!(
            matches!(action, KeyAction::None),
            "Ctrl+Number with no selection should be None, got {:?}",
            action
        );
        assert_eq!(
            tags.all_tags().len(),
            0,
            "No tags should have been assigned"
        );
    }

    // --- TC-4.17: Number key (no modifier) switches to tagged window ---
    #[test]
    fn test_number_key_switches_to_tagged_window() {
        // Use GetDesktopWindow() which is always a valid HWND on Windows.
        let valid_hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow() };
        let mut tags = SessionTags::new();
        tags.assign(1, valid_hwnd);
        let windows: Vec<WindowInfo> = vec![];
        let state = active_state(None);
        // Press '1' without Ctrl — should switch to the tagged window.
        let action = handle_key_down_test(VK_1.0 as u32, &state, &windows, &mut tags, false, false);
        assert!(
            matches!(action, KeyAction::SwitchTo(_)),
            "Number key with valid tag should produce SwitchTo, got {:?}",
            action
        );
    }

    // --- TC-4.18: Unassigned number key is a no-op ---
    #[test]
    fn test_unassigned_number_key_is_noop() {
        let mut tags = SessionTags::new();
        // Tag 5 is not assigned
        let windows: Vec<WindowInfo> = vec![];
        let state = active_state(None);
        let action = handle_key_down_test(VK_5.0 as u32, &state, &windows, &mut tags, false, false);
        assert!(
            matches!(action, KeyAction::None),
            "Unassigned number key should produce None, got {:?}",
            action
        );
    }

    // --- Direct switch mode: letter key immediately switches ---
    #[test]
    fn test_direct_switch_letter_switches_immediately() {
        let h = hwnd(42);
        let mut w = WindowInfo::new(h, "Test".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![w];
        let mut tags = SessionTags::new();
        let state = active_state(None);
        let action = handle_key_down_test(VK_A.0 as u32, &state, &windows, &mut tags, true, false);
        assert!(
            matches!(action, KeyAction::SwitchTo(_)),
            "Direct switch: letter key should produce SwitchTo, got {:?}",
            action
        );
    }

    #[test]
    fn test_confirm_mode_letter_selects() {
        let windows = vec![make_window_info(1, 'a')];
        let mut tags = SessionTags::new();
        let state = active_state(None);
        let action = handle_key_down_test(VK_A.0 as u32, &state, &windows, &mut tags, false, false);
        assert!(
            matches!(action, KeyAction::Select(0)),
            "Confirm mode: letter key should produce Select, got {:?}",
            action
        );
    }

    // --- Shift+Number moves the selected window to the corresponding monitor ---
    #[test]
    fn test_shift_number_with_selection_yields_move_to_monitor() {
        let h = hwnd(77);
        let mut w = WindowInfo::new(h, "Movable".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![w];
        let mut tags = SessionTags::new();
        let state = active_state(Some(0));

        // Shift+3 with at least 3 monitors → MoveToMonitor { monitor_index: 2 }
        let action = handle_key_down_test_full(
            VK_3.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
            true,
            3,
        );
        assert!(
            matches!(
                action,
                KeyAction::MoveToMonitor {
                    hwnd: target,
                    monitor_index: 2
                } if target == h
            ),
            "Shift+3 with a selection should produce MoveToMonitor{{monitor_index: 2}}, got {:?}",
            action
        );
    }

    #[test]
    fn test_shift_number_out_of_range_monitor_count_is_noop() {
        let h = hwnd(77);
        let mut w = WindowInfo::new(h, "Movable".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![w];
        let mut tags = SessionTags::new();
        let state = active_state(Some(0));

        // Shift+3 with only 2 monitors → out of range → None
        let action = handle_key_down_test_full(
            VK_3.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
            true,
            2,
        );
        assert!(
            matches!(action, KeyAction::None),
            "Shift+3 with monitor_count == 2 should produce None, got {:?}",
            action
        );
    }

    #[test]
    fn test_shift_number_with_no_selection_is_noop() {
        let windows: Vec<WindowInfo> = vec![];
        let mut tags = SessionTags::new();
        let state = active_state(None);

        let action = handle_key_down_test_full(
            VK_1.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
            true,
            9,
        );
        assert!(
            matches!(action, KeyAction::None),
            "Shift+1 with no selection should produce None, got {:?}",
            action
        );
    }

    #[test]
    fn test_shift_number_in_label_mode_is_noop() {
        let windows: Vec<WindowInfo> = vec![make_window_info(1, 'a')];
        let mut tags = SessionTags::new();
        let state = OverlayState::LabelMode { selected: Some(0) };

        let action = handle_key_down_test_full(
            VK_1.0 as u32,
            &state,
            &windows,
            &mut tags,
            false,
            false,
            true,
            9,
        );
        assert!(
            matches!(action, KeyAction::None),
            "Shift+1 in LabelMode should produce None, got {:?}",
            action
        );
    }

    #[test]
    fn test_shift_letter_selects_instead_of_switching_when_direct_switch() {
        let h = hwnd(42);
        let mut w = WindowInfo::new(h, "Test".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![w];
        let mut tags = SessionTags::new();
        let state = active_state(None);

        // direct_switch is true, but Shift is held → should select, not switch.
        let action = handle_key_down_test_full(
            VK_A.0 as u32,
            &state,
            &windows,
            &mut tags,
            true,
            false,
            true,
            9,
        );
        assert!(
            matches!(action, KeyAction::Select(0)),
            "Shift+Letter under direct_switch should produce Select, got {:?}",
            action
        );
    }

    #[test]
    fn test_plain_letter_still_switches_when_direct_switch() {
        let h = hwnd(42);
        let mut w = WindowInfo::new(h, "Test".into(), false, 0);
        w.letter = Some('a');
        let windows = vec![w];
        let mut tags = SessionTags::new();
        let state = active_state(None);

        // No Shift, direct_switch true → still switches immediately (regression guard).
        let action = handle_key_down_test_full(
            VK_A.0 as u32,
            &state,
            &windows,
            &mut tags,
            true,
            false,
            false,
            9,
        );
        assert!(
            matches!(action, KeyAction::SwitchTo(_)),
            "Plain letter under direct_switch should still produce SwitchTo, got {:?}",
            action
        );
    }
}
