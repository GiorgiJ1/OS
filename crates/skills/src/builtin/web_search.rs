use crate::skill::{Skill, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::debug;

pub struct WebSearchSkill {
    client: Client,
}

impl WebSearchSkill {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("Skvanchi/0.1 (personal AI assistant)")
                .build()
                .expect("http client"),
        }
    }
}

#[derive(Deserialize)]
struct DuckDuckGoResult {
    #[serde(rename = "AbstractText")]
    abstract_text: Option<String>,
    #[serde(rename = "AbstractURL")]
    abstract_url:  Option<String>,
    #[serde(rename = "RelatedTopics")]
    related_topics: Option<Vec<RelatedTopic>>,
}

#[derive(Deserialize)]
struct RelatedTopic {
    #[serde(rename = "Text")]
    text: Option<String>,
    #[serde(rename = "FirstURL")]
    url:  Option<String>,
}

#[async_trait]
impl Skill for WebSearchSkill {
    fn name(&self) -> &str { "web_search" }

    fn description(&self) -> &str {
        "Search the web for current information"
    }

    fn can_handle(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        lower.contains("search")
            || lower.contains("look up")
            || lower.contains("find online")
            || lower.contains("google")
            || lower.contains("what is the latest")
            || lower.contains("current price")
            || lower.contains("news about")
            || lower.contains("who is")
            || lower.contains("when did")
            || lower.contains("how much does")
    }

    async fn execute(&self, input: &str) -> Result<SkillOutput> {
        // Extract search query — strip trigger words
        let query = input
            .to_lowercase()
            .replace("search for", "")
            .replace("search", "")
            .replace("look up", "")
            .replace("find online", "")
            .replace("google", "")
            .trim()
            .to_string();

        debug!("Web search: {}", query);

        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding::encode(&query)
        );

        let resp = self.client.get(&url).send().await?;
        let result: DuckDuckGoResult = resp.json().await?;

        let mut output = String::new();

        if let Some(text) = &result.abstract_text {
            if !text.is_empty() {
                output.push_str(text);
                if let Some(url) = &result.abstract_url {
                    output.push_str(&format!("\nSource: {}", url));
                }
                output.push('\n');
            }
        }

        if let Some(topics) = &result.related_topics {
            for topic in topics.iter().take(3) {
                if let Some(text) = &topic.text {
                    if !text.is_empty() {
                        output.push_str(&format!("- {}\n", text));
                        if let Some(url) = &topic.url {
                            output.push_str(&format!("  {}\n", url));
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = format!("No results found for: {}", query);
        }

        Ok(SkillOutput::ok(
            format!("web: {}", &query[..query.len().min(40)]),
            output,
        ))
    }
}