//! Warm dark-mode skeuomorphic Vizia GUI for `CrushClone-rs`.
//!
//! Layout is organized into four hardware modules:
//! - **DISTORTION**: `ON` LED toggle, `TUBE`, `MECH`
//! - **FILTER**: `ON` LED toggle, `CUTOFF`, `RESONANCE`
//! - **COMPRESSOR**: `PHAT` LED toggle, `ON` LED toggle, `AMOUNT`, live `GR` meter
//! - **MASTER / MIX**: `MASTER`, `MIX`

use std::f32::consts::PI;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crushclone_dsp::{CrushPreset, FACTORY_PRESETS};
use nih_plug::prelude::{Editor, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};

use crate::CrushClonePluginParams;

const BASE_WIDTH: u32 = 520;
const BASE_HEIGHT: u32 = 420;
const DEFAULT_SCALE: f64 = 1.25;

const KNOB_DRAG_PIXELS: f32 = 160.0;
const KNOB_FINE_MULTIPLIER: f32 = 0.12;

#[derive(Lens)]
pub(crate) struct UiData {
    pub params: Arc<CrushClonePluginParams>,
    pub preset_index: usize,
    pub preset_name: String,
    #[lens(ignore)]
    pub gui_context: Arc<dyn GuiContext>,
}

pub(crate) enum UiEvent {
    PrevPreset,
    NextPreset,
}

impl Model for UiData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event, _| match ui_event {
            UiEvent::PrevPreset => {
                if self.preset_index == 0 {
                    self.preset_index = FACTORY_PRESETS.len() - 1;
                } else {
                    self.preset_index -= 1;
                }
                let preset = FACTORY_PRESETS[self.preset_index];
                self.preset_name = preset.name.to_string();
                apply_preset(&self.params, self.gui_context.as_ref(), &preset);
            }
            UiEvent::NextPreset => {
                self.preset_index = (self.preset_index + 1) % FACTORY_PRESETS.len();
                let preset = FACTORY_PRESETS[self.preset_index];
                self.preset_name = preset.name.to_string();
                apply_preset(&self.params, self.gui_context.as_ref(), &preset);
            }
        });
    }
}

fn apply_preset(params: &CrushClonePluginParams, gui_context: &dyn GuiContext, p: &CrushPreset) {
    let setter = ParamSetter::new(gui_context);
    let set_f = |param: &nih_plug::prelude::FloatParam, val: f32| {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, val);
        setter.end_set_parameter(param);
    };
    let set_b = |param: &nih_plug::prelude::BoolParam, val: bool| {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, val);
        setter.end_set_parameter(param);
    };

    set_f(&params.tube_amount, p.tube);
    set_f(&params.mech_amount, p.mech);
    set_b(&params.dist_enable, p.dist_on);
    set_f(&params.filter_cutoff, p.cutoff);
    set_f(&params.filter_reso, p.reso);
    set_b(&params.filter_enable, p.filter_on);
    set_f(&params.comp_amount, p.comp);
    set_b(&params.phat_mode, p.phat);
    set_b(&params.comp_enable, p.comp_on);
    set_f(&params.master_volume, p.master_db);
    set_f(&params.dry_wet, p.mix);
}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new_with_default_scale_factor(|| (BASE_WIDTH, BASE_HEIGHT), DEFAULT_SCALE)
}

#[derive(Debug, Clone, Copy)]
struct DragState {
    start_y: f32,
    start_val: f32,
}

/// Skeuomorphic 3D rotary knob widget rendered via `femtovg` (`vizia::vg`).
pub struct RotaryKnob {
    param_base: ParamWidgetBase,
    drag_active: bool,
    drag_state: Option<DragState>,
    scrolled_lines: f32,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
}

impl RotaryKnob {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        bipolar: bool,
        accent_rgb: (u8, u8, u8),
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            drag_active: false,
            drag_state: None,
            scrolled_lines: 0.0,
            bipolar,
            accent_rgb,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let norm_lens =
                    param_data.make_lens(|param| param.modulated_normalized_value());
                Binding::new(cx, norm_lens, |cx, _| {
                    cx.needs_redraw();
                    Element::new(cx).hoverable(false);
                });
            }),
        )
    }

    fn reset_to_default(&mut self, cx: &mut EventContext) {
        if self.drag_active {
            self.drag_active = false;
            self.drag_state = None;
            cx.release();
            cx.set_active(false);
            self.param_base.end_set_parameter(cx);
        }
        let target = self.param_base.default_normalized_value();
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, target);
        self.param_base.end_set_parameter(cx);
        cx.needs_redraw();
    }
}

impl View for RotaryKnob {
    fn element(&self) -> Option<&'static str> {
        Some("rotary-knob")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    self.reset_to_default(cx);
                } else {
                    self.drag_active = true;
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.param_base.begin_set_parameter(cx);
                    self.drag_state = Some(DragState {
                        start_y: cx.mouse().cursory,
                        start_val: self.param_base.unmodulated_normalized_value(),
                    });
                    cx.needs_redraw();
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right)
            | WindowEvent::MouseTripleClick(MouseButton::Right) => {
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    self.drag_active = false;
                    self.drag_state = None;
                    cx.release();
                    cx.set_active(false);
                    self.param_base.end_set_parameter(cx);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
                if self.drag_active {
                    let state = self.drag_state.get_or_insert(DragState {
                        start_y: *y,
                        start_val: self.param_base.unmodulated_normalized_value(),
                    });

                    let sensitivity = if cx.modifiers().shift() {
                        KNOB_FINE_MULTIPLIER
                    } else {
                        1.0
                    };

                    let dy = (state.start_y - *y) / (KNOB_DRAG_PIXELS * cx.scale_factor());
                    let new_val = (state.start_val + dy * sensitivity).clamp(0.0, 1.0);
                    self.param_base.set_normalized_value(cx, new_val);

                    state.start_y = *y;
                    state.start_val = new_val;

                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseScroll(_scroll_x, scroll_y) => {
                self.scrolled_lines += scroll_y;
                if self.scrolled_lines.abs() >= 0.5 {
                    let finer = cx.modifiers().shift();
                    if !self.drag_active {
                        self.param_base.begin_set_parameter(cx);
                    }

                    let mut current = self.param_base.unmodulated_normalized_value();
                    let step = if finer { 0.004 } else { 0.02 };
                    let delta = self.scrolled_lines * step;
                    current = (current + delta).clamp(0.0, 1.0);
                    self.param_base.set_normalized_value(cx, current);
                    self.scrolled_lines = 0.0;

                    if !self.drag_active {
                        self.param_base.end_set_parameter(cx);
                    }
                    cx.needs_redraw();
                }
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let dpx = cx.scale_factor();
        let center_x = bounds.x + bounds.w * 0.5;
        let center_y = bounds.y + bounds.h * 0.52;
        let max_r = (bounds.w.min(bounds.h) * 0.5) - 2.0 * dpx;

        let arc_r = max_r * 0.90;
        let body_r = max_r * 0.70;
        let cap_r = max_r * 0.57;

        let norm = self.param_base.modulated_normalized_value().clamp(0.0, 1.0);

        let start_angle = 0.75 * PI;
        let sweep_angle = 1.50 * PI;
        let end_angle = start_angle + sweep_angle;
        let val_angle = start_angle + norm * sweep_angle;

        // 1. Tick marks
        let num_ticks = 11;
        for i in 0..num_ticks {
            let t = i as f32 / (num_ticks - 1) as f32;
            let a = start_angle + t * sweep_angle;
            let r_in = max_r * 0.94;
            let r_out = max_r * 1.02;
            let mut tick = vg::Path::new();
            tick.move_to(center_x + a.cos() * r_in, center_y + a.sin() * r_in);
            tick.line_to(center_x + a.cos() * r_out, center_y + a.sin() * r_out);
            let is_major = i == 0 || i == num_ticks - 1 || i == num_ticks / 2;
            let mut paint = if is_major {
                vg::Paint::color(vg::Color::rgba(218, 198, 175, 215))
            } else {
                vg::Paint::color(vg::Color::rgba(155, 138, 120, 155))
            };
            paint.set_line_width((if is_major { 1.4 } else { 1.0 }) * dpx);
            canvas.stroke_path(&tick, &paint);
        }

        // 2. Recessed LED track
        let mut track_path = vg::Path::new();
        track_path.arc(center_x, center_y, arc_r, start_angle, end_angle, vg::Solidity::Hole);
        let mut track_paint = vg::Paint::color(vg::Color::rgba(30, 25, 21, 245));
        track_paint.set_line_width(3.4 * dpx);
        track_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&track_path, &track_paint);

        // 3. Illuminated LED arc
        let (ar, ag, ab) = self.accent_rgb;
        let (arc_a0, arc_a1) = if self.bipolar {
            let mid_angle = start_angle + 0.5 * sweep_angle;
            if val_angle >= mid_angle {
                (mid_angle, val_angle)
            } else {
                (val_angle, mid_angle)
            }
        } else {
            (start_angle, val_angle)
        };

        if (arc_a1 - arc_a0).abs() > 0.015 {
            let mut glow_path = vg::Path::new();
            glow_path.arc(center_x, center_y, arc_r, arc_a0, arc_a1, vg::Solidity::Hole);
            let mut glow_paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 80));
            glow_paint.set_line_width(6.5 * dpx);
            glow_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&glow_path, &glow_paint);

            let mut val_path = vg::Path::new();
            val_path.arc(center_x, center_y, arc_r, arc_a0, arc_a1, vg::Solidity::Hole);
            let mut val_paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 250));
            val_paint.set_line_width(2.8 * dpx);
            val_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&val_path, &val_paint);
        }

        // 4. Cast drop shadow
        let mut shadow_path = vg::Path::new();
        shadow_path.circle(center_x, center_y + 3.2 * dpx, body_r * 1.03);
        let shadow_paint = vg::Paint::radial_gradient(
            center_x,
            center_y + 3.2 * dpx,
            body_r * 0.5,
            body_r * 1.12,
            vg::Color::rgba(14, 11, 8, 200),
            vg::Color::rgba(14, 11, 8, 0),
        );
        canvas.fill_path(&shadow_path, &shadow_paint);

        // 5. Knurled outer skirt
        let mut body_path = vg::Path::new();
        body_path.circle(center_x, center_y, body_r);
        let body_paint = vg::Paint::linear_gradient(
            center_x,
            center_y - body_r,
            center_x,
            center_y + body_r,
            vg::Color::rgb(112, 98, 86),
            vg::Color::rgb(50, 42, 36),
        );
        canvas.fill_path(&body_path, &body_paint);

        let mut rim_stroke = vg::Paint::color(vg::Color::rgba(24, 20, 17, 235));
        rim_stroke.set_line_width(1.3 * dpx);
        canvas.stroke_path(&body_path, &rim_stroke);

        let serrations = 24;
        for i in 0..serrations {
            let a = (i as f32 / serrations as f32) * 2.0 * PI + norm * 1.2;
            let r0 = cap_r * 1.02;
            let r1 = body_r * 0.96;
            let mut groove = vg::Path::new();
            groove.move_to(center_x + a.cos() * r0, center_y + a.sin() * r0);
            groove.line_to(center_x + a.cos() * r1, center_y + a.sin() * r1);
            let mut gpaint = vg::Paint::color(vg::Color::rgba(32, 26, 22, 135));
            gpaint.set_line_width(1.0 * dpx);
            canvas.stroke_path(&groove, &gpaint);
        }

        // 6. Top brushed-bronze cap
        let mut cap_path = vg::Path::new();
        cap_path.circle(center_x, center_y, cap_r);
        let cap_paint = vg::Paint::radial_gradient(
            center_x - cap_r * 0.3,
            center_y - cap_r * 0.35,
            cap_r * 0.08,
            cap_r * 1.1,
            vg::Color::rgb(138, 122, 108),
            vg::Color::rgb(62, 53, 46),
        );
        canvas.fill_path(&cap_path, &cap_paint);

        let mut cap_ring = vg::Paint::linear_gradient(
            center_x,
            center_y - cap_r,
            center_x,
            center_y + cap_r,
            vg::Color::rgba(225, 204, 182, 140),
            vg::Color::rgba(28, 23, 19, 200),
        );
        cap_ring.set_line_width(1.1 * dpx);
        canvas.stroke_path(&cap_path, &cap_ring);

        // 7. Pointer line
        let p_in = cap_r * 0.24;
        let p_out = cap_r * 0.88;
        let px0 = center_x + val_angle.cos() * p_in;
        let py0 = center_y + val_angle.sin() * p_in;
        let px1 = center_x + val_angle.cos() * p_out;
        let py1 = center_y + val_angle.sin() * p_out;

        let mut ptr_shadow = vg::Path::new();
        ptr_shadow.move_to(px0, py0 + 1.0 * dpx);
        ptr_shadow.line_to(px1, py1 + 1.0 * dpx);
        let mut ps_paint = vg::Paint::color(vg::Color::rgba(0, 0, 0, 195));
        ps_paint.set_line_width(3.0 * dpx);
        ps_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&ptr_shadow, &ps_paint);

        let mut ptr = vg::Path::new();
        ptr.move_to(px0, py0);
        ptr.line_to(px1, py1);
        let mut p_paint = vg::Paint::color(vg::Color::rgb(252, 240, 224));
        p_paint.set_line_width(2.2 * dpx);
        p_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&ptr, &p_paint);
    }
}

/// Skeuomorphic hardware LED toggle pill switch.
pub struct LedToggleSwitch {
    param_base: ParamWidgetBase,
    accent_rgb: (u8, u8, u8),
}

impl LedToggleSwitch {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        label_text: &'static str,
        accent_rgb: (u8, u8, u8),
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            accent_rgb,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params.clone(), params_to_param, move |cx, param_data| {
                let is_on_lens =
                    param_data.make_lens(|param| param.modulated_normalized_value() >= 0.5);
                Binding::new(cx, is_on_lens, move |cx, _| {
                    cx.needs_redraw();
                    HStack::new(cx, |cx| {
                        Element::new(cx)
                            .width(Pixels(15.0))
                            .height(Stretch(1.0))
                            .hoverable(false);
                        Label::new(cx, label_text)
                            .class("switch-label")
                            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                            .font_weight(FontWeightKeyword::Bold)
                            .hoverable(false);
                    })
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .child_left(Pixels(6.0))
                    .child_right(Pixels(8.0))
                    .col_between(Pixels(4.0))
                    .hoverable(false);
                });
            }),
        )
        .checked(ParamWidgetBase::make_lens(
            params,
            params_to_param,
            |param| param.modulated_normalized_value() >= 0.5,
        ))
    }

    fn toggle(&self, cx: &mut EventContext) {
        let current = self.param_base.unmodulated_normalized_value();
        let next = if current >= 0.5 { 0.0 } else { 1.0 };
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, next);
        self.param_base.end_set_parameter(cx);
        cx.needs_redraw();
    }
}

impl View for LedToggleSwitch {
    fn element(&self) -> Option<&'static str> {
        Some("led-toggle")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.set_checked(self.param_base.modulated_normalized_value() >= 0.5);
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    let def = self.param_base.default_normalized_value();
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, def);
                    self.param_base.end_set_parameter(cx);
                    cx.needs_redraw();
                } else {
                    self.toggle(cx);
                }
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let dpx = cx.scale_factor();
        let is_on = self.param_base.modulated_normalized_value() >= 0.5;
        let (ar, ag, ab) = self.accent_rgb;

        let r = 4.0 * dpx;
        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, r);

        let housing_paint = if is_on {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                vg::Color::rgb(82, 70, 60),
                vg::Color::rgb(52, 44, 38),
            )
        } else {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                vg::Color::rgb(54, 46, 40),
                vg::Color::rgb(38, 32, 28),
            )
        };
        canvas.fill_path(&bg, &housing_paint);

        let mut border = vg::Paint::color(if is_on {
            vg::Color::rgba(ar, ag, ab, 175)
        } else {
            vg::Color::rgba(24, 20, 17, 235)
        });
        border.set_line_width(1.1 * dpx);
        canvas.stroke_path(&bg, &border);

        let led_x = bounds.x + 12.0 * dpx;
        let led_y = bounds.y + bounds.h * 0.5;
        let led_r = 3.8 * dpx;

        if is_on {
            let mut glow = vg::Path::new();
            glow.circle(led_x, led_y, led_r * 2.2);
            let glow_paint = vg::Paint::radial_gradient(
                led_x,
                led_y,
                led_r * 0.2,
                led_r * 2.2,
                vg::Color::rgba(ar, ag, ab, 165),
                vg::Color::rgba(ar, ag, ab, 0),
            );
            canvas.fill_path(&glow, &glow_paint);
        }

        let mut lamp = vg::Path::new();
        lamp.circle(led_x, led_y, led_r);
        let lamp_paint = if is_on {
            vg::Paint::radial_gradient(
                led_x - led_r * 0.25,
                led_y - led_r * 0.25,
                led_r * 0.1,
                led_r,
                vg::Color::rgb(255, 250, 240),
                vg::Color::rgb(ar, ag, ab),
            )
        } else {
            vg::Paint::radial_gradient(
                led_x - led_r * 0.2,
                led_y - led_r * 0.2,
                led_r * 0.1,
                led_r,
                vg::Color::rgb(92, 80, 70),
                vg::Color::rgb(44, 38, 33),
            )
        };
        canvas.fill_path(&lamp, &lamp_paint);

        let mut lamp_rim = vg::Paint::color(vg::Color::rgba(12, 10, 8, 225));
        lamp_rim.set_line_width(1.0 * dpx);
        canvas.stroke_path(&lamp, &lamp_rim);
    }
}

/// Live vertical LED Gain Reduction meter for the Compressor module.
pub struct GainReductionMeter {
    gr_db: Arc<AtomicU32>,
    accent_rgb: (u8, u8, u8),
}

impl GainReductionMeter {
    pub fn new(cx: &mut Context, gr_db: Arc<AtomicU32>, accent_rgb: (u8, u8, u8)) -> Handle<'_, Self> {
        let timer = cx.add_timer(Duration::from_millis(33), None, |cx, _| {
            cx.needs_redraw();
        });
        cx.start_timer(timer);

        Self { gr_db, accent_rgb }.build(cx, |_| {})
    }
}

impl View for GainReductionMeter {
    fn element(&self) -> Option<&'static str> {
        Some("gr-meter")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let dpx = cx.scale_factor();
        let gr = f32::from_bits(self.gr_db.load(Ordering::Relaxed));
        // gr is negative (0 dB .. -24 dB)
        let norm = ((-gr).clamp(0.0, 24.0)) / 24.0;

        let mut housing = vg::Path::new();
        housing.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        let bg_paint = vg::Paint::color(vg::Color::rgb(26, 22, 18));
        canvas.fill_path(&housing, &bg_paint);
        let mut rim = vg::Paint::color(vg::Color::rgb(68, 58, 50));
        rim.set_line_width(1.0 * dpx);
        canvas.stroke_path(&housing, &rim);

        let segments = 10;
        let pad = 3.0 * dpx;
        let seg_gap = 2.0 * dpx;
        let inner_h = (bounds.h - 2.0 * pad).max(1.0);
        let seg_h = ((inner_h - seg_gap * (segments - 1) as f32) / segments as f32).max(1.0);
        let seg_w = (bounds.w - 2.0 * pad).max(1.0);

        let active_segs = (norm * segments as f32).ceil() as usize;
        let (ar, ag, ab) = self.accent_rgb;

        for i in 0..segments {
            let sy = bounds.y + pad + i as f32 * (seg_h + seg_gap);
            let mut seg = vg::Path::new();
            seg.rounded_rect(bounds.x + pad, sy, seg_w, seg_h, 1.5 * dpx);
            let lit = i < active_segs;
            let color = if lit {
                vg::Color::rgba(ar, ag, ab, 240)
            } else {
                vg::Color::rgba(52, 44, 38, 180)
            };
            canvas.fill_path(&seg, &vg::Paint::color(color));
        }
    }
}

/// Build a complete rotary knob cell with title, 3D skeuomorphic dial, and digital readout screen.
fn build_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    let cell_w = 82.0;
    let dial_size = 62.0;

    VStack::new(cx, |cx| {
        Label::new(cx, title)
            .class("knob-title")
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .width(Stretch(1.0))
            .height(Pixels(16.0))
            .child_space(Stretch(1.0));

        RotaryKnob::new(cx, params.clone(), params_to_param, bipolar, accent_rgb)
            .width(Pixels(dial_size))
            .height(Pixels(dial_size))
            .left(Stretch(1.0))
            .right(Stretch(1.0));

        ZStack::new(cx, |cx| {
            ParamWidgetBase::view(cx, params, params_to_param, |cx, param_data| {
                let readout_lens = param_data.make_lens(|param| {
                    param.normalized_value_to_string(param.modulated_normalized_value(), true)
                });
                Label::new(cx, readout_lens)
                    .class("knob-readout-text")
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_weight(FontWeightKeyword::Bold)
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0));
            });
        })
        .class("knob-readout-box")
        .width(Pixels(cell_w - 6.0))
        .height(Pixels(22.0))
        .left(Stretch(1.0))
        .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Pixels(cell_w))
    .height(Auto)
    .row_between(Pixels(3.0));
}

fn build_rack_screw(cx: &mut Context) {
    Element::new(cx).class("rack-screw");
}

pub(crate) fn create(
    params: Arc<CrushClonePluginParams>,
    editor_state: Arc<ViziaState>,
    gain_reduction_db: Arc<AtomicU32>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        assets::register_noto_sans_bold(cx);
        cx.set_default_font(&[assets::NOTO_SANS]);

        let _ = cx.add_stylesheet(include_style!("src/theme.css"));

        UiData {
            params: params.clone(),
            preset_index: 0,
            preset_name: FACTORY_PRESETS[0].name.to_string(),
            gui_context,
        }
        .build(cx);

        let amber = (245, 179, 73);
        let gold = (242, 203, 97);
        let coral = (235, 140, 85);
        let gr_db = gain_reduction_db.clone();

        VStack::new(cx, move |cx| {
            // =========================================================================
            // TOP HARDWARE HEADER BAR
            // =========================================================================
            HStack::new(cx, |cx| {
                build_rack_screw(cx);

                HStack::new(cx, |cx| {
                    Element::new(cx).class("brand-pill");
                    Label::new(cx, "CRUSHCLONE-RS")
                        .class("header-title")
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_weight(FontWeightKeyword::Bold);
                })
                .width(Auto)
                .col_between(Pixels(8.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

                Element::new(cx).width(Stretch(1.0));

                // Factory Preset Selector Pill (4 + 20 + 4 + 138 + 4 + 20 + 4 = 194px)
                HStack::new(cx, |cx| {
                    Button::new(
                        cx,
                        |cx| cx.emit(UiEvent::PrevPreset),
                        |cx| Label::new(cx, "<"),
                    )
                    .class("preset-btn")
                    .width(Pixels(20.0))
                    .height(Pixels(20.0));

                    Label::new(cx, UiData::preset_name)
                        .class("preset-name")
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_weight(FontWeightKeyword::Bold)
                        .width(Pixels(138.0));

                    Button::new(
                        cx,
                        |cx| cx.emit(UiEvent::NextPreset),
                        |cx| Label::new(cx, ">"),
                    )
                    .class("preset-btn")
                    .width(Pixels(20.0))
                    .height(Pixels(20.0));
                })
                .class("preset-bar")
                .width(Pixels(194.0))
                .height(Pixels(28.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

                build_rack_screw(cx);
            })
            .class("header-bar")
            .height(Pixels(40.0))
            .width(Stretch(1.0))
            .child_left(Pixels(14.0))
            .child_right(Pixels(14.0))
            .col_between(Pixels(12.0));

            // =========================================================================
            // MAIN RACK CHASSIS — 2x2 Hardware Module Grid
            // =========================================================================
            VStack::new(cx, move |cx| {
                // Top Row: DISTORTION + FILTER
                HStack::new(cx, move |cx| {
                    // Module 1: DISTORTION
                    VStack::new(cx, |cx| {
                        HStack::new(cx, |cx| {
                            Label::new(cx, "DISTORTION")
                                .class("module-title")
                                .font_weight(FontWeightKeyword::Bold);
                            Element::new(cx).width(Stretch(1.0));
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.dist_enable,
                                "ON",
                                amber,
                            )
                            .height(Pixels(20.0))
                            .width(Pixels(52.0));
                        })
                        .class("module-header")
                        .width(Stretch(1.0))
                        .height(Pixels(26.0));

                        HStack::new(cx, |cx| {
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.tube_amount,
                                "TUBE",
                                false,
                                amber,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.mech_amount,
                                "MECH",
                                false,
                                amber,
                            );
                        })
                        .class("knob-row")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));

                    // Module 2: FILTER
                    VStack::new(cx, |cx| {
                        HStack::new(cx, |cx| {
                            Label::new(cx, "FILTER")
                                .class("module-title")
                                .font_weight(FontWeightKeyword::Bold);
                            Element::new(cx).width(Stretch(1.0));
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.filter_enable,
                                "ON",
                                gold,
                            )
                            .height(Pixels(20.0))
                            .width(Pixels(52.0));
                        })
                        .class("module-header")
                        .width(Stretch(1.0))
                        .height(Pixels(26.0));

                        HStack::new(cx, |cx| {
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.filter_cutoff,
                                "CUTOFF",
                                false,
                                gold,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.filter_reso,
                                "RESONANCE",
                                false,
                                gold,
                            );
                        })
                        .class("knob-row")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                })
                .col_between(Pixels(10.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));

                // Bottom Row: COMPRESSOR + MASTER / MIX
                HStack::new(cx, move |cx| {
                    // Module 3: COMPRESSOR
                    VStack::new(cx, move |cx| {
                        HStack::new(cx, |cx| {
                            Label::new(cx, "COMPRESSOR")
                                .class("module-title")
                                .font_weight(FontWeightKeyword::Bold);
                            Element::new(cx).width(Stretch(1.0));
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.phat_mode,
                                "PHAT",
                                coral,
                            )
                            .height(Pixels(20.0))
                            .width(Pixels(62.0));
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.comp_enable,
                                "ON",
                                coral,
                            )
                            .height(Pixels(20.0))
                            .width(Pixels(52.0));
                        })
                        .class("module-header")
                        .col_between(Pixels(6.0))
                        .width(Stretch(1.0))
                        .height(Pixels(26.0));

                        HStack::new(cx, move |cx| {
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.comp_amount,
                                "AMOUNT",
                                false,
                                coral,
                            );

                            VStack::new(cx, move |cx| {
                                Label::new(cx, "GR")
                                    .class("gr-meter-label")
                                    .font_weight(FontWeightKeyword::Bold)
                                    .width(Stretch(1.0))
                                    .child_space(Stretch(1.0));
                                GainReductionMeter::new(cx, gr_db.clone(), coral)
                                    .width(Pixels(20.0))
                                    .height(Pixels(92.0))
                                    .left(Stretch(1.0))
                                    .right(Stretch(1.0));
                            })
                            .width(Pixels(28.0))
                            .height(Auto)
                            .row_between(Pixels(4.0))
                            .top(Stretch(1.0))
                            .bottom(Stretch(1.0));
                        })
                        .class("knob-row")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));

                    // Module 4: MASTER / MIX
                    VStack::new(cx, |cx| {
                        HStack::new(cx, |cx| {
                            Label::new(cx, "MASTER / MIX")
                                .class("module-title")
                                .font_weight(FontWeightKeyword::Bold);
                        })
                        .class("module-header")
                        .width(Stretch(1.0))
                        .height(Pixels(26.0));

                        HStack::new(cx, |cx| {
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.master_volume,
                                "MASTER",
                                true,
                                amber,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.dry_wet,
                                "MIX",
                                false,
                                amber,
                            );
                        })
                        .class("knob-row")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                })
                .col_between(Pixels(10.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .class("main-chassis")
            .class("section-panel")
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .row_between(Pixels(10.0));
        })
        .class("plugin-root")
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}
