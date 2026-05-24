use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::animation::loop_time;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_seconds: f32,
    pub lossy_quality: u8,
    pub fast: bool,
    pub output_path: PathBuf,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            fps: 24,
            duration_seconds: 4.0,
            lossy_quality: 100,
            fast: false,
            output_path: PathBuf::from("exports/pattern-loop.gif"),
        }
    }
}

impl ExportSettings {
    pub const MAX_DIMENSION: u32 = 1000;
    pub const MAX_GIF_FPS: u32 = 50;

    pub fn sanitize(&mut self) {
        self.width = self.width.clamp(64, Self::MAX_DIMENSION);
        self.height = self.height.clamp(64, Self::MAX_DIMENSION);
        self.fps = self.fps.clamp(1, Self::MAX_GIF_FPS);
        self.duration_seconds = self.duration_seconds.clamp(0.25, 30.0);
        self.lossy_quality = self.lossy_quality.clamp(1, 100);
        if self.output_path.extension().and_then(|ext| ext.to_str()) != Some("gif") {
            self.output_path.set_extension("gif");
        }
    }

    pub fn total_frames(&self) -> u32 {
        loop_time::total_frames(self.fps, self.duration_seconds)
    }

    pub fn frame_bytes(&self) -> u64 {
        self.width as u64 * self.height as u64 * 4
    }

    pub fn total_raw_bytes(&self) -> u64 {
        self.frame_bytes() * self.total_frames() as u64
    }
}
