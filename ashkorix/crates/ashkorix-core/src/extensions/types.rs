use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionPermissions {
    pub read_input_file: bool,
    pub write_temp_files: bool,
    pub network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub runtime: String,
    pub supported_extensions: Vec<String>,
    pub supported_mime_types: Vec<String>,
    pub permissions: ExtensionPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub runtime: String,
    pub supported_extensions: Vec<String>,
    pub permissions: ExtensionPermissions,
    pub enabled: bool,
    pub builtin: bool,
}
