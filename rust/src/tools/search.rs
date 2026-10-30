use std::collections::HashMap;
use std::sync::OnceLock;

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

    // Keep only the first N sentences
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
            text = text[..limit].to_string();
        }
    }

    text.trim_end_matches(|c: char| " ,;:-".contains(c))
        .to_string()
}

#[derive(Debug)]
pub struct SearchError(String);

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SearchError {}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub type SearchFn = Box<dyn Fn(&str) -> Result<Vec<SearchResult>, SearchError> + Send + Sync>;

pub struct WebSearch {
    region: String,
    backend: String,
    max_results: usize,
    timeout: u64,
    search_fn: SearchFn,
}

impl WebSearch {
    pub fn new(
        region: &str,
        backend: &str,
        max_results: usize,
        timeout: u64,
        search_fn: Option<SearchFn>,
    ) -> Self {
        let search_fn = search_fn.unwrap_or_else(|| {
            // Default: placeholder that explains the missing dependency.
            // Replace this with a real implementation (see note below).
            Box::new(|_query: &str| {
                Err(SearchError(
                    "Web search backend is not configured.".to_string(),
                ))
            })
        });

        Self {
            region: region.to_string(),
            backend: backend.to_string(),
            max_results,
            timeout,
            search_fn,
        }
    }

    /// Convenience constructor with defaults matching the Python version.
    pub fn with_defaults() -> Self {
        Self::new("us-en", "auto", 5, 8, None)
    }

    pub fn web_search(&self, slots: &HashMap<String, String>) -> ToolResult {
        let query = slots
            .get("query")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if query.is_empty() {
            return ToolResult::err("What should I search for?")
                .with_follow_up(FollowUp::new("web_search", "query"));
        }

        let raw = match (self.search_fn)(&query) {
            Ok(r) => r,
            Err(e) => return ToolResult::err(e.to_string()),
        };

        if raw.is_empty() {
            return ToolResult::err(format!("I didn't find anything for {query}."));
        }

        // Prefer a result that has a speakable snippet
        let best = raw.iter().find(|r| !speakable(&r.snippet, 280, 1).is_empty());

        let Some(best) = best else {
            // Fall back to just the title of the first result
            let mut data = HashMap::new();
            // Store a simple summary; richer serialisation can be added later
            data.insert("count".to_string(), raw.len().to_string());
            return ToolResult::ok(format!("The top result is {}.", raw[0].title))
                .with_data(data);
        };

        let domain = Url::parse(&best.url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
            .unwrap_or_default();

        let intro = if domain.is_empty() {
            String::new()
        } else {
            format!("According to {domain}: ")
        };

        let mut data = HashMap::new();
        data.insert("count".to_string(), raw.len().to_string());

        ToolResult::ok(intro + &speakable(&best.snippet, 280, 1)).with_data(data)
    }
}
