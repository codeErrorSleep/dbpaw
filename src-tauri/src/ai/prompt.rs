use super::types::{AiPromptBundle, AiSchemaOverview, AiTableSummary, AiChatMessage};

const PROMPT_VERSION: &str = "v1.0.0";
const MAX_TABLES: usize = 8;
const MAX_COLUMNS: usize = 12;

pub fn build_prompt_bundle(
    scenario: &str,
    input: &str,
    schema_overview: Option<&AiSchemaOverview>,
) -> AiPromptBundle {
    let selected = select_tables(input, schema_overview);
    let schema_text = render_schema_summary(&selected);

    let system = AiChatMessage {
        role: "system".to_string(),
        content: "You are an expert SQL assistant. Follow user requirements accurately, prioritize correctness, and avoid destructive statements unless explicitly requested.".to_string(),
    };

    // Many OpenAI-compatible providers only support system/user/assistant roles.
    // Keep template instructions in a second system message for compatibility.
    let developer = AiChatMessage {
        role: "system".to_string(),
        content: build_template(scenario, &schema_text),
    };

    AiPromptBundle {
        prompt_version: PROMPT_VERSION.to_string(),
        messages: vec![system, developer],
    }
}

fn build_template(scenario: &str, schema_summary: &str) -> String {
    let base_rules = "Rules:\n1. Return concise output.\n2. If scenario is sql_generate or sql_optimize, output only SQL without markdown fences.\n3. Prefer safe SQL and explain assumptions briefly only for sql_explain.";

    let scenario_rules = match scenario {
        "sql_explain" => {
            "Task: explain the SQL clearly with execution intent and optimization opportunities."
        }
        "sql_optimize" => {
            "Task: optimize the SQL for performance while keeping semantics unchanged. Return SQL only."
        }
        _ => "Task: generate SQL from natural language. Return SQL only.",
    };

    format!(
        "{base_rules}\n{scenario_rules}\n\nAvailable schema summary:\n{schema_summary}",
    )
}

fn select_tables(input: &str, schema_overview: Option<&AiSchemaOverview>) -> Vec<AiTableSummary> {
    let Some(overview) = schema_overview else {
        return vec![];
    };

    let tokens: Vec<String> = input
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .collect();

    let mut scored: Vec<(i32, &AiTableSummary)> = overview
        .tables
        .iter()
        .map(|t| {
            let table_name = t.name.to_lowercase();
            let mut score = 0;
            for tk in &tokens {
                if table_name.contains(tk) {
                    score += 5;
                }
                for c in &t.columns {
                    if c.name.to_lowercase().contains(tk) {
                        score += 1;
                    }
                }
            }
            (score, t)
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));

    let mut selected = Vec::new();
    for (idx, (_, table)) in scored.iter().enumerate() {
        if idx >= MAX_TABLES {
            break;
        }
        if idx > 0 && selected.is_empty() {
            break;
        }
        if idx > 0 && scored[idx].0 <= 0 {
            continue;
        }
        let mut copy = (*table).clone();
        if copy.columns.len() > MAX_COLUMNS {
            copy.columns = copy.columns.into_iter().take(MAX_COLUMNS).collect();
        }
        selected.push(copy);
    }

    if selected.is_empty() {
        selected = overview
            .tables
            .iter()
            .take(MAX_TABLES)
            .map(|t| {
                let mut copy = t.clone();
                if copy.columns.len() > MAX_COLUMNS {
                    copy.columns = copy.columns.into_iter().take(MAX_COLUMNS).collect();
                }
                copy
            })
            .collect();
    }

    selected
}

fn render_schema_summary(tables: &[AiTableSummary]) -> String {
    if tables.is_empty() {
        return "(No schema provided)".to_string();
    }

    let mut out = String::new();
    for table in tables {
        out.push_str(&format!("- {}.{}: ", table.schema, table.name));
        let cols = table
            .columns
            .iter()
            .map(|c| format!("{}:{}", c.name, c.column_type))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&cols);
        out.push('\n');
    }
    out
}
