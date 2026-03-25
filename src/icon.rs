use windows::core::PCWSTR;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    LoadImageW, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LR_SHARED,
};

/// Load the application icon from the embedded resource (ID 1, set by resources/app.rc).
pub fn load_app_icon() -> windows::core::Result<HICON> {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null())?;
        let handle = LoadImageW(
            instance,
            PCWSTR(1 as *const u16), // resource ID 1 (MAKEINTRESOURCE)
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTCOLOR | LR_SHARED,
        )?;
        Ok(HICON(handle.0))
    }
}
