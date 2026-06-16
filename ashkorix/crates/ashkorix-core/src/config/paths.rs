use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataDirSource {
    Env,
    ExeRelative,
    CurrentDirFallback,
}

impl DataDirSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::ExeRelative => "exe-relative",
            Self::CurrentDirFallback => "cwd-fallback",
        }
    }
}

/// Resolved data root and how it was chosen.
pub fn data_dir_source() -> (PathBuf, DataDirSource) {
    if let Ok(v) = std::env::var("ASHKORIX_DATA_DIR") {
        return (PathBuf::from(v), DataDirSource::Env);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return (parent.join("Data"), DataDirSource::ExeRelative);
        }
    }
    let fallback = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Data");
    (fallback, DataDirSource::CurrentDirFallback)
}

pub fn resolve_data_dir() -> PathBuf {
    data_dir_source().0
}

fn executable_relative_data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("Data")))
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("Data")
        })
}

pub fn default_data_dir() -> PathBuf {
    if std::env::var("ASHKORIX_DATA_DIR").is_ok() {
        resolve_data_dir()
    } else {
        executable_relative_data_dir()
    }
}
