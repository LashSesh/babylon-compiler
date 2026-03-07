//! TIC (Temporal Information Certificate) tracking,
//! condensation predicate, and crystallization condition.
//! Implements Section 14 of the Glyph Foundry Specification v2.1.

use glyph_q16::Q16;
use serde::{Deserialize, Serialize};

/// A single TIC observation entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicEntry {
    pub tick: u64,
    pub delta: Q16,
    pub kappa: Q16,
}

/// Temporal Information Certificate — a sequence of convergence observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tic {
    pub entries: Vec<TicEntry>,
}

impl Tic {
    /// Create a new empty TIC.
    pub fn new() -> Self {
        Tic {
            entries: Vec::new(),
        }
    }

    /// Append an observation.
    /// kappa = min(len/s_min, 1.0) in Q16 (where 1.0 = 65536).
    pub fn append(&mut self, tick: u64, delta: Q16, s_min: usize) {
        let count = (self.entries.len() + 1) as i64;
        let s_min_val = s_min.max(1) as i64;
        // kappa = min(count * 65536 / s_min, 65536)
        let kappa_raw = ((count << 16) / s_min_val).min(65536);
        let kappa = Q16::from_raw(kappa_raw as i32);

        self.entries.push(TicEntry { tick, delta, kappa });
    }

    /// Check if the TIC is condensing over a window of size w.
    /// Condensing iff the last w deltas are monotonically non-increasing.
    pub fn condensing(&self, w: usize) -> bool {
        let n = self.entries.len();
        if n < w {
            return false;
        }
        let start = n - w;
        for j in start..(n - 1) {
            if self.entries[j + 1].delta.raw() > self.entries[j].delta.raw() {
                return false;
            }
        }
        true
    }

    /// Check the crystallization condition (Definition 14.3):
    /// |TIC| >= s_min AND condensing(w)
    pub fn crystallization_ready(&self, s_min: usize, w: usize) -> bool {
        self.entries.len() >= s_min && self.condensing(w)
    }

    /// Get the last delta value (or ZERO if empty).
    pub fn last_delta(&self) -> Q16 {
        self.entries.last().map(|e| e.delta).unwrap_or(glyph_q16::ZERO)
    }

    /// Get the last kappa value (or ZERO if empty).
    pub fn last_kappa(&self) -> Q16 {
        self.entries.last().map(|e| e.kappa).unwrap_or(glyph_q16::ZERO)
    }
}

impl Default for Tic {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tic_new() {
        let tic = Tic::new();
        assert!(tic.entries.is_empty());
    }

    #[test]
    fn test_tic_append_kappa() {
        let mut tic = Tic::new();
        // s_min = 5, after 1 entry: kappa = 1/5 = 0.2 in Q16 = 13107
        tic.append(0, Q16::from_raw(0), 5);
        assert_eq!(tic.entries[0].kappa.raw(), 13107); // floor(65536/5)

        // After 5 entries: kappa should be 1.0 = 65536
        for i in 1..5 {
            tic.append(i as u64, Q16::from_raw(0), 5);
        }
        assert_eq!(tic.entries[4].kappa.raw(), 65536);
    }

    #[test]
    fn test_condensing_true() {
        let mut tic = Tic::new();
        // Monotonically non-increasing deltas
        tic.append(0, Q16::from_raw(100), 1);
        tic.append(1, Q16::from_raw(80), 1);
        tic.append(2, Q16::from_raw(50), 1);
        assert!(tic.condensing(3));
    }

    #[test]
    fn test_condensing_false() {
        let mut tic = Tic::new();
        tic.append(0, Q16::from_raw(50), 1);
        tic.append(1, Q16::from_raw(80), 1); // Increases!
        tic.append(2, Q16::from_raw(30), 1);
        assert!(!tic.condensing(3));
    }

    #[test]
    fn test_condensing_equal_values() {
        let mut tic = Tic::new();
        tic.append(0, Q16::from_raw(50), 1);
        tic.append(1, Q16::from_raw(50), 1); // Equal is non-increasing
        tic.append(2, Q16::from_raw(50), 1);
        assert!(tic.condensing(3));
    }

    #[test]
    fn test_crystallization_ready() {
        let mut tic = Tic::new();
        for i in 0..5 {
            tic.append(i, Q16::from_raw((100 - i * 10) as i32), 5);
        }
        assert!(tic.crystallization_ready(5, 3));
    }

    #[test]
    fn test_crystallization_not_ready_too_few() {
        let mut tic = Tic::new();
        tic.append(0, Q16::from_raw(100), 5);
        tic.append(1, Q16::from_raw(50), 5);
        assert!(!tic.crystallization_ready(5, 3));
    }
}
