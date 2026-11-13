//! Zero-allocation RBJ biquad filters.

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct BiquadCoeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

impl BiquadCoeffs {
    #[inline]
    fn normalize(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        let inv_a0 = 1.0 / a0;
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        }
    }

    #[inline]
    fn safe_w0(sample_rate: f64, freq_hz: f64) -> f64 {
        let clamped_freq = freq_hz.clamp(1.0, sample_rate * 0.45);
        2.0 * PI * clamped_freq / sample_rate
    }

    /// 2nd-order RBJ Low Shelf filter with linear gain factor.
    pub fn low_shelf_linear(sample_rate: f64, freq_hz: f64, q: f64, linear_gain: f64) -> Self {
        let w0 = Self::safe_w0(sample_rate, freq_hz);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let a = linear_gain.max(1e-6).sqrt();
        let alpha = sin_w0 / (2.0 * q.max(0.01));
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::normalize(b0, b1, b2, a0, a1, a2)
    }

    /// 2nd-order RBJ High Shelf filter with linear gain factor.
    pub fn high_shelf_linear(sample_rate: f64, freq_hz: f64, q: f64, linear_gain: f64) -> Self {
        let w0 = Self::safe_w0(sample_rate, freq_hz);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let a = linear_gain.max(1e-6).sqrt();
        let alpha = sin_w0 / (2.0 * q.max(0.01));
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::normalize(b0, b1, b2, a0, a1, a2)
    }
}

/// Direct Form II Transposed biquad filter (64-bit internal state for precision).
#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    coeffs: BiquadCoeffs,
    z1: f64,
    z2: f64,
}

impl Biquad {
    #[inline]
    pub fn set_coeffs(&mut self, coeffs: BiquadCoeffs) {
        self.coeffs = coeffs;
    }

    #[inline]
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let x = input as f64;
        let y = self.coeffs.b0 * x + self.z1;
        self.z1 = self.coeffs.b1 * x - self.coeffs.a1 * y + self.z2;
        self.z2 = self.coeffs.b2 * x - self.coeffs.a2 * y;
        y as f32
    }
}
