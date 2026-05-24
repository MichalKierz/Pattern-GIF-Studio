use serde::{Deserialize, Serialize};

use crate::{
    export::export_settings::ExportSettings,
    render::{formula::FormulaSource, renderer::RenderParams},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectState {
    pub render_params: RenderParams,
    pub export_settings: ExportSettings,
}

impl ProjectState {
    pub fn sanitize(&mut self) {
        self.render_params.ensure_layers();
        self.render_params.patterns.truncate(24);
        self.render_params.effects.truncate(24);
        for (index, layer) in self.render_params.patterns.iter_mut().enumerate() {
            if layer.name.trim().is_empty() {
                layer.name = format!("Pattern {}", index + 1);
            }
            layer.strength = layer.strength.clamp(0.0, 1.0);
            layer.scale = layer.scale.clamp(0.1, 8.0);
            layer.motion = layer.motion.clamp(-4.0, 4.0);
            sanitize_formula_source(&mut layer.source);
        }
        for (index, layer) in self.render_params.effects.iter_mut().enumerate() {
            if layer.name.trim().is_empty() {
                layer.name = format!("Effect {}", index + 1);
            }
            layer.strength = layer.strength.clamp(0.0, 1.0);
            layer.scale = layer.scale.clamp(0.1, 8.0);
            layer.motion = layer.motion.clamp(-4.0, 4.0);
            sanitize_formula_source(&mut layer.source);
        }
        self.render_params.zoom = self.render_params.zoom.clamp(0.1, 12.0);
        self.render_params.center_x = self.render_params.center_x.clamp(-2.0, 2.0);
        self.render_params.center_y = self.render_params.center_y.clamp(-2.0, 2.0);
        self.render_params.rotation_speed = self.render_params.rotation_speed.clamp(-4.0, 4.0);
        self.render_params.color_speed = self.render_params.color_speed.clamp(-6.0, 6.0);
        self.render_params.color_phase = self.render_params.color_phase.clamp(0.0, 1.0);
        self.render_params.normalize_color_source();
        self.render_params.symmetry = self.render_params.symmetry.clamp(1, 24);
        self.render_params.distortion = self.render_params.distortion.clamp(0.0, 4.0);
        self.render_params.detail = self.render_params.detail.clamp(0.25, 4.0);
        self.render_params.smoothing = self.render_params.smoothing.clamp(0.0, 20.0);
        self.render_params.smoothing_radius_pixels =
            self.render_params.smoothing_radius_pixels.clamp(0.0, 10.0);
        self.render_params.custom_gradient.transition = self
            .render_params
            .custom_gradient
            .transition
            .clamp(0.0, 1.0);
        self.render_params.brightness = self.render_params.brightness.clamp(0.1, 2.0);
        self.render_params.contrast = self.render_params.contrast.clamp(0.1, 3.0);
        self.export_settings.sanitize();
    }
}

fn sanitize_formula_source(source: &mut FormulaSource) {
    source.gain = source.gain.clamp(-4.0, 4.0);
    source.bias = source.bias.clamp(-2.0, 2.0);
    for layer in &mut source.layers {
        layer.gain = layer.gain.clamp(-4.0, 4.0);
        layer.bias = layer.bias.clamp(-2.0, 2.0);
        layer.opacity = layer.opacity.clamp(0.0, 1.0);
        layer.repeat_x = layer.repeat_x.clamp(0.05, 32.0);
        layer.repeat_y = layer.repeat_y.clamp(0.05, 32.0);
        layer.warp_x = layer.warp_x.clamp(-8.0, 8.0);
        layer.warp_y = layer.warp_y.clamp(-8.0, 8.0);
        layer.offset_x = layer.offset_x.clamp(-8.0, 8.0);
        layer.offset_y = layer.offset_y.clamp(-8.0, 8.0);
        layer.motion_x = layer.motion_x.clamp(-8.0, 8.0);
        layer.motion_y = layer.motion_y.clamp(-8.0, 8.0);
    }
}
