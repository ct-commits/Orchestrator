//! Load a `roadmap.yaml`, validate it against `roadmap.schema.json`, and
//! return the typed [`Roadmap`].
//!
//! Order matters: we validate the raw document against the JSON Schema
//! *before* deserializing into typed structs. The schema is the wire
//! contract and gives precise, path-anchored error messages; the structs
//! then give us a checked, ergonomic value to work with.
//!
//! The schema is embedded in the binary via `include_str!`, so validation
//! can never drift from the shipped code or depend on a file at runtime.

use std::path::Path;
use std::sync::OnceLock;

use crate::model::Roadmap;

/// The canonical schema, compiled into the binary.
pub const SCHEMA_SOURCE: &str = include_str!("../roadmap.schema.json");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read roadmap file {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("roadmap is not valid YAML: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("embedded schema is not valid JSON: {0}")]
    SchemaJson(serde_json::Error),

    #[error("embedded schema failed to compile: {0}")]
    SchemaCompile(String),

    #[error("roadmap does not match the schema:\n{}", .0.join("\n"))]
    Invalid(Vec<String>),

    #[error("roadmap passed the schema but could not be mapped to the model: {0}")]
    Deserialize(serde_json::Error),
}

/// Compile the embedded schema once and reuse it.
fn validator() -> Result<&'static jsonschema::Validator, Error> {
    static VALIDATOR: OnceLock<Result<jsonschema::Validator, String>> = OnceLock::new();
    VALIDATOR
        .get_or_init(|| {
            let schema: serde_json::Value =
                serde_json::from_str(SCHEMA_SOURCE).map_err(|e| e.to_string())?;
            jsonschema::validator_for(&schema).map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| Error::SchemaCompile(e.clone()))
}

/// Parse and validate a roadmap from YAML text.
pub fn parse_str(yaml: &str) -> Result<Roadmap, Error> {
    // YAML -> generic JSON value, so the JSON Schema can check it.
    let instance: serde_json::Value = serde_yaml_ng::from_str(yaml)?;

    let validator = validator()?;
    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|e| format!("  at {}: {}", e.instance_path, e))
        .collect();
    if !errors.is_empty() {
        return Err(Error::Invalid(errors));
    }

    // Validated — deserialize into the typed model.
    let roadmap: Roadmap = serde_json::from_value(instance).map_err(Error::Deserialize)?;
    Ok(roadmap)
}

/// Parse and validate a roadmap from a file path.
pub fn load(path: impl AsRef<Path>) -> Result<Roadmap, Error> {
    let path = path.as_ref();
    let yaml = std::fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })?;
    parse_str(&yaml)
}
