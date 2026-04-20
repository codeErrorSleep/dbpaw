export const CUSTOM_TYPE_SENTINEL = "__custom__";

const TEXT_BLOB_RE = /^(tiny|medium|long)?(text|blob)$/i;

export function isTextBlobType(dataType: string): boolean {
  return TEXT_BLOB_RE.test(dataType.trim().split("(")[0]);
}

export function splitSqlStatements(sql: string): string[] {
  return sql
    .split(/;[ \t]*\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => (s.endsWith(";") ? s : s + ";"));
}

export function columnGridTemplate(showAutoIncrement: boolean): string {
  // grip | name | type | length | NN | PK | [AI] | default | comment | actions
  return showAutoIncrement
    ? "20px 1fr 1.4fr 80px 56px 40px 40px 100px 120px 64px"
    : "20px 1fr 1.4fr 80px 56px 40px 100px 120px 64px";
}

export function indexGridTemplate(showMethod: boolean): string {
  return showMethod ? "1fr 56px 2fr 120px 40px" : "1fr 56px 2fr 40px";
}
