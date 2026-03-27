/// Manages the WS_EX_LAYERED alpha fade animation state.
/// The actual SetLayeredWindowAttributes calls are made by the overlay module
/// on each WM_TIMER tick.

/// Starting alpha (transparent).
pub const ALPHA_MIN: u8 = 0;
/// Timer interval in milliseconds (~60fps).
pub const FADE_TIMER_INTERVAL_MS: u32 = 16;
/// Timer ID for the fade animation.
pub const FADE_TIMER_ID: usize = 1001;

/// Direction of the current fade animation.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum FadeDirection {
    In,
    Out,
}

/// Manages the current alpha value and fade state.
/// Parameterized with `alpha_max` and `fade_duration_ms` at construction time
/// so these values can be driven from `AppConfig`.
#[derive(Debug, Clone)]
pub struct FadeAnimator {
    pub current_alpha: u8,
    pub direction: Option<FadeDirection>,
    /// Maximum alpha target (configurable via overlay_opacity).
    pub alpha_max: u8,
    /// Alpha change per timer tick, computed from alpha_max and fade_duration_ms.
    pub alpha_delta: u8,
    /// Whether fade_duration_ms was 0 (instant mode — skip animation entirely).
    pub instant: bool,
}

impl FadeAnimator {
    /// Create a new `FadeAnimator` with configurable alpha_max and fade_duration_ms.
    ///
    /// If `fade_duration_ms` is 0, the animator is in "instant" mode: `start_fade_in()`
    /// immediately sets `current_alpha = alpha_max` with no intermediate ticks.
    pub fn new_with_params(alpha_max: u8, fade_duration_ms: u32) -> Self {
        let instant = fade_duration_ms == 0;
        let alpha_delta = if instant {
            255 // not used in instant mode, but safe sentinel
        } else {
            let ticks = fade_duration_ms as f32 / FADE_TIMER_INTERVAL_MS as f32;
            ((alpha_max as f32 / ticks).ceil() as u8).max(1)
        };
        Self {
            current_alpha: ALPHA_MIN,
            direction: None,
            alpha_max,
            alpha_delta,
            instant,
        }
    }

    /// Create with default parameters (220 alpha_max, 150ms fade).
    pub fn new() -> Self {
        Self::new_with_params(220, 150)
    }

    /// Start a fade-in animation.
    /// In instant mode, immediately jumps to alpha_max with no intermediate state.
    #[allow(dead_code)]
    pub fn start_fade_in(&mut self) {
        if self.instant {
            self.current_alpha = self.alpha_max;
            self.direction = None;
        } else {
            self.current_alpha = ALPHA_MIN;
            self.direction = Some(FadeDirection::In);
        }
    }

    /// Start a fade-out animation.
    /// In instant mode, immediately drops to ALPHA_MIN.
    #[allow(dead_code)]
    pub fn start_fade_out(&mut self) {
        if self.instant {
            self.current_alpha = ALPHA_MIN;
            self.direction = None;
        } else {
            self.current_alpha = self.alpha_max;
            self.direction = Some(FadeDirection::Out);
        }
    }

    /// Advance the animation by one tick.
    /// Returns true if the animation is still running, false if it has completed.
    pub fn tick(&mut self) -> bool {
        match self.direction {
            Some(FadeDirection::In) => {
                let new_alpha = self.current_alpha.saturating_add(self.alpha_delta);
                if new_alpha >= self.alpha_max {
                    self.current_alpha = self.alpha_max;
                    self.direction = None;
                    false // Animation complete
                } else {
                    self.current_alpha = new_alpha;
                    true // Still running
                }
            }
            Some(FadeDirection::Out) => {
                if self.current_alpha <= self.alpha_delta {
                    self.current_alpha = ALPHA_MIN;
                    self.direction = None;
                    false // Animation complete
                } else {
                    self.current_alpha = self.current_alpha.saturating_sub(self.alpha_delta);
                    true // Still running
                }
            }
            None => false,
        }
    }

    #[allow(dead_code)]
    pub fn is_animating(&self) -> bool {
        self.direction.is_some()
    }
}

impl Default for FadeAnimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_in_reaches_max() {
        let mut anim = FadeAnimator::new_with_params(220, 150);
        anim.start_fade_in();
        assert!(anim.is_animating());

        // Run ticks until complete
        let mut ticks = 0;
        while anim.tick() {
            ticks += 1;
            assert!(ticks < 20, "Fade-in should complete within 20 ticks");
        }
        assert_eq!(anim.current_alpha, 220);
        assert!(!anim.is_animating());
    }

    #[test]
    fn test_fade_out_reaches_min() {
        let mut anim = FadeAnimator::new_with_params(220, 150);
        anim.start_fade_out();
        assert!(anim.is_animating());

        let mut ticks = 0;
        while anim.tick() {
            ticks += 1;
            assert!(ticks < 20, "Fade-out should complete within 20 ticks");
        }
        assert_eq!(anim.current_alpha, ALPHA_MIN);
        assert!(!anim.is_animating());
    }

    #[test]
    fn test_alpha_increases_during_fade_in() {
        let mut anim = FadeAnimator::new_with_params(220, 150);
        anim.start_fade_in();
        let prev = anim.current_alpha;
        anim.tick();
        assert!(anim.current_alpha >= prev);
    }

    #[test]
    fn test_alpha_decreases_during_fade_out() {
        let mut anim = FadeAnimator::new_with_params(220, 150);
        anim.start_fade_out();
        let prev = anim.current_alpha;
        anim.tick();
        assert!(anim.current_alpha <= prev);
    }

    #[test]
    fn test_instant_fade_in_skips_animation() {
        let mut anim = FadeAnimator::new_with_params(200, 0);
        assert!(anim.instant);
        anim.start_fade_in();
        // Should immediately be at alpha_max with no animation active
        assert_eq!(anim.current_alpha, 200);
        assert!(!anim.is_animating());
    }

    #[test]
    fn test_instant_fade_out_skips_animation() {
        let mut anim = FadeAnimator::new_with_params(200, 0);
        anim.current_alpha = 200;
        anim.start_fade_out();
        assert_eq!(anim.current_alpha, ALPHA_MIN);
        assert!(!anim.is_animating());
    }

    #[test]
    fn test_custom_alpha_max_respected() {
        let mut anim = FadeAnimator::new_with_params(180, 100);
        anim.start_fade_in();
        while anim.tick() {}
        assert_eq!(anim.current_alpha, 180);
    }

    #[test]
    fn test_params_valid() {
        let anim = FadeAnimator::new_with_params(220, 150);
        assert_eq!(anim.alpha_max, 220);
        assert!(!anim.instant);
        assert!(anim.alpha_delta > 0);
        // Should complete in ~10 ticks: 220 / 24 ≈ 9.2
        let expected_ticks = (anim.alpha_max as f32 / anim.alpha_delta as f32).ceil() as u32;
        assert!(expected_ticks <= 12, "Should complete in <= 12 ticks");
    }

    #[test]
    fn test_new_uses_defaults() {
        let anim = FadeAnimator::new();
        assert_eq!(anim.alpha_max, 220);
        assert!(!anim.instant);
    }
}