import { describe, expect, test } from "bun:test";
import {
  allowsHostWithPort,
  isFileBasedDriver,
  isMysqlFamilyDriver,
  normalizeConnectionFormInput,
  normalizePortNumber,
  normalizeTextValue,
  parseHostEmbeddedPort,
  requiresPasswordOnCreate,
} from "./rules";

describe("isMysqlFamilyDriver", () => {
  test("recognizes mysql family", () => {
    expect(isMysqlFamilyDriver("mysql")).toBe(true);
    expect(isMysqlFamilyDriver("mariadb")).toBe(true);
    expect(isMysqlFamilyDriver("tidb")).toBe(true);
    expect(isMysqlFamilyDriver("starrocks")).toBe(true);
  });

  test("rejects non-mysql drivers", () => {
    expect(isMysqlFamilyDriver("postgres")).toBe(false);
    expect(isMysqlFamilyDriver("sqlite")).toBe(false);
  });
});

describe("isFileBasedDriver", () => {
  test("recognizes file-based drivers", () => {
    expect(isFileBasedDriver("sqlite")).toBe(true);
    expect(isFileBasedDriver("duckdb")).toBe(true);
  });

  test("rejects network drivers", () => {
    expect(isFileBasedDriver("mysql")).toBe(false);
    expect(isFileBasedDriver("postgres")).toBe(false);
  });
});

describe("allowsHostWithPort / requiresPasswordOnCreate", () => {
  test("only mysql family allows host:port notation", () => {
    expect(allowsHostWithPort("mysql")).toBe(true);
    expect(allowsHostWithPort("starrocks")).toBe(true);
    expect(allowsHostWithPort("postgres")).toBe(false);
  });

  test("non-mysql drivers require password on create", () => {
    expect(requiresPasswordOnCreate("postgres")).toBe(true);
    expect(requiresPasswordOnCreate("mysql")).toBe(false);
    expect(requiresPasswordOnCreate("starrocks")).toBe(false);
  });
});

describe("normalizeTextValue", () => {
  test("returns undefined for undefined and null", () => {
    expect(normalizeTextValue(undefined)).toBeUndefined();
    expect(normalizeTextValue(null as any)).toBeUndefined();
  });

  test("returns undefined for blank strings when emptyToUndefined is true (default)", () => {
    expect(normalizeTextValue("")).toBeUndefined();
    expect(normalizeTextValue("   ")).toBeUndefined();
  });

  test("returns empty string for blank when emptyToUndefined is false", () => {
    expect(normalizeTextValue("", false)).toBe("");
    expect(normalizeTextValue("  ", false)).toBe("");
  });

  test("trims surrounding whitespace", () => {
    expect(normalizeTextValue("  hello  ")).toBe("hello");
  });
});

describe("parseHostEmbeddedPort", () => {
  test("parses host:port when valid", () => {
    expect(parseHostEmbeddedPort("127.0.0.1:3306", undefined)).toEqual({
      host: "127.0.0.1",
      port: 3306,
    });
  });

  test("prefers embedded port over fallback port", () => {
    expect(parseHostEmbeddedPort("127.0.0.1:3307", 3306)).toEqual({
      host: "127.0.0.1",
      port: 3307,
    });
  });

  test("keeps host and fallback port when no port provided", () => {
    expect(parseHostEmbeddedPort("localhost", 5432)).toEqual({
      host: "localhost",
      port: 5432,
    });
  });

  test("does not parse ipv6 or whitespace hosts", () => {
    expect(parseHostEmbeddedPort("[::1]:3306", 5432)).toEqual({
      host: "[::1]:3306",
      port: 5432,
    });
    expect(parseHostEmbeddedPort("local host:3306", 5432)).toEqual({
      host: "local host:3306",
      port: 5432,
    });
  });

  test("does not parse invalid port", () => {
    expect(parseHostEmbeddedPort("db:abc", 3306)).toEqual({
      host: "db:abc",
      port: 3306,
    });
    expect(parseHostEmbeddedPort("db:3306:1", 3306)).toEqual({
      host: "db:3306:1",
      port: 3306,
    });
  });
});

describe("normalizePortNumber", () => {
  test("returns undefined for invalid values", () => {
    expect(normalizePortNumber(undefined)).toBeUndefined();
    expect(normalizePortNumber(null as unknown as number)).toBeUndefined();
    expect(normalizePortNumber(Number.NaN)).toBeUndefined();
    expect(normalizePortNumber(3306.5)).toBeUndefined();
  });

  test("keeps valid integers", () => {
    expect(normalizePortNumber(3306)).toBe(3306);
  });
});

describe("normalizeConnectionFormInput", () => {
  test("splits host:port for mysql family drivers", () => {
    const normalized = normalizeConnectionFormInput({
      driver: "mysql",
      host: " db:3306 ",
      port: undefined,
      name: "  test  ",
      password: "",
    } as any);

    expect(normalized.host).toBe("db");
    expect(normalized.port).toBe(3306);
    expect(normalized.name).toBe("test");
    expect(normalized.password).toBe("");
  });

  test("uses embedded host port even when a default port is already set", () => {
    const normalized = normalizeConnectionFormInput({
      driver: "starrocks",
      host: " db:9031 ",
      port: 9030,
      password: "",
    } as any);

    expect(normalized.host).toBe("db");
    expect(normalized.port).toBe(9031);
  });

  test("does not split host:port for non-mysql drivers", () => {
    const normalized = normalizeConnectionFormInput({
      driver: "postgres",
      host: "db:3306",
      port: 5432,
      password: "",
    } as any);

    expect(normalized.host).toBe("db:3306");
    expect(normalized.port).toBe(5432);
  });

  test("trims text values and keeps empty secrets", () => {
    const normalized = normalizeConnectionFormInput({
      driver: "postgres",
      host: "  db  ",
      database: "  test_db ",
      password: " ",
      sslCaCert: "",
      sshPassword: "",
    } as any);

    expect(normalized.host).toBe("db");
    expect(normalized.database).toBe("test_db");
    expect(normalized.password).toBe("");
    expect(normalized.sslCaCert).toBe("");
    expect(normalized.sshPassword).toBe("");
  });
});
