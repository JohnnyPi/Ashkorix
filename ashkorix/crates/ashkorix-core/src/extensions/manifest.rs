use crate::error::{AshkorixError, Result};
use crate::extensions::types::{ExtensionManifest, ExtensionPermissions};
use std::path::Path;

pub fn parse_manifest(path: &Path) -> Result<ExtensionManifest> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AshkorixError::Extension(e.to_string()))?;
    validate_no_network_in_raw(&content)?;
    let manifest: ExtensionManifest = toml::from_str(&content)
        .map_err(|e| AshkorixError::Extension(e.to_string()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &ExtensionManifest) -> Result<()> {
    if manifest.id.is_empty() {
        return Err(AshkorixError::Extension("missing id".into()));
    }
    if manifest.kind != "importer" {
        return Err(AshkorixError::Extension(format!(
            "unsupported kind: {}",
            manifest.kind
        )));
    }
    if manifest.permissions.network {
        return Err(AshkorixError::Extension(
            "network permission not allowed in local-only mode".into(),
        ));
    }
    Ok(())
}

fn validate_no_network_in_raw(content: &str) -> Result<()> {
    if content.contains("network = true") {
        return Err(AshkorixError::Extension(
            "network permission not allowed".into(),
        ));
    }
    Ok(())
}

pub fn default_builtin_manifest(id: &str, name: &str, extensions: &[&str]) -> ExtensionManifest {
    ExtensionManifest {
        id: id.to_string(),
        name: name.to_string(),
        version: "0.1.0".into(),
        kind: "importer".into(),
        runtime: "builtin".into(),
        supported_extensions: extensions.iter().map(|s| s.to_string()).collect(),
        supported_mime_types: vec![],
        permissions: ExtensionPermissions {
            read_input_file: true,
            write_temp_files: false,
            network: false,
        },
    }
}
