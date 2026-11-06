import { describe, expect, test } from "bun:test";
import {
  DEFAULT_NETWORK_PROXY,
  supportsNetworkProxy,
  validateNetworkProxy,
} from "./proxy";
import { normalizeConnectionFormInput } from "./rules";
import { validateConnectionFormInput } from "./validate";
import type { ConnectionForm } from "@/services/api";

const form = (overrides: Partial<ConnectionForm> = {}): ConnectionForm => ({
  driver: "mysql",
  host: "db.invalid",
  port: 3306,
  username: "root",
  proxy: { ...DEFAULT_NETWORK_PROXY, enabled: true },
  ...overrides,
});

describe("network proxy validation", () => {
  test("validates before driver-specific early returns", () => {
    for (const driver of ["mysql", "elasticsearch", "redis"] as const) {
      const input = form({ driver });
      input.proxy!.port = 70000;
      expect(
        validateConnectionFormInput(input, "edit").map((issue) => issue.key),
      ).toContain("connection.dialog.proxy.portInvalid");
    }
  });
  test("normalizes addresses while preserving password whitespace", () => {
    const input = form();
    Object.assign(input.proxy!, {
      host: " 127.0.0.1 ",
      username: " user ",
      password: " pass ",
    });
    const normalized = normalizeConnectionFormInput(input);
    expect(normalized.proxy).toMatchObject({
      host: "127.0.0.1",
      username: "user",
      password: " pass ",
    });
    expect(validateNetworkProxy(normalized)).toEqual([]);
  });
  test("rejects schemes, invalid ports, missing username and SSH conflicts", () => {
    const input = form({ sshEnabled: true });
    Object.assign(input.proxy!, {
      host: "http://localhost",
      port: 1.5,
      password: "pass",
    });
    expect(validateNetworkProxy(input).map((issue) => issue.key)).toEqual([
      "connection.dialog.proxy.sshConflict",
      "connection.dialog.proxy.hostInvalid",
      "connection.dialog.proxy.portInvalid",
      "connection.dialog.proxy.authInvalid",
    ]);
  });
  test("measures SOCKS5 credentials in bytes", () => {
    const input = form();
    input.proxy!.username = "名".repeat(86);
    expect(validateNetworkProxy(input).map((issue) => issue.key)).toContain(
      "connection.dialog.proxy.authInvalid",
    );
  });
  test("reports unsupported TLS forwarding without weakening certificate validation", () => {
    for (const driver of ["mongodb", "redis"] as const) {
      expect(
        validateNetworkProxy(form({ driver, ssl: true })).map(
          (issue) => issue.key,
        ),
      ).toContain("connection.dialog.proxy.tlsUnsupported");
    }
  });
  test("hides unavailable routes and ignores disabled proxy fields", () => {
    for (const input of [
      form({ driver: "sqlite" }),
      form({ driver: "cassandra" }),
      form({ driver: "redis", mode: "cluster" }),
      form({ driver: "redis", mode: "sentinel" }),
    ]) {
      expect(supportsNetworkProxy(input)).toBe(false);
      expect(validateNetworkProxy(input).map((issue) => issue.key)).toContain(
        "connection.dialog.proxy.unsupported",
      );
      input.proxy!.enabled = false;
      expect(validateNetworkProxy(input)).toEqual([]);
    }
  });
});
