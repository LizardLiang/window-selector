use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;

/// Extracted Windows accent color.
#[derive(Debug, Clone, Copy)]
pub struct AccentColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl AccentColor {
    /// Create from normalized RGB components.
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    /// Default accent color (Windows 11 blue) for fallback.
    pub fn default_blue() -> Self {
        Self {
            r: 0.0,
            g: 0.478,
            b: 1.0,
        }
    }

    /// Create an RGBA tuple suitable for Direct2D (alpha as last component).
    pub fn to_d2d_color(&self, alpha: f32) -> (f32, f32, f32, f32) {
        (self.r, self.g, self.b, alpha)
    }
}

/// Query the current Windows accent color from DWM.
/// Returns the system accent color, or a default blue if the query fails.
pub fn get_accent_color() -> AccentColor {
    unsafe {
        let mut color_ref: u32 = 0;
        let mut opaque_blend: windows::Win32::Foundation::BOOL = windows::Win32::Foundation::BOOL(0);

        if DwmGetColorizationColor(&mut color_ref, &mut opaque_blend).is_ok() {
            // DWM returns ARGB as 0xAARRGGBB
            let r = ((color_ref >> 16) & 0xFF) as f32 / 255.0;
            let g = ((color_ref >> 8) & 0xFF) as f32 / 255.0;
            let b = (color_ref & 0xFF) as f32 / 255.0;
            AccentColor::new(r, g, b)
        } else {
            tracing::warn!("DwmGetColorizationColor failed; using default accent color");
            AccentColor::default_blue()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accent_color_to_d2d() {
        let color = AccentColor::new(0.5, 0.6, 0.7);
        let (r, g, b, a) = color.to_d2d_color(0.9);
        assert!((r - 0.5).abs() < 0.001);
        assert!((g - 0.6).abs() < 0.001);
        assert!((b - 0.7).abs() < 0.001);
        assert!((a - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_get_accent_color_does_not_panic() {
        // Should return either the real color or the fallback without panicking
        let _color = get_accent_color();
    }
}
