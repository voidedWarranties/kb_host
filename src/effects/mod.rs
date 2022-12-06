use crate::{config::QMKKey, threading::KeyState};
use palette::Hsva;

#[derive(Default, Clone)]
pub struct LedState<'key> {
    pub color: Hsva,
    pub key: Option<&'key QMKKey>,
}

impl<'key> LedState<'key> {
    pub fn key(&self) -> &QMKKey {
        self.key.unwrap()
    }
}

pub trait LedEffect {
    fn update(&mut self, delta: f32, state: &mut Vec<LedState>, key_state: &[Vec<KeyState>]);
}

mod rainbow1;
pub use rainbow1::*;
