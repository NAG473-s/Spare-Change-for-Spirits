//! Tiny dependency-free random number generator (xorshift64).
//! Plenty for a shop game and reproducible with a fixed seed.

use bevy::prelude::*;

#[derive(Resource)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn from_time() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self::new(nanos)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Number in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.f32()
    }

    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len.max(1) as u64) as usize
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Picks an index according to weights. `None` if all weights are 0.
    pub fn weighted(&mut self, weights: &[f32]) -> Option<usize> {
        let sum: f32 = weights.iter().filter(|w| **w > 0.0).sum();
        if sum <= 0.0 {
            return None;
        }
        let mut roll = self.f32() * sum;
        for (i, w) in weights.iter().enumerate() {
            if *w <= 0.0 {
                continue;
            }
            if roll < *w {
                return Some(i);
            }
            roll -= w;
        }
        weights.iter().rposition(|w| *w > 0.0)
    }
}
