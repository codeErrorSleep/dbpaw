import { describe, expect, test } from "bun:test";
import {
  formatInsertSQLValue,
  formatSQLValue,
  getQualifiedTableName,
  isInsertColumnRequired,
} from "./utils";

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

describe("formatInsertSQLValue", () => {
  test("supports NULL literal", () => {
    expect(
      formatInsertSQLValue("null", { name: "memo", type: "text" }, "postgres"),
    ).toBe("NULL");
  });

  test("formats numeric values by column type", () => {
    expect(
      formatInsertSQLValue("123", { name: "age", type: "integer" }, "postgres"),
    ).toBe("123");
    expect(
      formatInsertSQLValue(
        "-45.67",
        { name: "price", type: "numeric(10,2)" },
        "postgres",
      ),
    ).toBe("-45.67");
  });

  test("throws for invalid numeric values", () => {
    expect(() =>
      formatInsertSQLValue("12a", { name: "age", type: "integer" }, "postgres"),
    ).toThrow('Invalid numeric value for column "age": "12a"');
  });

  test("formats boolean values", () => {
    expect(
      formatInsertSQLValue("true", { name: "enabled", type: "boolean" }, "postgres"),
    ).toBe("TRUE");
    expect(
      formatInsertSQLValue("0", { name: "enabled", type: "boolean" }, "mssql"),
    ).toBe("0");
  });

  test("throws for invalid boolean values", () => {
    expect(() =>
      formatInsertSQLValue("yes", { name: "enabled", type: "boolean" }, "postgres"),
    ).toThrow('Invalid boolean value for column "enabled": "yes"');
  });

  test("quotes non-numeric and non-boolean values", () => {
    expect(
      formatInsertSQLValue("alice", { name: "name", type: "varchar" }, "mysql"),
    ).toBe("'alice'");
  });
});

describe("isInsertColumnRequired", () => {
  test("returns true for NOT NULL without default", () => {
    expect(
      isInsertColumnRequired({ nullable: false, defaultValue: null }),
    ).toBe(true);
  });

  test("returns false when nullable", () => {
    expect(
      isInsertColumnRequired({ nullable: true, defaultValue: null }),
    ).toBe(false);
  });

  test("returns false when default value exists", () => {
    expect(
      isInsertColumnRequired({
        nullable: false,
        defaultValue: "CURRENT_TIMESTAMP",
      }),
    ).toBe(false);
  });
});

describe("getQualifiedTableName", () => {
  test("does not qualify sqlite main/public schema", () => {
    expect(getQualifiedTableName("sqlite", "main", "users")).toBe("\"users\"");
    expect(getQualifiedTableName("sqlite", "public", "users")).toBe("\"users\"");
    expect(getQualifiedTableName("sqlite", "", "users")).toBe("\"users\"");
  });

  test("keeps non-main sqlite schema qualification", () => {
    expect(getQualifiedTableName("sqlite", "analytics", "events")).toBe(
      "\"analytics\".\"events\"",
    );
  });
});
