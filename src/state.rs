use std::collections::HashMap;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::IsWindow;

/// The overlay interaction state machine.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OverlayState {
    /// Overlay is hidden; app is idle in the tray.
    Hidden,
    /// Overlay is animating in (fade-in). Accepts dismiss but not selection.
    FadingIn,
    /// Overlay is visible and accepting input.
    Active {
        /// Index of the currently highlighted window in the snapshot, if any.
        selected: Option<usize>,
    },
    /// Overlay is animating out (fade-out). No input accepted.
    FadingOut {
        /// The HWND to focus after fade-out completes, if any.
        switch_target: Option<HWND>,
    },
}

impl OverlayState {
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        !matches!(self, OverlayState::Hidden)
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        matches!(self, OverlayState::Active { .. })
    }

    pub fn selected_index(&self) -> Option<usize> {
        if let OverlayState::Active { selected } = self {
            *selected
        } else {
            None
        }
    }
}

/// Session-only number tag store. Maps tag number (1-9) to window handle.
/// Persists across overlay show/hide cycles within the process session.
pub struct SessionTags {
    tags: HashMap<u8, HWND>,
}

impl SessionTags {
    pub fn new() -> Self {
        Self {
            tags: HashMap::new(),
        }
    }

    /// Assign a tag number to a window handle. Removes any prior assignment for this number.
    pub fn assign(&mut self, number: u8, hwnd: HWND) {
        self.tags.insert(number, hwnd);
    }

    /// Get the HWND for a tag number, if assigned.
    pub fn get(&self, number: u8) -> Option<HWND> {
        self.tags.get(&number).copied()
    }

    /// Get the tag number assigned to the given HWND, if any.
    pub fn get_tag_for_hwnd(&self, hwnd: HWND) -> Option<u8> {
        self.tags
            .iter()
            .find(|(_, &h)| h == hwnd)
            .map(|(&n, _)| n)
    }

    /// Remove tags pointing to windows that are no longer valid.
    pub fn release_closed(&mut self) {
        self.tags.retain(|_, hwnd| unsafe { IsWindow(*hwnd).as_bool() });
    }

    /// Remove the tag for a specific HWND.
    pub fn remove_by_hwnd(&mut self, hwnd: HWND) {
        self.tags.retain(|_, h| *h != hwnd);
    }

    /// Return all current tag assignments as a sorted Vec.
    #[allow(dead_code)]
    pub fn all_tags(&self) -> Vec<(u8, HWND)> {
        let mut result: Vec<(u8, HWND)> = self.tags.iter().map(|(&n, &h)| (n, h)).collect();
        result.sort_by_key(|(n, _)| *n);
        result
    }
}

impl Default for SessionTags {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hwnd(n: isize) -> HWND {
        HWND(n as *mut _)
    }

    #[test]
    fn test_assign_and_get_tag() {
        let mut tags = SessionTags::new();
        let hw = hwnd(1);
        tags.assign(1, hw);
        assert_eq!(tags.get(1), Some(hw));
        assert_eq!(tags.get(2), None);
    }

    #[test]
    fn test_assign_overwrites_prior_assignment() {
        let mut tags = SessionTags::new();
        let hw_a = hwnd(1);
        let hw_b = hwnd(2);
        tags.assign(1, hw_a);
        tags.assign(1, hw_b);
        assert_eq!(tags.get(1), Some(hw_b));
    }

    #[test]
    fn test_get_tag_for_hwnd() {
        let mut tags = SessionTags::new();
        let hw = hwnd(42);
        tags.assign(3, hw);
        assert_eq!(tags.get_tag_for_hwnd(hw), Some(3));
        assert_eq!(tags.get_tag_for_hwnd(hwnd(99)), None);
    }

    #[test]
    fn test_remove_by_hwnd() {
        let mut tags = SessionTags::new();
        let hw = hwnd(5);
        tags.assign(2, hw);
        assert_eq!(tags.get(2), Some(hw));
        tags.remove_by_hwnd(hw);
        assert_eq!(tags.get(2), None);
    }

    #[test]
    fn test_overlay_state_active_selected() {
        let state = OverlayState::Active { selected: Some(3) };
        assert!(state.is_active());
        assert!(state.is_visible());
        assert_eq!(state.selected_index(), Some(3));
    }

    #[test]
    fn test_overlay_state_hidden() {
        let state = OverlayState::Hidden;
        assert!(!state.is_active());
        assert!(!state.is_visible());
        assert_eq!(state.selected_index(), None);
    }

    #[test]
    fn test_overlay_state_fading_in() {
        let state = OverlayState::FadingIn;
        assert!(!state.is_active());
        assert!(state.is_visible());
    }

    #[test]
    fn test_overlay_state_fading_out() {
        let state = OverlayState::FadingOut { switch_target: None };
        assert!(!state.is_active());
        assert!(state.is_visible());
    }
}