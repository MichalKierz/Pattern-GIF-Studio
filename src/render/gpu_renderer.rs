use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{
    animation::loop_time::LoopTime,
    render::{
        color::{CustomGradient, MAX_GRADIENT_COLORS, PaletteKind},
        formula::{CompiledFormula, FormulaBlendMode, FormulaLayer, FormulaSource},
        frame_buffer::{FrameBuffer, pixel_len},
        renderer::RenderParams,
        renderer::Renderer,
    },
};

pub const GPU_GRADIENT_COLOR_LIMIT: usize = MAX_GRADIENT_COLORS;

#[derive(Debug)]
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline_fingerprint: String,
    pipeline: Option<wgpu::ComputePipeline>,
}

impl GpuRenderer {
    pub fn new() -> Result<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .context("no compatible GPU adapter found")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Pattern GIF Studio GPU device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .context("failed to create GPU device")?;

        Ok(Self {
            device,
            queue,
            pipeline_fingerprint: String::new(),
            pipeline: None,
        })
    }

    pub fn try_render_frame(
        &mut self,
        params: &RenderParams,
        time: LoopTime,
        width: u32,
        height: u32,
    ) -> Result<FrameBuffer> {
        self.ensure_pipeline(params)?;
        let uniform = GpuUniform::from_params(params, time, width, height);
        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("preview uniform buffer"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let output_size = pixel_len(width, height) as wgpu::BufferAddress;
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("preview gpu output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("preview gpu readback"),
            size: output_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let pipeline = self.pipeline.as_ref().expect("pipeline initialized");
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("preview gpu bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("preview gpu encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("preview gpu compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, output_size);
        self.queue.submit(Some(encoder.finish()));

        let slice = readback_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .context("GPU polling failed")?;
        rx.recv()
            .context("GPU readback channel closed")?
            .context("GPU readback failed")?;
        let mapped = slice.get_mapped_range();
        let pixels = mapped.to_vec();
        drop(mapped);
        readback_buffer.unmap();

        Ok(FrameBuffer {
            width,
            height,
            pixels,
        })
    }

    fn ensure_pipeline(&mut self, params: &RenderParams) -> Result<()> {
        let fingerprint = shader_fingerprint(params);
        if self.pipeline.is_none() || self.pipeline_fingerprint != fingerprint {
            let shader_source = build_shader(params)?;
            let shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("fractal preview gpu shader"),
                    source: wgpu::ShaderSource::Wgsl(shader_source.into()),
                });
            self.pipeline = Some(self.device.create_compute_pipeline(
                &wgpu::ComputePipelineDescriptor {
                    label: Some("fractal preview gpu pipeline"),
                    layout: None,
                    module: &shader,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                },
            ));
            self.pipeline_fingerprint = fingerprint;
        }
        Ok(())
    }
}

impl Renderer for GpuRenderer {
    fn render_frame(
        &mut self,
        params: &RenderParams,
        time: LoopTime,
        width: u32,
        height: u32,
    ) -> FrameBuffer {
        self.try_render_frame(params, time, width, height)
            .unwrap_or_else(|_| FrameBuffer {
                width,
                height,
                pixels: vec![0; pixel_len(width, height)],
            })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GpuUniform {
    width: u32,
    height: u32,
    palette: u32,
    custom_gradient_enabled: u32,
    seed: u32,
    symmetry: u32,
    gradient_color_count: u32,
    _pad0: [u32; 3],
    angle: f32,
    zoom: f32,
    center_x: f32,
    center_y: f32,
    rotation_speed: f32,
    distortion: f32,
    detail: f32,
    smoothing: f32,
    smoothing_radius_pixels: f32,
    color_speed: f32,
    color_phase: f32,
    brightness: f32,
    contrast: f32,
    gradient_transition: f32,
    _pad1: [f32; 4],
    gradient_colors: [[f32; 4]; GPU_GRADIENT_COLOR_LIMIT],
    _pad2: [f32; 2],
}

impl GpuUniform {
    fn from_params(params: &RenderParams, time: LoopTime, width: u32, height: u32) -> Self {
        let color_animation_phase =
            params.color_phase + time.phase * params.color_speed.abs().max(0.05);
        let active_gradient =
            CustomGradient::active_for_palette(params.palette, &params.custom_gradient);
        let gradient_colors = gradient_colors(&active_gradient, color_animation_phase);
        Self {
            width,
            height,
            palette: palette_code(params.palette),
            custom_gradient_enabled: 1,
            seed: params.seed,
            symmetry: params.symmetry,
            gradient_color_count: gradient_color_count(&active_gradient),
            _pad0: [0; 3],
            angle: time.angle,
            zoom: params.zoom,
            center_x: params.center_x,
            center_y: params.center_y,
            rotation_speed: params.rotation_speed,
            distortion: params.distortion,
            detail: params.detail,
            smoothing: params.smoothing,
            smoothing_radius_pixels: params.smoothing_radius_pixels,
            color_speed: params.color_speed,
            color_phase: params.color_phase,
            brightness: params.brightness,
            contrast: params.contrast,
            gradient_transition: active_gradient.transition,
            _pad1: [0.0; 4],
            gradient_colors,
            _pad2: [0.0; 2],
        }
    }
}

fn gradient_color_count(gradient: &crate::render::color::CustomGradient) -> u32 {
    let count = if gradient.stops().is_empty() {
        4
    } else {
        gradient.stops().len().min(GPU_GRADIENT_COLOR_LIMIT)
    };
    count.max(2) as u32
}

fn gradient_colors(
    gradient: &crate::render::color::CustomGradient,
    color_animation_phase: f32,
) -> [[f32; 4]; GPU_GRADIENT_COLOR_LIMIT] {
    let mut colors = [[0.0; 4]; GPU_GRADIENT_COLOR_LIMIT];
    let stops = gradient.animated_stops(color_animation_phase);
    for (index, color) in stops.iter().take(GPU_GRADIENT_COLOR_LIMIT).enumerate() {
        colors[index] = color4(*color);
    }
    for index in stops.len().min(GPU_GRADIENT_COLOR_LIMIT)..GPU_GRADIENT_COLOR_LIMIT {
        colors[index] = colors[index.saturating_sub(1)];
    }
    colors
}

fn color4(color: [f32; 3]) -> [f32; 4] {
    [color[0], color[1], color[2], 1.0]
}

fn palette_code(palette: PaletteKind) -> u32 {
    match palette {
        PaletteKind::Neon => 0,
        PaletteKind::Aurora => 1,
        PaletteKind::Fire => 2,
        PaletteKind::Candy => 3,
        PaletteKind::MonoChrome => 4,
    }
}

fn shader_fingerprint(params: &RenderParams) -> String {
    format!(
        "{}|{}",
        serde_json::to_string(&params.patterns).unwrap_or_default(),
        serde_json::to_string(&params.effects).unwrap_or_default()
    )
}

fn build_shader(params: &RenderParams) -> Result<String> {
    let mut source_functions = String::new();
    let mut pattern_sampler = String::from(
        "fn sample_pattern_layers(x: f32, y: f32) -> f32 {
    var total = 0.0;
    var weight = 0.0;
",
    );
    for (index, source) in params.patterns.iter().enumerate() {
        if !source.enabled {
            continue;
        }
        let name = format!("pattern_source_{index}");
        source_functions.push_str(&source_function(
            &name,
            source.scale,
            source.motion,
            source.strength,
            source.morph,
            &source.source,
        )?);
        source_functions.push('\n');
        let strength = wgsl_f32(source.strength.clamp(0.0, 1.0));
        pattern_sampler.push_str(&format!(
            "    let {name}_coords = source_coords(x, y, {zoom_loop}, {orbit});
    total = total + {name}({name}_coords.x, {name}_coords.y) * {strength};
    weight = weight + {strength};
",
            zoom_loop = wgsl_f32(source.camera_zoom_loop),
            orbit = wgsl_f32(source.camera_orbit),
        ));
    }
    pattern_sampler.push_str(
        "    if weight > 0.0 { return clamp(total / weight, 0.0, 1.0); }
    return 0.5;
}
",
    );

    let mut effects_sampler = String::from(
        "fn apply_effect_layers(value_in: f32, x: f32, y: f32) -> f32 {
    var value = value_in;
",
    );
    for (index, source) in params.effects.iter().enumerate() {
        if !source.enabled {
            continue;
        }
        let name = format!("effect_source_{index}");
        source_functions.push_str(&source_function(
            &name,
            source.scale,
            source.motion,
            source.strength,
            source.morph,
            &source.source,
        )?);
        source_functions.push('\n');
        let strength = wgsl_f32(source.strength.clamp(0.0, 1.0));
        let mode = effect_blend_mode_code(source.blend_mode);
        effects_sampler.push_str(&format!(
            "    let {name}_coords = source_coords(x, y, {zoom_loop}, {orbit});
    let {name}_effect = {name}({name}_coords.x, {name}_coords.y);
    let {name}_offset = ({name}_effect - 0.5) * {strength} * {scale} * 0.18;
    let {name}_displaced = sample_pattern_layers(x + {name}_offset, y - {name}_offset);
    value = apply_effect_blend(value, {name}_effect, {name}_displaced, {strength}, {mode});
",
            zoom_loop = wgsl_f32(source.camera_zoom_loop),
            orbit = wgsl_f32(source.camera_orbit),
            scale = wgsl_f32(source.scale.max(0.1)),
        ));
    }
    effects_sampler.push_str(
        "    return value;
}
",
    );

    Ok(format!(
        "{SHADER_HEADER}\n{source_functions}\n{pattern_sampler}\n{effects_sampler}\n{SHADER_MAIN}"
    ))
}

fn effect_blend_mode_code(mode: crate::render::renderer::EffectBlendMode) -> &'static str {
    match mode {
        crate::render::renderer::EffectBlendMode::Multiply => "0u",
        crate::render::renderer::EffectBlendMode::Screen => "1u",
        crate::render::renderer::EffectBlendMode::Add => "2u",
        crate::render::renderer::EffectBlendMode::Subtract => "3u",
        crate::render::renderer::EffectBlendMode::Difference => "4u",
        crate::render::renderer::EffectBlendMode::Mask => "5u",
        crate::render::renderer::EffectBlendMode::Contrast => "6u",
        crate::render::renderer::EffectBlendMode::Displace => "7u",
    }
}

fn source_function(
    name: &str,
    scale: f32,
    motion: f32,
    blend: f32,
    transition: f32,
    source: &FormulaSource,
) -> Result<String> {
    let control = |index: usize| {
        source
            .controls
            .get(index)
            .map(|control| control.value)
            .unwrap_or(0.5)
    };
    let layer_vars = format!(
        "    let formula_scale = {};\n    let formula_motion = {};\n    let formula_blend = {};\n    let formula_transition = {};\n    let formula_p1 = {};\n    let formula_p2 = {};\n    let formula_p3 = {};\n    let formula_p4 = {};\n",
        wgsl_f32(scale),
        wgsl_f32(motion),
        wgsl_f32(blend),
        wgsl_f32(transition),
        wgsl_f32(control(0)),
        wgsl_f32(control(1)),
        wgsl_f32(control(2)),
        wgsl_f32(control(3))
    );
    if source.layers.is_empty() {
        let formula = CompiledFormula::compile(&source.expression)
            .with_context(|| format!("invalid GPU formula {name}"))?;
        let value_statement = formula.wgsl_value_statement("raw_value", "    ");
        return Ok(format!(
            "fn {name}(x0: f32, y0: f32) -> f32 {{
    let x = x0;
    let y = y0;
    let origin_x = x0;
    let origin_y = y0;
    let formula_prev = 0.0;
{layer_vars}
{value_statement}
    return apply_formula_output(raw_value, {}, {});
}}",
            wgsl_f32(source.gain),
            wgsl_f32(source.bias)
        ));
    }

    let mut body = String::from(
        "    let origin_x = x0;\n    let origin_y = y0;\n    var domain_x = x0;\n    var domain_y = y0;\n    var value = 0.0;\n",
    );
    let mut active_index = 0usize;
    for layer in &source.layers {
        if !layer.enabled {
            continue;
        }
        body.push_str(&layer_block(layer, active_index)?);
        active_index += 1;
    }
    Ok(format!(
        "fn {name}(x0: f32, y0: f32) -> f32 {{
{layer_vars}
{body}
    return apply_formula_output(value, {}, {});
}}",
        wgsl_f32(source.gain),
        wgsl_f32(source.bias)
    ))
}

fn layer_block(layer: &FormulaLayer, active_index: usize) -> Result<String> {
    let formula = CompiledFormula::compile(&layer.expression)
        .with_context(|| format!("invalid GPU layer formula {}", layer.name))?;
    let domain_x_formula = CompiledFormula::compile(&layer.domain_x)
        .with_context(|| format!("invalid GPU layer domain X {}", layer.name))?;
    let domain_y_formula = CompiledFormula::compile(&layer.domain_y)
        .with_context(|| format!("invalid GPU layer domain Y {}", layer.name))?;
    let value_statement = formula.wgsl_value_statement("raw_layer_value", "        ");
    let domain_x_statement = domain_x_formula.wgsl_value_statement("target_x", "        ");
    let domain_y_statement = domain_y_formula.wgsl_value_statement("target_y", "        ");
    let blend = match layer.blend_mode {
        FormulaBlendMode::Replace => "0u",
        FormulaBlendMode::Add => "1u",
        FormulaBlendMode::Multiply => "2u",
        FormulaBlendMode::Screen => "3u",
        FormulaBlendMode::Difference => "4u",
        FormulaBlendMode::Min => "5u",
        FormulaBlendMode::Max => "6u",
    };
    Ok(format!(
        "    {{
        let formula_prev = value;
        let wave_x = sin(params.angle) * {motion_x};
        let wave_y = cos(params.angle) * {motion_y};
        let base_x = domain_x * {repeat_x} + {offset_x} + wave_x + sin(domain_y * {warp_x}) * {warp_x};
        let base_y = domain_y * {repeat_y} + {offset_y} + wave_y + cos(domain_x * {warp_y}) * {warp_y};
        var x = base_x;
        var y = base_y;
{domain_x_statement}
{domain_y_statement}
        let domain_influence = {domain_influence};
        x = mix(base_x, target_x, domain_influence);
        y = mix(base_y, target_y, domain_influence);
{value_statement}
        let layer_value = apply_formula_output(raw_layer_value, {gain}, {bias});
        value = apply_layer(value, layer_value, {opacity}, {blend}, {first});
        domain_x = mix(domain_x, x, domain_influence);
        domain_y = mix(domain_y, y, domain_influence);
    }}
",
        motion_x = wgsl_f32(layer.motion_x),
        motion_y = wgsl_f32(layer.motion_y),
        repeat_x = wgsl_f32(layer.repeat_x.max(0.001)),
        repeat_y = wgsl_f32(layer.repeat_y.max(0.001)),
        offset_x = wgsl_f32(layer.offset_x),
        offset_y = wgsl_f32(layer.offset_y),
        warp_x = wgsl_f32(layer.warp_x),
        warp_y = wgsl_f32(layer.warp_y),
        gain = wgsl_f32(layer.gain),
        bias = wgsl_f32(layer.bias),
        opacity = wgsl_f32(layer.opacity.clamp(0.0, 1.0)),
        domain_influence = wgsl_f32(layer.domain_influence.clamp(0.0, 1.0)),
        first = if active_index == 0 { "true" } else { "false" },
    ))
}

fn wgsl_f32(value: f32) -> String {
    if value.is_finite() {
        let text = format!("{value:.8}");
        let text = text.trim_end_matches('0').trim_end_matches('.');
        if text.contains('.') {
            text.to_owned()
        } else {
            format!("{text}.0")
        }
    } else {
        "0.0".to_owned()
    }
}

const SHADER_HEADER: &str = r#"
struct Params {
    width: u32,
    height: u32,
    palette: u32,
    custom_gradient_enabled: u32,
    seed: u32,
    symmetry: u32,
    gradient_color_count: u32,
    _pad0a: u32,
    _pad0b: u32,
    _pad0c: u32,
    angle: f32,
    zoom: f32,
    center_x: f32,
    center_y: f32,
    rotation_speed: f32,
    distortion: f32,
    detail: f32,
    smoothing: f32,
    smoothing_radius_pixels: f32,
    color_speed: f32,
    color_phase: f32,
    brightness: f32,
    contrast: f32,
    gradient_transition: f32,
    _pad1a: f32,
    _pad1b: f32,
    _pad1c: f32,
    gradient_colors: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> pixels: array<u32>;

fn fract01(v: f32) -> f32 { return v - floor(v); }
fn safe_div(a: f32, b: f32) -> f32 { if abs(b) < 0.000001 { return 0.0; } return a / b; }
fn safe_rem(a: f32, b: f32) -> f32 { if abs(b) < 0.000001 { return 0.0; } return a - floor(a / b) * b; }
fn apply_formula_output(raw: f32, gain: f32, bias: f32) -> f32 {
    if raw == raw {
        return clamp(raw * gain + bias, 0.0, 1.0);
    }
    return 0.0;
}
fn hash01(seed: u32, xi: i32, yi: i32) -> f32 {
    var n = u32(xi) * 374761393u + u32(yi) * 668265263u + seed * 1442695041u;
    n = (n ^ (n >> 13u)) * 1274126177u;
    n = n ^ (n >> 16u);
    return f32(n & 16777215u) / 16777215.0;
}
fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let xi = i32(floor(x));
    let yi = i32(floor(y));
    let xf = x - f32(xi);
    let yf = y - f32(yi);
    let tx = smoothstep(0.0, 1.0, xf);
    let ty = smoothstep(0.0, 1.0, yf);
    let a = hash01(seed, xi, yi);
    let b = hash01(seed, xi + 1, yi);
    let c = hash01(seed, xi, yi + 1);
    let d = hash01(seed, xi + 1, yi + 1);
    return mix(mix(a, b, tx), mix(c, d, tx), ty);
}
fn source_coords(x: f32, y: f32, camera_zoom_loop: f32, camera_orbit: f32) -> vec2<f32> {
    let orbit_x = sin(params.angle) * camera_orbit;
    let orbit_y = cos(params.angle) * camera_orbit;
    let zoom_wave = max(1.0 + camera_zoom_loop * (0.5 - 0.5 * cos(params.angle)), 0.05);
    return vec2<f32>((x + orbit_x) / zoom_wave, (y + orbit_y) / zoom_wave);
}
fn fbm_noise(x: f32, y: f32, seed: u32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    var norm = 0.0;
    for (var octave = 0u; octave < 5u; octave = octave + 1u) {
        sum = sum + value_noise(x * freq, y * freq, seed + octave * 101u) * amp;
        norm = norm + amp;
        freq = freq * 2.03;
        amp = amp * 0.5;
    }
    return sum / max(norm, 0.000001);
}
fn cell_noise(x: f32, y: f32, seed: u32) -> f32 {
    let base_x = i32(floor(x));
    let base_y = i32(floor(y));
    var nearest = 9999.0;
    for (var oy = -1; oy <= 1; oy = oy + 1) {
        for (var ox = -1; ox <= 1; ox = ox + 1) {
            let cell_x = base_x + ox;
            let cell_y = base_y + oy;
            let point_x = f32(cell_x) + hash01(seed, cell_x, cell_y);
            let point_y = f32(cell_y) + hash01(seed + 17u, cell_x, cell_y);
            let delta = vec2<f32>(x - point_x, y - point_y);
            nearest = min(nearest, length(delta));
        }
    }
    return clamp(nearest, 0.0, 1.0);
}
fn mandelbrot_formula(cx: f32, cy: f32) -> f32 {
    var zx = 0.0;
    var zy = 0.0;
    for (var i = 0u; i < 96u; i = i + 1u) {
        let next_x = zx * zx - zy * zy + cx;
        let next_y = 2.0 * zx * zy + cy;
        zx = next_x;
        zy = next_y;
        let radius_sq = zx * zx + zy * zy;
        if radius_sq > 4.0 {
            let smooth_iter = f32(i) + 1.0 - log2(max(log2(radius_sq), 0.000001));
            return clamp(smooth_iter / 96.0, 0.0, 1.0);
        }
    }
    return 0.0;
}
fn burning_ship_formula(cx: f32, cy: f32) -> f32 {
    var zx = 0.0;
    var zy = 0.0;
    for (var i = 0u; i < 96u; i = i + 1u) {
        let ax = abs(zx);
        let ay = abs(zy);
        let next_x = ax * ax - ay * ay + cx;
        let next_y = 2.0 * ax * ay + cy;
        zx = next_x;
        zy = next_y;
        let radius_sq = zx * zx + zy * zy;
        if radius_sq > 4.0 {
            let smooth_iter = f32(i) + 1.0 - log2(max(log2(radius_sq), 0.000001));
            return clamp(smooth_iter / 96.0, 0.0, 1.0);
        }
    }
    return 0.0;
}
fn julia_formula(zx0: f32, zy0: f32, cx: f32, cy: f32) -> f32 {
    var zx = zx0;
    var zy = zy0;
    for (var i = 0u; i < 96u; i = i + 1u) {
        let next_x = zx * zx - zy * zy + cx;
        let next_y = 2.0 * zx * zy + cy;
        zx = next_x;
        zy = next_y;
        let radius_sq = zx * zx + zy * zy;
        if radius_sq > 4.0 {
            let smooth_iter = f32(i) + 1.0 - log2(max(log2(radius_sq), 0.000001));
            return clamp(smooth_iter / 96.0, 0.0, 1.0);
        }
    }
    return 0.0;
}
fn apply_layer(base: f32, layer: f32, opacity: f32, mode: u32, first: bool) -> f32 {
    if first || mode == 0u { return mix(base, layer, opacity); }
    var blended = layer;
    if mode == 1u { blended = clamp(base + layer, 0.0, 1.0); }
    if mode == 2u { blended = base * layer; }
    if mode == 3u { blended = 1.0 - (1.0 - base) * (1.0 - layer); }
    if mode == 4u { blended = abs(base - layer); }
    if mode == 5u { blended = min(base, layer); }
    if mode == 6u { blended = max(base, layer); }
    return mix(base, blended, opacity);
}
fn apply_effect_blend(base: f32, effect: f32, displaced: f32, strength: f32, mode: u32) -> f32 {
    var blended = base * effect;
    if mode == 1u {
        blended = 1.0 - (1.0 - base) * (1.0 - effect);
    }
    if mode == 2u {
        blended = base + (effect - 0.5) * 1.35;
    }
    if mode == 3u {
        blended = base - effect * 0.85;
    }
    if mode == 4u {
        blended = abs(base - effect);
    }
    if mode == 5u {
        blended = base * effect;
    }
    if mode == 6u {
        blended = (base - 0.5) * (1.0 + effect * 3.0) + 0.5;
    }
    if mode == 7u {
        blended = displaced;
    }
    return clamp(mix(base, blended, strength), 0.0, 1.0);
}
fn rotate_xy(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}
fn sample_layer_stack(x: f32, y: f32) -> f32 {
    return apply_effect_layers(sample_pattern_layers(x, y), x, y);
}
fn sample_scene_value(x: f32, y: f32, footprint: vec2<f32>) -> f32 {
    return sample_layer_stack(x, y);
}
fn smoothing_radius_scale(smoothing_in: f32) -> f32 {
    let smoothing = clamp(smoothing_in, 0.0, 20.0);
    return min(0.25 + sqrt(smoothing / 20.0) * 0.75, 1.0);
}
fn sample_scene_color(x: f32, y: f32) -> vec4<f32> {
    let value = sample_scene_value(x, y, vec2<f32>(0.0, 0.0));
    let shifted = value + params.color_phase + sin(params.angle) * params.color_speed * 0.18;
    return sample_palette(shifted);
}
fn sample_rendered_color(x: f32, y: f32, footprint: vec2<f32>) -> vec4<f32> {
    let smoothing = clamp(params.smoothing, 0.0, 20.0);
    if smoothing <= 0.0001 {
        return sample_scene_color(x, y);
    }

    let radius = footprint * smoothing_radius_scale(smoothing) * clamp(params.smoothing_radius_pixels, 0.0, 10.0);
    var total = sample_scene_color(x, y).rgb;
    var weight = 1.0;
    total = total + sample_scene_color(x - radius.x * 0.32, y - radius.y * 0.18).rgb;
    total = total + sample_scene_color(x + radius.x * 0.28, y - radius.y * 0.31).rgb;
    total = total + sample_scene_color(x - radius.x * 0.22, y + radius.y * 0.29).rgb;
    total = total + sample_scene_color(x + radius.x * 0.34, y + radius.y * 0.22).rgb;
    weight = weight + 4.0;

    if smoothing >= 1.5 {
        total = total + sample_scene_color(x - radius.x * 0.48, y + radius.y * 0.04).rgb * 0.85;
        total = total + sample_scene_color(x + radius.x * 0.46, y - radius.y * 0.05).rgb * 0.85;
        total = total + sample_scene_color(x - radius.x * 0.07, y - radius.y * 0.46).rgb * 0.85;
        total = total + sample_scene_color(x + radius.x * 0.08, y + radius.y * 0.48).rgb * 0.85;
        weight = weight + 3.4;
    }
    if smoothing >= 6.0 {
        total = total + sample_scene_color(x - radius.x * 0.48, y - radius.y * 0.42).rgb;
        total = total + sample_scene_color(x + radius.x * 0.43, y - radius.y * 0.47).rgb;
        total = total + sample_scene_color(x - radius.x * 0.39, y + radius.y * 0.45).rgb;
        total = total + sample_scene_color(x + radius.x * 0.49, y + radius.y * 0.36).rgb;
        total = total + sample_scene_color(x - radius.x * 0.24, y - radius.y * 0.12).rgb * 0.75;
        total = total + sample_scene_color(x + radius.x * 0.18, y - radius.y * 0.25).rgb * 0.75;
        total = total + sample_scene_color(x - radius.x * 0.14, y + radius.y * 0.21).rgb * 0.75;
        total = total + sample_scene_color(x + radius.x * 0.27, y + radius.y * 0.13).rgb * 0.75;
        total = total + sample_scene_color(x - radius.x * 0.49, y + radius.y * 0.24).rgb * 0.65;
        total = total + sample_scene_color(x + radius.x * 0.48, y + radius.y * 0.19).rgb * 0.65;
        total = total + sample_scene_color(x - radius.x * 0.22, y - radius.y * 0.49).rgb * 0.65;
        total = total + sample_scene_color(x + radius.x * 0.25, y + radius.y * 0.49).rgb * 0.65;
        weight = weight + 10.6;
    }
    return vec4<f32>(clamp(total / weight, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#;

const SHADER_MAIN: &str = r#"
fn sample_plasma(x: f32, y: f32) -> f32 {
    let orbit_x = cos(params.angle) * params.distortion * 0.45;
    let orbit_y = sin(params.angle) * params.distortion * 0.45;
    let detail = max(params.detail, 0.25);
    let wave_a = sin(x * 7.0 * detail + orbit_x * 3.0);
    let wave_b = cos(y * 6.5 * detail + orbit_y * 3.0);
    let wave_c = sin(length(vec2<f32>(x + orbit_x, y - orbit_y)) * 8.0 * detail - params.angle * 1.3);
    let wave_d = cos((x * 3.0 + y * 5.0) * detail + cos(params.angle) * 2.0);
    let value = (wave_a + wave_b + wave_c + wave_d) * 0.125 + 0.5;
    return value;
}
fn sample_mandelbrot(x: f32, y: f32) -> f32 {
    let center_x = -0.62 + cos(params.angle) * 0.045 * params.distortion;
    let center_y = sin(params.angle) * 0.045 * params.distortion;
    let scale = 1.55 / max(params.zoom, 0.1) / sqrt(max(params.detail, 0.25));
    let c_re = center_x + x * scale;
    let c_im = center_y + y * scale;
    var z_re = 0.0;
    var z_im = 0.0;
    for (var i = 0u; i < 96u; i = i + 1u) {
        let next_re = z_re * z_re - z_im * z_im + c_re;
        let next_im = 2.0 * z_re * z_im + c_im;
        z_re = next_re;
        z_im = next_im;
        let radius_sq = z_re * z_re + z_im * z_im;
        if radius_sq > 4.0 {
            let smooth_iter = f32(i) + 1.0 - log2(log2(radius_sq));
            return clamp(smooth_iter / 96.0, 0.0, 1.0);
        }
    }
    return 0.0;
}
fn sample_julia(x: f32, y: f32) -> f32 {
    let c_re = -0.72 + cos(params.angle) * 0.18 * params.distortion;
    let c_im = 0.27 + sin(params.angle) * 0.18 * params.distortion;
    let detail = sqrt(max(params.detail, 0.25));
    var z_re = x * 1.35 / detail;
    var z_im = y * 1.35 / detail;
    for (var i = 0u; i < 96u; i = i + 1u) {
        let next_re = z_re * z_re - z_im * z_im + c_re;
        let next_im = 2.0 * z_re * z_im + c_im;
        z_re = next_re;
        z_im = next_im;
        let radius_sq = z_re * z_re + z_im * z_im;
        if radius_sq > 4.0 {
            let smooth_iter = f32(i) + 1.0 - log2(log2(radius_sq));
            return clamp(smooth_iter / 96.0, 0.0, 1.0);
        }
    }
    return 0.0;
}
fn sample_tunnel(x: f32, y: f32) -> f32 {
    let radius = max(length(vec2<f32>(x, y)), 0.001);
    let angle = atan2(y, x);
    let symmetry = f32(max(params.symmetry, 1u));
    let detail = max(params.detail, 0.25);
    let rings = 0.26 * detail / radius + sin(params.angle) * 0.5;
    let spokes = angle / 6.283185307179586 * symmetry;
    let warp = sin(angle * symmetry + params.angle) * params.distortion * 0.09;
    let v = rings + spokes * 0.25 + warp;
    let shade = clamp(1.0 - radius * 0.55, 0.0, 1.0);
    let value = fract01(v) * 0.72 + shade * 0.28;
    return value;
}
fn sample_kaleidoscope(x: f32, y: f32) -> f32 {
    let radius = length(vec2<f32>(x, y));
    let sectors = f32(max(params.symmetry, 2u));
    let sector_angle = 6.283185307179586 / sectors;
    let raw_angle = atan2(y, x) + sin(params.angle) * 0.25;
    let folded = fract01(raw_angle / sector_angle);
    let mirror = abs(folded - 0.5) * 2.0;
    let pulse = sin(radius * 12.0 * max(params.detail, 0.25) - params.angle * 1.5) * 0.5 + 0.5;
    let spokes = abs(cos(mirror * 3.141592653589793));
    let warp = sin(x * 5.0) * cos(y * 5.0) * params.distortion * 0.14;
    let value = fract01(pulse * 0.5 + spokes * 0.42 + warp);
    return value;
}
fn sample_noise_warp(x: f32, y: f32) -> f32 {
    let orbit_x = cos(params.angle) * 2.0;
    let orbit_y = sin(params.angle) * 2.0;
    let scale = (3.5 + params.zoom * 0.6) * max(params.detail, 0.25);
    let n1 = value_noise(x * scale + orbit_x, y * scale + orbit_y, params.seed + 11u);
    let n2 = value_noise(x * scale * 2.0 - orbit_y, y * scale * 2.0 + orbit_x, params.seed + 29u);
    let wave = sin((x + n1 * params.distortion) * 7.0 + params.angle) + cos((y - n2 * params.distortion) * 7.5 - params.angle);
    let value = fract01(n1 * 0.45 + n2 * 0.35 + wave * 0.12);
    return value;
}
fn gradient4(c0: vec3<f32>, c1: vec3<f32>, c2: vec3<f32>, c3: vec3<f32>, value: f32) -> vec3<f32> {
    let scaled = fract01(value) * 4.0;
    let idx = u32(floor(scaled));
    let t = scaled - floor(scaled);
    if idx == 0u { return mix(c0, c1, t); }
    if idx == 1u { return mix(c1, c2, t); }
    if idx == 2u { return mix(c2, c3, t); }
    return mix(c3, c0, t);
}
fn custom_gradient(value: f32) -> vec3<f32> {
    let count = max(params.gradient_color_count, 2u);
    let scaled = fract01(value) * f32(count);
    let idx = u32(floor(scaled)) % count;
    let next = (idx + 1u) % count;
    let t = scaled - floor(scaled);
    return mix(params.gradient_colors[idx].rgb, params.gradient_colors[next].rgb, t);
}
fn hsv_to_rgb(h_in: f32, s: f32, v: f32) -> vec3<f32> {
    let h = fract01(h_in) * 6.0;
    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    if i < 1.0 { return vec3<f32>(v, t, p); }
    if i < 2.0 { return vec3<f32>(q, v, p); }
    if i < 3.0 { return vec3<f32>(p, v, t); }
    if i < 4.0 { return vec3<f32>(p, q, v); }
    if i < 5.0 { return vec3<f32>(t, p, v); }
    return vec3<f32>(v, p, q);
}
fn apply_contrast(value: f32) -> f32 {
    return clamp((fract01(value) - 0.5) * max(params.contrast, 0.05) + 0.5, 0.0, 1.0);
}
fn sample_palette(value_in: f32) -> vec4<f32> {
    let value = apply_contrast(value_in);
    var rgb: vec3<f32>;
    if params.custom_gradient_enabled == 1u {
        rgb = custom_gradient(value);
    } else if params.palette == 0u {
        rgb = hsv_to_rgb(fract01(value + 0.72), 0.88, 1.0);
    } else if params.palette == 1u {
        rgb = gradient4(vec3<f32>(0.02,0.09,0.16), vec3<f32>(0.05,0.55,0.45), vec3<f32>(0.30,0.85,0.95), vec3<f32>(0.95,0.35,0.75), value);
    } else if params.palette == 2u {
        rgb = gradient4(vec3<f32>(0.02,0.00,0.02), vec3<f32>(0.35,0.02,0.04), vec3<f32>(0.95,0.18,0.02), vec3<f32>(1.00,0.82,0.24), value);
    } else if params.palette == 3u {
        rgb = gradient4(vec3<f32>(0.05,0.02,0.12), vec3<f32>(0.95,0.10,0.65), vec3<f32>(0.10,0.85,1.00), vec3<f32>(1.00,0.90,0.25), value);
    } else {
        rgb = vec3<f32>(value, value, value);
    }
    return vec4<f32>(clamp(rgb * params.brightness, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
fn pack_rgba(color: vec4<f32>) -> u32 {
    let c = clamp(color, vec4<f32>(0.0), vec4<f32>(1.0));
    let r = u32(c.r * 255.0 + 0.5);
    let g = u32(c.g * 255.0 + 0.5);
    let b = u32(c.b * 255.0 + 0.5);
    let a = u32(c.a * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.width || gid.y >= params.height { return; }
    let aspect = f32(params.width) / max(f32(params.height), 1.0);
    var nx = ((f32(gid.x) + 0.5) / max(f32(params.width), 1.0)) * 2.0 - 1.0;
    var ny = ((f32(gid.y) + 0.5) / max(f32(params.height), 1.0)) * 2.0 - 1.0;
    var footprint = vec2<f32>(2.0 / max(f32(params.width), 1.0), 2.0 / max(f32(params.height), 1.0));
    if aspect >= 1.0 { nx = nx * aspect; } else { ny = ny / max(aspect, 0.001); }
    if aspect >= 1.0 { footprint.x = footprint.x * aspect; } else { footprint.y = footprint.y / max(aspect, 0.001); }
    nx = nx / max(params.zoom, 0.1) + params.center_x;
    ny = ny / max(params.zoom, 0.1) + params.center_y;
    footprint = footprint / max(params.zoom, 0.1);
    let rotated = rotate_xy(vec2<f32>(nx, ny), sin(params.angle) * params.rotation_speed);
    let rgba = sample_rendered_color(rotated.x, rotated.y, footprint);
    let index = gid.y * params.width + gid.x;
    pixels[index] = pack_rgba(rgba);
}
"#;

#[cfg(test)]
mod tests {
    use super::{GPU_GRADIENT_COLOR_LIMIT, build_shader};
    use crate::{
        animation::loop_time::LoopTime,
        render::{
            color::MAX_GRADIENT_COLORS,
            formula::{FormulaLayer, FormulaSource},
            renderer::{EffectLayer, PatternLayer, RenderParams},
        },
    };

    #[test]
    fn gpu_shader_is_generated_for_layered_custom_formulas() {
        let mut params = RenderParams::default();
        params.patterns[0].source.layers = vec![FormulaLayer {
            name: "Custom fractal DSL".to_owned(),
            expression: "fractal\niterations = 32\nzx = x\nzy = y\ncx = -0.5\ncy = 0.25\nnext_zx = zx * zx - zy * zy + cx\nnext_zy = 2 * zx * zy + cy\nescape = zx * zx + zy * zy\nescape_radius = 4\noutput = iter / iterations".to_owned(),
            gain: 1.0,
            bias: 0.0,
            ..FormulaLayer::default()
        }];
        params.effects.push(EffectLayer::default());
        params.effects[0].source.layers[0].expression = "noise(x * scale, y * scale)".to_owned();

        let shader = build_shader(&params).expect("gpu shader");

        assert!(shader.contains("fn pattern_source_0"));
        assert!(shader.contains("fn effect_source_0"));
        assert!(shader.contains("fractal_zx"));
        assert!(shader.contains("params.smoothing"));
        assert!(shader.contains("params.smoothing_radius_pixels"));
        assert!(shader.contains("sample_scene_value"));
        assert!(shader.contains("gradient_colors: array<vec4<f32>, 16>"));
        assert!(shader.contains("@compute"));
    }

    #[test]
    fn gpu_gradient_limit_matches_model_limit() {
        assert_eq!(GPU_GRADIENT_COLOR_LIMIT, MAX_GRADIENT_COLORS);
    }

    #[test]
    fn gpu_shader_ignores_disabled_pattern_and_effect_sources() {
        let mut params = RenderParams::default();
        params.patterns.push(PatternLayer {
            enabled: false,
            source: FormulaSource {
                expression: "unknown_function(x)".to_owned(),
                layers: Vec::new(),
                ..FormulaSource::default()
            },
            ..PatternLayer::default()
        });
        params.effects.push(EffectLayer {
            enabled: false,
            source: FormulaSource {
                expression: "unknown_function(y)".to_owned(),
                layers: Vec::new(),
                ..FormulaSource::default()
            },
            ..EffectLayer::default()
        });

        let shader = build_shader(&params).expect("gpu shader");

        assert!(!shader.contains("pattern_source_1"));
        assert!(!shader.contains("effect_source_0"));
    }

    #[test]
    fn gpu_renderer_renders_when_adapter_is_available() {
        let Ok(mut renderer) = super::GpuRenderer::new() else {
            return;
        };
        let params = RenderParams::default();
        let frame = renderer
            .try_render_frame(&params, LoopTime::from_frame(0, 24), 16, 16)
            .expect("gpu render frame");

        assert_eq!(frame.width, 16);
        assert_eq!(frame.height, 16);
        assert_eq!(frame.pixels.len(), 16 * 16 * 4);
        assert!(frame.pixels.chunks_exact(4).all(|px| px[3] == 255));
    }
}
