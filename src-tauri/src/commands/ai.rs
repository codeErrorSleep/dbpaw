use crate::ai::openai_compat::OpenAICompatProvider;
use crate::ai::prompt::build_prompt_bundle;
use crate::ai::provider::AIProvider;
use crate::ai::types::{
    AiChatMessage, AiChatRequest, AiChunkPayload, AiDonePayload, AiErrorPayload, AiStartResponse,
    AiStartedPayload,
};
use crate::models::{AiConversation, AiMessage, AiProviderForm};
use crate::state::AppState;
use std::sync::Arc;
use tauri::{Emitter, State};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConversationDetail {
    pub conversation: AiConversation,
    pub messages: Vec<AiMessage>,
}

fn normalize_provider_type(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" | "openai_compat" => Some("openai"),
        "kimi" => Some("kimi"),
        "glm" => Some("glm"),
        _ => None,
    }
}

fn normalize_provider_form(
    form: &mut AiProviderForm,
    fallback_type: Option<&str>,
) -> Result<(), String> {
    let raw = match form.provider_type.as_deref() {
        Some(v) => v,
        None => match fallback_type {
            Some(v) => v,
            None => return Ok(()),
        },
    };
    let normalized = normalize_provider_type(raw)
        .ok_or_else(|| "providerType must be one of: openai, kimi, glm".to_string())?;
    form.provider_type = Some(normalized.to_string());
    Ok(())
}

async fn get_db(state: &State<'_, AppState>) -> Result<Arc<crate::db::local::LocalDb>, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    local_db.ok_or_else(|| "Local DB not initialized".to_string())
}

fn provider_from_model(p: crate::models::AiProvider) -> OpenAICompatProvider {
    OpenAICompatProvider {
        name: p.name,
        base_url: p.base_url,
        api_key: p.api_key,
        model: p.model,
        temperature: 0.1,
        max_tokens: 2048,
        extra_json: p.extra_json,
    }
}

fn emit_ai_error(
    app: &tauri::AppHandle,
    request_id: String,
    conversation_id: Option<i64>,
    error: String,
) {
    let _ = app.emit(
        "ai.error",
        AiErrorPayload {
            request_id,
            conversation_id,
            error,
        },
    );
}

#[tauri::command]
pub async fn ai_list_providers(state: State<'_, AppState>) -> Result<Vec<crate::models::AiProvider>, String> {
    let db = get_db(&state).await?;
    db.list_ai_providers().await
}

#[tauri::command]
pub async fn ai_create_provider(
    state: State<'_, AppState>,
    mut config: AiProviderForm,
) -> Result<crate::models::AiProvider, String> {
    normalize_provider_form(&mut config, Some("openai"))?;
    let db = get_db(&state).await?;
    db.create_ai_provider(config).await
}

#[tauri::command]
pub async fn ai_update_provider(
    state: State<'_, AppState>,
    id: i64,
    mut config: AiProviderForm,
) -> Result<crate::models::AiProvider, String> {
    normalize_provider_form(&mut config, None)?;
    let db = get_db(&state).await?;
    db.update_ai_provider(id, config).await
}

#[tauri::command]
pub async fn ai_delete_provider(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = get_db(&state).await?;
    db.delete_ai_provider(id).await
}

#[tauri::command]
pub async fn ai_set_default_provider(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = get_db(&state).await?;
    db.set_default_ai_provider(id).await
}

#[tauri::command]
pub async fn ai_chat_start(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: AiChatRequest,
) -> Result<AiStartResponse, String> {
    run_chat(app, state, request, true).await
}

#[tauri::command]
pub async fn ai_chat_continue(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: AiChatRequest,
) -> Result<AiStartResponse, String> {
    run_chat(app, state, request, false).await
}

async fn run_chat(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: AiChatRequest,
    create_if_missing: bool,
) -> Result<AiStartResponse, String> {
    let db = get_db(&state).await?;

    let provider_record = if let Some(provider_id) = request.provider_id {
        db.get_ai_provider_by_id(provider_id).await?
    } else {
        db.get_default_ai_provider().await?
    };

    if !provider_record.enabled {
        let msg = "Selected AI provider is disabled".to_string();
        emit_ai_error(&app, request.request_id, request.conversation_id, msg.clone());
        return Err(msg);
    }

    let provider = provider_from_model(provider_record.clone());
    if let Err(e) = provider.validate_config() {
        emit_ai_error(&app, request.request_id, request.conversation_id, e.clone());
        return Err(e);
    }

    let conversation = match request.conversation_id {
        Some(id) => db.get_ai_conversation(id).await?,
        None if create_if_missing => {
            let title = request
                .title
                .clone()
                .unwrap_or_else(|| request.input.chars().take(36).collect());
            db.create_ai_conversation(
                title,
                request.scenario.clone(),
                request.connection_id,
                request.database.clone(),
            )
            .await?
        }
        None => return Err("conversationId is required".to_string()),
    };

    let user_message = db
        .create_ai_message(
            conversation.id,
            "user".to_string(),
            request.input.clone(),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    let bundle = build_prompt_bundle(
        &request.scenario,
        &request.input,
        request.schema_overview.as_ref(),
    );

    let mut history: Vec<AiChatMessage> = Vec::new();
    let mut existing = db.list_ai_messages(conversation.id).await.unwrap_or_default();
    if existing.len() > 16 {
        existing = existing.split_off(existing.len() - 16);
    }
    for item in existing {
        if item.role == "user" || item.role == "assistant" {
            history.push(AiChatMessage {
                role: item.role,
                content: item.content,
            });
        }
    }

    let mut final_messages = bundle.messages.clone();
    final_messages.extend(history);

    let _ = app.emit(
        "ai.started",
        AiStartedPayload {
            request_id: request.request_id.clone(),
            conversation_id: conversation.id,
            model: provider.model.clone(),
        },
    );

    let start = std::time::Instant::now();
    let response = match provider
        .chat_stream(final_messages, |piece| {
            let _ = app.emit(
                "ai.chunk",
                AiChunkPayload {
                    request_id: request.request_id.clone(),
                    conversation_id: conversation.id,
                    chunk: piece.to_string(),
                },
            );
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            emit_ai_error(&app, request.request_id, Some(conversation.id), e.clone());
            return Err(e);
        }
    };
    let latency_ms = start.elapsed().as_millis() as i64;

    let assistant_message = db
        .create_ai_message(
            conversation.id,
            "assistant".to_string(),
            response.content.clone(),
            Some(bundle.prompt_version),
            Some(response.model.clone()),
            response.usage.as_ref().and_then(|u| u.prompt_tokens),
            response.usage.as_ref().and_then(|u| u.completion_tokens),
            Some(latency_ms),
        )
        .await?;

    let _ = db.touch_ai_conversation(conversation.id).await;

    let _ = app.emit(
        "ai.done",
        AiDonePayload {
            request_id: request.request_id,
            conversation_id: conversation.id,
            message_id: assistant_message.id,
            full_response: response.content,
            model: response.model,
            usage: response.usage,
        },
    );

    Ok(AiStartResponse {
        conversation_id: conversation.id,
        user_message_id: user_message.id,
        assistant_message_id: assistant_message.id,
    })
}

#[tauri::command]
pub async fn ai_list_conversations(
    state: State<'_, AppState>,
    connection_id: Option<i64>,
    database: Option<String>,
) -> Result<Vec<AiConversation>, String> {
    let db = get_db(&state).await?;
    db.list_ai_conversations(connection_id, database).await
}

#[tauri::command]
pub async fn ai_get_conversation(
    state: State<'_, AppState>,
    conversation_id: i64,
) -> Result<AiConversationDetail, String> {
    let db = get_db(&state).await?;
    let conversation = db.get_ai_conversation(conversation_id).await?;
    let messages = db.list_ai_messages(conversation_id).await?;
    Ok(AiConversationDetail {
        conversation,
        messages,
    })
}

#[tauri::command]
pub async fn ai_delete_conversation(
    state: State<'_, AppState>,
    conversation_id: i64,
) -> Result<(), String> {
    let db = get_db(&state).await?;
    db.delete_ai_conversation(conversation_id).await
}
