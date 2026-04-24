import type { ConnectionForm, Driver } from "@/services/api";
import {
  getDefaultPort,
  isFileBasedDriver,
  isMysqlFamilyDriver,
} from "@/lib/driver-registry";

export { isMysqlFamilyDriver, isFileBasedDriver };

export interface ConnectionFormCapabilities {
  showHost: boolean;
  showPort: boolean;
  showUsername: boolean;
  showPassword: boolean;
  showDatabase: boolean;
  showSchema: boolean;
  showSsl: boolean;
  showSsh: boolean;
  showFilePath: boolean;
  showSqliteKey: boolean;
}

export const getConnectionFormCapabilities = (
  driver: Driver,
): ConnectionFormCapabilities => {
  if (isFileBasedDriver(driver)) {
    return {
      showHost: false,
      showPort: false,
      showUsername: false,
      showPassword: driver === "sqlite",
      showDatabase: false,
      showSchema: false,
      showSsl: false,
      showSsh: false,
      showFilePath: true,
      showSqliteKey: driver === "sqlite",
    };
  }

  if (driver === "redis") {
    return {
      showHost: true,
      showPort: true,
      showUsername: true,
      showPassword: true,
      showDatabase: false,
      showSchema: false,
      showSsl: false,
      showSsh: false,
      showFilePath: false,
      showSqliteKey: false,
    };
  }

  return {
    showHost: true,
    showPort: true,
    showUsername: true,
    showPassword: true,
    showDatabase: true,
    showSchema:
      driver === "postgres" || driver === "mssql" || driver === "oracle",
    showSsl: true,
    showSsh: true,
    showFilePath: false,
    showSqliteKey: false,
  };
};

export const buildConnectionFormDefaults = (
  driver: Driver,
  overrides: Partial<ConnectionForm> = {},
): ConnectionForm => ({
  driver,
  name: "",
  host: "",
  port: getDefaultPort(driver) ?? undefined,
  database: "",
  schema: "",
  username: "",
  password: "",
  ssl: false,
  sslMode: "require",
  sslCaCert: "",
  filePath: "",
  sshEnabled: false,
  sshHost: "",
  sshPort: undefined,
  sshUsername: "",
  sshPassword: "",
  sshKeyPath: "",
  ...overrides,
});

export const allowsHostWithPort = (driver: Driver) =>
  isMysqlFamilyDriver(driver) || driver === "redis";

export const requiresPasswordOnCreate = (driver: Driver) =>
  !isMysqlFamilyDriver(driver) && driver !== "redis";

export const requiresUsername = (driver: Driver) => driver !== "redis";

export const normalizePortNumber = (value: number | undefined) => {
  if (value === undefined || value === null || !Number.isFinite(value)) {
    return undefined;
  }
  if (!Number.isInteger(value)) {
    return undefined;
  }
  return value;
};

export const normalizeTextValue = (
  value: string | undefined,
  emptyToUndefined = true,
) => {
  if (value === undefined || value === null) {
    return undefined;
  }
  const trimmed = value.trim();
  if (!trimmed && emptyToUndefined) {
    return undefined;
  }
  return trimmed;
};

export const parseHostEmbeddedPort = (
  host: string | undefined,
  fallbackPort: number | undefined,
) => {
  if (!host) {
    return { host, port: fallbackPort };
  }
  if (host.startsWith("[") || host.includes(" ")) {
    return { host, port: fallbackPort };
  }
  if (host.split(":").length !== 2) {
    return { host, port: fallbackPort };
  }
  const [hostPart, portPart] = host.split(":");
  if (!hostPart || !portPart || !/^\d+$/.test(portPart)) {
    return { host, port: fallbackPort };
  }
  return {
    host: hostPart,
    port: Number(portPart),
  };
};

export const normalizeConnectionFormInput = (
  raw: ConnectionForm,
): ConnectionForm => {
  const driver = raw.driver;
  const normalizedHost = normalizeTextValue(raw.host);
  const normalizedPort = normalizePortNumber(raw.port);
  const hostPortNormalized =
    allowsHostWithPort(driver) && normalizedHost
      ? parseHostEmbeddedPort(normalizedHost, normalizedPort)
      : { host: normalizedHost, port: normalizedPort };

  return {
    ...raw,
    name: normalizeTextValue(raw.name),
    host: hostPortNormalized.host,
    port: hostPortNormalized.port,
    database: normalizeTextValue(raw.database),
    schema: normalizeTextValue(raw.schema),
    username: normalizeTextValue(raw.username),
    password: normalizeTextValue(raw.password, false),
    sslCaCert: normalizeTextValue(raw.sslCaCert, false),
    filePath: normalizeTextValue(raw.filePath),
    sshHost: normalizeTextValue(raw.sshHost),
    sshPort: normalizePortNumber(raw.sshPort),
    sshUsername: normalizeTextValue(raw.sshUsername),
    sshPassword: normalizeTextValue(raw.sshPassword, false),
    sshKeyPath: normalizeTextValue(raw.sshKeyPath),
  };
};
