use crate::skill::{Skill, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use tracing::debug;

pub struct WebSearchSkill {
    client: Client,
}

impl WebSearchSkill {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .expect("http client"),
        }
    }

    fn extract_query(input: &str) -> String {
        let lower = input.to_lowercase();
        let triggers = [
            "search for ", "search ", "look up ", "find online ",
            "google ", "latest news on ", "news about ",
        ];
        for trigger in &triggers {
            if lower.starts_with(trigger) {
                return input[trigger.len()..].trim().to_string();
            }
        }
        input.trim().to_string()
    }
}

#[async_trait]
impl Skill for WebSearchSkill {
    fn name(&self) -> &str { "web_search" }

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

        // Use SearXNG public instance — no API key, no scraping
        let url = format!(
            "https://searx.be/search?q={}&format=json&categories=general",
            urlencoding::encode(&query)
        );

        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let text = r.text().await?;
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(results) = json["results"].as_array() {
                        if results.is_empty() {
                            return Ok(SkillOutput::err("web_search", "No results found"));
                        }

                        let output = results
                            .iter()
                            .take(5)
                            .map(|r| {
                                let title   = r["title"].as_str().unwrap_or("").to_string();
                                let content = r["content"].as_str().unwrap_or("").to_string();
                                let url     = r["url"].as_str().unwrap_or("").to_string();
                                format!("**{}**\n{}\n{}\n", title, content, url)
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        return Ok(SkillOutput::ok(
                            format!("web: {}", &query[..query.len().min(40)]),
                            output,
                        ));
                    }
                }
                Ok(SkillOutput::err("web_search", "Could not parse search results"))
            }
            Ok(r) => Ok(SkillOutput::err(
                "web_search",
                &format!("Search failed with status: {}", r.status()),
            )),
            Err(e) => Ok(SkillOutput::err(
                "web_search",
                &format!("Search request failed: {}", e),
            )),
        }
    }
}