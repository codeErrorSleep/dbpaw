use crate::ai::openai_compat::OpenAICompatProvider;
use crate::ai::prompt::build_prompt_bundle;
use crate::ai::provider::AIProvider;
use crate::ai::types::{
    AiChatMessage, AiChatRequest, AiChunkPayload, AiColumnSummary, AiDonePayload, AiErrorPayload,
    AiSchemaOverview, AiStartResponse, AiStartedPayload, AiTableSummary,
};
use crate::models::{AiConversation, AiMessage, AiProviderForm, AiProviderPublic};
use crate::state::AppState;
use std::sync::Arc;
use tauri::{Emitter, State};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConversationDetail {
    pub conversation: AiConversation,
    pub messages: Vec<AiMessage>,
}

fn normalize_provider_type(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err("providerType is required".to_string());
    }
    if normalized == "openai_compat" {
        return Ok("openai".to_string());
    }
    if normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
    {
        Ok(normalized)
    } else {
        Err("providerType has invalid format".to_string())
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
    let normalized = normalize_provider_type(raw)?;
    form.provider_type = Some(normalized);
    Ok(())
}

fn map_provider_lookup_error(e: &str) -> String {
    if e.contains("[GET_AI_PROVIDER_ERROR]") {
        "Selected AI provider does not exist".to_string()
    } else {
        e.to_string()
    }
}

fn map_default_provider_error(e: &str) -> String {
    if e.contains("[NO_ENABLED_AI_PROVIDER]") {
        "No enabled AI provider is configured. Please enable one in AI Provider settings."
            .to_string()
    } else {
        e.to_string()
    }
}

fn ensure_provider_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        Ok(())
    } else {
        Err("Selected AI provider is disabled".to_string())
    }
}

fn validate_conversation_requirement(
    conversation_id: Option<i64>,
    create_if_missing: bool,
) -> Result<(), String> {
    if conversation_id.is_none() && !create_if_missing {
        Err("conversationId is required".to_string())
    } else {
        Ok(())
    }
}

fn map_history_load_error(conversation_id: i64, e: &str) -> String {
    eprintln!(
        "[AI_HISTORY_LOAD_ERROR] Failed to load messages for conversation {}: {}",
        conversation_id, e
    );
    "Failed to load conversation history".to_string()
}

async fn get_db(state: &State<'_, AppState>) -> Result<Arc<crate::db::local::LocalDb>, String> {
    let local_db = {
        let lock = state.local_db.lock().await;
        lock.clone()
    };
    local_db.ok_or_else(|| "Local DB not initialized".to_string())
}

fn provider_from_model(p: crate::models::AiProvider, api_key: String) -> OpenAICompatProvider {
    OpenAICompatProvider {
        name: p.name,
        base_url: p.base_url,
        api_key,
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
        "ai/error",
        AiErrorPayload {
            request_id,
            conversation_id,
            error,
        },
    );
}

#[tauri::command]
pub async fn ai_list_providers(
    state: State<'_, AppState>,
) -> Result<Vec<AiProviderPublic>, String> {
    let db = get_db(&state).await?;
    db.list_ai_providers_public().await
}

#[tauri::command]
pub async fn ai_create_provider(
    state: State<'_, AppState>,
    mut config: AiProviderForm,
) -> Result<AiProviderPublic, String> {
    normalize_provider_form(&mut config, Some("openai"))?;
    let db = get_db(&state).await?;
    let created = db.create_ai_provider(config).await?;
    db.get_ai_provider_public_by_id(created.id).await
}

#[tauri::command]
pub async fn ai_update_provider(
    state: State<'_, AppState>,
    id: i64,
    mut config: AiProviderForm,
) -> Result<AiProviderPublic, String> {
    normalize_provider_form(&mut config, None)?;
    let db = get_db(&state).await?;
    let updated = db.update_ai_provider(id, config).await?;
    db.get_ai_provider_public_by_id(updated.id).await
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
pub async fn ai_clear_provider_api_key(
    state: State<'_, AppState>,
    provider_type: String,
) -> Result<(), String> {
    let provider_type = normalize_provider_type(&provider_type)?;
    let db = get_db(&state).await?;
    db.clear_ai_provider_api_key(&provider_type).await
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
        match db.get_ai_provider_by_id(provider_id).await {
            Ok(provider) => provider,
            Err(e) => {
                let msg = map_provider_lookup_error(&e);
                emit_ai_error(
                    &app,
                    request.request_id,
                    request.conversation_id,
                    msg.clone(),
                );
                return Err(msg);
            }
        }
    } else {
        match db.get_default_ai_provider().await {
            Ok(provider) => provider,
            Err(e) => {
                let msg = map_default_provider_error(&e);
                emit_ai_error(
                    &app,
                    request.request_id,
                    request.conversation_id,
                    msg.clone(),
                );
                return Err(msg);
            }
        }
    };

    if let Err(msg) = ensure_provider_enabled(provider_record.enabled) {
        emit_ai_error(
            &app,
            request.request_id,
            request.conversation_id,
            msg.clone(),
        );
        return Err(msg);
    }

    validate_conversation_requirement(request.conversation_id, create_if_missing)?;

    let api_key = db
        .decrypt_ai_api_key(&provider_record.api_key)
        .map_err(|_| {
            "AI provider apiKey is missing or invalid. Please re-save it in AI Provider settings."
                .to_string()
        })?;
    let provider = provider_from_model(provider_record.clone(), api_key);
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
        None => unreachable!("conversation requirement should be validated before this branch"),
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

    let mut schema_override: Option<AiSchemaOverview> = None;
    let mut selection_hint = String::new();
    if let (Some(conn_id), Some(selected)) =
        (request.connection_id, request.selected_tables.as_ref())
    {
        if !selected.is_empty() {
            let driver =
                super::ensure_connection_with_db(&state, conn_id, request.database.clone()).await?;
            let mut tables: Vec<AiTableSummary> = Vec::new();
            for t in selected {
                let structure = driver
                    .get_table_structure(t.schema.clone(), t.name.clone())
                    .await?;
                let columns = structure
                    .columns
                    .into_iter()
                    .map(|c| AiColumnSummary {
                        name: c.name,
                        column_type: c.r#type,
                        nullable: Some(c.nullable),
                    })
                    .collect();
                tables.push(AiTableSummary {
                    schema: t.schema.clone(),
                    name: t.name.clone(),
                    columns,
                });
            }
            selection_hint = selected
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            schema_override = Some(AiSchemaOverview { tables });
        }
    }

    let input_for_prompt = if selection_hint.is_empty() {
        request.input.clone()
    } else {
        format!("{} {}", request.input, selection_hint)
    };

    let bundle = build_prompt_bundle(
        &request.scenario,
        &input_for_prompt,
        schema_override
            .as_ref()
            .or_else(|| request.schema_overview.as_ref()),
    );

    let mut history: Vec<AiChatMessage> = Vec::new();
    let mut existing = match db.list_ai_messages(conversation.id).await {
        Ok(messages) => messages,
        Err(e) => {
            let client_error = map_history_load_error(conversation.id, &e);
            emit_ai_error(
                &app,
                request.request_id.clone(),
                Some(conversation.id),
                client_error.clone(),
            );
            return Err(client_error);
        }
    };
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
        "ai/started",
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
                "ai/chunk",
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
        "ai/done",
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

#[cfg(test)]
mod tests {
    use super::{
        ensure_provider_enabled, map_default_provider_error, map_history_load_error,
        map_provider_lookup_error, normalize_provider_type, validate_conversation_requirement,
    };

    #[test]
    fn normalize_provider_type_rejects_empty_value() {
        assert_eq!(
            normalize_provider_type("   ").unwrap_err(),
            "providerType is required"
        );
    }

    #[test]
    fn normalize_provider_type_maps_openai_compat_to_openai() {
        assert_eq!(
            normalize_provider_type("OpenAI_Compat").unwrap(),
            "openai".to_string()
        );
    }

    #[test]
    fn normalize_provider_type_rejects_invalid_chars() {
        assert_eq!(
            normalize_provider_type("bad type!").unwrap_err(),
            "providerType has invalid format"
        );
    }

    #[test]
    fn normalize_provider_type_accepts_supported_chars() {
        assert_eq!(
            normalize_provider_type("x.y-z_1").unwrap(),
            "x.y-z_1".to_string()
        );
    }

    #[test]
    fn provider_lookup_error_maps_not_found_to_user_friendly_message() {
        assert_eq!(
            map_provider_lookup_error("[GET_AI_PROVIDER_ERROR] row not found"),
            "Selected AI provider does not exist"
        );
    }

    #[test]
    fn default_provider_error_maps_no_enabled_provider_to_user_friendly_message() {
        assert_eq!(
            map_default_provider_error("[NO_ENABLED_AI_PROVIDER] nothing configured"),
            "No enabled AI provider is configured. Please enable one in AI Provider settings."
        );
    }

    #[test]
    fn ensure_provider_enabled_rejects_disabled_provider() {
        assert_eq!(
            ensure_provider_enabled(false).unwrap_err(),
            "Selected AI provider is disabled"
        );
    }

    #[test]
    fn continue_requires_conversation_id() {
        assert_eq!(
            validate_conversation_requirement(None, false).unwrap_err(),
            "conversationId is required"
        );
    }

    #[test]
    fn history_load_error_maps_to_client_message() {
        assert_eq!(
            map_history_load_error(42, "[DB_ERROR] broken"),
            "Failed to load conversation history"
        );
    }
}
