use crate::documents::types::{Document, FileType, Table};
use crate::error::{AshkorixError, Result};
use crate::traits::DocumentImporter;
use async_trait::async_trait;
use std::path::Path;

pub struct TextImporter {
    id: &'static str,
    exts: &'static [&'static str],
}

impl TextImporter {
    pub fn txt() -> Self {
        Self {
            id: "builtin.txt",
            exts: &["txt"],
        }
    }
    pub fn markdown() -> Self {
        Self {
            id: "builtin.markdown",
            exts: &["md", "markdown"],
        }
    }
}

#[async_trait]
impl DocumentImporter for TextImporter {
    fn id(&self) -> &str {
        self.id
    }
    fn name(&self) -> &str {
        self.id
    }
    fn supported_extensions(&self) -> &[&str] {
        self.exts
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| self.exts.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        let bytes = std::fs::read(path)?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        let file_type = FileType::from_extension(ext);
        Ok(partial_document(path, file_type, text, Vec::new()))
    }
}

pub struct HtmlImporter;

#[async_trait]
impl DocumentImporter for HtmlImporter {
    fn id(&self) -> &str {
        "builtin.html"
    }
    fn name(&self) -> &str {
        "HTML Importer"
    }
    fn supported_extensions(&self) -> &[&str] {
        &["html", "htm"]
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_lowercase().as_str(), "html" | "htm"))
            .unwrap_or(false)
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        let html = std::fs::read_to_string(path)?;
        let doc = scraper::Html::parse_document(&html);
        let title = doc
            .select(&scraper::Selector::parse("title").unwrap())
            .next()
            .map(|el| el.text().collect::<String>());
        let body_text: String = doc
            .select(&scraper::Selector::parse("body").unwrap())
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| doc.root_element().text().collect::<Vec<_>>().join(" "));
        let mut doc = partial_document(path, FileType::Html, body_text, Vec::new());
        doc.title = title;
        Ok(doc)
    }
}

pub struct CsvImporter;

#[async_trait]
impl DocumentImporter for CsvImporter {
    fn id(&self) -> &str {
        "builtin.csv"
    }
    fn name(&self) -> &str {
        "CSV Importer"
    }
    fn supported_extensions(&self) -> &[&str] {
        &["csv"]
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("csv")
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        let mut reader = csv::Reader::from_path(path)
            .map_err(|e| AshkorixError::Import(e.to_string()))?;
        let headers: Vec<String> = reader
            .headers()
            .map_err(|e| AshkorixError::Import(e.to_string()))?
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut rows = Vec::new();
        let mut text_lines = Vec::new();
        for result in reader.records() {
            let record = result.map_err(|e| AshkorixError::Import(e.to_string()))?;
            let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            text_lines.push(row.join(", "));
            rows.push(row);
        }
        let table = Table {
            name: path.file_stem().and_then(|s| s.to_str()).map(String::from),
            headers,
            rows,
        };
        Ok(partial_document(
            path,
            FileType::Csv,
            text_lines.join("\n"),
            vec![table],
        ))
    }
}

pub struct JsonImporter;

#[async_trait]
impl DocumentImporter for JsonImporter {
    fn id(&self) -> &str {
        "builtin.json"
    }
    fn name(&self) -> &str {
        "JSON Importer"
    }
    fn supported_extensions(&self) -> &[&str] {
        &["json"]
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("json")
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        let content = std::fs::read_to_string(path)?;
        let value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| AshkorixError::Import(e.to_string()))?;
        let text = serde_json::to_string_pretty(&value)
            .map_err(|e| AshkorixError::Import(e.to_string()))?;
        Ok(partial_document(path, FileType::Json, text, Vec::new()))
    }
}

pub struct XmlImporter;

#[async_trait]
impl DocumentImporter for XmlImporter {
    fn id(&self) -> &str {
        "builtin.xml"
    }
    fn name(&self) -> &str {
        "XML Importer"
    }
    fn supported_extensions(&self) -> &[&str] {
        &["xml"]
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("xml")
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        let content = std::fs::read_to_string(path)?;
        let text = extract_xml_text(&content);
        Ok(partial_document(path, FileType::Xml, text, Vec::new()))
    }
}

fn extract_xml_text(xml: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub struct PdfImporter;

#[async_trait]
impl DocumentImporter for PdfImporter {
    fn id(&self) -> &str {
        "builtin.pdf"
    }
    fn name(&self) -> &str {
        "PDF Importer"
    }
    fn supported_extensions(&self) -> &[&str] {
        &["pdf"]
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("pdf")
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        let bytes = std::fs::read(path)?;
        let text = pdf_extract::extract_text_from_mem(&bytes)
            .map_err(|e| AshkorixError::Import(e.to_string()))?;
        Ok(partial_document(path, FileType::Pdf, text, Vec::new()))
    }
}

pub struct DocxImporter;

#[async_trait]
impl DocumentImporter for DocxImporter {
    fn id(&self) -> &str {
        "builtin.docx"
    }
    fn name(&self) -> &str {
        "DOCX Importer"
    }
    fn supported_extensions(&self) -> &[&str] {
        &["docx"]
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("docx")
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        let bytes = std::fs::read(path)?;
        let docx = docx_rs::read_docx(&bytes)
            .map_err(|e| AshkorixError::Import(e.to_string()))?;
        let mut text = String::new();
        for child in docx.document.children {
            if let docx_rs::DocumentChild::Paragraph(p) = child {
                for run in p.children {
                    if let docx_rs::ParagraphChild::Run(r) = run {
                        for rc in r.children {
                            if let docx_rs::RunChild::Text(t) = rc {
                                text.push_str(&t.text);
                            }
                        }
                    }
                }
                text.push('\n');
            }
        }
        Ok(partial_document(path, FileType::Docx, text, Vec::new()))
    }
}

pub struct XlsxImporter;

#[async_trait]
impl DocumentImporter for XlsxImporter {
    fn id(&self) -> &str {
        "builtin.xlsx"
    }
    fn name(&self) -> &str {
        "XLSX Importer"
    }
    fn supported_extensions(&self) -> &[&str] {
        &["xlsx", "xls"]
    }
    fn can_handle(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_lowercase().as_str(), "xlsx" | "xls"))
            .unwrap_or(false)
    }
    async fn import(&self, path: &Path) -> Result<Document> {
        use calamine::{open_workbook_auto, Reader};
        let mut workbook = open_workbook_auto(path)
            .map_err(|e| AshkorixError::Import(e.to_string()))?;
        let mut tables = Vec::new();
        let mut text = String::new();
        for sheet_name in workbook.sheet_names().to_vec() {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let mut headers = Vec::new();
                let mut rows = Vec::new();
                for (i, row) in range.rows().enumerate() {
                    let cells: Vec<String> = row.iter().map(|c| c.to_string()).collect();
                    if i == 0 {
                        headers = cells.clone();
                    } else {
                        rows.push(cells.clone());
                    }
                    text.push_str(&format!("{sheet_name}: {}\n", cells.join(", ")));
                }
                tables.push(Table {
                    name: Some(sheet_name),
                    headers,
                    rows,
                });
            }
        }
        Ok(partial_document(path, FileType::Xlsx, text, tables))
    }
}

fn partial_document(
    path: &Path,
    file_type: FileType,
    extracted_text: String,
    extracted_tables: Vec<Table>,
) -> Document {
    use crate::documents::types::ImportStatus;
    use crate::types::{hash_bytes, short_id_from_hash, DocumentId};
    use chrono::Utc;

    let bytes = std::fs::read(path).unwrap_or_default();
    let content_hash = hash_bytes(&bytes);
    let id = DocumentId(short_id_from_hash(&content_hash));

    Document {
        id,
        content_hash,
        original_filename: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        file_path: path.to_path_buf(),
        file_type,
        collection_id: String::new(),
        imported_at: Utc::now(),
        title: None,
        author: None,
        created_date: None,
        modified_date: None,
        extracted_text,
        extracted_tables,
        metadata: serde_json::json!({}),
        chunk_count: 0,
        import_status: ImportStatus::Imported,
    }
}
