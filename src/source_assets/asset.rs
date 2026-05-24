use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::render::{
    color::{CustomGradient, PaletteKind},
    formula::FormulaSource,
    renderer::RenderParams,
};

pub const ASSET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    PatternSource,
    EffectSource,
    ColorSet,
    WorkflowPreset,
}

impl AssetType {
    pub fn label(self) -> &'static str {
        match self {
            AssetType::PatternSource => "pattern_source",
            AssetType::EffectSource => "effect_source",
            AssetType::ColorSet => "color_set",
            AssetType::WorkflowPreset => "workflow_preset",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaSourceAsset {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_type: Option<AssetType>,
    #[serde(default)]
    pub schema_version: u32,
    pub name: String,
    pub source: FormulaSource,
}

impl FormulaSourceAsset {
    pub fn new(name: impl Into<String>, source: FormulaSource) -> Self {
        Self::new_pattern(name, source)
    }

    pub fn new_pattern(name: impl Into<String>, source: FormulaSource) -> Self {
        let mut source = source;
        source.effect_blend_mode = None;
        Self::new_with_type(name, source, AssetType::PatternSource)
    }

    pub fn new_effect(name: impl Into<String>, source: FormulaSource) -> Self {
        Self::new_with_type(name, source, AssetType::EffectSource)
    }

    pub fn new_with_type(
        name: impl Into<String>,
        source: FormulaSource,
        asset_type: AssetType,
    ) -> Self {
        Self {
            asset_type: Some(asset_type),
            schema_version: ASSET_SCHEMA_VERSION,
            name: name.into(),
            source,
        }
    }

    pub fn validate_for_type(&self, expected: AssetType) -> Result<()> {
        validate_asset_metadata(self.asset_type, self.schema_version, expected)?;
        if self.asset_type.is_none() {
            validate_legacy_formula_source(&self.source, expected)?;
        } else {
            validate_typed_formula_source(&self.source, expected)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomColorSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_type: Option<AssetType>,
    #[serde(default)]
    pub schema_version: u32,
    pub name: String,
    pub palette: PaletteKind,
    pub custom_gradient: CustomGradient,
    pub color_speed: f32,
    pub color_phase: f32,
    pub brightness: f32,
    pub contrast: f32,
}

impl CustomColorSet {
    pub fn from_render_params(name: impl Into<String>, params: &RenderParams) -> Self {
        let custom_gradient =
            CustomGradient::active_for_palette(params.palette, &params.custom_gradient);
        Self {
            asset_type: Some(AssetType::ColorSet),
            schema_version: ASSET_SCHEMA_VERSION,
            name: name.into(),
            palette: params.palette,
            custom_gradient,
            color_speed: params.color_speed,
            color_phase: params.color_phase,
            brightness: params.brightness,
            contrast: params.contrast,
        }
    }

    pub fn validate_asset_type(&self) -> Result<()> {
        validate_asset_metadata(self.asset_type, self.schema_version, AssetType::ColorSet)
    }

    pub fn apply_to_render_params(&self, params: &mut RenderParams) {
        params.palette = self.palette;
        params.custom_gradient =
            CustomGradient::active_for_palette(self.palette, &self.custom_gradient);
        params.color_speed = self.color_speed;
        params.color_phase = self.color_phase;
        params.brightness = self.brightness;
        params.contrast = self.contrast;
    }
}

pub(crate) fn validate_asset_metadata(
    actual: Option<AssetType>,
    schema_version: u32,
    expected: AssetType,
) -> Result<()> {
    let Some(actual) = actual else {
        return Ok(());
    };
    if actual != expected {
        bail!(
            "wrong asset type: expected {}, got {}",
            expected.label(),
            actual.label()
        );
    }
    if schema_version != ASSET_SCHEMA_VERSION {
        bail!(
            "unsupported asset schema version: expected {}, got {}",
            ASSET_SCHEMA_VERSION,
            schema_version
        );
    }
    Ok(())
}

pub(crate) fn validate_required_asset_metadata(
    actual: Option<AssetType>,
    schema_version: u32,
    expected: AssetType,
) -> Result<()> {
    if actual.is_none() {
        bail!("missing asset_type: expected {}", expected.label());
    }
    validate_asset_metadata(actual, schema_version, expected)
}

pub fn validate_legacy_formula_source(source: &FormulaSource, expected: AssetType) -> Result<()> {
    match expected {
        AssetType::PatternSource if source.effect_blend_mode.is_none() => Ok(()),
        AssetType::PatternSource => bail!(
            "legacy source asset has effect blend mode and cannot be loaded as pattern source without asset_type"
        ),
        AssetType::EffectSource if source.effect_blend_mode.is_some() => Ok(()),
        AssetType::EffectSource => bail!(
            "legacy source asset has no asset_type and no effect_blend_mode; cannot safely load as effect source"
        ),
        _ => bail!(
            "formula source validation requires pattern_source or effect_source, got {}",
            expected.label()
        ),
    }
}

fn validate_typed_formula_source(source: &FormulaSource, expected: AssetType) -> Result<()> {
    match expected {
        AssetType::PatternSource if source.effect_blend_mode.is_none() => Ok(()),
        AssetType::PatternSource => {
            bail!("pattern_source asset cannot carry effect_blend_mode")
        }
        AssetType::EffectSource if source.effect_blend_mode.is_some() => Ok(()),
        AssetType::EffectSource => bail!("effect_source asset must declare effect_blend_mode"),
        _ => bail!(
            "formula source validation requires pattern_source or effect_source, got {}",
            expected.label()
        ),
    }
}
