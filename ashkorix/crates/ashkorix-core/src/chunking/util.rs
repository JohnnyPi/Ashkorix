pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() / 4).max(1) as u32
}

pub fn file_extension(filename: &str) -> String {
    std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

pub fn matches_file_types(filename: &str, file_types: &[String]) -> bool {
    if file_types.is_empty() {
        return true;
    }
    let ext = file_extension(filename);
    file_types.iter().any(|ft| {
        let ft = ft.trim_start_matches('.').to_lowercase();
        ft == ext
    })
}
