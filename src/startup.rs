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
fn get_exe_path_quoted() -> String {
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