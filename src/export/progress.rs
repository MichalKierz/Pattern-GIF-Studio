use std::path::PathBuf;

use crate::project::render_settings::RenderBackendStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTerminalState {
    Finished,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone)]
pub enum ExportProgress {
    Started { total_frames: u32 },
    BackendStatus { status: RenderBackendStatus },
    Rendering { frame: u32, total_frames: u32 },
    Encoding { frame: u32, total_frames: u32 },
    Finished { output_path: PathBuf },
    Cancelled,
    Failed { message: String },
}

impl ExportProgress {
    pub fn terminal_state(&self) -> Option<ExportTerminalState> {
        match self {
            ExportProgress::Finished { .. } => Some(ExportTerminalState::Finished),
            ExportProgress::Cancelled => Some(ExportTerminalState::Cancelled),
            ExportProgress::Failed { .. } => Some(ExportTerminalState::Failed),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal_state().is_some()
    }

    pub fn fraction(&self) -> Option<f32> {
        match *self {
            ExportProgress::Started { .. } => Some(0.0),
            ExportProgress::BackendStatus { .. } => None,
            ExportProgress::Rendering {
                frame,
                total_frames,
            } => {
                if total_frames == 0 {
                    Some(0.0)
                } else {
                    Some(frame as f32 / total_frames as f32)
                }
            }
            ExportProgress::Encoding {
                frame,
                total_frames,
            } => {
                if total_frames == 0 {
                    Some(0.0)
                } else {
                    Some(frame as f32 / total_frames as f32)
                }
            }
            ExportProgress::Finished { .. } => Some(1.0),
            ExportProgress::Cancelled => None,
            ExportProgress::Failed { .. } => None,
        }
    }

    pub fn label(&self) -> String {
        match self {
            ExportProgress::Started { total_frames } => {
                format!("Preparing {total_frames} frames")
            }
            ExportProgress::BackendStatus { status } => {
                format!("Renderer: {}", status.short_label())
            }
            ExportProgress::Rendering {
                frame,
                total_frames,
            } => format!("Rendering frame {frame}/{total_frames}"),
            ExportProgress::Encoding {
                frame,
                total_frames,
            } => format!("Encoding frame {frame}/{total_frames}"),
            ExportProgress::Finished { output_path } => {
                format!("Finished: {}", output_path.display())
            }
            ExportProgress::Cancelled => "Export cancelled".to_owned(),
            ExportProgress::Failed { message } => format!("Export failed: {message}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportProgress, ExportTerminalState};

    #[test]
    fn terminal_progress_states_are_distinct_and_keep_error_message() {
        let finished = ExportProgress::Finished {
            output_path: "out.gif".into(),
        };
        let cancelled = ExportProgress::Cancelled;
        let failed = ExportProgress::Failed {
            message: "disk full".to_owned(),
        };

        assert_eq!(
            finished.terminal_state(),
            Some(ExportTerminalState::Finished)
        );
        assert_eq!(
            cancelled.terminal_state(),
            Some(ExportTerminalState::Cancelled)
        );
        assert_eq!(failed.terminal_state(), Some(ExportTerminalState::Failed));
        assert!(finished.is_terminal());
        assert!(cancelled.is_terminal());
        assert!(failed.is_terminal());
        assert_eq!(finished.fraction(), Some(1.0));
        assert_eq!(cancelled.fraction(), None);
        assert_eq!(failed.fraction(), None);
        assert!(failed.label().contains("disk full"));
    }
}
