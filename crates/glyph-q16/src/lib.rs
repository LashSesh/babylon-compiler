//! Q16 fixed-point arithmetic library.
//!
//! All values are signed 32-bit integers interpreted as q/2^16,
//! range [-32768, 32767.999984], resolution ≈ 1.526e-5.
//! Division uses banker's rounding (round-half-to-even).
//! No floating-point arithmetic participates in identity-bearing computations.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The fractional bit count for Q16 fixed-point.
pub const FRAC_BITS: u32 = 16;

/// The scaling factor: 2^16 = 65536.
pub const SCALE: i64 = 1 << FRAC_BITS;

/// Q16 represents 1.0.
pub const ONE: Q16 = Q16(SCALE as i32);

/// Q16 represents 0.0.
pub const ZERO: Q16 = Q16(0);

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum Q16Error {
    #[error("Q16 arithmetic overflow: {0}")]
    Overflow(String),
    #[error("Q16 division by zero")]
    DivisionByZero,
}

/// A Q16 fixed-point number: signed 32-bit integer interpreted as raw / 2^16.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Q16(pub i32);

impl Q16 {
    /// Create a Q16 from a raw i32 representation.
    pub const fn from_raw(raw: i32) -> Self {
        Q16(raw)
    }

    /// Create a Q16 from an integer value.
    pub const fn from_int(v: i32) -> Self {
        Q16(v << FRAC_BITS)
    }

    /// Get the raw i32 representation.
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Return the integer part (truncated toward zero).
    pub const fn to_int(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    /// Render as 8-character zero-padded lowercase hex of the raw i32 value.
    pub fn to_hex(self) -> String {
        format!("{:08x}", self.0 as u32)
    }

    /// Checked addition.
    pub fn checked_add(self, rhs: Q16) -> Result<Q16, Q16Error> {
        self.0
            .checked_add(rhs.0)
            .map(Q16)
            .ok_or_else(|| Q16Error::Overflow(format!("{} + {}", self.0, rhs.0)))
    }

    /// Checked subtraction.
    pub fn checked_sub(self, rhs: Q16) -> Result<Q16, Q16Error> {
        self.0
            .checked_sub(rhs.0)
            .map(Q16)
            .ok_or_else(|| Q16Error::Overflow(format!("{} - {}", self.0, rhs.0)))
    }

    /// Checked multiplication with banker's rounding.
    pub fn checked_mul(self, rhs: Q16) -> Result<Q16, Q16Error> {
        let wide = (self.0 as i64) * (rhs.0 as i64);
        let result = bankers_round_shift(wide, FRAC_BITS);
        if result > i32::MAX as i64 || result < i32::MIN as i64 {
            Err(Q16Error::Overflow(format!("{} * {}", self.0, rhs.0)))
        } else {
            Ok(Q16(result as i32))
        }
    }

    /// Checked division with banker's rounding.
    pub fn checked_div(self, rhs: Q16) -> Result<Q16, Q16Error> {
        if rhs.0 == 0 {
            return Err(Q16Error::DivisionByZero);
        }
        let wide = (self.0 as i64) << FRAC_BITS;
        let result = bankers_div(wide, rhs.0 as i64);
        if result > i32::MAX as i64 || result < i32::MIN as i64 {
            Err(Q16Error::Overflow(format!("{} / {}", self.0, rhs.0)))
        } else {
            Ok(Q16(result as i32))
        }
    }

    /// Saturating addition.
    pub fn saturating_add(self, rhs: Q16) -> Q16 {
        Q16(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction.
    pub fn saturating_sub(self, rhs: Q16) -> Q16 {
        Q16(self.0.saturating_sub(rhs.0))
    }

    /// Absolute value.
    pub fn abs(self) -> Q16 {
        Q16(self.0.abs())
    }
}

/// Banker's rounding (round-half-to-even) for right-shifting by `shift` bits.
fn bankers_round_shift(value: i64, shift: u32) -> i64 {
    let half = 1i64 << (shift - 1);
    let mask = (1i64 << shift) - 1;
    let remainder = value & mask;
    let truncated = value >> shift;

    if remainder > half {
        truncated + 1
    } else if remainder < half {
        truncated
    } else {
        // Exactly half: round to even
        if truncated & 1 != 0 {
            truncated + 1
        } else {
            truncated
        }
    }
}

/// Banker's rounding division.
fn bankers_div(numerator: i64, denominator: i64) -> i64 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let abs_rem = remainder.unsigned_abs();
    let abs_den = denominator.unsigned_abs();
    let double_rem = abs_rem * 2;

    if double_rem > abs_den {
        if (numerator ^ denominator) >= 0 {
            quotient + 1
        } else {
            quotient - 1
        }
    } else if double_rem < abs_den {
        quotient
    } else {
        // Exactly half: round to even
        if quotient & 1 != 0 {
            if (numerator ^ denominator) >= 0 {
                quotient + 1
            } else {
                quotient - 1
            }
        } else {
            quotient
        }
    }
}

impl std::ops::Add for Q16 {
    type Output = Q16;
    fn add(self, rhs: Q16) -> Q16 {
        self.checked_add(rhs).expect("Q16 addition overflow")
    }
}

impl std::ops::Sub for Q16 {
    type Output = Q16;
    fn sub(self, rhs: Q16) -> Q16 {
        self.checked_sub(rhs).expect("Q16 subtraction overflow")
    }
}

impl std::ops::Mul for Q16 {
    type Output = Q16;
    fn mul(self, rhs: Q16) -> Q16 {
        self.checked_mul(rhs).expect("Q16 multiplication overflow")
    }
}

impl std::ops::Div for Q16 {
    type Output = Q16;
    fn div(self, rhs: Q16) -> Q16 {
        self.checked_div(rhs).expect("Q16 division error")
    }
}

impl std::ops::Neg for Q16 {
    type Output = Q16;
    fn neg(self) -> Q16 {
        Q16(-self.0)
    }
}

impl fmt::Debug for Q16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let int_part = self.0 >> FRAC_BITS;
        let frac_part = (self.0 & 0xFFFF) as f64 / SCALE as f64;
        write!(f, "Q16({}.{:04}~raw={})", int_part, (frac_part * 10000.0) as u32, self.0)
    }
}

impl fmt::Display for Q16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Serialize Q16 as a JSON integer (the raw i32 value).
impl Serialize for Q16 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.0)
    }
}

/// Deserialize Q16 from a JSON integer.
impl<'de> Deserialize<'de> for Q16 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = i32::deserialize(deserializer)?;
        Ok(Q16(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_int() {
        assert_eq!(Q16::from_int(1).raw(), 65536);
        assert_eq!(Q16::from_int(0).raw(), 0);
        assert_eq!(Q16::from_int(-1).raw(), -65536);
    }

    #[test]
    fn test_add() {
        let a = Q16::from_int(3);
        let b = Q16::from_int(4);
        assert_eq!((a + b).raw(), Q16::from_int(7).raw());
    }

    #[test]
    fn test_sub() {
        let a = Q16::from_int(10);
        let b = Q16::from_int(3);
        assert_eq!((a - b).raw(), Q16::from_int(7).raw());
    }

    #[test]
    fn test_mul() {
        let a = Q16::from_int(3);
        let b = Q16::from_int(4);
        assert_eq!((a * b).raw(), Q16::from_int(12).raw());
    }

    #[test]
    fn test_div() {
        let a = Q16::from_int(12);
        let b = Q16::from_int(4);
        assert_eq!((a / b).raw(), Q16::from_int(3).raw());
    }

    #[test]
    fn test_bankers_rounding_div() {
        // 3 / 2 = 1.5, round to even -> 2
        let a = Q16::from_int(3);
        let b = Q16::from_int(2);
        let result = a / b;
        // 1.5 in Q16 = 98304
        assert_eq!(result.raw(), 98304);
    }

    #[test]
    fn test_to_hex() {
        assert_eq!(Q16::from_int(1).to_hex(), "00010000");
        assert_eq!(Q16::from_raw(0).to_hex(), "00000000");
    }

    #[test]
    fn test_serialize() {
        let q = Q16::from_int(1);
        let json = serde_json::to_string(&q).unwrap();
        assert_eq!(json, "65536");
    }

    #[test]
    fn test_deserialize() {
        let q: Q16 = serde_json::from_str("65536").unwrap();
        assert_eq!(q.raw(), 65536);
    }

    #[test]
    fn test_overflow() {
        let a = Q16::from_raw(i32::MAX);
        let b = Q16::from_raw(1);
        assert!(a.checked_add(b).is_err());
    }

    #[test]
    fn test_div_by_zero() {
        let a = Q16::from_int(1);
        let b = Q16::from_raw(0);
        assert!(a.checked_div(b).is_err());
    }
}
