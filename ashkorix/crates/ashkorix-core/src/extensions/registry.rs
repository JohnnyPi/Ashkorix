use crate::documents::registry::ImporterRegistry;
use crate::error::Result;
use crate::extensions::manifest::{default_builtin_manifest, parse_manifest};
use crate::extensions::types::ExtensionInfo;
use crate::traits::ExtensionHost;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct AshkorixExtensionHost {
    extensions_dir: PathBuf,
    importers: ImporterRegistry,
    manifests: Vec<ExtensionInfo>,
    disabled: HashMap<String, bool>,
    audit_log: Vec<String>,
}

impl AshkorixExtensionHost {
    pub fn new(extensions_dir: PathBuf) -> Self {
        Self {
            extensions_dir,
            importers: ImporterRegistry::builtin(),
            manifests: Vec::new(),
            disabled: HashMap::new(),
            audit_log: Vec::new(),
        }
    }

    pub fn audit_log(&self) -> &[String] {
        &self.audit_log
    }

    fn register_builtin_extensions(&mut self) {
        for info in self.importers.list() {
            let manifest = default_builtin_manifest(
                &info.id,
                &info.name,
                &info.extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            );
            self.manifests.push(ExtensionInfo {
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                kind: manifest.kind,
                runtime: manifest.runtime,
                supported_extensions: manifest.supported_extensions,
                permissions: manifest.permissions,
                enabled: true,
                builtin: true,
            });
        }
    }
}

impl ExtensionHost for AshkorixExtensionHost {
    fn discover(&mut self) -> Result<()> {
        self.manifests.clear();
        self.register_builtin_extensions();

        let importers_dir = self.extensions_dir.join("importers");
        if importers_dir.exists() {
            for entry in std::fs::read_dir(&importers_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("extension.toml");
                    if manifest_path.exists() {
                        let manifest = parse_manifest(&manifest_path)?;
                        self.audit_log.push(format!(
                            "discovered extension: {} v{}",
                            manifest.id, manifest.version
                        ));
                        self.manifests.push(ExtensionInfo {
                            enabled: true,
                            builtin: false,
                            id: manifest.id,
                            name: manifest.name,
                            version: manifest.version,
                            kind: manifest.kind,
                            runtime: manifest.runtime,
                            supported_extensions: manifest.supported_extensions,
                            permissions: manifest.permissions,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn list_extensions(&self) -> Vec<ExtensionInfo> {
        self.manifests
            .iter()
            .map(|e| ExtensionInfo {
                enabled: !self.disabled.get(&e.id).copied().unwrap_or(false),
                ..e.clone()
            })
            .collect()
    }

    fn is_importer_enabled(&self, importer_id: &str) -> bool {
        !self.disabled.get(importer_id).copied().unwrap_or(false)
    }

    fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        if !enabled {
            self.disabled.insert(id.to_string(), true);
            self.audit_log
                .push(format!("disabled extension: {id}"));
        } else {
            self.disabled.remove(id);
            self.audit_log
                .push(format!("enabled extension: {id}"));
        }
        Ok(())
    }
}

impl AshkorixExtensionHost {
    pub fn importer_registry(&self) -> &ImporterRegistry {
        &self.importers
    }

    pub fn validate_extension_dir(&mut self, path: &Path) -> Result<()> {
        let manifest_path = path.join("extension.toml");
        parse_manifest(&manifest_path)?;
        self.audit_log
            .push(format!("validated extension at {}", path.display()));
        Ok(())
    }
}
