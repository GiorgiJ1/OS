use crate::vision::CaptureResult;
use anyhow::{Context, Result};
use rusty_tesseract::{Args, Image};
use std::io::Write;
use tracing::info;

pub fn extract_text(capture: &CaptureResult) -> Result<String> {
    use base64::Engine;

    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(&capture.image_base64)
        .context("Failed to decode base64 image for OCR")?;

    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("skvanchi_ocr_{}.png", uuid::Uuid::new_v4()));

    {
        let mut file = std::fs::File::create(&tmp_path)
            .context("Failed to create temp OCR file")?;
        file.write_all(&png_bytes)?;
    }

    info!("Running OCR on {}", tmp_path.display());

    let image = Image::from_path(&tmp_path)
        .context("Failed to load image for OCR")?;

    let args = Args {
        lang:             "eng".to_string(),
        config_variables: Default::default(),
        dpi:              Some(150),
        psm:              Some(3),
        oem:              Some(3),
    };

    let text = rusty_tesseract::image_to_string(&image, &args)
        .context("Tesseract OCR failed — is Tesseract installed and in PATH?")?;

    let _ = std::fs::remove_file(&tmp_path);

    let cleaned = text.trim().to_string();
    info!("OCR extracted {} chars", cleaned.len());

    Ok(cleaned)
}