use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    animation::loop_time::LoopTime,
    render::renderer::{EffectBlendMode, RenderParams},
    utils::math::clamp01,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FormulaSource {
    pub expression: String,
    pub gain: f32,
    pub bias: f32,
    pub effect_blend_mode: Option<EffectBlendMode>,
    pub controls: Vec<FormulaControl>,
    pub layers: Vec<FormulaLayer>,
}

impl FormulaSource {
    pub fn fractal_a() -> Self {
        Self {
            expression: "sin(r * 12 + t * 3) + cos(a * symmetry + u * 2)".to_owned(),
            gain: 0.5,
            bias: 0.5,
            effect_blend_mode: None,
            controls: Vec::new(),
            layers: vec![
                FormulaLayer {
                    name: "Radial structure".to_owned(),
                    expression: "sin(r * 12 + t * 3)".to_owned(),
                    gain: 0.5,
                    bias: 0.5,
                    ..FormulaLayer::default()
                },
                FormulaLayer {
                    name: "Angular detail".to_owned(),
                    expression: "cos(a * symmetry + u * 2)".to_owned(),
                    opacity: 0.45,
                    blend_mode: FormulaBlendMode::Add,
                    ..FormulaLayer::default()
                },
            ],
        }
    }

    pub fn fractal_b() -> Self {
        Self {
            expression: "sin((x * x - y * y) * 18 + u * 4) * cos(r * 10 - t * 3)".to_owned(),
            gain: 0.5,
            bias: 0.5,
            effect_blend_mode: None,
            controls: Vec::new(),
            layers: vec![
                FormulaLayer {
                    name: "Quadratic field".to_owned(),
                    expression: "sin((x * x - y * y) * 18 + u * 4)".to_owned(),
                    gain: 0.5,
                    bias: 0.5,
                    ..FormulaLayer::default()
                },
                FormulaLayer {
                    name: "Radial mask".to_owned(),
                    expression: "cos(r * 10 - t * 3)".to_owned(),
                    opacity: 0.55,
                    blend_mode: FormulaBlendMode::Multiply,
                    ..FormulaLayer::default()
                },
            ],
        }
    }

    pub fn pattern() -> Self {
        Self {
            expression: "sin(r * scale * 18 + a * symmetry + t * motion)".to_owned(),
            gain: 0.5,
            bias: 0.5,
            effect_blend_mode: None,
            controls: Vec::new(),
            layers: vec![FormulaLayer {
                name: "Pattern layer".to_owned(),
                expression: "sin(r * scale * 18 + a * symmetry + t * motion)".to_owned(),
                gain: 0.5,
                bias: 0.5,
                repeat_x: 1.0,
                repeat_y: 1.0,
                ..FormulaLayer::default()
            }],
        }
    }

    pub fn validate(&self, label: &str) -> Vec<FormulaIssue> {
        let mut issues = Vec::new();
        if self.layers.is_empty() {
            if let Err(error) = CompiledFormula::compile(&self.expression) {
                issues.push(FormulaIssue {
                    label: label.to_owned(),
                    message: error.to_string(),
                });
            }
            return issues;
        }

        for (index, layer) in self.layers.iter().enumerate() {
            if !layer.enabled {
                continue;
            }
            if let Err(error) = CompiledFormula::compile(&layer.expression) {
                issues.push(FormulaIssue {
                    label: format!("{label} / {}", layer_label(index, layer)),
                    message: error.to_string(),
                });
            }
            if let Err(error) = CompiledFormula::compile(&layer.domain_x) {
                issues.push(FormulaIssue {
                    label: format!("{label} / {} domain X", layer_label(index, layer)),
                    message: error.to_string(),
                });
            }
            if let Err(error) = CompiledFormula::compile(&layer.domain_y) {
                issues.push(FormulaIssue {
                    label: format!("{label} / {} domain Y", layer_label(index, layer)),
                    message: error.to_string(),
                });
            }
        }
        issues
    }

    pub fn editable_expression(&self) -> &str {
        self.layers
            .first()
            .map(|layer| layer.expression.as_str())
            .unwrap_or(self.expression.as_str())
    }

    pub fn set_editable_expression(&mut self, expression: String) {
        if let Some(layer) = self.layers.first_mut() {
            layer.expression = expression.clone();
        }
        self.expression = expression;
    }
}

impl Default for FormulaSource {
    fn default() -> Self {
        Self {
            expression: "sin(r * 12 + t * 3)".to_owned(),
            gain: 0.5,
            bias: 0.5,
            effect_blend_mode: None,
            controls: Vec::new(),
            layers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FormulaControl {
    pub name: String,
    pub value: f32,
}

impl Default for FormulaControl {
    fn default() -> Self {
        Self {
            name: "Control".to_owned(),
            value: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FormulaLayer {
    pub name: String,
    pub enabled: bool,
    pub expression: String,
    pub gain: f32,
    pub bias: f32,
    pub opacity: f32,
    pub blend_mode: FormulaBlendMode,
    pub repeat_x: f32,
    pub repeat_y: f32,
    pub warp_x: f32,
    pub warp_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub motion_x: f32,
    pub motion_y: f32,
    pub domain_x: String,
    pub domain_y: String,
    pub domain_influence: f32,
}

impl Default for FormulaLayer {
    fn default() -> Self {
        Self {
            name: "Layer".to_owned(),
            enabled: true,
            expression: "sin(r * 12 + t * 3)".to_owned(),
            gain: 0.5,
            bias: 0.5,
            opacity: 1.0,
            blend_mode: FormulaBlendMode::Replace,
            repeat_x: 1.0,
            repeat_y: 1.0,
            warp_x: 0.0,
            warp_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            motion_x: 0.0,
            motion_y: 0.0,
            domain_x: "x".to_owned(),
            domain_y: "y".to_owned(),
            domain_influence: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FormulaBlendMode {
    #[default]
    Replace,
    Add,
    Multiply,
    Screen,
    Difference,
    Min,
    Max,
}

impl FormulaBlendMode {
    pub const ALL: [Self; 7] = [
        Self::Replace,
        Self::Add,
        Self::Multiply,
        Self::Screen,
        Self::Difference,
        Self::Min,
        Self::Max,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Add => "Add",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Difference => "Difference",
            Self::Min => "Min",
            Self::Max => "Max",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaIssue {
    pub label: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CompiledFormula {
    kind: CompiledFormulaKind,
}

#[derive(Debug, Clone)]
enum CompiledFormulaKind {
    Scalar(Expr),
    Fractal(Box<CompiledFractal>),
}

impl CompiledFormula {
    pub fn compile(source: &str) -> Result<Self, FormulaError> {
        if is_fractal_dsl(source) {
            return Ok(Self {
                kind: CompiledFormulaKind::Fractal(Box::new(CompiledFractal::compile(source)?)),
            });
        }
        Ok(Self {
            kind: CompiledFormulaKind::Scalar(compile_scalar_expr(source)?),
        })
    }

    pub fn sample(&self, vars: FormulaVars) -> f32 {
        match &self.kind {
            CompiledFormulaKind::Scalar(expr) => expr.eval(vars),
            CompiledFormulaKind::Fractal(fractal) => fractal.sample(vars),
        }
    }

    pub fn wgsl_expression(&self) -> String {
        match &self.kind {
            CompiledFormulaKind::Scalar(expr) => expr.to_wgsl(),
            CompiledFormulaKind::Fractal(_) => "0.0".to_owned(),
        }
    }

    pub fn wgsl_value_statement(&self, output_var: &str, indent: &str) -> String {
        match &self.kind {
            CompiledFormulaKind::Scalar(expr) => {
                format!("{indent}let {output_var} = {};\n", expr.to_wgsl())
            }
            CompiledFormulaKind::Fractal(fractal) => fractal.to_wgsl(output_var, indent),
        }
    }
}

fn compile_scalar_expr(source: &str) -> Result<Expr, FormulaError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, index: 0 };
    let expr = parser.parse_expression()?;
    if !parser.is_done() {
        return Err(FormulaError::UnexpectedToken);
    }
    Ok(expr)
}

#[derive(Debug, Clone, Copy)]
pub struct FormulaVars<'a> {
    pub x: f32,
    pub y: f32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub r: f32,
    pub a: f32,
    pub time: LoopTime,
    pub params: &'a RenderParams,
    pub scale: f32,
    pub motion: f32,
    pub blend: f32,
    pub transition: f32,
    pub prev: f32,
    pub p1: f32,
    pub p2: f32,
    pub p3: f32,
    pub p4: f32,
    pub zx: f32,
    pub zy: f32,
    pub cx: f32,
    pub cy: f32,
    pub iter: f32,
    pub iterations: f32,
    pub escape_value: f32,
}

#[derive(Debug, Clone, Copy)]
struct FractalVars {
    zx: f32,
    zy: f32,
    cx: f32,
    cy: f32,
    iter: f32,
    iterations: f32,
    escape_value: f32,
}

impl<'a> FormulaVars<'a> {
    pub fn new(x: f32, y: f32, time: LoopTime, params: &'a RenderParams) -> Self {
        Self {
            x,
            y,
            origin_x: x,
            origin_y: y,
            r: (x * x + y * y).sqrt(),
            a: y.atan2(x),
            time,
            params,
            scale: 1.0,
            motion: 0.0,
            blend: 0.0,
            transition: 0.0,
            prev: 0.0,
            p1: 0.5,
            p2: 0.5,
            p3: 0.5,
            p4: 0.5,
            zx: 0.0,
            zy: 0.0,
            cx: 0.0,
            cy: 0.0,
            iter: 0.0,
            iterations: 1.0,
            escape_value: 0.0,
        }
    }

    pub fn with_layer_params(
        mut self,
        scale: f32,
        motion: f32,
        blend: f32,
        transition: f32,
    ) -> Self {
        self.scale = scale;
        self.motion = motion;
        self.blend = blend;
        self.transition = transition;
        self
    }

    pub fn with_controls(mut self, controls: &[FormulaControl]) -> Self {
        if let Some(control) = controls.first() {
            self.p1 = control.value;
        }
        if let Some(control) = controls.get(1) {
            self.p2 = control.value;
        }
        if let Some(control) = controls.get(2) {
            self.p3 = control.value;
        }
        if let Some(control) = controls.get(3) {
            self.p4 = control.value;
        }
        self
    }

    fn with_origin(mut self, origin_x: f32, origin_y: f32) -> Self {
        self.origin_x = origin_x;
        self.origin_y = origin_y;
        self
    }

    fn with_previous(mut self, prev: f32) -> Self {
        self.prev = prev;
        self
    }

    fn with_fractal_vars(mut self, fractal: FractalVars) -> Self {
        self.zx = fractal.zx;
        self.zy = fractal.zy;
        self.cx = fractal.cx;
        self.cy = fractal.cy;
        self.iter = fractal.iter;
        self.iterations = fractal.iterations;
        self.escape_value = fractal.escape_value;
        self
    }

    fn variable(self, name: &str) -> f32 {
        let params = self.params;
        match name {
            "x" => self.x,
            "y" => self.y,
            "origin_x" => self.origin_x,
            "origin_y" => self.origin_y,
            "r" => self.r,
            "a" => self.a,
            "t" => self.time.angle.sin(),
            "u" => self.time.angle.cos(),
            "phase" => 0.5 - 0.5 * self.time.angle.cos(),
            "pi" => std::f32::consts::PI,
            "tau" => std::f32::consts::TAU,
            "seed" => params.seed as f32,
            "zoom" => params.zoom,
            "symmetry" => params.symmetry as f32,
            "detail" => params.detail,
            "distortion" => params.distortion,
            "scale" => self.scale,
            "motion" => self.motion,
            "blend" => self.blend,
            "transition" => self.transition,
            "prev" => self.prev,
            "p1" => self.p1,
            "p2" => self.p2,
            "p3" => self.p3,
            "p4" => self.p4,
            "zx" => self.zx,
            "zy" => self.zy,
            "cx" => self.cx,
            "cy" => self.cy,
            "iter" => self.iter,
            "iterations" => self.iterations,
            "escape_value" => self.escape_value,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormulaError {
    EmptyExpression,
    InvalidFractalDsl(String),
    InvalidNumber,
    UnexpectedCharacter(char),
    UnexpectedToken,
    MissingClosingParen,
    UnknownFunction(String),
    UnknownVariable(String),
    WrongArity(String),
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExpression => write!(f, "Formula is empty"),
            Self::InvalidFractalDsl(message) => write!(f, "Invalid fractal DSL: {message}"),
            Self::InvalidNumber => write!(f, "Invalid number"),
            Self::UnexpectedCharacter(ch) => write!(f, "Unexpected character `{ch}`"),
            Self::UnexpectedToken => write!(f, "Unexpected token"),
            Self::MissingClosingParen => write!(f, "Missing closing parenthesis"),
            Self::UnknownFunction(name) => write!(f, "Unknown function `{name}`"),
            Self::UnknownVariable(name) => write!(f, "Unknown variable `{name}`"),
            Self::WrongArity(name) => write!(f, "Wrong number of arguments for `{name}`"),
        }
    }
}

impl std::error::Error for FormulaError {}

#[derive(Debug, Clone)]
enum Expr {
    Number(f32),
    Var(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

impl Expr {
    fn eval(&self, vars: FormulaVars<'_>) -> f32 {
        match self {
            Self::Number(value) => *value,
            Self::Var(name) => vars.variable(name),
            Self::Unary { op, expr } => match op {
                UnaryOp::Neg => -expr.eval(vars),
            },
            Self::Binary { op, left, right } => {
                let a = left.eval(vars);
                let b = right.eval(vars);
                match op {
                    BinaryOp::Add => a + b,
                    BinaryOp::Sub => a - b,
                    BinaryOp::Mul => a * b,
                    BinaryOp::Div => {
                        if b.abs() < 0.000_001 {
                            0.0
                        } else {
                            a / b
                        }
                    }
                    BinaryOp::Rem => {
                        if b.abs() < 0.000_001 {
                            0.0
                        } else {
                            a % b
                        }
                    }
                    BinaryOp::Pow => a.abs().powf(b),
                }
            }
            Self::Call { name, args } => eval_function(name, args, vars),
        }
    }

    fn to_wgsl(&self) -> String {
        match self {
            Self::Number(value) => format_float(*value),
            Self::Var(name) => wgsl_variable(name),
            Self::Unary { op, expr } => match op {
                UnaryOp::Neg => format!("(-{})", expr.to_wgsl()),
            },
            Self::Binary { op, left, right } => {
                let a = left.to_wgsl();
                let b = right.to_wgsl();
                match op {
                    BinaryOp::Add => format!("({a} + {b})"),
                    BinaryOp::Sub => format!("({a} - {b})"),
                    BinaryOp::Mul => format!("({a} * {b})"),
                    BinaryOp::Div => format!("safe_div({a}, {b})"),
                    BinaryOp::Rem => format!("safe_rem({a}, {b})"),
                    BinaryOp::Pow => format!("pow(abs({a}), {b})"),
                }
            }
            Self::Call { name, args } => wgsl_function_call(name, args),
        }
    }
}

pub fn sample_formula_source(
    source: &FormulaSource,
    compiled: &CompiledFormulaSource,
    vars: FormulaVars<'_>,
) -> f32 {
    let vars = vars.with_controls(&source.controls);
    match compiled {
        CompiledFormulaSource::Single(formula) => {
            apply_formula_output(formula.sample(vars), source.gain, source.bias)
        }
        CompiledFormulaSource::Layers(layers) => {
            apply_formula_output(sample_layers(layers, vars), source.gain, source.bias)
        }
    }
}

#[derive(Debug, Clone)]
pub enum CompiledFormulaSource {
    Single(CompiledFormula),
    Layers(Vec<CompiledFormulaLayer>),
}

impl CompiledFormulaSource {
    pub fn compile(source: &FormulaSource) -> Result<Self, FormulaError> {
        if source.layers.is_empty() {
            return Ok(Self::Single(CompiledFormula::compile(&source.expression)?));
        }

        let mut layers = Vec::new();
        for layer in &source.layers {
            if !layer.enabled {
                continue;
            }
            layers.push(CompiledFormulaLayer {
                layer: layer.clone(),
                formula: CompiledFormula::compile(&layer.expression)?,
                domain_x: CompiledFormula::compile(&layer.domain_x)?,
                domain_y: CompiledFormula::compile(&layer.domain_y)?,
            });
        }
        Ok(Self::Layers(layers))
    }
}

#[derive(Debug, Clone)]
pub struct CompiledFormulaLayer {
    layer: FormulaLayer,
    formula: CompiledFormula,
    domain_x: CompiledFormula,
    domain_y: CompiledFormula,
}

fn sample_layers(layers: &[CompiledFormulaLayer], vars: FormulaVars<'_>) -> f32 {
    if layers.is_empty() {
        return 0.0;
    }

    let mut value = 0.0;
    let mut domain_x = vars.x;
    let mut domain_y = vars.y;
    for (index, layer) in layers.iter().enumerate() {
        let base_vars = FormulaVars::new(domain_x, domain_y, vars.time, vars.params)
            .with_layer_params(vars.scale, vars.motion, vars.blend, vars.transition)
            .with_controls_placeholder(vars)
            .with_origin(vars.origin_x, vars.origin_y)
            .with_previous(value);
        let layer_vars = layer_vars(&layer.layer, base_vars);
        let target_x = layer.domain_x.sample(layer_vars);
        let target_y = layer.domain_y.sample(layer_vars);
        let influence = layer.layer.domain_influence.clamp(0.0, 1.0);
        let sample_x = mix(layer_vars.x, target_x, influence);
        let sample_y = mix(layer_vars.y, target_y, influence);
        let sample_vars = FormulaVars::new(sample_x, sample_y, vars.time, vars.params)
            .with_layer_params(vars.scale, vars.motion, vars.blend, vars.transition)
            .with_controls_placeholder(vars)
            .with_origin(vars.origin_x, vars.origin_y)
            .with_previous(value);
        let raw = layer.formula.sample(sample_vars);
        let layer_value = apply_formula_output(raw, layer.layer.gain, layer.layer.bias);
        let opacity = layer.layer.opacity.clamp(0.0, 1.0);
        value = if index == 0 || layer.layer.blend_mode == FormulaBlendMode::Replace {
            value + (layer_value - value) * opacity
        } else {
            let blended = blend_value(value, layer_value, layer.layer.blend_mode);
            value + (blended - value) * opacity
        };
        domain_x = mix(domain_x, sample_x, influence);
        domain_y = mix(domain_y, sample_y, influence);
    }
    clamp01(value)
}

fn layer_vars<'a>(layer: &FormulaLayer, vars: FormulaVars<'a>) -> FormulaVars<'a> {
    let wave_x = vars.time.angle.sin() * layer.motion_x;
    let wave_y = vars.time.angle.cos() * layer.motion_y;
    let warped_x = vars.x * layer.repeat_x.max(0.001)
        + layer.offset_x
        + wave_x
        + (vars.y * layer.warp_x).sin() * layer.warp_x;
    let warped_y = vars.y * layer.repeat_y.max(0.001)
        + layer.offset_y
        + wave_y
        + (vars.x * layer.warp_y).cos() * layer.warp_y;
    FormulaVars::new(warped_x, warped_y, vars.time, vars.params)
        .with_layer_params(vars.scale, vars.motion, vars.blend, vars.transition)
        .with_controls_placeholder(vars)
        .with_origin(vars.origin_x, vars.origin_y)
        .with_previous(vars.prev)
}

impl<'a> FormulaVars<'a> {
    fn with_controls_placeholder(mut self, source: FormulaVars<'a>) -> Self {
        self.p1 = source.p1;
        self.p2 = source.p2;
        self.p3 = source.p3;
        self.p4 = source.p4;
        self
    }
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn blend_value(base: f32, layer: f32, mode: FormulaBlendMode) -> f32 {
    match mode {
        FormulaBlendMode::Replace => layer,
        FormulaBlendMode::Add => clamp01(base + layer),
        FormulaBlendMode::Multiply => base * layer,
        FormulaBlendMode::Screen => 1.0 - (1.0 - base) * (1.0 - layer),
        FormulaBlendMode::Difference => (base - layer).abs(),
        FormulaBlendMode::Min => base.min(layer),
        FormulaBlendMode::Max => base.max(layer),
    }
}

pub fn apply_formula_output(raw: f32, gain: f32, bias: f32) -> f32 {
    if raw.is_finite() {
        clamp01(raw * gain + bias)
    } else {
        0.0
    }
}

#[derive(Debug, Clone)]
struct CompiledFractal {
    iterations: u32,
    zx: Expr,
    zy: Expr,
    cx: Expr,
    cy: Expr,
    next_zx: Expr,
    next_zy: Expr,
    escape: Expr,
    escape_radius: Expr,
    output: Expr,
}

impl CompiledFractal {
    fn compile(source: &str) -> Result<Self, FormulaError> {
        let mut config = FractalDslConfig::default();
        for raw_line in source.lines() {
            let line = raw_line
                .split('#')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_end_matches(';')
                .trim();
            if line.is_empty()
                || line == "{"
                || line == "}"
                || line.eq_ignore_ascii_case("fractal")
                || line.eq_ignore_ascii_case("fractal {")
            {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(FormulaError::InvalidFractalDsl(format!(
                    "Expected `key = value`, got `{line}`"
                )));
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "iterations" | "iter_count" | "max_iter" => {
                    config.iterations = value
                        .parse::<u32>()
                        .map_err(|_| {
                            FormulaError::InvalidFractalDsl(
                                "`iterations` must be an integer".to_owned(),
                            )
                        })?
                        .clamp(1, 512);
                }
                "zx" | "z_x" | "start_zx" => config.zx = value.to_owned(),
                "zy" | "z_y" | "start_zy" => config.zy = value.to_owned(),
                "cx" | "c_x" => config.cx = value.to_owned(),
                "cy" | "c_y" => config.cy = value.to_owned(),
                "next_zx" | "next_x" | "zx_next" => config.next_zx = value.to_owned(),
                "next_zy" | "next_y" | "zy_next" => config.next_zy = value.to_owned(),
                "escape" | "escape_value" => config.escape = value.to_owned(),
                "escape_radius" | "bailout" => config.escape_radius = value.to_owned(),
                "output" | "color" | "value" => config.output = value.to_owned(),
                _ => {
                    return Err(FormulaError::InvalidFractalDsl(format!(
                        "Unknown fractal DSL key `{key}`"
                    )));
                }
            }
        }

        Ok(Self {
            iterations: config.iterations,
            zx: compile_scalar_expr(&config.zx)?,
            zy: compile_scalar_expr(&config.zy)?,
            cx: compile_scalar_expr(&config.cx)?,
            cy: compile_scalar_expr(&config.cy)?,
            next_zx: compile_scalar_expr(&config.next_zx)?,
            next_zy: compile_scalar_expr(&config.next_zy)?,
            escape: compile_scalar_expr(&config.escape)?,
            escape_radius: compile_scalar_expr(&config.escape_radius)?,
            output: compile_scalar_expr(&config.output)?,
        })
    }

    fn sample(&self, vars: FormulaVars<'_>) -> f32 {
        let iterations = self.iterations.max(1) as f32;
        let mut zx = self.zx.eval(vars);
        let mut zy = self.zy.eval(vars);
        let cx = self.cx.eval(vars);
        let cy = self.cy.eval(vars);
        let mut escape_value = 0.0;

        for index in 0..self.iterations {
            let iter = index as f32;
            let local_vars = vars.with_fractal_vars(FractalVars {
                zx,
                zy,
                cx,
                cy,
                iter,
                iterations,
                escape_value,
            });
            escape_value = self.escape.eval(local_vars);
            let escape_radius = self.escape_radius.eval(local_vars).max(0.000_001);
            if escape_value > escape_radius {
                return clamp01(self.output.eval(vars.with_fractal_vars(FractalVars {
                    zx,
                    zy,
                    cx,
                    cy,
                    iter,
                    iterations,
                    escape_value,
                })));
            }

            let next_vars = vars.with_fractal_vars(FractalVars {
                zx,
                zy,
                cx,
                cy,
                iter,
                iterations,
                escape_value,
            });
            let next_zx = self.next_zx.eval(next_vars);
            let next_zy = self.next_zy.eval(next_vars);
            zx = next_zx;
            zy = next_zy;
        }

        let iter = iterations;
        clamp01(self.output.eval(vars.with_fractal_vars(FractalVars {
            zx,
            zy,
            cx,
            cy,
            iter,
            iterations,
            escape_value,
        })))
    }

    fn to_wgsl(&self, output_var: &str, indent: &str) -> String {
        let iterations = self.iterations.max(1);
        let zx = self.zx.to_wgsl();
        let zy = self.zy.to_wgsl();
        let cx = self.cx.to_wgsl();
        let cy = self.cy.to_wgsl();
        let escape = self.escape.to_wgsl();
        let escape_radius = self.escape_radius.to_wgsl();
        let output = self.output.to_wgsl();
        let next_zx = self.next_zx.to_wgsl();
        let next_zy = self.next_zy.to_wgsl();
        format!(
            "{indent}var fractal_zx = {zx};
{indent}var fractal_zy = {zy};
{indent}let fractal_cx = {cx};
{indent}let fractal_cy = {cy};
{indent}let fractal_iterations = {iterations}.0;
{indent}var fractal_iter = 0.0;
{indent}var fractal_escape_value = 0.0;
{indent}var fractal_output = 0.0;
{indent}var fractal_escaped = false;
{indent}for (var fractal_i = 0u; fractal_i < {iterations}u; fractal_i = fractal_i + 1u) {{
{indent}    fractal_iter = f32(fractal_i);
{indent}    fractal_escape_value = {escape};
{indent}    let fractal_escape_radius = max({escape_radius}, 0.000001);
{indent}    if fractal_escape_value > fractal_escape_radius {{
{indent}        fractal_output = clamp({output}, 0.0, 1.0);
{indent}        fractal_escaped = true;
{indent}        break;
{indent}    }}
{indent}    let fractal_next_zx = {next_zx};
{indent}    let fractal_next_zy = {next_zy};
{indent}    fractal_zx = fractal_next_zx;
{indent}    fractal_zy = fractal_next_zy;
{indent}}}
{indent}if !fractal_escaped {{
{indent}    fractal_iter = fractal_iterations;
{indent}    fractal_escape_value = {escape};
{indent}    fractal_output = clamp({output}, 0.0, 1.0);
{indent}}}
{indent}let {output_var} = fractal_output;
"
        )
    }
}

#[derive(Debug, Clone)]
struct FractalDslConfig {
    iterations: u32,
    zx: String,
    zy: String,
    cx: String,
    cy: String,
    next_zx: String,
    next_zy: String,
    escape: String,
    escape_radius: String,
    output: String,
}

impl Default for FractalDslConfig {
    fn default() -> Self {
        Self {
            iterations: 96,
            zx: "x".to_owned(),
            zy: "y".to_owned(),
            cx: "-0.72".to_owned(),
            cy: "0.27".to_owned(),
            next_zx: "zx * zx - zy * zy + cx".to_owned(),
            next_zy: "2 * zx * zy + cy".to_owned(),
            escape: "zx * zx + zy * zy".to_owned(),
            escape_radius: "4".to_owned(),
            output: "iter / iterations".to_owned(),
        }
    }
}

fn is_fractal_dsl(source: &str) -> bool {
    let trimmed = source.trim_start();
    trimmed.eq_ignore_ascii_case("fractal")
        || trimmed
            .lines()
            .next()
            .map(|line| line.trim().eq_ignore_ascii_case("fractal"))
            .unwrap_or(false)
        || trimmed
            .lines()
            .next()
            .map(|line| line.trim().eq_ignore_ascii_case("fractal {"))
            .unwrap_or(false)
}

fn eval_function(name: &str, args: &[Expr], vars: FormulaVars<'_>) -> f32 {
    let arg = |index: usize| args[index].eval(vars);
    match name {
        "sin" => arg(0).sin(),
        "cos" => arg(0).cos(),
        "tan" => arg(0).tan(),
        "abs" => arg(0).abs(),
        "sqrt" => arg(0).abs().sqrt(),
        "floor" => arg(0).floor(),
        "ceil" => arg(0).ceil(),
        "round" => arg(0).round(),
        "fract" => arg(0).fract(),
        "clamp" => arg(0).clamp(arg(1), arg(2)),
        "min" => arg(0).min(arg(1)),
        "max" => arg(0).max(arg(1)),
        "pow" => arg(0).abs().powf(arg(1)),
        "atan2" => arg(0).atan2(arg(1)),
        "mix" => arg(0) + (arg(1) - arg(0)) * arg(2),
        "smoothstep" => {
            let edge0 = arg(0);
            let edge1 = arg(1);
            let x = ((arg(2) - edge0) / (edge1 - edge0).max(0.000_001)).clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        }
        "noise" => value_noise(arg(0), arg(1), vars.params.seed),
        "fbm" => fbm_noise(arg(0), arg(1), vars.params.seed),
        "ridge" => 1.0 - fbm_noise(arg(0), arg(1), vars.params.seed).abs(),
        "cell" => cell_noise(arg(0), arg(1), vars.params.seed),
        "mandelbrot" => mandelbrot_escape(arg(0), arg(1)),
        "burning_ship" => burning_ship_escape(arg(0), arg(1)),
        "julia" => julia_escape(arg(0), arg(1), arg(2), arg(3)),
        _ => 0.0,
    }
}

fn wgsl_function_call(name: &str, args: &[Expr]) -> String {
    let arg = |index: usize| args[index].to_wgsl();
    match name {
        "sin" | "cos" | "tan" | "abs" | "sqrt" | "floor" | "ceil" | "round" | "fract" => {
            format!("{name}({})", arg(0))
        }
        "clamp" => format!("clamp({}, {}, {})", arg(0), arg(1), arg(2)),
        "min" => format!("min({}, {})", arg(0), arg(1)),
        "max" => format!("max({}, {})", arg(0), arg(1)),
        "pow" => format!("pow(abs({}), {})", arg(0), arg(1)),
        "atan2" => format!("atan2({}, {})", arg(0), arg(1)),
        "mix" => format!("mix({}, {}, {})", arg(0), arg(1), arg(2)),
        "smoothstep" => format!("smoothstep({}, {}, {})", arg(0), arg(1), arg(2)),
        "noise" => format!("value_noise({}, {}, params.seed)", arg(0), arg(1)),
        "fbm" => format!("fbm_noise({}, {}, params.seed)", arg(0), arg(1)),
        "ridge" => format!(
            "(1.0 - abs(fbm_noise({}, {}, params.seed)))",
            arg(0),
            arg(1)
        ),
        "cell" => format!("cell_noise({}, {}, params.seed)", arg(0), arg(1)),
        "mandelbrot" => format!("mandelbrot_formula({}, {})", arg(0), arg(1)),
        "burning_ship" => format!("burning_ship_formula({}, {})", arg(0), arg(1)),
        "julia" => format!(
            "julia_formula({}, {}, {}, {})",
            arg(0),
            arg(1),
            arg(2),
            arg(3)
        ),
        _ => "0.0".to_owned(),
    }
}

fn wgsl_variable(name: &str) -> String {
    match name {
        "x" => "x".to_owned(),
        "y" => "y".to_owned(),
        "origin_x" => "origin_x".to_owned(),
        "origin_y" => "origin_y".to_owned(),
        "r" => "sqrt(x * x + y * y)".to_owned(),
        "a" => "atan2(y, x)".to_owned(),
        "t" => "sin(params.angle)".to_owned(),
        "u" => "cos(params.angle)".to_owned(),
        "phase" => "(0.5 - 0.5 * cos(params.angle))".to_owned(),
        "pi" => "3.141592653589793".to_owned(),
        "tau" => "6.283185307179586".to_owned(),
        "seed" => "f32(params.seed)".to_owned(),
        "zoom" => "params.zoom".to_owned(),
        "symmetry" => "f32(params.symmetry)".to_owned(),
        "detail" => "params.detail".to_owned(),
        "distortion" => "params.distortion".to_owned(),
        "scale" => "formula_scale".to_owned(),
        "motion" => "formula_motion".to_owned(),
        "blend" => "formula_blend".to_owned(),
        "transition" => "formula_transition".to_owned(),
        "prev" => "formula_prev".to_owned(),
        "p1" => "formula_p1".to_owned(),
        "p2" => "formula_p2".to_owned(),
        "p3" => "formula_p3".to_owned(),
        "p4" => "formula_p4".to_owned(),
        "zx" => "fractal_zx".to_owned(),
        "zy" => "fractal_zy".to_owned(),
        "cx" => "fractal_cx".to_owned(),
        "cy" => "fractal_cy".to_owned(),
        "iter" => "fractal_iter".to_owned(),
        "iterations" => "fractal_iterations".to_owned(),
        "escape_value" => "fractal_escape_value".to_owned(),
        _ => "0.0".to_owned(),
    }
}

fn format_float(value: f32) -> String {
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

#[derive(Debug, Clone, Copy)]
enum UnaryOp {
    Neg,
}

#[derive(Debug, Clone, Copy)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f32),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
}

fn tokenize(source: &str) -> Result<Vec<Token>, FormulaError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        match ch {
            ' ' | '\t' | '\n' | '\r' => index += 1,
            '+' => push_token(&mut tokens, Token::Plus, &mut index),
            '-' => push_token(&mut tokens, Token::Minus, &mut index),
            '*' => push_token(&mut tokens, Token::Star, &mut index),
            '/' => push_token(&mut tokens, Token::Slash, &mut index),
            '%' => push_token(&mut tokens, Token::Percent, &mut index),
            '^' => push_token(&mut tokens, Token::Caret, &mut index),
            '(' => push_token(&mut tokens, Token::LParen, &mut index),
            ')' => push_token(&mut tokens, Token::RParen, &mut index),
            ',' => push_token(&mut tokens, Token::Comma, &mut index),
            '0'..='9' | '.' => {
                let start = index;
                index += 1;
                while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.')
                {
                    index += 1;
                }
                let number = source[start..index]
                    .parse::<f32>()
                    .map_err(|_| FormulaError::InvalidNumber)?;
                tokens.push(Token::Number(number));
            }
            _ if ch.is_ascii_alphabetic() || ch == '_' => {
                let start = index;
                index += 1;
                while index < chars.len()
                    && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
                {
                    index += 1;
                }
                tokens.push(Token::Ident(source[start..index].to_ascii_lowercase()));
            }
            _ => return Err(FormulaError::UnexpectedCharacter(ch)),
        }
    }
    if tokens.is_empty() {
        Err(FormulaError::EmptyExpression)
    } else {
        Ok(tokens)
    }
}

fn push_token(tokens: &mut Vec<Token>, token: Token, index: &mut usize) {
    tokens.push(token);
    *index += 1;
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn parse_expression(&mut self) -> Result<Expr, FormulaError> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<Expr, FormulaError> {
        let mut expr = self.parse_mul_div()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinaryOp::Add,
                Some(Token::Minus) => BinaryOp::Sub,
                _ => return Ok(expr),
            };
            self.index += 1;
            let right = self.parse_mul_div()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
    }

    fn parse_mul_div(&mut self) -> Result<Expr, FormulaError> {
        let mut expr = self.parse_power()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinaryOp::Mul,
                Some(Token::Slash) => BinaryOp::Div,
                Some(Token::Percent) => BinaryOp::Rem,
                _ => return Ok(expr),
            };
            self.index += 1;
            let right = self.parse_power()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
    }

    fn parse_power(&mut self) -> Result<Expr, FormulaError> {
        let expr = self.parse_unary()?;
        if matches!(self.peek(), Some(Token::Caret)) {
            self.index += 1;
            let right = self.parse_power()?;
            Ok(Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(expr),
                right: Box::new(right),
            })
        } else {
            Ok(expr)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, FormulaError> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.index += 1;
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(self.parse_unary()?),
            })
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, FormulaError> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expr::Number(value)),
            Some(Token::Ident(name)) => {
                if matches!(self.peek(), Some(Token::LParen)) {
                    self.index += 1;
                    let args = self.parse_args()?;
                    validate_function(&name, args.len())?;
                    Ok(Expr::Call { name, args })
                } else if is_known_variable(&name) {
                    Ok(Expr::Var(name))
                } else {
                    Err(FormulaError::UnknownVariable(name))
                }
            }
            Some(Token::LParen) => {
                let expr = self.parse_expression()?;
                if !matches!(self.next(), Some(Token::RParen)) {
                    return Err(FormulaError::MissingClosingParen);
                }
                Ok(expr)
            }
            _ => Err(FormulaError::UnexpectedToken),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, FormulaError> {
        if matches!(self.peek(), Some(Token::RParen)) {
            self.index += 1;
            return Ok(Vec::new());
        }
        let mut args = Vec::new();
        loop {
            args.push(self.parse_expression()?);
            match self.next() {
                Some(Token::Comma) => {}
                Some(Token::RParen) => return Ok(args),
                _ => return Err(FormulaError::MissingClosingParen),
            }
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn is_done(&self) -> bool {
        self.index >= self.tokens.len()
    }
}

fn is_known_variable(name: &str) -> bool {
    matches!(
        name,
        "x" | "y"
            | "origin_x"
            | "origin_y"
            | "r"
            | "a"
            | "t"
            | "u"
            | "phase"
            | "pi"
            | "tau"
            | "seed"
            | "zoom"
            | "symmetry"
            | "detail"
            | "distortion"
            | "scale"
            | "motion"
            | "blend"
            | "transition"
            | "prev"
            | "p1"
            | "p2"
            | "p3"
            | "p4"
            | "zx"
            | "zy"
            | "cx"
            | "cy"
            | "iter"
            | "iterations"
            | "escape_value"
    )
}

fn validate_function(name: &str, arity: usize) -> Result<(), FormulaError> {
    let valid = match name {
        "sin" | "cos" | "tan" | "abs" | "sqrt" | "floor" | "ceil" | "round" | "fract" => arity == 1,
        "min" | "max" | "pow" | "atan2" | "noise" | "fbm" | "ridge" | "cell" | "mandelbrot"
        | "burning_ship" => arity == 2,
        "clamp" | "mix" | "smoothstep" => arity == 3,
        "julia" => arity == 4,
        _ => return Err(FormulaError::UnknownFunction(name.to_owned())),
    };
    if valid {
        Ok(())
    } else {
        Err(FormulaError::WrongArity(name.to_owned()))
    }
}

fn layer_label(index: usize, layer: &FormulaLayer) -> String {
    if layer.name.trim().is_empty() {
        format!("Layer {}", index + 1)
    } else {
        layer.name.clone()
    }
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let xi = (x.floor() as i32).wrapping_mul(374_761_393);
    let yi = (y.floor() as i32).wrapping_mul(668_265_263);
    let mut n = (xi ^ yi ^ seed as i32).wrapping_mul(1_274_126_177);
    n ^= n >> 13;
    ((n & 0x7fff_ffff) as f32 / 0x7fff_ffff as f32) * 2.0 - 1.0
}

fn fbm_noise(x: f32, y: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for octave in 0..5 {
        sum += value_noise(x * freq, y * freq, seed.wrapping_add(octave * 101)) * amp;
        norm += amp;
        freq *= 2.03;
        amp *= 0.5;
    }
    if norm > 0.0 { sum / norm } else { 0.0 }
}

fn cell_noise(x: f32, y: f32, seed: u32) -> f32 {
    let base_x = x.floor() as i32;
    let base_y = y.floor() as i32;
    let mut nearest = f32::MAX;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let cell_x = base_x + ox;
            let cell_y = base_y + oy;
            let point_x = cell_x as f32 + hash01(cell_x, cell_y, seed);
            let point_y = cell_y as f32 + hash01(cell_x, cell_y, seed.wrapping_add(17));
            let dx = x - point_x;
            let dy = y - point_y;
            nearest = nearest.min((dx * dx + dy * dy).sqrt());
        }
    }
    clamp01(nearest)
}

fn hash01(x: i32, y: i32, seed: u32) -> f32 {
    let mut n = (x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263) ^ seed as i32)
        .wrapping_mul(1_274_126_177);
    n ^= n >> 13;
    (n & 0x7fff_ffff) as f32 / 0x7fff_ffff as f32
}

fn mandelbrot_escape(cx: f32, cy: f32) -> f32 {
    let mut zx = 0.0;
    let mut zy = 0.0;
    escape_value(cx, cy, &mut zx, &mut zy)
}

fn julia_escape(mut zx: f32, mut zy: f32, cx: f32, cy: f32) -> f32 {
    escape_value(cx, cy, &mut zx, &mut zy)
}

fn burning_ship_escape(cx: f32, cy: f32) -> f32 {
    let mut zx: f32 = 0.0;
    let mut zy: f32 = 0.0;
    const MAX_ITER: u32 = 96;
    for index in 0..MAX_ITER {
        let ax = zx.abs();
        let ay = zy.abs();
        zx = ax * ax - ay * ay + cx;
        zy = 2.0 * ax * ay + cy;
        let radius_sq = zx * zx + zy * zy;
        if radius_sq > 4.0 {
            let smooth = index as f32 + 1.0 - radius_sq.log2().max(0.000_001).log2();
            return clamp01(smooth / MAX_ITER as f32);
        }
    }
    0.0
}

fn escape_value(cx: f32, cy: f32, zx: &mut f32, zy: &mut f32) -> f32 {
    const MAX_ITER: u32 = 96;
    for index in 0..MAX_ITER {
        let next_x = *zx * *zx - *zy * *zy + cx;
        let next_y = 2.0 * *zx * *zy + cy;
        *zx = next_x;
        *zy = next_y;
        let radius_sq = *zx * *zx + *zy * *zy;
        if radius_sq > 4.0 {
            let smooth = index as f32 + 1.0 - radius_sq.log2().max(0.000_001).log2();
            return clamp01(smooth / MAX_ITER as f32);
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::renderer::RenderParams;

    #[test]
    fn formula_parser_evaluates_variables_and_functions() {
        let params = RenderParams::default();
        let formula = CompiledFormula::compile("sin(x) + max(y, 2)").expect("compile");
        let vars = FormulaVars::new(0.0, 1.0, LoopTime::from_frame(0, 24), &params);

        assert_eq!(formula.sample(vars), 2.0);
    }

    #[test]
    fn time_variables_are_loop_safe() {
        let params = RenderParams::default();
        let formula = CompiledFormula::compile("t + u + phase").expect("compile");
        let start = FormulaVars::new(0.0, 0.0, LoopTime::from_seconds(0.0, 2.0), &params);
        let end = FormulaVars::new(0.0, 0.0, LoopTime::from_seconds(2.0, 2.0), &params);

        assert!((formula.sample(start) - formula.sample(end)).abs() < 0.000_01);
    }

    #[test]
    fn unknown_variables_are_validation_errors() {
        assert_eq!(
            CompiledFormula::compile("sin(not_a_var)").unwrap_err(),
            FormulaError::UnknownVariable("not_a_var".to_owned())
        );
    }

    #[test]
    fn burning_ship_formula_is_available_for_custom_sources() {
        let params = RenderParams::default();
        let formula = CompiledFormula::compile("burning_ship(x, y)").expect("compile");
        let vars = FormulaVars::new(-0.5, -0.5, LoopTime::from_frame(0, 24), &params);
        let value = formula.sample(vars);

        assert!((0.0..=1.0).contains(&value));
    }

    #[test]
    fn fractal_dsl_evaluates_custom_iteration() {
        let params = RenderParams::default();
        let source = r#"
fractal
iterations = 80
zx = x
zy = y
cx = -0.72 + t * motion * 0.1
cy = 0.27 + u * motion * 0.1
next_zx = zx * zx - zy * zy + cx
next_zy = 2 * zx * zy + cy
escape = zx * zx + zy * zy
escape_radius = 4
output = iter / iterations
"#;
        let formula = CompiledFormula::compile(source).expect("compile fractal DSL");
        let vars = FormulaVars::new(0.2, 0.3, LoopTime::from_frame(4, 24), &params)
            .with_layer_params(1.0, 1.0, 1.0, 0.0);
        let value = formula.sample(vars);

        assert!((0.0..=1.0).contains(&value));
    }

    #[test]
    fn layered_sources_validate_each_enabled_layer() {
        let source = FormulaSource {
            layers: vec![
                FormulaLayer {
                    expression: "sin(x)".to_owned(),
                    ..FormulaLayer::default()
                },
                FormulaLayer {
                    expression: "wat(x)".to_owned(),
                    ..FormulaLayer::default()
                },
            ],
            ..FormulaSource::default()
        };

        let issues = source.validate("Pattern");

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("Unknown function"));
    }

    #[test]
    fn editable_expression_updates_rendered_layer_and_source_expression() {
        let mut source = FormulaSource {
            expression: "0.0".to_owned(),
            layers: vec![FormulaLayer {
                expression: "0.25".to_owned(),
                gain: 1.0,
                bias: 0.0,
                ..FormulaLayer::default()
            }],
            gain: 1.0,
            bias: 0.0,
            effect_blend_mode: None,
            controls: Vec::new(),
        };

        assert_eq!(source.editable_expression(), "0.25");
        source.set_editable_expression("0.75".to_owned());

        assert_eq!(source.expression, "0.75");
        assert_eq!(source.layers[0].expression, "0.75");
        let compiled = CompiledFormulaSource::compile(&source).expect("compile source");
        let params = RenderParams::default();
        let value = sample_formula_source(
            &source,
            &compiled,
            FormulaVars::new(0.0, 0.0, LoopTime::from_frame(0, 24), &params),
        );
        assert_eq!(value, 0.75);
    }
}
