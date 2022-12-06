use super::LedEffect;
use palette::Hsva;

// hue degrees per second
const SPEED: f32 = 36.0;

// hue degrees per key unit (kinda)
const FACTOR: f32 = 4.0;

#[derive(Default)]
pub struct Rainbow1Effect {
    base_hue: f32,
}

impl LedEffect for Rainbow1Effect {
    fn update(
        &mut self,
        delta: f32,
        state: &mut Vec<super::LedState>,
        _key_state: &[Vec<crate::threading::KeyState>],
    ) {
        for led in state {
            let mut key_hue = self.base_hue + (led.key().x + led.key().y) * FACTOR;
            key_hue %= 360.0;

            led.color = Hsva::new(key_hue, 1.0, 1.0, 1.0);
        }

        self.base_hue += SPEED * delta;
        self.base_hue %= 360.0;
    }
}
