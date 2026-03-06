import type { Completion } from "@codemirror/autocomplete";

const CLICKHOUSE_KEYWORDS = [
  "PREWHERE",
  "FINAL",
  "SAMPLE",
  "ARRAY JOIN",
  "LIMIT BY",
  "SETTINGS",
  "TTL",
  "MATERIALIZED VIEW",
  "LIVE VIEW",
  "OPTIMIZE TABLE",
  "ALTER TABLE",
  "PARTITION BY",
  "ORDER BY",
  "PRIMARY KEY",
  "WITH FILL",
  "FORMAT JSON",
  "FORMAT CSV",
  "FORMAT TSV",
];

const CLICKHOUSE_FUNCTIONS = [
  "toDate",
  "toDateTime",
  "toStartOfDay",
  "toStartOfHour",
  "now",
  "today",
  "yesterday",
  "countIf",
  "sumIf",
  "avgIf",
  "uniq",
  "uniqExact",
  "argMax",
  "argMin",
  "arrayJoin",
  "groupArray",
  "groupUniqArray",
  "ifNull",
  "coalesce",
];

const keywordCompletions: Completion[] = CLICKHOUSE_KEYWORDS.map((label) => ({
  label,
  type: "keyword",
  detail: "ClickHouse",
  boost: 1,
}));

const functionCompletions: Completion[] = CLICKHOUSE_FUNCTIONS.map((label) => ({
  label,
  type: "function",
  detail: "ClickHouse",
  boost: 1,
}));

export const CLICKHOUSE_COMPLETIONS: Completion[] = [
  ...keywordCompletions,
  ...functionCompletions,
];
