pub mod lm_studio;
pub mod prompt;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("LM Studio is not running at {0}")]
    NotRunning(String),
    #[error("Model unavailable: {0}")]
    ModelUnavailable(String),
    #[error("Request timed out after {0}s")]
    Timeout(u64),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Empty or blank response from model")]
    EmptyResponse,
    #[error("Invalid API response: {0}")]
    InvalidResponse(String),
    #[error("HTTP client error: {0}")]
    HttpError(String),
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub model_id: String,
    pub system_prompt: String,
    pub user_content: String,
    pub temperature: f32,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
    pub model_id: String,
    pub elapsed_ms: u64,
}

// Re-export provider
pub use lm_studio::LmStudioProvider;
