import { describe, expect, test } from "bun:test";
import { formatSQLValue } from "./utils";

describe("formatSQLValue", () => {
  test("uses numeric boolean literals for mssql", () => {
    expect(formatSQLValue("true", true, "execution", "mssql")).toBe("1");
    expect(formatSQLValue("false", true, "execution", "mssql")).toBe("0");
    expect(formatSQLValue("1", true, "execution", "mssql")).toBe("1");
    expect(formatSQLValue("0", true, "execution", "mssql")).toBe("0");
  });

  test("keeps TRUE/FALSE for non-mssql drivers", () => {
    expect(formatSQLValue("true", true, "execution", "postgres")).toBe("TRUE");
    expect(formatSQLValue("false", true, "execution", "mysql")).toBe("FALSE");
  });

  test("throws for invalid boolean in execution mode", () => {
    expect(() => formatSQLValue("yes", true, "execution", "mssql")).toThrow(
      'Invalid boolean value: "yes"',
    );
  });
});
