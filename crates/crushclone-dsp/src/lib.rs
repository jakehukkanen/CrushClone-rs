//! Zero-allocation DSP core for `CrushClone-rs`.

pub mod biquad;
pub mod compressor;
pub mod distortion;
pub mod filter;
pub mod oversample;
pub mod presets;

pub use compressor::CompressorModule;
pub use distortion::DistortionModule;
pub use filter::LadderFilter;
pub use presets::{CrushPreset, FACTORY_PRESETS};

#[derive(Debug, Clone, Copy)]
pub struct CrushCloneParams {
    pub tube_amount: f32,
    pub mech_amount: f32,
    pub dist_enabled: bool,
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
    pub filter_enabled: bool,
    pub comp_amount: f32,
    pub phat_mode: bool,
    pub comp_enabled: bool,
    pub master_db: f32,
    pub dry_wet: f32,
}

impl Default for CrushCloneParams {
    fn default() -> Self {
        let p = FACTORY_PRESETS[0];
        Self {
            tube_amount: p.tube,
            mech_amount: p.mech,
            dist_enabled: p.dist_on,
            filter_cutoff_hz: p.cutoff,
            filter_resonance: p.reso,
            filter_enabled: p.filter_on,
            comp_amount: p.comp,
            phat_mode: p.phat,
            comp_enabled: p.comp_on,
            master_db: p.master_db,
            dry_wet: p.mix,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrushCloneDsp {
    distortion: DistortionModule,
    filter: LadderFilter,
    compressor: CompressorModule,
}

impl CrushCloneDsp {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            distortion: DistortionModule::new(sample_rate),
            filter: LadderFilter::new(sample_rate),
            compressor: CompressorModule::new(sample_rate),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.distortion.set_sample_rate(sample_rate);
        self.filter.set_sample_rate(sample_rate);
        self.compressor.set_sample_rate(sample_rate);
    }

    pub fn reset(&mut self) {
        self.distortion.reset();
        self.filter.reset();
        self.compressor.reset();
    }

    #[inline]
    pub fn gain_reduction_db(&self) -> f32 {
        self.compressor.gain_reduction_db()
    }

    /// Process a stereo block in-place with zero heap allocations.
    pub fn process_block(&mut self, left: &mut [f32], right: &mut [f32], params: &CrushCloneParams) {
        let len = left.len().min(right.len());
        let master_gain = if params.master_db.abs() > 0.05 {
            10.0_f32.powf(params.master_db / 20.0)
        } else {
            1.0
        };
        let dry_wet = params.dry_wet.clamp(0.0, 1.0);

        for i in 0..len {
            let dry_l = left[i];
            let dry_r = right[i];

            let (mut wet_l, mut wet_r) = self.distortion.process_stereo_sample(
                dry_l,
                dry_r,
                params.tube_amount,
                params.mech_amount,
                params.dist_enabled,
            );

            (wet_l, wet_r) = self.filter.process_stereo_sample(
                wet_l,
                wet_r,
                params.filter_cutoff_hz,
                params.filter_resonance,
                params.filter_enabled,
            );

            (wet_l, wet_r) = self.compressor.process_stereo_sample(
                wet_l,
                wet_r,
                params.comp_amount,
                params.phat_mode,
                params.comp_enabled,
            );

            wet_l *= master_gain;
            wet_r *= master_gain;

            if dry_wet < 0.999 {
                wet_l = dry_l * (1.0 - dry_wet) + wet_l * dry_wet;
                wet_r = dry_r * (1.0 - dry_wet) + wet_r * dry_wet;
            }

            left[i] = sanitize_sample(wet_l);
            right[i] = sanitize_sample(wet_r);
        }
    }
}

#[inline]
fn sanitize_sample(x: f32) -> f32 {
    if !x.is_finite() {
        0.0
    } else {
        x.clamp(-2.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bypass_is_clean() {
        let mut dsp = CrushCloneDsp::new(44100.0);
        let params = CrushCloneParams::default();
        let mut left = [0.25_f32, -0.5, 0.125, 0.0];
        let mut right = [-0.25_f32, 0.5, -0.125, 0.0];
        let orig_l = left;
        let orig_r = right;
        dsp.process_block(&mut left, &mut right, &params);
        assert_eq!(left, orig_l);
        assert_eq!(right, orig_r);
    }

    #[test]
    fn distortion_tube_and_mech_produce_bounded_finite_output() {
        let mut dsp = CrushCloneDsp::new(48000.0);
        let params = CrushCloneParams {
            tube_amount: 0.75,
            mech_amount: 0.65,
            dist_enabled: true,
            ..CrushCloneParams::default()
        };
        let mut left = [0.5_f32; 512];
        let mut right = [-0.5_f32; 512];
        dsp.process_block(&mut left, &mut right, &params);
        assert!(left.iter().all(|s| s.is_finite() && s.abs() <= 2.0));
        assert!(right.iter().all(|s| s.is_finite() && s.abs() <= 2.0));
    }

    #[test]
    fn compressor_and_phat_reduce_peaks_and_stay_finite() {
        let mut dsp = CrushCloneDsp::new(44100.0);
        let params = CrushCloneParams {
            comp_amount: 0.8,
            phat_mode: true,
            comp_enabled: true,
            ..CrushCloneParams::default()
        };
        let mut left = [0.9_f32; 256];
        let mut right = [0.9_f32; 256];
        dsp.process_block(&mut left, &mut right, &params);
        assert!(dsp.gain_reduction_db() < 0.0);
        assert!(left.iter().all(|s| s.is_finite() && s.abs() <= 2.0));
    }

    #[test]
    fn filter_attenuates_high_frequencies() {
        let mut dsp = CrushCloneDsp::new(44100.0);
        let params = CrushCloneParams {
            filter_enabled: true,
            filter_cutoff_hz: 400.0,
            filter_resonance: 0.2,
            ..CrushCloneParams::default()
        };
        let mut left = [0.0_f32; 512];
        let mut right = [0.0_f32; 512];
        for i in 0..512 {
            let s = (2.0 * std::f32::consts::PI * 10000.0 * (i as f32) / 44100.0).sin();
            left[i] = s;
            right[i] = s;
        }
        dsp.process_block(&mut left, &mut right, &params);
        let rms_tail: f32 = (left[256..].iter().map(|x| x * x).sum::<f32>() / 256.0).sqrt();
        assert!(rms_tail < 0.05, "expected high freq attenuation, got rms={rms_tail}");
    }
}
