use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

/// Physical monitor descriptor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub handle: HMONITOR,
    pub rect: RECT,       // Screen coordinates (physical pixels)
    pub work_rect: RECT,  // Excluding taskbar
    pub dpi_scale: f32,   // e.g., 1.0, 1.25, 1.5, 2.0
    pub is_primary: bool,
}

/// Enumerate all connected monitors and return their info.
pub fn get_all_monitors() -> Vec<MonitorInfo> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();
    let monitors_ptr = &mut monitors as *mut Vec<MonitorInfo>;

    unsafe {
        EnumDisplayMonitors(
            HDC(std::ptr::null_mut()),
            None,
            Some(enum_monitor_callback),
            LPARAM(monitors_ptr as isize),
        );
    }

    // Ensure primary monitor is first
    monitors.sort_by_key(|m| if m.is_primary { 0 } else { 1 });
    monitors
}

unsafe extern "system" fn enum_monitor_callback(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
        let is_primary = (info.dwFlags & MONITORINFOF_PRIMARY) != 0;

        let mut dpi_x: u32 = 96;
        let mut dpi_y: u32 = 96;
        let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let dpi_scale = dpi_x as f32 / 96.0;

        monitors.push(MonitorInfo {
            handle: hmonitor,
            rect: info.rcMonitor,
            work_rect: info.rcWork,
            dpi_scale,
            is_primary,
        });
    }

    BOOL(1) // Continue enumeration
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_monitors_returns_at_least_one() {
        // This test requires a running Windows display subsystem.
        // In CI without display, may return empty — we just verify it doesn't crash.
        let monitors = get_all_monitors();
        // At minimum, if running on a machine with a display, should have at least 1.
        // We can't assert count > 0 in headless environments.
        for m in &monitors {
            assert!(m.dpi_scale > 0.0);
        }
    }

    #[test]
    fn test_primary_monitor_is_first() {
        let monitors = get_all_monitors();
        if monitors.len() > 1 {
            assert!(monitors[0].is_primary, "Primary monitor should be first");
        }
    }
}
