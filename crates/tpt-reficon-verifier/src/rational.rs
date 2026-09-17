//! Exact rational arithmetic for the QF_LRA decision procedure.
//!
//! Rationals are stored as normalized `n/d` with `d > 0` and
//! `gcd(|n|, d) == 1`. All arithmetic is **checked**: any operation that would
//! overflow `i128` returns `None`, and callers (the Fourier–Motzkin engine)
//! conservatively degrade to "could not prove" rather than risk a spurious
//! verdict. This mirrors the soundness-first policy of the original
//! `tpt-telos-verifier` solver, which degraded to satisfiable on `i128`
//! overflow.

use core::cmp::Ordering;
use core::fmt;

/// An exact rational number.
///
/// # Invariants
/// - `d > 0`
/// - `gcd(|n|, d) == 1` (fully reduced)
/// - zero is always represented as `0/1`
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rat {
    pub(crate) n: i128,
    pub(crate) d: i128,
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

impl Rat {
    /// The rational zero.
    pub const ZERO: Rat = Rat { n: 0, d: 1 };
    /// The rational one.
    pub const ONE: Rat = Rat { n: 1, d: 1 };

    /// Constructs a normalized rational, or `None` on `d == 0` / overflow.
    pub fn new(n: i128, d: i128) -> Option<Rat> {
        if d == 0 {
            return None;
        }
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        if n == 0 {
            return Some(Rat { n: 0, d: 1 });
        }
        let g = gcd(n.unsigned_abs(), d.unsigned_abs());
        // g <= min(|n|, d), so these divisions cannot overflow.
        Some(Rat {
            n: n / g as i128,
            d: d / g as i128,
        })
    }

    /// Constructs a rational from a signed integer.
    pub fn from_i128(v: i128) -> Rat {
        Rat { n: v, d: 1 }
    }

    /// Parses an exact rational from a decimal literal string such as
    /// `"42"`, `"-1.25"` or `"3.14"`. Returns `None` for malformed input,
    /// overflow, or values needing more than 30 fractional digits.
    pub fn parse_decimal(text: &str) -> Option<Rat> {
        let text = text.trim();
        let (int_part, frac_part) = match text.split_once('.') {
            Some((a, b)) => (a, b),
            None => (text, ""),
        };
        let negative = int_part.starts_with('-');
        let int_digits = int_part.trim_start_matches(['+', '-']);
        if int_digits.is_empty() && frac_part.is_empty() {
            return None;
        }
        if !int_digits.chars().all(|c| c.is_ascii_digit())
            || !frac_part.chars().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        if frac_part.len() > 30 {
            return None;
        }
        let mut num: i128 = 0;
        for c in int_digits.chars().chain(frac_part.chars()) {
            num = num.checked_mul(10)?.checked_add((c as u8 - b'0') as i128)?;
        }
        let mut den: i128 = 1;
        for _ in 0..frac_part.len() {
            den = den.checked_mul(10)?;
        }
        Rat::new(if negative { -num } else { num }, den)
    }

    /// Numerator (signed, denominator positive).
    pub fn numer(&self) -> i128 {
        self.n
    }

    /// Denominator (always positive).
    pub fn denom(&self) -> i128 {
        self.d
    }

    /// True iff the value is exactly zero.
    pub fn is_zero(&self) -> bool {
        self.n == 0
    }

    /// True iff the value is strictly greater than zero.
    pub fn is_positive(&self) -> bool {
        self.n > 0
    }

    /// True iff the value is strictly less than zero.
    pub fn is_negative(&self) -> bool {
        self.n < 0
    }

    /// True iff the value is an exact integer.
    pub fn is_integer(&self) -> bool {
        self.d == 1
    }

    /// `n1/d1 + n2/d2 = (n1*d2 + n2*d1) / (d1*d2)`, checked.
    pub fn checked_add(self, rhs: Rat) -> Option<Rat> {
        let n = self
            .n
            .checked_mul(rhs.d)?
            .checked_add(rhs.n.checked_mul(self.d)?)?;
        let d = self.d.checked_mul(rhs.d)?;
        Rat::new(n, d)
    }

    /// Exact subtraction, checked for overflow.
    pub fn checked_sub(self, rhs: Rat) -> Option<Rat> {
        self.checked_add(rhs.checked_neg()?)
    }

    /// Exact negation, checked for overflow.
    pub fn checked_neg(self) -> Option<Rat> {
        Some(Rat {
            n: self.n.checked_neg()?,
            d: self.d,
        })
    }

    /// Exact multiplication, checked for overflow.
    pub fn checked_mul(self, rhs: Rat) -> Option<Rat> {
        Rat::new(self.n.checked_mul(rhs.n)?, self.d.checked_mul(rhs.d)?)
    }

    /// Multiplies by a raw integer, checking overflow.
    pub fn checked_scale(self, k: i128) -> Option<Rat> {
        Rat::new(self.n.checked_mul(k)?, self.d)
    }

    /// Division; `None` on division by zero or overflow.
    pub fn checked_div(self, rhs: Rat) -> Option<Rat> {
        if rhs.is_zero() {
            return None;
        }
        self.checked_mul(Rat { n: rhs.d, d: rhs.n })
    }

    /// Total order comparison with overflow detection.
    ///
    /// Returns `None` if the cross-multiplication overflows, signalling the
    /// caller to abort with a conservative verdict.
    pub fn checked_cmp(self, rhs: Rat) -> Option<Ordering> {
        // n1/d1 ? n2/d2  <=>  n1*d2 ? n2*d1   (both denominators positive)
        let l = self.n.checked_mul(rhs.d)?;
        let r = rhs.n.checked_mul(self.d)?;
        Some(l.cmp(&r))
    }

    /// Exact midpoint `(self + rhs) / 2`. Used for witness reconstruction.
    pub fn checked_midpoint(self, rhs: Rat) -> Option<Rat> {
        self.checked_add(rhs)?.checked_div(Rat::from_i128(2))
    }

    /// `self + 1` (used when reconstructing witnesses with one-sided bounds).
    pub fn checked_inc(self) -> Option<Rat> {
        self.checked_add(Rat::ONE)
    }

    /// `self - 1`.
    pub fn checked_dec(self) -> Option<Rat> {
        self.checked_sub(Rat::ONE)
    }

    /// Approximate `f64` rendering (diagnostics only, never for decisions).
    pub fn approx_f64(self) -> f64 {
        self.n as f64 / self.d as f64
    }
}

impl fmt::Display for Rat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer() {
            write!(f, "{}", self.n)
        } else {
            write!(f, "{}/{}", self.n, self.d)
        }
    }
}

impl fmt::Debug for Rat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl PartialOrd for Rat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rat {
    fn cmp(&self, other: &Self) -> Ordering {
        // Panics only on overflow, which decision paths avoid via
        // `checked_cmp`; this impl exists for sorting/bookkeeping on small
        // values only.
        self.checked_cmp(*other)
            .expect("rational comparison overflowed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_signs() {
        assert_eq!(Rat::new(6, 4), Some(Rat::new(3, 2).unwrap()));
        assert_eq!(Rat::new(-3, -2), Rat::new(3, 2));
        assert_eq!(Rat::new(0, 7), Some(Rat::ZERO));
    }

    #[test]
    fn rejects_zero_denominator() {
        assert_eq!(Rat::new(1, 0), None);
    }

    #[test]
    fn arithmetic_exact() {
        let half = Rat::new(1, 2).unwrap();
        let third = Rat::new(1, 3).unwrap();
        assert_eq!(half.checked_add(third), Rat::new(5, 6));
        assert_eq!(half.checked_sub(third), Rat::new(1, 6));
        assert_eq!(half.checked_mul(third), Rat::new(1, 6));
        assert_eq!(third.checked_div(half), Rat::new(2, 3));
        assert_eq!(Rat::from_i128(5).checked_div(Rat::ZERO), None);
    }

    #[test]
    fn decimal_parsing_is_exact() {
        let quarter = Rat::parse_decimal("0.25").unwrap();
        assert_eq!(quarter, Rat::new(1, 4).unwrap());
        let neg = Rat::parse_decimal("-2.5").unwrap();
        assert_eq!(neg.approx_f64(), -2.5);
        assert!(Rat::parse_decimal("abc").is_none());
        assert!(Rat::parse_decimal("").is_none());
    }

    #[test]
    fn comparison_and_midpoints() {
        let a = Rat::new(1, 3).unwrap();
        let b = Rat::new(1, 2).unwrap();
        assert_eq!(a.checked_cmp(b), Some(Ordering::Less));
        let mid = a.checked_midpoint(b).unwrap();
        assert_eq!(mid, Rat::new(5, 12).unwrap());
    }

    #[test]
    fn overflow_is_caught_not_panicking() {
        let big = Rat::from_i128(i128::MAX);
        assert_eq!(big.checked_add(Rat::ONE), None);
        assert_eq!(big.checked_mul(big), None);
        // Cross-multiplication overflows: (MAX/1) vs (MAX/2) → MAX*2.
        let a = Rat::new(i128::MAX, 1).unwrap();
        let b = Rat::new(i128::MAX, 2).unwrap();
        assert_eq!(a.checked_cmp(b), None);
    }
}
