use crate::utils::math::TAU;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopTime {
    pub phase: f32,
    pub angle: f32,
}

impl LoopTime {
    pub fn from_frame(frame: u32, total_frames: u32) -> Self {
        let phase = loop_phase(frame, total_frames);
        Self {
            phase,
            angle: loop_angle(phase),
        }
    }

    pub fn from_seconds(elapsed_seconds: f64, duration_seconds: f32) -> Self {
        let duration = duration_seconds.max(0.1) as f64;
        let phase = ((elapsed_seconds / duration) % 1.0) as f32;
        Self {
            phase,
            angle: loop_angle(phase),
        }
    }
}

pub fn loop_phase(frame: u32, total_frames: u32) -> f32 {
    if total_frames == 0 {
        return 0.0;
    }
    (frame % total_frames) as f32 / total_frames as f32
}

pub fn loop_angle(phase: f32) -> f32 {
    phase * TAU
}

pub fn total_frames(fps: u32, duration_seconds: f32) -> u32 {
    ((fps.max(1) as f32) * duration_seconds.max(0.1)).round() as u32
}
