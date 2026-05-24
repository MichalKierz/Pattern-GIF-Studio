use serde::{Deserialize, Serialize};

use crate::utils::math::{clamp01, fract, lerp};

pub const MAX_GRADIENT_COLORS: usize = 16;
pub const MAX_COLOR_TRANSITIONS_PER_STOP: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaletteKind {
    #[default]
    Neon,
    Aurora,
    Fire,
    Candy,
    MonoChrome,
}

impl PaletteKind {
    pub const ALL: [PaletteKind; 5] = [
        PaletteKind::Neon,
        PaletteKind::Aurora,
        PaletteKind::Fire,
        PaletteKind::Candy,
        PaletteKind::MonoChrome,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaletteKind::Neon => "Neon",
            PaletteKind::Aurora => "Aurora",
            PaletteKind::Fire => "Fire",
            PaletteKind::Candy => "Candy",
            PaletteKind::MonoChrome => "Mono",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomGradient {
    #[serde(default = "default_gradient_enabled")]
    pub enabled: bool,
    pub colors: Vec<[f32; 3]>,
    pub color_transitions: Vec<Vec<ColorTransition>>,
    pub color_a: [f32; 3],
    pub color_b: [f32; 3],
    pub color_c: [f32; 3],
    pub color_d: [f32; 3],
    pub transition: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorTransition {
    pub color: [f32; 3],
}

impl Default for ColorTransition {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
        }
    }
}

impl Default for CustomGradient {
    fn default() -> Self {
        Self {
            enabled: true,
            colors: vec![
                [0.05, 0.02, 0.18],
                [0.00, 0.85, 1.00],
                [1.00, 0.08, 0.72],
                [1.00, 0.86, 0.18],
            ],
            color_transitions: vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            color_a: [0.05, 0.02, 0.18],
            color_b: [0.00, 0.85, 1.00],
            color_c: [1.00, 0.08, 0.72],
            color_d: [1.00, 0.86, 0.18],
            transition: 0.5,
        }
    }
}

impl CustomGradient {
    pub fn from_palette(kind: PaletteKind) -> Self {
        let mut gradient = match kind {
            PaletteKind::Neon => Self::default(),
            PaletteKind::Aurora => Self {
                enabled: true,
                colors: vec![
                    [0.02, 0.09, 0.16],
                    [0.05, 0.55, 0.45],
                    [0.30, 0.85, 0.95],
                    [0.95, 0.35, 0.75],
                ],
                color_transitions: vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()],
                color_a: [0.02, 0.09, 0.16],
                color_b: [0.05, 0.55, 0.45],
                color_c: [0.30, 0.85, 0.95],
                color_d: [0.95, 0.35, 0.75],
                transition: 0.5,
            },
            PaletteKind::Fire => Self {
                enabled: true,
                colors: vec![
                    [0.02, 0.00, 0.02],
                    [0.35, 0.02, 0.04],
                    [0.95, 0.18, 0.02],
                    [1.00, 0.82, 0.24],
                ],
                color_transitions: vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()],
                color_a: [0.02, 0.00, 0.02],
                color_b: [0.35, 0.02, 0.04],
                color_c: [0.95, 0.18, 0.02],
                color_d: [1.00, 0.82, 0.24],
                transition: 0.5,
            },
            PaletteKind::Candy => Self {
                enabled: true,
                colors: vec![
                    [0.05, 0.02, 0.12],
                    [0.95, 0.10, 0.65],
                    [0.10, 0.85, 1.00],
                    [1.00, 0.90, 0.25],
                ],
                color_transitions: vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()],
                color_a: [0.05, 0.02, 0.12],
                color_b: [0.95, 0.10, 0.65],
                color_c: [0.10, 0.85, 1.00],
                color_d: [1.00, 0.90, 0.25],
                transition: 0.5,
            },
            PaletteKind::MonoChrome => Self {
                enabled: true,
                colors: vec![
                    [0.0, 0.0, 0.0],
                    [0.33, 0.33, 0.33],
                    [0.66, 0.66, 0.66],
                    [1.0, 1.0, 1.0],
                ],
                color_transitions: vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()],
                color_a: [0.0, 0.0, 0.0],
                color_b: [0.33, 0.33, 0.33],
                color_c: [0.66, 0.66, 0.66],
                color_d: [1.0, 1.0, 1.0],
                transition: 0.5,
            },
        };
        gradient.enabled = true;
        gradient
    }

    pub fn active_for_palette(kind: PaletteKind, gradient: &Self) -> Self {
        let mut active = if gradient.enabled {
            gradient.clone()
        } else {
            Self::from_palette(kind)
        };
        active.enabled = true;
        active.ensure_color_stops();
        active
    }

    pub fn normalize_for_palette(&mut self, kind: PaletteKind) {
        if self.enabled {
            self.enabled = true;
            self.ensure_color_stops();
        } else {
            *self = Self::from_palette(kind);
        }
    }

    pub fn ensure_color_stops(&mut self) {
        if self.colors.is_empty() {
            self.colors = vec![self.color_a, self.color_b, self.color_c, self.color_d];
        }
        self.colors.truncate(MAX_GRADIENT_COLORS);
        for color in &mut self.colors {
            clamp_color(color);
        }
        self.color_transitions
            .resize_with(self.colors.len(), Vec::new);
        self.color_transitions.truncate(self.colors.len());
        for transitions in &mut self.color_transitions {
            transitions.truncate(MAX_COLOR_TRANSITIONS_PER_STOP);
            for transition in transitions {
                clamp_color(&mut transition.color);
            }
        }
        self.sync_legacy_fields();
    }

    pub fn add_color(&mut self) {
        self.ensure_color_stops();
        let color = self.colors.last().copied().unwrap_or(self.color_d);
        self.colors.push(color);
        self.color_transitions.push(Vec::new());
        self.ensure_color_stops();
    }

    pub fn remove_color(&mut self, index: usize) {
        self.ensure_color_stops();
        if self.colors.len() > 2 && index < self.colors.len() {
            self.colors.remove(index);
            self.color_transitions.remove(index);
        }
        self.ensure_color_stops();
    }

    pub fn add_transition(&mut self, index: usize) {
        self.ensure_color_stops();
        let Some(base_color) = self.colors.get(index).copied() else {
            return;
        };
        let Some(transitions) = self.color_transitions.get_mut(index) else {
            return;
        };
        if transitions.len() >= MAX_COLOR_TRANSITIONS_PER_STOP {
            return;
        }
        let color = transitions
            .last()
            .map(|transition| transition.color)
            .unwrap_or_else(|| suggested_transition_color(base_color, transitions.len()));
        transitions.push(ColorTransition { color });
        self.ensure_color_stops();
    }

    pub fn remove_transition(&mut self, color_index: usize, transition_index: usize) {
        self.ensure_color_stops();
        if let Some(transitions) = self.color_transitions.get_mut(color_index)
            && transition_index < transitions.len()
        {
            transitions.remove(transition_index);
        }
        self.ensure_color_stops();
    }

    pub fn stops(&self) -> &[[f32; 3]] {
        if self.colors.is_empty() {
            &[]
        } else {
            &self.colors
        }
    }

    pub fn animated_stops(&self, phase: f32) -> Vec<[f32; 3]> {
        let base_stops = self.stops();
        if base_stops.is_empty() {
            return self.fixed_stops().to_vec();
        }

        base_stops
            .iter()
            .enumerate()
            .map(|(index, base_color)| {
                animated_stop(
                    *base_color,
                    self.color_transitions
                        .get(index)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                    phase,
                )
            })
            .collect()
    }

    pub fn fixed_stops(&self) -> [[f32; 3]; 4] {
        let stops = self.stops();
        [
            stops.first().copied().unwrap_or(self.color_a),
            stops.get(1).copied().unwrap_or(self.color_b),
            stops.get(2).copied().unwrap_or(self.color_c),
            stops.get(3).copied().unwrap_or(self.color_d),
        ]
    }

    fn sync_legacy_fields(&mut self) {
        let stops = self.colors.clone();
        if let Some(color) = stops.first() {
            self.color_a = *color;
        }
        if let Some(color) = stops.get(1) {
            self.color_b = *color;
        }
        if let Some(color) = stops.get(2) {
            self.color_c = *color;
        }
        if let Some(color) = stops.get(3) {
            self.color_d = *color;
        }
    }
}

pub fn sample_palette(
    kind: PaletteKind,
    custom_gradient: &CustomGradient,
    value: f32,
    color_animation_phase: f32,
    brightness: f32,
    contrast: f32,
) -> [u8; 4] {
    let value = apply_contrast(fract(value), contrast);
    let legacy_gradient;
    let active_gradient = if custom_gradient.enabled {
        custom_gradient
    } else {
        legacy_gradient = CustomGradient::from_palette(kind);
        &legacy_gradient
    };
    let [r, g, b] = custom_gradient_color(active_gradient, value, color_animation_phase);

    [
        to_byte(r * brightness),
        to_byte(g * brightness),
        to_byte(b * brightness),
        255,
    ]
}

fn custom_gradient_color(
    custom_gradient: &CustomGradient,
    value: f32,
    color_animation_phase: f32,
) -> [f32; 3] {
    let stops = custom_gradient.animated_stops(color_animation_phase);
    gradient(&stops, value)
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = fract(h) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);

    match i as u32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn gradient(stops: &[[f32; 3]], value: f32) -> [f32; 3] {
    if stops.is_empty() {
        return [value, value, value];
    }
    if stops.len() == 1 {
        return stops[0];
    }

    let scaled = fract(value) * stops.len() as f32;
    let index = scaled.floor() as usize % stops.len();
    let next = (index + 1) % stops.len();
    let t = scaled - scaled.floor();

    [
        lerp(stops[index][0], stops[next][0], t),
        lerp(stops[index][1], stops[next][1], t),
        lerp(stops[index][2], stops[next][2], t),
    ]
}

fn apply_contrast(value: f32, contrast: f32) -> f32 {
    clamp01((value - 0.5) * contrast.max(0.05) + 0.5)
}

fn to_byte(value: f32) -> u8 {
    (clamp01(value) * 255.0).round() as u8
}

fn animated_stop(base_color: [f32; 3], transitions: &[ColorTransition], phase: f32) -> [f32; 3] {
    if transitions.is_empty() {
        return base_color;
    }

    let frame_count = transitions.len() + 1;
    let scaled = fract(phase) * frame_count as f32;
    let index = scaled.floor() as usize % frame_count;
    let next = (index + 1) % frame_count;
    let t = smoothstep01(scaled - scaled.floor());
    let current = color_keyframe(base_color, transitions, index);
    let target = color_keyframe(base_color, transitions, next);
    [
        lerp(current[0], target[0], t),
        lerp(current[1], target[1], t),
        lerp(current[2], target[2], t),
    ]
}

fn color_keyframe(base_color: [f32; 3], transitions: &[ColorTransition], index: usize) -> [f32; 3] {
    if index == 0 {
        base_color
    } else {
        transitions
            .get(index - 1)
            .map(|transition| transition.color)
            .unwrap_or(base_color)
    }
}

fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn clamp_color(color: &mut [f32; 3]) {
    for channel in color {
        *channel = channel.clamp(0.0, 1.0);
    }
}

fn suggested_transition_color(color: [f32; 3], transition_count: usize) -> [f32; 3] {
    match transition_count % 3 {
        0 => [
            (color[1] * 0.72 + 0.18).clamp(0.0, 1.0),
            (color[2] * 0.72 + 0.18).clamp(0.0, 1.0),
            (color[0] * 0.72 + 0.18).clamp(0.0, 1.0),
        ],
        1 => [
            (1.0 - color[0] * 0.65).clamp(0.0, 1.0),
            (1.0 - color[1] * 0.65).clamp(0.0, 1.0),
            (1.0 - color[2] * 0.65).clamp(0.0, 1.0),
        ],
        _ => [
            (color[0] * 0.55 + 0.35).clamp(0.0, 1.0),
            (color[1] * 0.55 + 0.35).clamp(0.0, 1.0),
            (color[2] * 0.55 + 0.35).clamp(0.0, 1.0),
        ],
    }
}

fn default_gradient_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_gradient_supports_adding_and_removing_color_stops() {
        let mut gradient = CustomGradient::default();
        gradient.ensure_color_stops();
        let initial_len = gradient.stops().len();

        gradient.add_color();
        assert_eq!(gradient.stops().len(), initial_len + 1);

        gradient.remove_color(1);
        assert_eq!(gradient.stops().len(), initial_len);

        while gradient.stops().len() > 2 {
            gradient.remove_color(0);
        }
        gradient.remove_color(0);

        assert_eq!(gradient.stops().len(), 2);
    }

    #[test]
    fn color_transitions_animate_stops_without_breaking_legacy_gradients() {
        let mut gradient = CustomGradient {
            colors: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            color_transitions: Vec::new(),
            ..CustomGradient::default()
        };
        gradient.ensure_color_stops();
        assert_eq!(gradient.color_transitions.len(), 2);
        assert_eq!(gradient.animated_stops(0.5)[0], [0.0, 0.0, 0.0]);

        gradient.color_transitions[0].push(ColorTransition {
            color: [0.0, 1.0, 0.0],
        });
        let animated = gradient.animated_stops(0.25);

        assert!(animated[0][1] > animated[0][0]);
    }

    #[test]
    fn disabled_legacy_gradient_uses_palette_as_seed() {
        let legacy = CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        };
        let custom_color = sample_palette(PaletteKind::Fire, &legacy, 0.75, 0.0, 1.0, 1.0);
        let seeded = CustomGradient::from_palette(PaletteKind::Fire);
        let seeded_color = sample_palette(PaletteKind::Neon, &seeded, 0.75, 0.0, 1.0, 1.0);

        assert_eq!(custom_color, seeded_color);
    }

    #[test]
    fn normalize_turns_legacy_palette_flag_into_active_gradient() {
        let mut gradient = CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        };

        gradient.normalize_for_palette(PaletteKind::MonoChrome);

        assert!(gradient.enabled);
        assert_eq!(gradient.stops()[0], [0.0, 0.0, 0.0]);
        assert_eq!(gradient.stops()[3], [1.0, 1.0, 1.0]);
    }
}
