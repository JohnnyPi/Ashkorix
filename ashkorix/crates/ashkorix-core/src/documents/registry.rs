use crate::documents::importers::{
    CsvImporter, DocxImporter, HtmlImporter, JsonImporter, PdfImporter, TextImporter, XmlImporter,
    XlsxImporter,
};
use crate::documents::types::{Document, ImportResult, ImportStatus};
use crate::error::Result;
use crate::traits::DocumentImporter;
use std::path::Path;
use std::sync::Arc;

pub struct ImporterRegistry {
    importers: Vec<Arc<dyn DocumentImporter>>,
}

impl ImporterRegistry {
    pub fn builtin() -> Self {
        let importers: Vec<Arc<dyn DocumentImporter>> = vec![
            Arc::new(TextImporter::txt()),
            Arc::new(TextImporter::markdown()),
            Arc::new(HtmlImporter),
            Arc::new(CsvImporter),
            Arc::new(JsonImporter),
            Arc::new(XmlImporter),
            Arc::new(PdfImporter),
            Arc::new(DocxImporter),
            Arc::new(XlsxImporter),
        ];
        Self { importers }
    }

    pub fn list(&self) -> Vec<ImporterInfo> {
        self.importers
            .iter()
            .map(|i| ImporterInfo {
                id: i.id().to_string(),
                name: i.name().to_string(),
                extensions: i.supported_extensions().iter().map(|s| s.to_string()).collect(),
            })
            .collect()
    }

    pub fn find(&self, path: &Path) -> Option<Arc<dyn DocumentImporter>> {
        self.importers
            .iter()
            .find(|i| i.can_handle(path))
            .cloned()
    }

    pub async fn import_file(&self, path: &Path) -> Result<Document> {
        let importer = self
            .find(path)
            .ok_or_else(|| crate::error::AshkorixError::Import(format!(
                "no importer for {}",
                path.display()
            )))?;
        importer.import(path).await
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImporterInfo {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
}

pub fn dedup_result(existing: Option<Document>, imported: Document) -> ImportResult {
    if let Some(doc) = existing {
        ImportResult {
            document: Some(doc),
            status: ImportStatus::Duplicate,
            message: "duplicate content hash".into(),
        }
    } else {
        ImportResult {
            document: Some(imported),
            status: ImportStatus::Imported,
            message: "imported successfully".into(),
        }
    }
}
