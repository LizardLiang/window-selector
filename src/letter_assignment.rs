use crate::window_info::WindowInfo;

/// Home-row-first ergonomic letter sequence for window assignment.
/// Most-recently-used window gets 'a', second gets 's', etc.
pub const LETTER_SEQUENCE: [char; 26] = [
    'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', // home row (9)
    'q', 'w', 'e', 'r', 't',                        // top row left (5)
    'y', 'u', 'i', 'o', 'p',                        // top row right (5)
    'z', 'x', 'c', 'v', 'b', 'n', 'm',             // bottom row (7)
];

/// Assign letters to windows in order. Windows beyond position 26 receive None.
pub fn assign_letters(windows: &mut Vec<WindowInfo>) {
    for (i, window) in windows.iter_mut().enumerate() {
        window.letter = if i < LETTER_SEQUENCE.len() {
            Some(LETTER_SEQUENCE[i])
        } else {
            None
        };
    }
}

/// Find the index of a window with the given letter in the snapshot.
pub fn find_by_letter(windows: &[WindowInfo], letter: char) -> Option<usize> {
    windows.iter().position(|w| w.letter == Some(letter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::HWND;

    fn make_windows(n: usize) -> Vec<WindowInfo> {
        (0..n)
            .map(|i| WindowInfo::new(HWND(i as isize as *mut _), format!("Window {}", i), false, 0))
            .collect()
    }

    #[test]
    fn test_zero_windows_no_letters_assigned() {
        let mut windows = make_windows(0);
        assign_letters(&mut windows);
        assert!(windows.is_empty());
    }

    #[test]
    fn test_one_window_gets_letter_a() {
        let mut windows = make_windows(1);
        assign_letters(&mut windows);
        assert_eq!(windows[0].letter, Some('a'));
    }

    #[test]
    fn test_26_windows_get_full_sequence() {
        let mut windows = make_windows(26);
        assign_letters(&mut windows);
        for (i, window) in windows.iter().enumerate() {
            assert_eq!(window.letter, Some(LETTER_SEQUENCE[i]), "Index {}", i);
        }
        // First window gets 'a'
        assert_eq!(windows[0].letter, Some('a'));
        // Second gets 's'
        assert_eq!(windows[1].letter, Some('s'));
        // Third gets 'd'
        assert_eq!(windows[2].letter, Some('d'));
    }

    #[test]
    fn test_windows_beyond_26_get_none() {
        let mut windows = make_windows(30);
        assign_letters(&mut windows);
        for i in 0..26 {
            assert!(windows[i].letter.is_some(), "Index {} should have a letter", i);
        }
        for i in 26..30 {
            assert_eq!(windows[i].letter, None, "Index {} should have None", i);
        }
    }

    #[test]
    fn test_letter_sequence_no_semicolons_26_distinct_letters() {
        assert_eq!(LETTER_SEQUENCE.len(), 26);
        // All entries must be lowercase letters a-z
        for &c in &LETTER_SEQUENCE {
            assert!(c >= 'a' && c <= 'z', "Expected a-z, got '{}'", c);
            assert_ne!(c, ';', "Semicolon must not be in sequence");
        }
        // No duplicates
        let mut seen = std::collections::HashSet::new();
        for &c in &LETTER_SEQUENCE {
            assert!(seen.insert(c), "Duplicate letter '{}' in sequence", c);
        }
    }

    #[test]
    fn test_find_by_letter() {
        let mut windows = make_windows(5);
        assign_letters(&mut windows);
        // 'a' should be at index 0
        assert_eq!(find_by_letter(&windows, 'a'), Some(0));
        // 's' should be at index 1
        assert_eq!(find_by_letter(&windows, 's'), Some(1));
        // 'z' is not in the first 5
        assert_eq!(find_by_letter(&windows, 'z'), None);
    }
}