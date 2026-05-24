#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    Gpu,
}

impl RenderBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gpu => "GPU",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderBackendStatus {
    pub used: RenderBackend,
    pub error: Option<String>,
}

impl RenderBackendStatus {
    pub fn gpu() -> Self {
        Self {
            used: RenderBackend::Gpu,
            error: None,
        }
    }

    pub fn gpu_error(reason: impl Into<String>) -> Self {
        Self {
            used: RenderBackend::Gpu,
            error: Some(reason.into()),
        }
    }

    pub fn short_label(&self) -> String {
        match &self.error {
            Some(reason) => format!("{} error: {reason}", self.used.label()),
            None => self.used.label().to_owned(),
        }
    }
}
