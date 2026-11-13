//! Dual-mode distortion module ported from `CrushClone/Source/DSP/Distortion.cpp`:
//! - **Tube**: Asymmetric `tanh` waveshaper with DC bias for even-harmonic warmth.
//! - **Mech**: Hard clipper with cubic knee preceded by low+high shelf pre-emphasis EQ.
//! Both paths are 4x oversampled via polyphase IIR half-band filters.

use crate::biquad::{Biquad, BiquadCoeffs};
use crate::oversample::Oversampler4x;

#[derive(Debug, Clone)]
pub struct DistortionModule {
    oversamplers: [Oversampler4x; 2],
    mech_low_shelf: [Biquad; 2],
    mech_high_shelf: [Biquad; 2],
    sample_rate: f64,
}

impl DistortionModule {
    pub fn new(sample_rate: f64) -> Self {
        let mut module = Self {
            oversamplers: [Oversampler4x::new(); 2],
            mech_low_shelf: [Biquad::default(); 2],
            mech_high_shelf: [Biquad::default(); 2],
            sample_rate: sample_rate.max(1.0),
        };
        module.update_mech_eq();
        module
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate.max(1.0);
        self.update_mech_eq();
        self.reset();
    }

    pub fn reset(&mut self) {
        for ch in 0..2 {
            self.oversamplers[ch].reset();
            self.mech_low_shelf[ch].reset();
            self.mech_high_shelf[ch].reset();
        }
    }

    fn update_mech_eq(&mut self) {
        // Pre-emphasis shelves run at the 4x oversampled rate
        let os_rate = self.sample_rate * 4.0;
        let low_coeffs = BiquadCoeffs::low_shelf_linear(os_rate, 200.0, 0.7, 1.8);
        let high_coeffs = BiquadCoeffs::high_shelf_linear(os_rate, 4000.0, 0.7, 2.0);
        for ch in 0..2 {
            self.mech_low_shelf[ch].set_coeffs(low_coeffs);
            self.mech_high_shelf[ch].set_coeffs(high_coeffs);
        }
    }

    #[inline]
    pub fn tube_saturate(x: f32, drive: f32) -> f32 {
        let gain = 1.0 + drive * 19.0;
        let bias = drive * 0.08;
        let s = (x * gain + bias).tanh();
        s - bias.tanh()
    }

    #[inline]
    pub fn mech_clip(x: f32, drive: f32) -> f32 {
        let gain = 1.0 + drive * 29.0;
        let s = x * gain;
        if s > 1.0 {
            1.0
        } else if s < -1.0 {
            -1.0
        } else {
            let k = 0.15 * drive;
            s - k * (s * s * s)
        }
    }

    #[inline]
    pub fn process_stereo_sample(
        &mut self,
        left: f32,
        right: f32,
        tube: f32,
        mech: f32,
        enabled: bool,
    ) -> (f32, f32) {
        if !enabled || (tube < 0.001 && mech < 0.001) {
            return (left, right);
        }

        let out_l = self.process_channel_sample(0, left, tube, mech);
        let out_r = self.process_channel_sample(1, right, tube, mech);
        (out_l, out_r)
    }

    #[inline]
    fn process_channel_sample(&mut self, ch: usize, input: f32, tube: f32, mech: f32) -> f32 {
        let low_shelf = &mut self.mech_low_shelf[ch];
        let high_shelf = &mut self.mech_high_shelf[ch];

        self.oversamplers[ch].process_sample(input, |dry| {
            let mut out = 0.0_f32;

            if tube > 0.001 {
                out += tube * Self::tube_saturate(dry, tube);
            }

            if mech > 0.001 {
                let mut m = dry;
                m = low_shelf.process(m);
                m = high_shelf.process(m);
                m = Self::mech_clip(m, mech);
                out += mech * m;
            }

            let total_drive = tube + mech;
            if total_drive > 1.0 {
                out /= total_drive;
            }

            out
        })
    }
}
