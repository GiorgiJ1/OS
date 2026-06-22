use crate::skill::{Skill, SkillOutput};
use aios_system::{CaptureTarget, capture_screen_with_target, list_screens, extract_text};
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, warn};

pub struct ScreenVisionSkill {
    ollama_url: String,
    model:      String,
}

impl ScreenVisionSkill {
    pub fn new() -> Self {
        Self {
            ollama_url: std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            model: std::env::var("VISION_MODEL")
                .unwrap_or_else(|_| "llava".to_string()),
        }
    }

    fn parse_capture_target(input: &str) -> CaptureTarget {
        let lower = input.to_lowercase();

        if lower.contains("primary monitor")
            || lower.contains("primary screen")
            || lower.contains("main monitor")
            || lower.contains("main screen")
        {
            return CaptureTarget::Primary;
        }

        if lower.contains("second monitor")
            || lower.contains("second screen")
            || lower.contains("monitor 2")
            || lower.contains("screen 2")
            || lower.contains("display 2")
        {
            return CaptureTarget::Index(2);
        }

        for prefix in ["/screen ", "screen ", "monitor ", "display "] {
            if let Some(rest) = lower.find(prefix) {
                let after = input[rest + prefix.len()..].trim();
                if let Some(num_str) = after.split_whitespace().next() {
                    if let Ok(n) = num_str.parse::<usize>() {
                        return CaptureTarget::Index(n);
                    }
                }
            }
        }

        CaptureTarget::Cursor
    }

    fn format_monitor_list() -> Result<String> {
        let screens = list_screens()?;
        if screens.is_empty() {
            return Ok("No displays detected.".to_string());
        }

        let mut out = String::from("Available displays:\n");
        for screen in screens {
            out.push_str(&format!(
                "  {} — {}x{} at ({}, {}){}\n",
                screen.index,
                screen.width,
                screen.height,
                screen.x,
                screen.y,
                if screen.is_primary { " [primary]" } else { "" },
            ));
        }
        out.push_str("\nUse `/screen 2` or `look at monitor 2` to target a specific display.");
        Ok(out)
    }
}

#[async_trait]
impl Skill for ScreenVisionSkill {
    fn name(&self) -> &str {
        "screen_vision"
    }

    fn description(&self) -> &str {
        "Capture and analyze what's currently on screen"
    }

    fn can_handle(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        lower.contains("what's on my screen")
            || lower.contains("whats on my screen")
            || lower.contains("what am i looking at")
            || lower.contains("what do you see")
            || lower.contains("look at my screen")
            || lower.contains("look at monitor")
            || lower.contains("look at screen")
            || lower.starts_with("/screen")
            || lower.contains("read my screen")
            || lower.contains("screenshot")
            || lower.contains("list monitor")
            || lower.contains("list screen")
            || lower == "/screens"
    }

    async fn execute(&self, input: &str) -> Result<SkillOutput> {
        let lower = input.to_lowercase();
        if lower.contains("list monitor")
            || lower.contains("list screen")
            || lower == "/screens"
        {
            return Ok(SkillOutput::ok(
                "screen_vision",
                Self::format_monitor_list()?,
            ));
        }

        let target = Self::parse_capture_target(input);
        info!("Capturing screen for vision analysis (target: {:?})", target);

        let capture = match tokio::task::spawn_blocking(move || {
            capture_screen_with_target(target)
        })
        .await
        {
            Ok(Ok(capture)) => capture,
            Ok(Err(e)) => {
                warn!("Screen capture failed: {e:#}");
                return Ok(SkillOutput::err(
                    "screen_vision",
                    format!("Screen capture failed: {e:#}"),
                ));
            }
            Err(e) => {
                warn!("Screen capture task failed: {e:#}");
                return Ok(SkillOutput::err(
                    "screen_vision",
                    format!("Screen capture task failed: {e:#}"),
                ));
            }
        };

        info!(
            "Screen captured from {} — {}x{}, {} bytes PNG, {} chars base64",
            capture.monitor_label,
            capture.width,
            capture.height,
            capture.png_bytes,
            capture.image_base64.len(),
        );

        // Run OCR concurrently with vision request
        let capture_for_ocr = capture.clone();
        let ocr_handle = tokio::task::spawn_blocking(move || {
            extract_text(&capture_for_ocr)
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        #[derive(serde::Serialize)]
        struct Req<'a> {
            model:  &'a str,
            prompt: &'a str,
            images: Vec<String>,
            stream: bool,
        }

        #[derive(serde::Deserialize)]
        struct Resp {
            response: String,
        }

        let prompt = format!(
            "You are analyzing a screenshot from the user's computer ({monitor}). \
             Describe what is visible in detail: open applications, readable text, \
             errors, notifications, and what the user appears to be doing. \
             Answer the user's question directly.\n\nUser question: {question}",
            monitor = capture.monitor_label,
            question = input,
        );

        let body = Req {
            model:  &self.model,
            prompt: &prompt,
            images: vec![capture.image_base64.clone()],
            stream: false,
        };

        let url = format!("{}/api/generate", self.ollama_url);
        info!(
            "Sending vision request to Ollama model '{}' at {}",
            self.model, self.ollama_url
        );
        let start = std::time::Instant::now();

        let resp = client.post(&url).json(&body).send().await?;
        let status = resp.status();
        let body_text = resp.text().await?;

        info!(
            "Vision HTTP response after {:?}, status: {}",
            start.elapsed(),
            status
        );

        if !status.is_success() {
            return Ok(SkillOutput::err(
                "screen_vision",
                format!(
                    "Vision model returned {status}: {body_text}. \
                     Is '{}' installed? Run: ollama pull {}",
                    self.model, self.model
                ),
            ));
        }

        let data: Resp = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!("Failed to parse Ollama vision response: {e}. Body: {body_text}")
        })?;

        if data.response.trim().is_empty() {
            return Ok(SkillOutput::err(
                "screen_vision",
                "Vision model returned an empty description.",
            ));
        }

        info!("Vision analysis complete after {:?}", start.elapsed());

        // Collect OCR result (already running in parallel above)
        let ocr_text = match ocr_handle.await {
            Ok(Ok(text)) if !text.trim().is_empty() => Some(text),
            Ok(Ok(_))  => None,
            Ok(Err(e)) => {
                warn!("OCR failed: {e:#}");
                None
            }
            Err(e) => {
                warn!("OCR task panicked: {e:#}");
                None
            }
        };

        let mut final_output = format!(
            "Screenshot from {} ({}x{}).\n\n{}",
            capture.monitor_label, capture.width, capture.height, data.response
        );

        if let Some(text) = ocr_text {
            final_output.push_str("\n\n--- Exact text read from screen (OCR) ---\n");
            final_output.push_str(&text);
        }

        Ok(SkillOutput::ok(
            format!("screen: {}", capture.monitor_label),
            final_output,
        ))
    }
}