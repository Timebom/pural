use std::collections::HashMap;
use std::sync::OnceLock;

use ddgs::{Ddgs, TextOptions, TextBackend};
use regex::Regex;
use url::Url;

use crate::interfaces::{FollowUp, ToolResult};

fn sentence_end() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?<=[.!?])\s+").unwrap())
}

fn strip_urls() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"https?://\S+").unwrap())
}

fn strip_symbols() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"[^\w\s.,;:!?'\"%$&()/+-]"#).unwrap())
}

fn trailing_dots() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s*\.{2,}\s*$").unwrap())
}

/// Turn a search snippet into something pleasant to hear.
fn speakable(snippet: &str, limit: usize, max_sentences: usize) -> String {
    let mut text = strip_urls().replace_all(snippet, "").into_owned();
    text = strip_symbols().replace_all(&text, " ").into_owned();
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text = trailing_dots().replace(&text, "").into_owned();

    let sentences: Vec<&str> = sentence_end().split(&text).collect();
    text = sentences
        .into_iter()
        .take(max_sentences)
        .collect::<Vec<_>>()
        .join(" ");

    if text.len() > limit {
        if let Some(idx) = text[..limit].rfind(' ') {
            text = text[..idx].to_string();
        } else {
            text.truncate(limit);
        }
    }

    text.trim_end_matches(|c: char| " ,;:-".contains(c))
        .to_string()
}

pub struct WebSearch {
    region: String,
    backend: String,
    max_results: usize,
    // ddgs client is created per-call (cheap) or you can cache it
}

impl WebSearch {
    pub fn new(region: &str, backend: &str, max_results: usize, _timeout: u64) -> Self {
        Self {
            region: region.to_string(),
            backend: backend.to_string(),
            max_results,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new("us-en", "auto", 5, 8)
    }

    async fn search(&self, query: &str) -> Result<Vec<ddgs::TextResult>, String> {
        let ddgs = Ddgs::new().map_err(|e| e.to_string())?;

        let backend = match self.backend.as_str() {
            "auto" => TextBackend::Auto,
            "yahoo" => TextBackend::Yahoo,
            "brave" => TextBackend::Brave,
            "google" => TextBackend::Google,
            "html" | "lite" | "api" => TextBackend::Auto, // fallback
            _ => TextBackend::Auto,
        };

        let opts = TextOptions::default()
            .max_results(self.max_results)
            .backend(backend);
            // .region(...) if the crate exposes it in your version

        ddgs.text_with_options(query, opts)
            .await
            .map_err(|e| {
                let msg = e.to_string().to_lowercase();
                if msg.contains("no results") {
                    String::new() // treat as empty results
                } else {
                    "I couldn't search the web right now.".to_string()
                }
            })
    }

    pub async fn web_search(&self, slots: &HashMap<String, String>) -> ToolResult {
        let query = slots
            .get("query")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if query.is_empty() {
            return ToolResult::err("What should I search for?")
                .with_follow_up(FollowUp::new("web_search", "query"));
        }

        let raw = match self.search(&query).await {
            Ok(r) => r,
            Err(e) if e.is_empty() => Vec::new(),
            Err(e) => return ToolResult::err(e),
        };

        if raw.is_empty() {
            return ToolResult::err(format!("I didn't find anything for {query}."));
        }

        // Prefer a result that has a usable snippet
        let best = raw.iter().find(|r| {
            let body = r.body.as_deref().unwrap_or("");
            !speakable(body, 280, 1).is_empty()
        });

        let Some(best) = best else {
            let mut data = HashMap::new();
            data.insert("count".to_string(), raw.len().to_string());
            return ToolResult::ok(format!("The top result is {}.", raw[0].title))
                .with_data(data);
        };

        let domain = Url::parse(&best.href)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
            .unwrap_or_default();

        let intro = if domain.is_empty() {
            String::new()
        } else {
            format!("According to {domain}: ")
        };

        let snippet = best.body.as_deref().unwrap_or("");
        let mut data = HashMap::new();
        data.insert("count".to_string(), raw.len().to_string());

        ToolResult::ok(intro + &speakable(snippet, 280, 1)).with_data(data)
    }
}
