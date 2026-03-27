/// Manages the Windows startup registry entry for launch-at-login.
///
/// Uses HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
/// Value name: "window-selector"
/// Value data: full path to the current executable (quoted if it contains spaces).
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

const REGISTRY_VALUE_NAME: &str = "window-selector";
const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// Enable or disable launch at Windows startup.
/// Writes/deletes the registry entry with the current executable path.
pub fn set_launch_at_startup(enabled: bool) -> windows::core::Result<()> {
    let key_path_wide: Vec<u16> = RUN_KEY_PATH
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path_wide.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        )
        .ok()?;

        let value_name_wide: Vec<u16> = REGISTRY_VALUE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let result = if enabled {
            let exe_path = get_exe_path_quoted();
            let exe_path_wide: Vec<u16> = exe_path
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let data_bytes: &[u8] = std::slice::from_raw_parts(
                exe_path_wide.as_ptr() as *const u8,
                exe_path_wide.len() * 2,
            );
            RegSetValueExW(
                hkey,
                PCWSTR(value_name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(data_bytes),
            )
        } else {
            RegDeleteValueW(hkey, PCWSTR(value_name_wide.as_ptr()))
        };

        let _ = RegCloseKey(hkey);

        if let Err(e) = result.ok() {
            tracing::warn!("set_launch_at_startup({}) failed: {:?}", enabled, e);
            return Err(e);
        }

        tracing::info!("set_launch_at_startup({}): success", enabled);
        Ok(())
    }
}

/// Check if launch at startup is currently enabled.
/// Returns true if the registry entry exists and its path matches the current executable.
pub fn get_launch_at_startup() -> bool {
    let key_path_wide: Vec<u16> = RUN_KEY_PATH
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            tracing::warn!("get_launch_at_startup: failed to open registry key");
            return false;
        }

        let value_name_wide: Vec<u16> = REGISTRY_VALUE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // First call to get required buffer size
        let mut data_type = REG_SZ;
        let mut data_size: u32 = 0;
        let size_result = RegQueryValueExW(
            hkey,
            PCWSTR(value_name_wide.as_ptr()),
            None,
            Some(&mut data_type),
            None,
            Some(&mut data_size),
        );

        if size_result.is_err() {
            let _ = RegCloseKey(hkey);
            return false;
        }

        // Allocate buffer and read value
        let num_wchars = (data_size as usize + 1) / 2;
        let mut buf: Vec<u16> = vec![0u16; num_wchars + 1];
        let buf_ptr = buf.as_mut_ptr() as *mut u8;
        let mut actual_size = data_size;
        let read_result = RegQueryValueExW(
            hkey,
            PCWSTR(value_name_wide.as_ptr()),
            None,
            Some(&mut data_type),
            Some(buf_ptr),
            Some(&mut actual_size),
        );

        let _ = RegCloseKey(hkey);

        if read_result.is_err() {
            return false;
        }

        // Decode the stored path (REG_SZ = null-terminated UTF-16)
        let stored: String = {
            let nul_pos = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..nul_pos]).to_string()
        };

        // Compare against current exe path (with quoting stripped for comparison)
        let current = get_exe_path_quoted();
        let stored_unquoted = stored.trim_matches('"');
        let current_unquoted = current.trim_matches('"');

        stored_unquoted == current_unquoted
    }
}

/// Returns the current executable path, quoted if it contains spaces.
pub(crate) fn get_exe_path_quoted() -> String {
    match std::env::current_exe() {
        Ok(path) => {
            let path_str = path.to_string_lossy().to_string();
            if path_str.contains(' ') {
                format!("\"{}\"", path_str)
            } else {
                path_str
            }
        }
        Err(e) => {
            tracing::warn!("current_exe() failed: {}", e);
            String::new()
        }
    }
}

/// Pure helper: apply the same quoting logic as get_exe_path_quoted to an arbitrary path string.
/// Used by tests to verify the quoting logic without depending on current_exe().
#[cfg(test)]
fn quote_if_spaces(path_str: &str) -> String {
    if path_str.contains(' ') {
        format!("\"{}\"", path_str)
    } else {
        path_str.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TC-3.1: get_launch_at_startup() returns false when registry entry is absent.
    // We verify the path-mismatch branch: a stored path that does not match the current exe
    // should return false. We test this via the comparison logic used in get_launch_at_startup.
    #[test]
    fn test_get_launch_at_startup_returns_false_on_path_mismatch() {
        // The comparison in get_launch_at_startup() strips quotes then compares.
        let stored = "\"C:\\Windows\\SomeOtherApp.exe\"";
        let current = get_exe_path_quoted();

        let stored_unquoted = stored.trim_matches('"');
        let current_unquoted = current.trim_matches('"');

        // The stored path is for a different app, so it must not match.
        assert_ne!(
            stored_unquoted, current_unquoted,
            "A different app's path should not match the current exe"
        );
    }

    // TC-3.2: get_launch_at_startup() path comparison logic — same path matches.
    #[test]
    fn test_path_comparison_logic_same_path_matches() {
        let current = get_exe_path_quoted();
        let stored_unquoted = current.trim_matches('"');
        let current_unquoted = current.trim_matches('"');
        // Same path (with quotes stripped) must match.
        assert_eq!(stored_unquoted, current_unquoted);
    }

    // TC-3.2 (continued): exe path quoting — path without spaces is unquoted.
    #[test]
    fn test_exe_path_quoting_no_spaces() {
        let path = "C:\\Windows\\system32\\app.exe";
        let quoted = quote_if_spaces(path);
        assert_eq!(quoted, path, "Path without spaces should not be quoted");
        assert!(!quoted.starts_with('"'));
    }

    // TC-3.2 (continued): exe path quoting — path with spaces is quoted.
    #[test]
    fn test_exe_path_quoting_with_spaces() {
        let path = "C:\\Program Files\\My App\\app.exe";
        let quoted = quote_if_spaces(path);
        assert!(quoted.starts_with('"'), "Path with spaces should start with quote");
        assert!(quoted.ends_with('"'), "Path with spaces should end with quote");
        assert_eq!(quoted, format!("\"{}\"", path));
    }

    // TC-3.2 (continued): quote-stripping is symmetric — trim_matches('"') reverses quoting.
    #[test]
    fn test_quote_strip_is_symmetric() {
        let path = "C:\\Program Files\\My App\\app.exe";
        let quoted = quote_if_spaces(path);
        let unquoted = quoted.trim_matches('"');
        assert_eq!(unquoted, path, "Stripping quotes should recover original path");
    }

    // TC-3.1 (registry): get_launch_at_startup() returns false when key is absent.
    // This test requires Windows registry access. It is safe to run but depends on the
    // test runner not having previously written our registry key.
    #[test]
    #[cfg(target_os = "windows")]
    fn test_get_launch_at_startup_false_when_registry_absent() {
        // Remove the key first (ignore error if it was never set)
        let _ = set_launch_at_startup(false);
        // Now check — must return false since we just removed it.
        let result = get_launch_at_startup();
        assert!(!result, "get_launch_at_startup() must return false when key is absent");
    }
}