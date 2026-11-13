//! Resonant low-pass filter ported from `CrushClone/Source/DSP/LadderFilter.cpp`.
//!
//! Uses a numerically stable trapezoidal State Variable TPT Filter (Cytomic / JUCE
//! `StateVariableTPTFilterType::lowpass`) with per-channel state.

use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, Default)]
struct TptSvf {
    s1: f64,
    s2: f64,
    g: f64,
    r2: f64,
    h: f64,
    last_cutoff: f32,
    last_reso: f32,
    last_sr: f64,
}

impl TptSvf {
    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline]
    fn update_coeffs(&mut self, sample_rate: f64, cutoff_hz: f32, resonance: f32) {
        if (cutoff_hz - self.last_cutoff).abs() > 1e-4
            || (resonance - self.last_reso).abs() > 1e-5
            || (sample_rate - self.last_sr).abs() > 1e-3
        {
            self.last_cutoff = cutoff_hz;
            self.last_reso = resonance;
            self.last_sr = sample_rate;

            let g = (PI * (cutoff_hz as f64) / sample_rate).tan();
            let r2 = 1.0 / (resonance as f64).max(0.01);
            let h = 1.0 / (1.0 + r2 * g + g * g);

            self.g = g;
            self.r2 = r2;
            self.h = h;
        }
    }

    #[inline]
    fn process_lowpass(&mut self, input: f32) -> f32 {
        let x = input as f64;
        let hp = (x - (self.r2 + self.g) * self.s1 - self.s2) * self.h;
        let bp = self.g * hp + self.s1;
        self.s1 = self.g * hp + bp;
        let lp = self.g * bp + self.s2;
        self.s2 = self.g * bp + lp;
        lp as f32
    }
}

#[derive(Debug, Clone)]
pub struct LadderFilter {
    svf: [TptSvf; 2],
    sample_rate: f64,
}

impl LadderFilter {
    pub fn new(sample_rate: f64) -> Self {
        let mut filter = Self {
            svf: [TptSvf::default(); 2],
            sample_rate: sample_rate.max(1.0),
        };
        for ch in 0..2 {
            filter.svf[ch].update_coeffs(filter.sample_rate, 20000.0, 0.7071);
        }
        filter
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate.max(1.0);
        self.reset();
    }

    pub fn reset(&mut self) {
        for ch in 0..2 {
            self.svf[ch].reset();
        }
    }

    #[inline]
    pub fn process_stereo_sample(
        &mut self,
        left: f32,
        right: f32,
        cutoff_hz: f32,
        resonance: f32,
        enabled: bool,
    ) -> (f32, f32) {
        if !enabled {
            return (left, right);
        }

        // Bypass when wide open with negligible resonance (matches CrushClone)
        if cutoff_hz >= 18000.0 && resonance < 0.05 {
            return (left, right);
        }

        let max_safe = (self.sample_rate as f32) * 0.4;
        let safe_cutoff = cutoff_hz.clamp(20.0, max_safe.max(20.0));
        let safe_reso = 0.7071 + resonance.clamp(0.0, 1.0) * 3.5;

        self.svf[0].update_coeffs(self.sample_rate, safe_cutoff, safe_reso);
        self.svf[1].update_coeffs(self.sample_rate, safe_cutoff, safe_reso);

        (
            self.svf[0].process_lowpass(left),
            self.svf[1].process_lowpass(right),
        )
    }
}
