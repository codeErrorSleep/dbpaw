import { describe, expect, test } from "bun:test";
import { validateConnectionFormInput } from "./validate";

const baseForm = {
  driver: "postgres",
  host: "127.0.0.1",
  port: 5432,
  username: "user",
  password: "pwd",
};

describe("validateConnectionFormInput: happy paths", () => {
  test("valid postgres form returns no issues", () => {
    expect(validateConnectionFormInput(baseForm as any, "create")).toEqual([]);
  });

  test("valid sqlite form with filePath returns no issues", () => {
    expect(
      validateConnectionFormInput(
        { driver: "sqlite", filePath: "/data/app.db" } as any,
        "create",
      ),
    ).toEqual([]);
  });
});

describe("validateConnectionFormInput: required fields", () => {
  test("requires host for network drivers", () => {
    const issues = validateConnectionFormInput(
      { ...baseForm, host: "" } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.inputValidation.hostRequired",
    );
  });

  test("requires username for network drivers", () => {
    const issues = validateConnectionFormInput(
      { ...baseForm, username: "" } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.inputValidation.usernameRequired",
    );
  });

  test("rejects out-of-range port", () => {
    const issues = validateConnectionFormInput(
      { ...baseForm, port: 99999 } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.inputValidation.portRange",
    );
  });

  test("requires password on create for non-mysql drivers", () => {
    const issues = validateConnectionFormInput(
      { ...baseForm, password: "" } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.inputValidation.passwordRequired",
    );
  });

  test("does not require password for mysql family on create", () => {
    const keys = validateConnectionFormInput(
      { ...baseForm, driver: "mysql", password: "" } as any,
      "create",
    ).map((i) => i.key);
    expect(keys).not.toContain(
      "connection.dialog.inputValidation.passwordRequired",
    );
  });

  test("does not require username or password for redis on create", () => {
    const keys = validateConnectionFormInput(
      {
        ...baseForm,
        driver: "redis",
        username: "",
        password: "",
        port: 6379,
      } as any,
      "create",
    ).map((i) => i.key);
    expect(keys).not.toContain(
      "connection.dialog.inputValidation.usernameRequired",
    );
    expect(keys).not.toContain(
      "connection.dialog.inputValidation.passwordRequired",
    );
  });

  test("does not require password in edit mode", () => {
    const keys = validateConnectionFormInput(
      { ...baseForm, password: "" } as any,
      "edit",
    ).map((i) => i.key);
    expect(keys).not.toContain(
      "connection.dialog.inputValidation.passwordRequired",
    );
  });
});

describe("validateConnectionFormInput: host format", () => {
  test("rejects host with whitespace", () => {
    const issues = validateConnectionFormInput(
      { ...baseForm, host: "my host" } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.inputValidation.hostWhitespace",
    );
  });

  test("does not reject IPv6 bracket notation", () => {
    const keys = validateConnectionFormInput(
      { ...baseForm, host: "[::1]" } as any,
      "create",
    ).map((i) => i.key);
    expect(keys).not.toContain(
      "connection.dialog.inputValidation.hostPortNotAllowed",
    );
  });
});

describe("validateConnectionFormInput", () => {
  test("requires filePath for file-based drivers", () => {
    const issues = validateConnectionFormInput(
      { driver: "sqlite", filePath: "" } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.inputValidation.filePathRequired",
    );
  });

  test("validates ssh fields when sshEnabled", () => {
    const issues = validateConnectionFormInput(
      {
        ...baseForm,
        sshEnabled: true,
        sshHost: "",
        sshUsername: "",
        sshPort: 70000,
        sshPassword: "",
        sshKeyPath: "",
      } as any,
      "create",
    );

    const keys = issues.map((i) => i.key);
    expect(keys).toContain("connection.dialog.inputValidation.sshHostRequired");
    expect(keys).toContain(
      "connection.dialog.inputValidation.sshUsernameRequired",
    );
    expect(keys).toContain("connection.dialog.inputValidation.sshPortRange");
    expect(keys).toContain("connection.dialog.inputValidation.sshAuthRequired");
  });

  test("requires ssl CA cert when verify_ca", () => {
    const issues = validateConnectionFormInput(
      {
        ...baseForm,
        ssl: true,
        sslMode: "verify_ca",
        sslCaCert: "   ",
      } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.sslValidation.caRequired",
    );
  });

  test("rejects host:port for drivers that disallow it", () => {
    const issues = validateConnectionFormInput(
      {
        ...baseForm,
        host: "db:5432",
        driver: "postgres",
      } as any,
      "create",
    );
    expect(issues.map((i) => i.key)).toContain(
      "connection.dialog.inputValidation.hostPortNotAllowed",
    );
  });
});
