//! API Access > AI (prompts per page) and Content > Embeddings.
//!
//! Providers: OpenAI, any OpenAI-compatible endpoint (Azure, DeepSeek, Grok,
//! OpenRouter, LM Studio...), Anthropic, Gemini and Ollama. Anthropic has no
//! embeddings API.

use crate::{keys, send_json, ApiRunner, Throttle};
use serde_json::{json, Value};
use toad_core::api_config::{AiContent, AiPrompt, AiProvider};
use toad_store::api::{ApiValue, AI};
use crate::StoreLock;

impl ApiRunner {
    fn openai_base(&self, provider: AiProvider) -> anyhow::Result<String> {
        let ai = &self.cfg.apis.ai;
        let base = match provider {
            AiProvider::OpenAi => self.endpoints.openai.clone().unwrap_or_else(|| ai.openai_base_url.clone()),
            AiProvider::OpenAiCompatible => self.endpoints.openai.clone().unwrap_or_else(|| ai.compatible_base_url.clone()),
            _ => unreachable!(),
        };
        anyhow::ensure!(!base.trim().is_empty(), "set the {} base URL", provider.name());
        Ok(base.trim_end_matches('/').to_string())
    }

    fn ollama_base(&self) -> String {
        self.endpoints.ollama.clone().unwrap_or_else(|| self.cfg.apis.ai.ollama_base_url.clone()).trim_end_matches('/').to_string()
    }

    fn key_for(&self, provider: AiProvider) -> anyhow::Result<Option<&str>> {
        Ok(match provider {
            AiProvider::OpenAi => Some(self.secrets.require(keys::OPENAI, "OpenAI")?),
            AiProvider::Anthropic => Some(self.secrets.require(keys::ANTHROPIC, "Anthropic")?),
            AiProvider::Gemini => Some(self.secrets.require(keys::GEMINI, "Gemini")?),
            // Local servers usually need no key.
            AiProvider::OpenAiCompatible => self.secrets.get(keys::COMPATIBLE),
            AiProvider::Ollama | AiProvider::Local => None,
        })
    }

    /// One chat completion: the instruction, then the page content.
    pub async fn complete(&self, provider: AiProvider, model: &str, instruction: &str, content: &str) -> anyhow::Result<String> {
        let ai = &self.cfg.apis.ai;
        let key = self.key_for(provider)?;
        let user = if content.is_empty() { instruction.to_string() } else { format!("{instruction}\n\n---\n{content}") };
        let json: Value = match provider {
            AiProvider::OpenAi | AiProvider::OpenAiCompatible => {
                let url = format!("{}/chat/completions", self.openai_base(provider)?);
                let body = json!({"model": model, "messages": [{"role": "user", "content": user}], "temperature": ai.temperature, "max_tokens": ai.max_output_tokens});
                send_json(|| {
                    let r = self.client.post(&url).json(&body);
                    match key {
                        Some(k) => r.bearer_auth(k),
                        None => r,
                    }
                })
                .await?
            }
            AiProvider::Anthropic => {
                let url = format!("{}/messages", self.endpoints.anthropic);
                let body = json!({"model": model, "max_tokens": ai.max_output_tokens, "temperature": ai.temperature, "messages": [{"role": "user", "content": user}]});
                send_json(|| self.client.post(&url).header("x-api-key", key.unwrap_or("")).header("anthropic-version", "2023-06-01").json(&body)).await?
            }
            AiProvider::Gemini => {
                let url = format!("{}/models/{model}:generateContent", self.endpoints.gemini);
                let body = json!({"contents": [{"role": "user", "parts": [{"text": user}]}], "generationConfig": {"temperature": ai.temperature, "maxOutputTokens": ai.max_output_tokens}});
                send_json(|| self.client.post(&url).header("x-goog-api-key", key.unwrap_or("")).json(&body)).await?
            }
            AiProvider::Local => anyhow::bail!("the built-in model only makes embeddings; choose a provider for AI prompts"),
            AiProvider::Ollama => {
                let url = format!("{}/api/chat", self.ollama_base());
                let body = json!({"model": model, "messages": [{"role": "user", "content": user}], "stream": false, "options": {"temperature": ai.temperature, "num_predict": ai.max_output_tokens}});
                send_json(|| self.client.post(&url).json(&body)).await?
            }
        };
        let text = match provider {
            AiProvider::OpenAi | AiProvider::OpenAiCompatible => json.pointer("/choices/0/message/content").and_then(Value::as_str).map(str::to_string),
            AiProvider::Anthropic => json["content"]
                .as_array()
                .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("")),
            AiProvider::Gemini => json
                .pointer("/candidates/0/content/parts")
                .and_then(Value::as_array)
                .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("")),
            AiProvider::Ollama => json.pointer("/message/content").and_then(Value::as_str).map(str::to_string),
            AiProvider::Local => None,
        };
        text.map(|t| t.trim().to_string()).ok_or_else(|| anyhow::anyhow!("{} returned no text", provider.name()))
    }

    /// One embedding vector.
    pub async fn embed(&self, provider: AiProvider, model: &str, text: &str) -> anyhow::Result<Vec<f32>> {
        let key = self.key_for(provider)?;
        let json: Value = match provider {
            AiProvider::OpenAi | AiProvider::OpenAiCompatible => {
                let url = format!("{}/embeddings", self.openai_base(provider)?);
                let body = json!({"model": model, "input": text});
                send_json(|| {
                    let r = self.client.post(&url).json(&body);
                    match key {
                        Some(k) => r.bearer_auth(k),
                        None => r,
                    }
                })
                .await?
            }
            AiProvider::Gemini => {
                let url = format!("{}/models/{model}:embedContent", self.endpoints.gemini);
                let body = json!({"content": {"parts": [{"text": text}]}});
                send_json(|| self.client.post(&url).header("x-goog-api-key", key.unwrap_or("")).json(&body)).await?
            }
            AiProvider::Ollama => {
                let url = format!("{}/api/embed", self.ollama_base());
                send_json(|| self.client.post(&url).json(&json!({"model": model, "input": text}))).await?
            }
            AiProvider::Anthropic => anyhow::bail!("Anthropic has no embeddings API; choose Built-in, OpenAI, Gemini, Ollama or an OpenAI-compatible provider"),
            AiProvider::Local => {
                let info = crate::local_embed::model(model)?;
                let dir = crate::local_embed::model_dir(info);
                let text = text.to_string();
                let (info, dir2) = (*info, dir.clone());
                return tokio::task::spawn_blocking(move || toad_embed::Embedder::load(&info, &dir2)?.embed(&text)).await?;
            }
        };
        let values = match provider {
            AiProvider::OpenAi | AiProvider::OpenAiCompatible => json.pointer("/data/0/embedding"),
            AiProvider::Gemini => json.pointer("/embedding/values"),
            AiProvider::Ollama => json.pointer("/embeddings/0"),
            AiProvider::Anthropic | AiProvider::Local => None,
        };
        let v: Vec<f32> = values.and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect()).unwrap_or_default();
        anyhow::ensure!(!v.is_empty(), "{} returned no embedding", provider.name());
        Ok(v)
    }
}

fn cut(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars.max(1)).collect()
}

pub(crate) fn page_content(store: &dyn StoreLock, url: &str, content: AiContent, max_chars: usize) -> anyhow::Result<String> {
    let s = store.lock_store()?;
    let text = s.content_for(url, content)?.ok_or_else(|| match content {
        AiContent::Html => anyhow::anyhow!("no stored HTML (switch on Store HTML)"),
        _ => anyhow::anyhow!("no page text stored for this page"),
    })?;
    Ok(cut(&text, max_chars))
}

/// Every enabled prompt over the crawled pages; one column per prompt.
pub async fn run_prompts(runner: &ApiRunner, store: &dyn StoreLock) -> anyhow::Result<()> {
    let ai = &runner.cfg.apis.ai;
    let prompts: Vec<AiPrompt> = ai.prompts.iter().filter(|p| p.enabled && !p.name.trim().is_empty() && !p.prompt.trim().is_empty()).cloned().collect();
    let targets = {
        let s = store.lock_store()?;
        s.api_targets(ai.indexable_only)?
    };
    for prompt in prompts {
        // Skip pages that already answered this prompt.
        let todo: Vec<String> = {
            let s = store.lock_store()?;
            let mut v = Vec::new();
            for t in &targets {
                if s.api_value(t, AI, &prompt.name)?.is_none() {
                    v.push(t.clone());
                }
            }
            v
        };
        let throttle = Throttle::new(4, ai.requests_per_minute);
        crate::for_each_url(runner, store, AI, todo, throttle, |url| {
            let prompt = prompt.clone();
            async move {
                let content = if prompt.content == AiContent::UrlOnly { String::new() } else { page_content(store, &url, prompt.content, ai.max_input_chars)? };
                let instruction = prompt.prompt.replace("{url}", &url);
                let answer = runner.complete(prompt.provider, &prompt.model, &instruction, &content).await?;
                Ok(vec![ApiValue::new(url, prompt.name.clone(), answer)])
            }
        })
        .await?;
    }
    Ok(())
}

/// Content > Embeddings: one vector per page (matching the filter regex).
pub async fn run_embeddings(runner: &ApiRunner, store: &dyn StoreLock) -> anyhow::Result<()> {
    let e = &runner.cfg.apis.embeddings;
    let include = if e.include_regex.trim().is_empty() { None } else { Some(regex::Regex::new(e.include_regex.trim())?) };
    let todo: Vec<String> = {
        let s = store.lock_store()?;
        let have: std::collections::HashSet<String> = s.embeddings()?.into_iter().map(|(a, _)| a).collect();
        s.api_targets(false)?.into_iter().filter(|u| !have.contains(u) && include.as_ref().is_none_or(|re| re.is_match(u))).collect()
    };
    if e.provider == AiProvider::Local {
        return crate::local_embed::run(runner, store, todo).await;
    }
    let max_chars = runner.cfg.apis.ai.max_input_chars;
    let throttle = Throttle::new(4, runner.cfg.apis.ai.requests_per_minute.max(60));
    crate::for_each_url(runner, store, toad_store::api::EMBED, todo, throttle, |url| async move {
        let text = page_content(store, &url, e.content, max_chars)?;
        anyhow::ensure!(!text.trim().is_empty(), "no text to embed");
        let v = runner.embed(e.provider, &e.model, &text).await?;
        store.lock_store()?.put_embedding(&url, &v)?;
        Ok(vec![])
    })
    .await
}
