use crate::skill::{Skill, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, warn};

pub struct WebSearchSkill {
    client: Client,
}

impl WebSearchSkill {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                     AppleWebKit/537.36 (KHTML, like Gecko) \
                     Chrome/120.0.0.0 Safari/537.36",
                )
                .build()
                .expect("http client"),
        }
    }

    fn extract_query(input: &str) -> String {
        let lower = input.to_lowercase();
        let triggers = [
            "search for ",
            "search ",
            "look up ",
            "find online ",
            "google ",
            "latest news on ",
            "news about ",
        ];
        for trigger in &triggers {
            if lower.starts_with(trigger) {
                return input[trigger.len()..].trim().to_string();
            }
        }
        input.trim().to_string()
    }

    fn decode_ddg_url(href: &str) -> String {
        if let Some(idx) = href.find("uddg=") {
            let encoded = href[idx + 5..].split('&').next().unwrap_or("");
            urlencoding::decode(encoded)
                .map(|s| s.into_owned())
                .unwrap_or_else(|_| href.to_string())
        } else if href.starts_with("//") {
            format!("https:{href}")
        } else {
            href.to_string()
        }
    }

    fn strip_html(text: &str) -> String {
        static TAGS: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let re = TAGS.get_or_init(|| Regex::new(r"<[^>]+>").expect("tag regex"));
        re.replace_all(text, "")
            .replace("&#x27;", "'")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .trim()
            .to_string()
    }

    fn parse_ddg_html(html: &str) -> Vec<(String, String, String)> {
        static RESULTS: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let re = RESULTS.get_or_init(|| {
            Regex::new(
                r#"(?s)class="result__a" href="([^"]*)"[^>]*>(.*?)</a>.*?class="result__snippet"[^>]*>(.*?)</a>"#,
            )
            .expect("result regex")
        });

        re.captures_iter(html)
            .map(|cap| {
                let href = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let title = Self::strip_html(cap.get(2).map(|m| m.as_str()).unwrap_or(""));
                let snippet = Self::strip_html(cap.get(3).map(|m| m.as_str()).unwrap_or(""));
                (title, snippet, Self::decode_ddg_url(href))
            })
            .filter(|(title, _, _)| !title.is_empty())
            .collect()
    }

    async fn search_duckduckgo(&self, query: &str) -> Result<Vec<(String, String, String)>> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("DuckDuckGo returned status {}", resp.status());
        }

        let html = resp.text().await?;
        let results = Self::parse_ddg_html(&html);
        if results.is_empty() {
            anyhow::bail!("No parseable results in DuckDuckGo response");
        }
        Ok(results)
    }

    fn format_results(results: &[(String, String, String)]) -> String {
        results
            .iter()
            .take(5)
            .map(|(title, snippet, url)| {
                if snippet.is_empty() {
                    format!("**{title}**\n{url}\n")
                } else {
                    format!("**{title}**\n{snippet}\n{url}\n")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait]
impl Skill for WebSearchSkill {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for current information"
    }

    fn can_handle(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        lower.starts_with("search")
            || lower.starts_with("look up")
            || lower.starts_with("find online")
            || lower.contains("latest news")
            || lower.contains("news about")
    }

    async fn execute(&self, input: &str) -> Result<SkillOutput> {
        let query = Self::extract_query(input);
        if query.is_empty() {
            return Ok(SkillOutput::err("web_search", "Empty search query"));
        }

        debug!("Web search query: {}", query);

        match self.search_duckduckgo(&query).await {
            Ok(results) => {
                let output = Self::format_results(&results);
                Ok(SkillOutput::ok(
                    format!("web: {}", &query[..query.len().min(40)]),
                    output,
                ))
            }
            Err(e) => {
                warn!("Web search failed for '{}': {}", query, e);
                Ok(SkillOutput::err(
                    "web_search",
                    format!("Search failed: {e}"),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ddg_sample() {
        let html = r##"
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&amp;rut=abc">Example Title</a>
            <a class="result__snippet" href="#">A short <b>summary</b> here.</a>
        "##;
        let results = WebSearchSkill::parse_ddg_html(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "Example Title");
        assert_eq!(results[0].1, "A short summary here.");
        assert_eq!(results[0].2, "https://example.com");
    }
}
