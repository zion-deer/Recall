//! Local, optional answers grounded in retrieved memories.

pub mod llama;
pub mod retrieve;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::memory::MemoryEvent;
use crate::settings::Settings;
use crate::storage::{now_ms, Database};
use llama::AiEngine;
use retrieve::{is_small_talk, retrieve};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskResponse {
    pub status: &'static str,
    pub answer: Option<String>,
    pub memories: Vec<MemoryEvent>,
    pub message: Option<String>,
}

pub fn ask(
    db: &Database,
    settings: &Settings,
    engine: &AiEngine,
    question: &str,
) -> AppResult<AskResponse> {
    let question = question.trim();
    if question.is_empty() || question.chars().count() > 1_000 {
        return Err(AppError::invalid(
            "Enter a question of up to 1000 characters",
        ));
    }
    if !settings.ai_enabled {
        return Ok(AskResponse {
            status: "ai_disabled",
            answer: None,
            memories: Vec::new(),
            message: Some("Local AI is turned off. You can enable it in Settings → AI.".into()),
        });
    }
    let retrieval = retrieve(db, question, now_ms())?;
    let chatting = is_small_talk(question);
    if engine.status().phase == llama::AiPhase::NotInstalled {
        if chatting {
            return Ok(AskResponse {
                status: "answered",
                answer: Some(
                    "Hey. I'm here. Once the local model is installed I can talk through what you've been doing, or you can search your memory in the meantime."
                        .into(),
                ),
                memories: Vec::new(),
                message: None,
            });
        }
        return Ok(AskResponse {
            status: "model_not_installed",
            answer: None,
            memories: retrieval.events,
            message: Some("AI model isn't installed.".into()),
        });
    }
    let context = if retrieval.context.is_empty() {
        if chatting {
            "The person is just saying hello. No activity notes are attached. Greet them back and invite them to ask what they were doing.".into()
        } else {
            "No saved activity matched this. Say that naturally, in a sentence or two, and invite another question. Do not use a stock refusal.".into()
        }
    } else {
        retrieval.context
    };
    match engine.generate(&context, question) {
        Ok(answer) => Ok(AskResponse {
            status: "answered",
            answer: Some(answer),
            memories: retrieval.events,
            message: None,
        }),
        Err(error) => Ok(AskResponse {
            status: "error",
            answer: None,
            memories: retrieval.events,
            message: Some(error.to_string()),
        }),
    }
}
