//! Local Llama 3.2 1B Instruct runtime.
//!
//! The model is not bundled with Recall. It is downloaded only after an
//! explicit user action, verified against a pinned SHA-256, and loaded only
//! while generating an answer.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use futures_util::StreamExt;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, AppResult};

pub const MODEL_NAME: &str = "Llama 3.2 1B Instruct";
const FILE_NAME: &str = "Llama-3.2-1B-Instruct-Q4_K_M.gguf";
const DOWNLOAD_URL: &str = "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf";
const SHA256: &str = "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83";
const SIZE_BYTES: u64 = 807_694_464;
const MAX_TOKENS: usize = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPhase {
    NotInstalled,
    Ready,
    Downloading,
    Generating,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub phase: AiPhase,
    pub model_name: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub path: Option<String>,
    pub message: Option<String>,
}

enum Phase {
    Idle,
    Downloading,
    Generating,
}

pub struct AiEngine {
    directory: PathBuf,
    cancel: AtomicBool,
    phase: Mutex<Phase>,
    progress: Mutex<(u64, u64)>,
    error: Mutex<Option<String>>,
}

impl AiEngine {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            cancel: AtomicBool::new(false),
            phase: Mutex::new(Phase::Idle),
            progress: Mutex::new((0, SIZE_BYTES)),
            error: Mutex::new(None),
        }
    }

    pub fn status(&self) -> AiStatus {
        let path = self.model_path();
        let installed = self.model_is_valid();
        let phase = match *self.phase.lock().unwrap_or_else(|e| e.into_inner()) {
            Phase::Downloading => AiPhase::Downloading,
            Phase::Generating => AiPhase::Generating,
            Phase::Idle if installed => AiPhase::Ready,
            Phase::Idle => AiPhase::NotInstalled,
        };
        let (downloaded, total) = *self.progress.lock().unwrap_or_else(|e| e.into_inner());
        let message = self.error.lock().unwrap_or_else(|e| e.into_inner()).clone();
        AiStatus {
            phase: if message.is_some() && matches!(phase, AiPhase::NotInstalled) {
                AiPhase::Error
            } else {
                phase
            },
            model_name: MODEL_NAME,
            downloaded_bytes: if installed { SIZE_BYTES } else { downloaded },
            total_bytes: total,
            path: installed.then(|| path.to_string_lossy().into_owned()),
            message,
        }
    }

    pub async fn download(&self) -> AppResult<()> {
        if self.model_is_valid() {
            return Ok(());
        }
        {
            let mut phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
            if !matches!(*phase, Phase::Idle) {
                return Err(AppError::invalid("The AI model is already busy"));
            }
            *phase = Phase::Downloading;
        }
        self.cancel.store(false, Ordering::Relaxed);
        *self.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let result = self.download_inner().await;
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Idle;
        if let Err(ref error) = result {
            *self.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(error.to_string());
        }
        result
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn remove(&self) -> AppResult<()> {
        let mut phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(*phase, Phase::Idle) {
            return Err(AppError::invalid(
                "Wait until the model is idle before removing it",
            ));
        }
        *phase = Phase::Idle;
        for path in [self.model_path(), self.partial_path()] {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        *self.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }

    pub fn generate(&self, context: &str, question: &str) -> AppResult<String> {
        if !self.model_is_valid() {
            return Err(AppError::NotFound("AI model isn't installed.".into()));
        }
        {
            let mut phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
            if !matches!(*phase, Phase::Idle) {
                return Err(AppError::invalid("The AI model is already busy"));
            }
            *phase = Phase::Generating;
        }
        let result = generate_with_model(&self.model_path(), context, question);
        *self.phase.lock().unwrap_or_else(|e| e.into_inner()) = Phase::Idle;
        result
    }

    fn model_path(&self) -> PathBuf {
        self.directory.join(FILE_NAME)
    }

    fn partial_path(&self) -> PathBuf {
        self.directory.join(format!("{FILE_NAME}.partial"))
    }

    fn model_is_valid(&self) -> bool {
        hash_file(&self.model_path()).ok().as_deref() == Some(SHA256)
    }

    async fn download_inner(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.directory)?;
        let partial = self.partial_path();
        if partial.exists() {
            let _ = std::fs::remove_file(&partial);
        }
        let client = reqwest::Client::builder().build().map_err(net_error)?;
        let response = client.get(DOWNLOAD_URL).send().await.map_err(net_error)?;
        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "The model download failed ({})",
                response.status()
            )));
        }
        let total = response.content_length().unwrap_or(SIZE_BYTES);
        *self.progress.lock().unwrap_or_else(|e| e.into_inner()) = (0, total);
        let mut file = tokio::fs::File::create(&partial).await?;
        let mut hasher = Sha256::new();
        let mut received = 0u64;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if self.cancel.load(Ordering::Relaxed) {
                drop(file);
                let _ = std::fs::remove_file(&partial);
                return Err(AppError::invalid("Model download cancelled"));
            }
            let chunk = chunk.map_err(net_error)?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            received += chunk.len() as u64;
            *self.progress.lock().unwrap_or_else(|e| e.into_inner()) = (received, total);
        }
        file.flush().await?;
        drop(file);
        let actual = hex(&hasher.finalize());
        if actual != SHA256 {
            let _ = std::fs::remove_file(&partial);
            return Err(AppError::Internal(
                "The downloaded AI model didn't match its expected checksum.".into(),
            ));
        }
        std::fs::rename(&partial, self.model_path())?;
        Ok(())
    }
}

fn generate_with_model(path: &Path, context: &str, question: &str) -> AppResult<String> {
    let backend =
        LlamaBackend::init().map_err(|e| AppError::Internal(format!("AI runtime failed: {e}")))?;
    let model = LlamaModel::load_from_file(&backend, path, &LlamaModelParams::default())
        .map_err(|e| AppError::Internal(format!("Could not load the AI model: {e}")))?;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().min(4) as i32)
        .unwrap_or(2);
    let params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(2048))
        .with_n_batch(512)
        .with_n_threads(threads)
        .with_n_threads_batch(threads);
    let mut ctx = model
        .new_context(&backend, params)
        .map_err(|e| AppError::Internal(format!("Could not start the AI model: {e}")))?;
    let template = model
        .chat_template(None)
        .map_err(|e| AppError::Internal(format!("This model has no chat template: {e}")))?;
    let messages = vec![
        LlamaChatMessage::new(
            "system".into(),
            "You are Recall, a local memory assistant. Answer only from the recorded activity in the user message. If it does not contain the answer, say you do not have enough recorded information. Never invent websites, files, applications, times, or events.".into(),
        ).map_err(|e| AppError::Internal(e.to_string()))?,
        LlamaChatMessage::new("user".into(), format!("Question: {question}\n\nRecorded activity:\n{context}"))
            .map_err(|e| AppError::Internal(e.to_string()))?,
    ];
    let prompt = model
        .apply_chat_template(&template, &messages, true)
        .map_err(|e| AppError::Internal(format!("Could not prepare the AI prompt: {e}")))?;
    let tokens = model.vocab().tokenize(prompt.as_bytes(), true, true);
    if tokens.is_empty() || tokens.len() > 1800 {
        return Err(AppError::invalid(
            "The question needed too much context for the local model",
        ));
    }
    let mut batch = LlamaBatch::new(tokens.len().max(8), 1);
    let last = tokens.len() - 1;
    for (index, token) in tokens.iter().copied().enumerate() {
        batch
            .add(token, index as i32, &[0], index == last)
            .map_err(|e| AppError::Internal(format!("AI input failed: {e}")))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| AppError::Internal(format!("AI generation failed: {e}")))?;
    let mut sampler = LlamaSampler::chain([LlamaSampler::greedy()], false);
    let mut output = String::new();
    for position in (tokens.len() as i32..).take(MAX_TOKENS) {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.vocab().is_eog(token) {
            break;
        }
        let piece =
            String::from_utf8_lossy(&model.vocab().token_to_piece(token, false, None)).into_owned();
        output.push_str(&piece);
        batch.clear();
        batch
            .add(token, position, &[0], true)
            .map_err(|e| AppError::Internal(format!("AI generation failed: {e}")))?;
        ctx.decode(&mut batch)
            .map_err(|e| AppError::Internal(format!("AI generation failed: {e}")))?;
    }
    let answer = output.trim().to_string();
    if answer.is_empty() {
        Err(AppError::Internal(
            "The local model returned an empty answer.".into(),
        ))
    } else {
        Ok(answer)
    }
}

fn hash_file(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex(&Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn net_error(error: reqwest::Error) -> AppError {
    AppError::Internal(format!("The model download failed: {error}"))
}
