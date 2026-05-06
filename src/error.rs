use thiserror::Error;

#[derive(Debug, Error)]
pub enum KubeviewError {
    #[error("{0}")]
    Kubernetes(String),
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("configuration error: {0}")]
    Config(String),
}

impl KubeviewError {
    pub(crate) fn kubernetes_context(context: impl AsRef<str>, error: kube::Error) -> Self {
        Self::Kubernetes(format!("{}: {error}", context.as_ref()))
    }
}

impl From<kube::Error> for KubeviewError {
    fn from(error: kube::Error) -> Self {
        Self::Kubernetes(error.to_string())
    }
}

impl From<kube::config::KubeconfigError> for KubeviewError {
    fn from(error: kube::config::KubeconfigError) -> Self {
        Self::Config(error.to_string())
    }
}

impl From<kube::config::InferConfigError> for KubeviewError {
    fn from(error: kube::config::InferConfigError) -> Self {
        Self::Config(error.to_string())
    }
}
