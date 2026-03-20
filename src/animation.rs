/// Manages the WS_EX_LAYERED alpha fade animation state.
/// The actual SetLayeredWindowAttributes calls are made by the overlay module
/// on each WM_TIMER tick.

/// Target alpha value when overlay is fully shown (not fully opaque — preserves backdrop bleed-through).
pub const ALPHA_MAX: u8 = 220;
/// Starting alpha (transparent).
pub const ALPHA_MIN: u8 = 0;
/// Timer interval in milliseconds (~60fps).
pub const FADE_TIMER_INTERVAL_MS: u32 = 16;
/// Fade duration in milliseconds.
pub const FADE_DURATION_MS: u32 = 80;
/// Alpha delta per timer tick.
/// 220 / (80ms / 16ms) = 220 / 5 = 44
pub const ALPHA_DELTA: u8 = 44;
/// Timer ID for the fade animation.
pub const FADE_TIMER_ID: usize = 1001;

/// Direction of the current fade animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FadeDirection {
    In,
    Out,
}

/// Manages the current alpha value and fade state.
#[derive(Debug, Clone)]
pub struct FadeAnimator {
    pub current_alpha: u8,
    pub direction: Option<FadeDirection>,
}

impl FadeAnimator {
    pub fn new() -> Self {
        Self {
            current_alpha: ALPHA_MIN,
            direction: None,
        }
    }

    /// Start a fade-in animation.
    pub fn start_fade_in(&mut self) {
        self.current_alpha = ALPHA_MIN;
        self.direction = Some(FadeDirection::In);
    }

    /// Start a fade-out animation.
    pub fn start_fade_out(&mut self) {
        self.current_alpha = ALPHA_MAX;
        self.direction = Some(FadeDirection::Out);
    }

    /// Advance the animation by one tick.
    /// Returns true if the animation is still running, false if it has completed.
    pub fn tick(&mut self) -> bool {
        match self.direction {
            Some(FadeDirection::In) => {
                let new_alpha = self.current_alpha.saturating_add(ALPHA_DELTA);
                if new_alpha >= ALPHA_MAX {
                    self.current_alpha = ALPHA_MAX;
                    self.direction = None;
                    false // Animation complete
                } else {
                    self.current_alpha = new_alpha;
                    true // Still running
                }
            }
            Some(FadeDirection::Out) => {
                if self.current_alpha <= ALPHA_DELTA {
                    self.current_alpha = ALPHA_MIN;
                    self.direction = None;
                    false // Animation complete
                } else {
                    self.current_alpha = self.current_alpha.saturating_sub(ALPHA_DELTA);
                    true // Still running
                }
            }
            None => false,
        }
    }

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
        let mut anim = FadeAnimator::new();
        anim.start_fade_in();
        assert!(anim.is_animating());

        // Run ticks until complete
        let mut ticks = 0;
        while anim.tick() {
            ticks += 1;
            assert!(ticks < 20, "Fade-in should complete within 20 ticks");
        }
        assert_eq!(anim.current_alpha, ALPHA_MAX);
        assert!(!anim.is_animating());
    }

    #[test]
    fn test_fade_out_reaches_min() {
        let mut anim = FadeAnimator::new();
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
        let mut anim = FadeAnimator::new();
        anim.start_fade_in();
        let prev = anim.current_alpha;
        anim.tick();
        assert!(anim.current_alpha >= prev);
    }

    #[test]
    fn test_alpha_decreases_during_fade_out() {
        let mut anim = FadeAnimator::new();
        anim.start_fade_out();
        let prev = anim.current_alpha;
        anim.tick();
        assert!(anim.current_alpha <= prev);
    }

    #[test]
    fn test_constants_valid() {
        assert_eq!(ALPHA_MAX, 220);
        assert_eq!(ALPHA_MIN, 0);
        assert_eq!(FADE_TIMER_INTERVAL_MS, 16);
        assert!(ALPHA_DELTA > 0);
        // Should complete in ~5 ticks: 220 / 44 = 5
        let expected_ticks = (ALPHA_MAX as f32 / ALPHA_DELTA as f32).ceil() as u32;
        assert!(expected_ticks <= 6, "Should complete in <= 6 ticks");
    }
}
