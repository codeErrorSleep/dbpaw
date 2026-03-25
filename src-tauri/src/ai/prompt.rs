use super::types::{AiChatMessage, AiPromptBundle, AiSchemaOverview, AiTableSummary};

const PROMPT_VERSION: &str = "v2.0.0";
const MAX_TABLES: usize = 8;
const MAX_COLUMNS: usize = 12;
const MAX_SCHEMA_CHARS: usize = 6000;

/// Build a minimal prompt bundle with system context only.
/// User and assistant turns should come from persisted conversation history.
pub fn build_prompt_bundle(
    _scenario: &str,
    input: &str,
    schema_overview: Option<&AiSchemaOverview>,
) -> AiPromptBundle {
    let selected = select_tables(input, schema_overview);
    let schema_text = render_schema_summary(&selected);

    let mut content =
        "Use the conversation history as the source of truth for user and assistant turns."
            .to_string();

    let content = if schema_text.is_empty() || schema_text == "(No schema provided)" {
        content
    } else {
        content.push_str("\n\nDatabase schema:\n");
        content.push_str(&schema_text);
        content
    };

    AiPromptBundle {
        prompt_version: PROMPT_VERSION.to_string(),
        messages: vec![AiChatMessage {
            role: "system".to_string(),
            content,
        }],
    }
}

/// Select relevant tables based on keyword matching with user input.
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

    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.schema.cmp(&b.1.schema))
            .then_with(|| a.1.name.cmp(&b.1.name))
    });

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
        selected.push((*table).clone());
    }

    if selected.is_empty() {
        selected = overview.tables.iter().take(MAX_TABLES).cloned().collect();
    }

    selected
}

/// Render schema summary in a readable format.
fn render_schema_summary(tables: &[AiTableSummary]) -> String {
    if tables.is_empty() {
        return "(No schema provided)".to_string();
    }

    let mut out = String::new();
    let mut rendered_tables = 0usize;
    for table in tables.iter().take(MAX_TABLES) {
        if !out.is_empty() {
            out.push('\n');
        }

        out.push_str(&format!("{}.{}", table.schema, table.name));

        let mut cols = Vec::new();
        let mut truncated_cols = false;
        for (idx, c) in table.columns.iter().enumerate() {
            if idx >= MAX_COLUMNS {
                truncated_cols = true;
                break;
            }
            let mut piece = format!("{} ({}", c.name, c.column_type);
            if c.nullable.unwrap_or(false) {
                piece.push_str(", nullable");
            }
            piece.push(')');
            cols.push(piece);
        }

        out.push_str("\n  Columns: ");
        out.push_str(&cols.join(", "));
        if truncated_cols {
            out.push_str(", ... (truncated)");
        }
        out.push('\n');

        rendered_tables += 1;
        if out.len() >= MAX_SCHEMA_CHARS {
            out.truncate(MAX_SCHEMA_CHARS);
            out.push_str("\n... (truncated)");
            return out;
        }
    }

    if tables.len() > rendered_tables {
        if out.len() + 18 < MAX_SCHEMA_CHARS {
            out.push_str("\n... (truncated)");
        } else {
            out.truncate(MAX_SCHEMA_CHARS);
            out.push_str("\n... (truncated)");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::types;

    fn col(name: &str, ty: &str, nullable: Option<bool>) -> types::AiColumnSummary {
        types::AiColumnSummary {
            name: name.to_string(),
            column_type: ty.to_string(),
            nullable,
        }
    }

    fn table(schema: &str, name: &str, columns: Vec<types::AiColumnSummary>) -> AiTableSummary {
        AiTableSummary {
            schema: schema.to_string(),
            name: name.to_string(),
            columns,
        }
    }

    #[test]
    fn schema_format_matches_spec() {
        let users = table(
            "public",
            "users",
            vec![
                col("id", "int", Some(false)),
                col("email", "text", Some(true)),
            ],
        );
        let orders = table(
            "public",
            "orders",
            vec![
                col("id", "int", Some(false)),
                col("user_id", "int", Some(false)),
            ],
        );

        let out = render_schema_summary(&[users, orders]);
        assert!(out.contains("public.users\n  Columns: id (int), email (text, nullable)\n"));
        assert!(out.contains("public.orders\n  Columns: id (int), user_id (int)\n"));
    }

    #[test]
    fn schema_truncates_columns_deterministically() {
        let mut columns = Vec::new();
        for i in 0..(MAX_COLUMNS + 3) {
            columns.push(col(&format!("c{i}"), "text", Some(true)));
        }
        let t = table("public", "wide", columns);
        let out = render_schema_summary(&[t]);
        assert!(out.contains("... (truncated)\n"));
    }

    #[test]
    fn schema_truncates_tables_deterministically() {
        let mut tables = Vec::new();
        for i in 0..(MAX_TABLES + 2) {
            tables.push(table(
                "public",
                &format!("t{i}"),
                vec![col("id", "int", Some(false))],
            ));
        }
        let out = render_schema_summary(&tables);
        assert!(out.contains("... (truncated)"));
    }

    #[test]
    fn build_prompt_bundle_includes_schema() {
        let users = table(
            "public",
            "users",
            vec![
                col("id", "int", Some(false)),
                col("email", "text", Some(true)),
            ],
        );
        let overview = AiSchemaOverview {
            tables: vec![users],
        };

        let bundle = build_prompt_bundle("sql_generate", "List all users", Some(&overview));

        assert_eq!(bundle.messages.len(), 1);
        assert_eq!(bundle.messages[0].role, "system");
        assert!(bundle.messages[0]
            .content
            .contains("Use the conversation history as the source of truth"));
        assert!(bundle.messages[0].content.contains("Database schema:"));
        assert!(bundle.messages[0].content.contains("public.users"));
    }

    #[test]
    fn build_prompt_bundle_without_schema() {
        let bundle = build_prompt_bundle("sql_generate", "Hello", None);

        assert_eq!(bundle.messages.len(), 1);
        assert_eq!(bundle.messages[0].role, "system");
        assert_eq!(
            bundle.messages[0].content,
            "Use the conversation history as the source of truth for user and assistant turns."
        );
    }
}
