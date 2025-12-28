//! Error types for Soyuz

use thiserror::Error;

/// Result type alias using Soyuz's Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in Soyuz operations
#[derive(Error, Debug)]
pub enum Error {
    /// Mesh generation failed
    #[error("Mesh generation failed: {0}")]
    MeshGeneration(String),

    /// Export failed
    #[error("Export failed: {0}")]
    Export(String),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Image encoding/decoding error
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    /// GLTF error
    #[error("GLTF error: {0}")]
    Gltf(String),
}
