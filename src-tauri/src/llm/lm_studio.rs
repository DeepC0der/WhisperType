use crate::llm::{LlmError, LlmRequest, LlmResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub struct LmStudioProvider {
    pub base_url: String,
}

#[derive(Deserialize)]
struct ModelListResponse {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

impl LmStudioProvider {
    pub fn new(base_url: String) -> Self {
        let clean = base_url.trim().trim_end_matches('/');
        let normalized = if let Some(stripped) = clean.strip_suffix("/v1") {
            stripped.to_string()
        } else {
            clean.to_string()
        };
        Self { base_url: normalized }
    }

    fn map_error(e: reqwest::Error) -> LlmError {
        if e.is_timeout() {
            LlmError::Timeout(0) // Duration will be set higher up if needed
        } else if e.is_connect() {
            LlmError::NotRunning(e.to_string())
        } else {
            LlmError::HttpError(e.to_string())
        }
    }

    pub async fn test_connection(&self, timeout_secs: u64) -> Result<(), LlmError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs.min(5)))
            .build()
            .map_err(|e| LlmError::HttpError(e.to_string()))?;

        let url = format!("{}/v1/models", self.base_url.trim_end_matches('/'));
        let res = client.get(&url).send().await.map_err(Self::map_error)?;

        if res.status().is_success() {
            Ok(())
        } else {
            Err(LlmError::ApiError(format!("HTTP {}", res.status())))
        }
    }

    pub async fn list_models(&self, timeout_secs: u64) -> Result<Vec<String>, LlmError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| LlmError::HttpError(e.to_string()))?;

        let url = format!("{}/v1/models", self.base_url.trim_end_matches('/'));
        let res = client.get(&url).send().await.map_err(Self::map_error)?;

        if res.status().is_success() {
            let models_resp = res.json::<ModelListResponse>().await.map_err(|e| LlmError::InvalidResponse(e.to_string()))?;
            Ok(models_resp.data.into_iter().map(|m| m.id).collect())
        } else {
            Err(LlmError::ApiError(format!("HTTP {}", res.status())))
        }
    }

    pub async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let start_time = Instant::now();
        
        let mut model_id = request.model_id.clone();
        if model_id.is_empty() {
            let models = self.list_models(request.timeout_secs).await?;
            if let Some(first) = models.into_iter().next() {
                model_id = first;
            } else {
                return Err(LlmError::ModelUnavailable("No models available in LM Studio".into()));
            }
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(request.timeout_secs))
            .build()
            .map_err(|e| LlmError::HttpError(e.to_string()))?;

        let url = format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'));
        
        let chat_req = ChatRequest {
            model: &model_id,
            messages: vec![
                ChatMessage { role: "system", content: &request.system_prompt },
                ChatMessage { role: "user", content: &request.user_content },
            ],
            temperature: request.temperature,
        };

        let res = client.post(&url)
            .json(&chat_req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LlmError::Timeout(request.timeout_secs)
                } else {
                    Self::map_error(e)
                }
            })?;

        if !res.status().is_success() {
            return Err(LlmError::ApiError(format!("HTTP {}", res.status())));
        }

        let chat_res = res.json::<ChatResponse>().await.map_err(|e| LlmError::InvalidResponse(e.to_string()))?;
        
        let mut text = chat_res.choices.into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        // Strip <think>...</think> blocks if produced by reasoning models
        if let Some(end_idx) = text.find("</think>") {
            text = text[end_idx + 8..].trim().to_string();
        }

        if text.trim().is_empty() {
            return Err(LlmError::EmptyResponse);
        }

        Ok(LlmResponse {
            text,
            model_id,
            elapsed_ms: start_time.elapsed().as_millis() as u64,
        })
    }
}
