use anyhow::Result;
use std::path::Path;
use tracing::debug;

/// Extracts raw text from a file based on its extension.
pub fn extract_text(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" => read_plaintext(path),
        "pdf" => read_pdf(path),
        "docx" => read_docx(path),
        other => {
            debug!("Unsupported file type: {}", other);
            anyhow::bail!("Unsupported file type: {}", other)
        }
    }
}

fn read_plaintext(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)?)
}

fn read_pdf(path: &Path) -> Result<String> {
    let doc = lopdf::Document::load(path)?;
    let mut text = String::new();

    let pages: Vec<u32> = doc.get_pages().keys().cloned().collect();
    for page_num in pages {
        match doc.extract_text(&[page_num]) {
            Ok(page_text) => {
                text.push_str(&page_text);
                text.push('\n');
            }
            Err(e) => {
                debug!("Could not extract text from page {}: {}", page_num, e);
            }
        }
    }

    Ok(text)
}

fn read_docx(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let docx = docx_rs::read_docx(&bytes)?;

    let mut text = String::new();
    for child in docx.document.children {
        if let docx_rs::DocumentChild::Paragraph(para) = child {
            for child in para.children {
                if let docx_rs::ParagraphChild::Run(run) = child {
                    for child in run.children {
                        if let docx_rs::RunChild::Text(t) = child {
                            text.push_str(&t.text);
                        }
                    }
                }
            }
            text.push('\n');
        }
    }

    Ok(text)
}