use serde::{Deserialize, Serialize};

use anyhow::Result;

use crate::{
    render::renderer::RenderParams,
    source_assets::asset::{
        ASSET_SCHEMA_VERSION, AssetType, validate_asset_metadata, validate_required_asset_metadata,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_type: Option<AssetType>,
    #[serde(default)]
    pub schema_version: u32,
    pub name: String,
    pub render_params: RenderParams,
}

impl Preset {
    pub fn new(name: impl Into<String>, render_params: RenderParams) -> Self {
        Self {
            asset_type: Some(AssetType::WorkflowPreset),
            schema_version: ASSET_SCHEMA_VERSION,
            name: name.into(),
            render_params,
        }
    }

    pub fn validate_asset_type(&self) -> Result<()> {
        validate_asset_metadata(
            self.asset_type,
            self.schema_version,
            AssetType::WorkflowPreset,
        )
    }

    pub fn validate_bundled_asset_type(&self) -> Result<()> {
        validate_required_asset_metadata(
            self.asset_type,
            self.schema_version,
            AssetType::WorkflowPreset,
        )
    }
}
