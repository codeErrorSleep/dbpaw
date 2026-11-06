import type { ConnectionForm } from "@/services/api";
import {
  allowsHostWithPort,
  getRedisConnectionMode,
  isFileBasedDriver,
  requiresPasswordOnCreate,
  requiresUsername,
} from "./rules";

import { validateNetworkProxy } from "./proxy";

type Mode = "create" | "edit";

export interface ConnectionValidationIssue {
  key: string;
}

const hasWhitespace = (value: string | undefined) =>
  !!value && /\s/.test(value);

const isPortInRange = (value: number | undefined) =>
  Number.isInteger(value) &&
  (value as number) >= 1 &&
  (value as number) <= 65535;

const hasAtLeastOneValue = (values: string[] | undefined) =>
  !!values?.some((value) => value.trim().length > 0);

export const validateConnectionFormInput = (
  form: ConnectionForm,
  mode: Mode,
): ConnectionValidationIssue[] => {
  const issues: ConnectionValidationIssue[] = validateNetworkProxy(form);

  if (isFileBasedDriver(form.driver)) {
    if (!form.filePath) {
      issues.push({
        key: "connection.dialog.inputValidation.filePathRequired",
      });
    }
    return issues;
  }

  if (form.driver === "redis") {
    const mode = getRedisConnectionMode(form);
    if (mode === "standalone") {
      if (!form.host) {
        issues.push({ key: "connection.dialog.inputValidation.hostRequired" });
      }
      if (!isPortInRange(form.port)) {
        issues.push({ key: "connection.dialog.inputValidation.portRange" });
      }
    }
    if (mode === "cluster") {
      if (
        (form.seedNodes?.filter((value) => value.trim().length > 0).length ??
          0) < 2
      ) {
        issues.push({
          key: "connection.dialog.inputValidation.redisSeedNodesRequired",
        });
      }
    }
    if (mode === "sentinel" && !hasAtLeastOneValue(form.sentinels)) {
      issues.push({
        key: "connection.dialog.inputValidation.redisSentinelsRequired",
      });
    }
    if (
      form.connectTimeoutMs !== undefined &&
      (!Number.isInteger(form.connectTimeoutMs) || form.connectTimeoutMs <= 0)
    ) {
      issues.push({
        key: "connection.dialog.inputValidation.redisConnectTimeoutRange",
      });
    }
    if (hasWhitespace(form.host)) {
      issues.push({ key: "connection.dialog.inputValidation.hostWhitespace" });
    }
    return issues;
  }

  if (form.driver === "elasticsearch") {
    if (!form.host && !form.cloudId) {
      issues.push({
        key: "connection.dialog.inputValidation.elasticsearchEndpointRequired",
      });
    }
    if (!form.cloudId && !isPortInRange(form.port)) {
      issues.push({ key: "connection.dialog.inputValidation.portRange" });
    }
    if (form.authMode === "basic" && !form.username) {
      issues.push({
        key: "connection.dialog.inputValidation.usernameRequired",
      });
    }
    if (mode === "create" && form.authMode === "api_key") {
      const hasEncoded = !!(form.apiKeyEncoded || "").trim();
      const hasIdSecret =
        !!(form.apiKeyId || "").trim() && !!(form.apiKeySecret || "").trim();
      if (!hasEncoded && !hasIdSecret) {
        issues.push({
          key: "connection.dialog.inputValidation.elasticsearchApiKeyRequired",
        });
      }
    }
    if (hasWhitespace(form.host)) {
      issues.push({ key: "connection.dialog.inputValidation.hostWhitespace" });
    }
    if (
      form.ssl &&
      form.sslMode === "verify_ca" &&
      !(form.sslCaCert || "").trim()
    ) {
      issues.push({ key: "connection.dialog.sslValidation.caRequired" });
    }
    return issues;
  }

  // MSSQL auth mode specific validation
  if (form.driver === "mssql") {
    const mssqlAuth = form.authMode || "sql_server";
    if (mssqlAuth === "sql_server" || mssqlAuth === "windows") {
      if (!form.username) {
        issues.push({
          key: "connection.dialog.inputValidation.usernameRequired",
        });
      }
      if (mode === "create" && !form.password) {
        issues.push({
          key: "connection.dialog.inputValidation.passwordRequired",
        });
      }
    }
    if (mssqlAuth === "aad_token") {
      if (mode === "create" && !form.password) {
        issues.push({
          key: "connection.dialog.inputValidation.passwordRequired",
        });
      }
    }
    // integrated: no username/password required
  }

  if (!form.host) {
    issues.push({ key: "connection.dialog.inputValidation.hostRequired" });
  }
  if (
    form.driver !== "mssql" &&
    requiresUsername(form.driver) &&
    !form.username
  ) {
    issues.push({ key: "connection.dialog.inputValidation.usernameRequired" });
  }
  if (!isPortInRange(form.port)) {
    issues.push({ key: "connection.dialog.inputValidation.portRange" });
  }

  if (
    form.driver !== "mssql" &&
    mode === "create" &&
    requiresPasswordOnCreate(form.driver) &&
    !form.password
  ) {
    issues.push({ key: "connection.dialog.inputValidation.passwordRequired" });
  }

  if (hasWhitespace(form.host)) {
    issues.push({ key: "connection.dialog.inputValidation.hostWhitespace" });
  }

  if (
    form.host &&
    form.host.includes(":") &&
    !allowsHostWithPort(form.driver) &&
    !form.host.startsWith("[")
  ) {
    issues.push({
      key: "connection.dialog.inputValidation.hostPortNotAllowed",
    });
  }

  if (
    form.ssl &&
    form.sslMode === "verify_ca" &&
    !(form.sslCaCert || "").trim()
  ) {
    issues.push({ key: "connection.dialog.sslValidation.caRequired" });
  }

  if (form.sshEnabled) {
    if (!form.sshHost) {
      issues.push({ key: "connection.dialog.inputValidation.sshHostRequired" });
    }
    if (!form.sshUsername) {
      issues.push({
        key: "connection.dialog.inputValidation.sshUsernameRequired",
      });
    }
    if (!isPortInRange(form.sshPort)) {
      issues.push({ key: "connection.dialog.inputValidation.sshPortRange" });
    }
    if (!form.sshPassword && !form.sshKeyPath) {
      issues.push({ key: "connection.dialog.inputValidation.sshAuthRequired" });
    }
  }

  return issues;
};
