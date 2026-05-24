use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    animation::loop_time::LoopTime,
    render::{
        color::{CustomGradient, PaletteKind},
        formula::{FormulaIssue, FormulaSource},
        frame_buffer::FrameBuffer,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PatternLayer {
    pub name: String,
    #[serde(default = "default_layer_enabled")]
    pub enabled: bool,
    pub source: FormulaSource,
    pub strength: f32,
    pub scale: f32,
    pub motion: f32,
    pub morph: f32,
    pub camera_zoom_loop: f32,
    pub camera_orbit: f32,
}

impl Default for PatternLayer {
    fn default() -> Self {
        Self {
            name: "Pattern 1".to_owned(),
            enabled: true,
            source: FormulaSource::fractal_a(),
            strength: 1.0,
            scale: 1.0,
            motion: 0.5,
            morph: 0.0,
            camera_zoom_loop: 0.0,
            camera_orbit: 0.0,
        }
    }
}

impl PatternLayer {
    pub fn new(name: impl Into<String>, source: FormulaSource) -> Self {
        Self {
            name: name.into(),
            source,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectLayer {
    pub name: String,
    #[serde(default = "default_layer_enabled")]
    pub enabled: bool,
    pub source: FormulaSource,
    pub blend_mode: EffectBlendMode,
    pub strength: f32,
    pub scale: f32,
    pub motion: f32,
    pub morph: f32,
    pub camera_zoom_loop: f32,
    pub camera_orbit: f32,
}

impl Default for EffectLayer {
    fn default() -> Self {
        Self {
            name: "Effect 1".to_owned(),
            enabled: true,
            source: FormulaSource::pattern(),
            blend_mode: EffectBlendMode::Multiply,
            strength: 0.5,
            scale: 1.0,
            motion: 0.5,
            morph: 0.0,
            camera_zoom_loop: 0.0,
            camera_orbit: 0.0,
        }
    }
}

impl EffectLayer {
    pub fn new(name: impl Into<String>, source: FormulaSource) -> Self {
        Self {
            name: name.into(),
            source,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EffectBlendMode {
    #[default]
    Multiply,
    Screen,
    Add,
    Subtract,
    Difference,
    Mask,
    Contrast,
    Displace,
}

impl EffectBlendMode {
    pub const ALL: [Self; 8] = [
        Self::Multiply,
        Self::Screen,
        Self::Add,
        Self::Subtract,
        Self::Difference,
        Self::Mask,
        Self::Contrast,
        Self::Displace,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Difference => "Difference",
            Self::Mask => "Mask",
            Self::Contrast => "Contrast",
            Self::Displace => "Displace",
        }
    }
}

fn default_layer_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderParams {
    pub patterns: Vec<PatternLayer>,
    pub effects: Vec<EffectLayer>,
    pub seed: u32,
    pub zoom: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub rotation_speed: f32,
    pub color_speed: f32,
    pub color_phase: f32,
    pub palette: PaletteKind,
    pub custom_gradient: CustomGradient,
    pub symmetry: u32,
    pub distortion: f32,
    pub detail: f32,
    pub smoothing: f32,
    pub smoothing_radius_pixels: f32,
    pub brightness: f32,
    pub contrast: f32,
}

#[derive(Deserialize)]
#[serde(default)]
struct RenderParamsSerde {
    patterns: Vec<PatternLayer>,
    effects: Vec<EffectLayer>,
    seed: u32,
    zoom: f32,
    center_x: f32,
    center_y: f32,
    rotation_speed: f32,
    color_speed: f32,
    color_phase: f32,
    palette: PaletteKind,
    custom_gradient: Option<CustomGradient>,
    symmetry: u32,
    distortion: f32,
    detail: f32,
    smoothing: f32,
    smoothing_radius_pixels: f32,
    brightness: f32,
    contrast: f32,
}

impl Default for RenderParamsSerde {
    fn default() -> Self {
        let defaults = RenderParams::default();
        Self {
            patterns: defaults.patterns,
            effects: defaults.effects,
            seed: defaults.seed,
            zoom: defaults.zoom,
            center_x: defaults.center_x,
            center_y: defaults.center_y,
            rotation_speed: defaults.rotation_speed,
            color_speed: defaults.color_speed,
            color_phase: defaults.color_phase,
            palette: defaults.palette,
            custom_gradient: None,
            symmetry: defaults.symmetry,
            distortion: defaults.distortion,
            detail: defaults.detail,
            smoothing: defaults.smoothing,
            smoothing_radius_pixels: defaults.smoothing_radius_pixels,
            brightness: defaults.brightness,
            contrast: defaults.contrast,
        }
    }
}

impl<'de> Deserialize<'de> for RenderParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = RenderParamsSerde::deserialize(deserializer)?;
        let mut custom_gradient = helper
            .custom_gradient
            .unwrap_or_else(|| CustomGradient::from_palette(helper.palette));
        custom_gradient.normalize_for_palette(helper.palette);
        Ok(Self {
            patterns: helper.patterns,
            effects: helper.effects,
            seed: helper.seed,
            zoom: helper.zoom,
            center_x: helper.center_x,
            center_y: helper.center_y,
            rotation_speed: helper.rotation_speed,
            color_speed: helper.color_speed,
            color_phase: helper.color_phase,
            palette: helper.palette,
            custom_gradient,
            symmetry: helper.symmetry,
            distortion: helper.distortion,
            detail: helper.detail,
            smoothing: helper.smoothing,
            smoothing_radius_pixels: helper.smoothing_radius_pixels,
            brightness: helper.brightness,
            contrast: helper.contrast,
        })
    }
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            patterns: vec![PatternLayer::default()],
            effects: Vec::new(),
            seed: 12_345,
            zoom: 1.0,
            center_x: 0.0,
            center_y: 0.0,
            rotation_speed: 0.35,
            color_speed: 0.8,
            color_phase: 0.0,
            palette: PaletteKind::Neon,
            custom_gradient: CustomGradient::default(),
            symmetry: 6,
            distortion: 0.65,
            detail: 1.0,
            smoothing: 0.0,
            smoothing_radius_pixels: 1.0,
            brightness: 1.0,
            contrast: 1.2,
        }
    }
}

impl RenderParams {
    pub fn ensure_layers(&mut self) {
        if self.patterns.is_empty() {
            self.patterns.push(PatternLayer::default());
        }
    }

    pub fn normalize_color_source(&mut self) {
        self.custom_gradient.normalize_for_palette(self.palette);
    }

    pub fn activate_editable_sources(&mut self) {
        self.ensure_layers();
        self.normalize_color_source();
    }

    pub fn formula_issues(&self) -> Vec<FormulaIssue> {
        let mut issues = Vec::new();
        for (index, layer) in self.patterns.iter().enumerate() {
            if !layer.enabled {
                continue;
            }
            issues.extend(layer.source.validate(&format!("Pattern {}", index + 1)));
        }
        for (index, layer) in self.effects.iter().enumerate() {
            if !layer.enabled {
                continue;
            }
            issues.extend(layer.source.validate(&format!("Effect {}", index + 1)));
        }
        issues
    }
}

pub trait Renderer {
    fn render_frame(
        &mut self,
        params: &RenderParams,
        time: LoopTime,
        width: u32,
        height: u32,
    ) -> FrameBuffer;
}
