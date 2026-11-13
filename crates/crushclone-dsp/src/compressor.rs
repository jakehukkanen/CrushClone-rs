//! One-knob compressor with Phat parallel mode, ported from `CrushClone/Source/DSP/Compressor.cpp`.

use crate::biquad::{Biquad, BiquadCoeffs};

#[derive(Debug, Clone)]
pub struct CompressorModule {
    phat_shelf: [Biquad; 2],
    envelope_state: f32,
    gain_reduction_db: f32,
    sample_rate: f64,
}

impl CompressorModule {
    pub fn new(sample_rate: f64) -> Self {
        let mut comp = Self {
            phat_shelf: [Biquad::default(); 2],
            envelope_state: 0.0,
            gain_reduction_db: 0.0,
            sample_rate: sample_rate.max(1.0),
        };
        comp.update_phat_shelf();
        comp
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate.max(1.0);
        self.update_phat_shelf();
        self.reset();
    }

    pub fn reset(&mut self) {
        self.envelope_state = 0.0;
        self.gain_reduction_db = 0.0;
        for ch in 0..2 {
            self.phat_shelf[ch].reset();
        }
    }

    #[inline]
    pub fn gain_reduction_db(&self) -> f32 {
        self.gain_reduction_db
    }

    fn update_phat_shelf(&mut self) {
        // Phat mode low shelf: +8 dB (2.5 linear) at 120 Hz
        let coeffs = BiquadCoeffs::low_shelf_linear(self.sample_rate, 120.0, 0.7, 2.5);
        for ch in 0..2 {
            self.phat_shelf[ch].set_coeffs(coeffs);
        }
    }

    #[inline]
    fn compute_envelope(&mut self, input_level: f32, amount: f32) -> f32 {
        let attack_ms = 10.0 + amount * (1.0 - 10.0);
        let release_ms = 150.0 + amount * (50.0 - 150.0);

        let sr = self.sample_rate as f32;
        let attack_coeff = (-1.0 / (attack_ms * 0.001 * sr)).exp();
        let release_coeff = (-1.0 / (release_ms * 0.001 * sr)).exp();

        let coeff = if input_level > self.envelope_state {
            attack_coeff
        } else {
            release_coeff
        };
        self.envelope_state = coeff * self.envelope_state + (1.0 - coeff) * input_level;
        self.envelope_state
    }

    #[inline]
    fn compute_gain(envelope_db: f32, amount: f32) -> f32 {
        let threshold = amount * -30.0;
        let ratio = 1.0 + amount * 7.0;
        let knee_width = 6.0;

        if envelope_db < threshold - knee_width * 0.5 {
            0.0
        } else if envelope_db > threshold + knee_width * 0.5 {
            (threshold + (envelope_db - threshold) / ratio) - envelope_db
        } else {
            let x = envelope_db - threshold + knee_width * 0.5;
            ((1.0 / ratio - 1.0) * x * x) / (2.0 * knee_width)
        }
    }

    #[inline]
    pub fn process_stereo_sample(
        &mut self,
        left: f32,
        right: f32,
        amount: f32,
        phat_mode: bool,
        enabled: bool,
    ) -> (f32, f32) {
        if !enabled || amount < 0.001 {
            self.gain_reduction_db = 0.0;
            return (left, right);
        }

        let makeup_db = amount * 12.0;
        let makeup_gain = 10.0_f32.powf(makeup_db / 20.0);

        let peak_level = left.abs().max(right.abs());
        let env = self.compute_envelope(peak_level, amount).max(1e-8);
        let env_db = 20.0 * env.log10();
        let gain_db = Self::compute_gain(env_db, amount);
        self.gain_reduction_db = gain_db;

        let gain_linear = 10.0_f32.powf(gain_db / 20.0) * makeup_gain;

        let wet_l = left * gain_linear;
        let wet_r = right * gain_linear;

        if phat_mode {
            let phat_mix = 0.2 + amount * (0.65 - 0.2);
            let shelved_l = self.phat_shelf[0].process(wet_l);
            let shelved_r = self.phat_shelf[1].process(wet_r);
            (
                left * (1.0 - phat_mix) + shelved_l * phat_mix,
                right * (1.0 - phat_mix) + shelved_r * phat_mix,
            )
        } else {
            (wet_l, wet_r)
        }
    }
}
