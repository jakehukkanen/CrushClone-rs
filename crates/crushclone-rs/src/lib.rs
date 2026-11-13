//! `CrushClone-rs`: Zero-allocation VST3 and CLAP distortion, filter, and compressor plugin
//! ported from `doxiedevops/CrushClone` using `nih-plug` and `nih_plug_vizia`.

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crushclone_dsp::{CrushCloneDsp, CrushCloneParams};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

mod editor;

pub struct CrushClonePlugin {
    params: Arc<CrushClonePluginParams>,
    dsp: CrushCloneDsp,
    gain_reduction_db: Arc<AtomicU32>,
}

#[derive(Params)]
pub struct CrushClonePluginParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "tube_amount"]
    pub tube_amount: FloatParam,

    #[id = "mech_amount"]
    pub mech_amount: FloatParam,

    #[id = "dist_enable"]
    pub dist_enable: BoolParam,

    #[id = "filter_cutoff"]
    pub filter_cutoff: FloatParam,

    #[id = "filter_reso"]
    pub filter_reso: FloatParam,

    #[id = "filter_enable"]
    pub filter_enable: BoolParam,

    #[id = "comp_amount"]
    pub comp_amount: FloatParam,

    #[id = "phat_mode"]
    pub phat_mode: BoolParam,

    #[id = "comp_enable"]
    pub comp_enable: BoolParam,

    #[id = "master_volume"]
    pub master_volume: FloatParam,

    #[id = "dry_wet"]
    pub dry_wet: FloatParam,
}

impl Default for CrushClonePlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(CrushClonePluginParams::default()),
            dsp: CrushCloneDsp::new(44100.0),
            gain_reduction_db: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
        }
    }
}

impl Default for CrushClonePluginParams {
    fn default() -> Self {
        let def = CrushCloneParams::default();
        Self {
            editor_state: editor::default_state(),

            tube_amount: FloatParam::new(
                "Tube",
                def.tube_amount,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            mech_amount: FloatParam::new(
                "Mech",
                def.mech_amount,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            dist_enable: BoolParam::new("Distortion On", def.dist_enabled),

            filter_cutoff: FloatParam::new(
                "Cutoff",
                def.filter_cutoff_hz,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(15.0))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),

            filter_reso: FloatParam::new(
                "Resonance",
                def.filter_resonance,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            filter_enable: BoolParam::new("Filter On", def.filter_enabled),

            comp_amount: FloatParam::new(
                "Comp Amount",
                def.comp_amount,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            phat_mode: BoolParam::new("Phat", def.phat_mode),

            comp_enable: BoolParam::new("Compressor On", def.comp_enabled),

            master_volume: FloatParam::new(
                "Master",
                def.master_db,
                FloatRange::Linear {
                    min: -24.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            dry_wet: FloatParam::new(
                "Mix",
                def.dry_wet,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}

impl Plugin for CrushClonePlugin {
    const NAME: &'static str = "CrushClone-rs";
    const VENDOR: &'static str = "Jake Hukkanen";
    const URL: &'static str = "https://github.com/doxiedevops/CrushClone";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.gain_reduction_db.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.dsp.set_sample_rate(buffer_config.sample_rate as f64);
        self.dsp.reset();
        self.gain_reduction_db
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
        true
    }

    fn reset(&mut self) {
        self.dsp.reset();
        self.gain_reduction_db
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();
        let channels = buffer.as_slice();
        if channels.len() < 2 {
            return ProcessStatus::Normal;
        }

        let (left_slice, right_rest) = channels.split_at_mut(1);
        let left = &mut left_slice[0][..num_samples];
        let right = &mut right_rest[0][..num_samples];

        const SUB_BLOCK: usize = 32;
        let mut offset = 0;
        while offset < num_samples {
            let len = (num_samples - offset).min(SUB_BLOCK);
            let step = len as u32;

            let dsp_params = CrushCloneParams {
                tube_amount: self.params.tube_amount.smoothed.next_step(step),
                mech_amount: self.params.mech_amount.smoothed.next_step(step),
                dist_enabled: self.params.dist_enable.value(),
                filter_cutoff_hz: self.params.filter_cutoff.smoothed.next_step(step),
                filter_resonance: self.params.filter_reso.smoothed.next_step(step),
                filter_enabled: self.params.filter_enable.value(),
                comp_amount: self.params.comp_amount.smoothed.next_step(step),
                phat_mode: self.params.phat_mode.value(),
                comp_enabled: self.params.comp_enable.value(),
                master_db: self.params.master_volume.smoothed.next_step(step),
                dry_wet: self.params.dry_wet.smoothed.next_step(step),
            };

            self.dsp.process_block(
                &mut left[offset..offset + len],
                &mut right[offset..offset + len],
                &dsp_params,
            );
            offset += len;
        }

        self.gain_reduction_db
            .store(self.dsp.gain_reduction_db().to_bits(), Ordering::Relaxed);

        ProcessStatus::Normal
    }
}

impl ClapPlugin for CrushClonePlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.crushclone";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Multi-mode distortion, resonant low-pass filter, and Phat compressor");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Filter,
        ClapFeature::Compressor,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for CrushClonePlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JH_CrushClone_rs";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Distortion,
        Vst3SubCategory::Filter,
        Vst3SubCategory::Dynamics,
    ];
}

nih_export_clap!(CrushClonePlugin);
nih_export_vst3!(CrushClonePlugin);
