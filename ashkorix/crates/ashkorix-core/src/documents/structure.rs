use crate::documents::graph_types::Section;
use crate::documents::types::Document;
use crate::types::{hash_text, short_id_from_hash};

/// Parsed section node from document text.
#[derive(Debug, Clone)]
pub struct SectionNode {
    pub title: String,
    pub level: u32,
    pub start_offset: usize,
    pub end_offset: usize,
    pub page_start: Option<u32>,
    pub page_end: Option<u32>,
    pub body: String,
}

/// Build a flat list of sections from document text with heading hierarchy.
pub fn build_section_tree(document: &Document) -> Vec<SectionNode> {
    let text = &document.extracted_text;
    let pages = split_pages(text, document);

    if pages.len() > 1 {
        return build_from_pages(document, &pages);
    }

    match document.file_type {
        crate::documents::types::FileType::Markdown => build_from_markdown_headings(document, text),
        crate::documents::types::FileType::Html => {
            build_from_markdown_headings(document, text)
        }
        _ => build_from_markdown_headings(document, text),
    }
}

fn split_pages(text: &str, document: &Document) -> Vec<(Option<u32>, String)> {
    if text.contains('\u{000C}') {
        return text
            .split('\u{000C}')
            .enumerate()
            .map(|(i, p)| (Some((i + 1) as u32), p.to_string()))
            .collect();
    }
    if matches!(
        document.file_type,
        crate::documents::types::FileType::Pdf
    ) && text.len() > 4000
    {
        let approx_page_chars = 3000;
        let mut pages = Vec::new();
        let mut page_num = 1u32;
        let mut start = 0;
        while start < text.len() {
            let end = (start + approx_page_chars).min(text.len());
            pages.push((Some(page_num), text[start..end].to_string()));
            page_num += 1;
            start = end;
        }
        return pages;
    }
    vec![(None, text.to_string())]
}

fn build_from_pages(document: &Document, pages: &[(Option<u32>, String)]) -> Vec<SectionNode> {
    let mut nodes = Vec::new();
    let mut global_offset = 0usize;
    for (page_num, page_text) in pages {
        let page_sections = build_from_markdown_headings(document, page_text);
        for mut s in page_sections {
            s.start_offset += global_offset;
            s.end_offset += global_offset;
            s.page_start = *page_num;
            s.page_end = *page_num;
            nodes.push(s);
        }
        global_offset += page_text.len() + 1;
    }
    if nodes.is_empty() {
        nodes.push(SectionNode {
            title: document.title.clone().unwrap_or_else(|| "Document".into()),
            level: 0,
            start_offset: 0,
            end_offset: document.extracted_text.len(),
            page_start: pages.first().and_then(|p| p.0),
            page_end: pages.last().and_then(|p| p.0),
            body: document.extracted_text.clone(),
        });
    }
    nodes
}

fn build_from_markdown_headings(document: &Document, text: &str) -> Vec<SectionNode> {
    let re = regex::Regex::new(r"(?m)^(#{1,6})\s+(.+)$").unwrap();
    let mut headings: Vec<(usize, usize, u32, String)> = Vec::new();
    for cap in re.captures_iter(text) {
        let m = cap.get(0).unwrap();
        let level = cap.get(1).unwrap().as_str().len() as u32;
        let title = cap.get(2).unwrap().as_str().trim().to_string();
        headings.push((m.start(), m.end(), level, title));
    }

    if headings.is_empty() {
        return vec![SectionNode {
            title: document
                .title
                .clone()
                .unwrap_or_else(|| document.original_filename.clone()),
            level: 0,
            start_offset: 0,
            end_offset: text.len(),
            page_start: None,
            page_end: None,
            body: text.to_string(),
        }];
    }

    let mut nodes = Vec::new();
    for (i, (start, _heading_end, level, title)) in headings.iter().enumerate() {
        let end = headings
            .get(i + 1)
            .map(|(s, _, _, _)| *s)
            .unwrap_or(text.len());
        let body = text[*start..end].to_string();
        nodes.push(SectionNode {
            title: title.clone(),
            level: *level,
            start_offset: *start,
            end_offset: end,
            page_start: None,
            page_end: None,
            body,
        });
    }
    nodes
}

pub fn heading_path_for(section: &SectionNode, all: &[SectionNode]) -> String {
    let mut path = vec![section.title.clone()];
    let mut current_level = section.level;
    let mut pos = all
        .iter()
        .position(|s| s.start_offset == section.start_offset)
        .unwrap_or(0);

    while pos > 0 && current_level > 0 {
        pos -= 1;
        if all[pos].level < current_level {
            path.insert(0, all[pos].title.clone());
            current_level = all[pos].level;
        }
    }
    path.join(" > ")
}

pub fn sections_to_stored(document_id: &str, nodes: &[SectionNode]) -> Vec<Section> {
    let mut stack: Vec<(u32, String)> = Vec::new();
    nodes
        .iter()
        .map(|node| {
            while stack.last().is_some_and(|(lvl, _)| *lvl >= node.level) {
                stack.pop();
            }
            let parent_section_id = stack.last().map(|(_, id)| id.clone());
            let heading_path = if stack.is_empty() {
                node.title.clone()
            } else {
                format!(
                    "{} > {}",
                    stack
                        .iter()
                        .map(|(_, t)| t.as_str())
                        .collect::<Vec<_>>()
                        .join(" > "),
                    node.title
                )
            };
            let id = format!(
                "{}-sec-{}",
                &document_id[..document_id.len().min(8)],
                short_id_from_hash(&hash_text(&format!("{heading_path}{}", node.start_offset)))
            );
            stack.push((node.level, id.clone()));
            Section {
                id,
                document_id: document_id.to_string(),
                parent_section_id,
                title: node.title.clone(),
                level: node.level,
                heading_path,
                start_offset: node.start_offset,
                end_offset: node.end_offset,
                page_start: node.page_start,
                page_end: node.page_end,
                summary: None,
            }
        })
        .collect()
}

pub fn find_section_for_offset(sections: &[Section], offset: usize) -> Option<&Section> {
    sections
        .iter()
        .find(|s| offset >= s.start_offset && offset < s.end_offset)
}
