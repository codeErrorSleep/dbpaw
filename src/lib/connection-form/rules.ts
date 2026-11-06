import type {
  ConnectionForm,
  Driver,
  RedisConnectionMode,
} from "@/services/api";
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
      showHost: false,
      showPort: false,
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

  if (driver === "elasticsearch") {
    return {
      showHost: true,
      showPort: true,
      showUsername: true,
      showPassword: true,
      showDatabase: false,
      showSchema: false,
      showSsl: true,
      showSsh: true,
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
  mode: driver === "redis" ? "standalone" : undefined,
  seedNodes: driver === "redis" ? [] : undefined,
  sentinels: driver === "redis" ? [] : undefined,
  connectTimeoutMs: driver === "redis" ? 5000 : undefined,
  serviceName: driver === "redis" ? "" : undefined,
  sentinelPassword: driver === "redis" ? "" : undefined,
  authMode:
    driver === "elasticsearch"
      ? "none"
      : driver === "mssql"
        ? "sql_server"
        : undefined,
  apiKeyId: "",
  apiKeySecret: "",
  apiKeyEncoded: "",
  cloudId: "",
  authSource: driver === "mongodb" ? "" : undefined,
  ...overrides,
});

export const allowsHostWithPort = (driver: Driver) =>
  isMysqlFamilyDriver(driver) ||
  driver === "redis" ||
  driver === "elasticsearch";

export const requiresPasswordOnCreate = (driver: Driver) =>
  !isMysqlFamilyDriver(driver) &&
  driver !== "redis" &&
  driver !== "elasticsearch";

export const requiresUsername = (driver: Driver) =>
  driver !== "redis" && driver !== "elasticsearch";

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

export const normalizeStringList = (values: string[] | undefined) => {
  if (!values) {
    return undefined;
  }
  const normalized = values
    .map((value) => value.trim())
    .filter(
      (value, index, items) =>
        value.length > 0 && items.indexOf(value) === index,
    );
  return normalized.length > 0 ? normalized : undefined;
};

export const normalizeRedisNodeListInput = (value: string | undefined) =>
  normalizeStringList((value || "").split(/[\n,]/).map((item) => item.trim()));

export const formatRedisNodeList = (values: string[] | undefined) =>
  values?.join("\n") ?? "";

export const getRedisConnectionMode = (
  form: Pick<ConnectionForm, "driver" | "mode" | "host" | "seedNodes">,
): RedisConnectionMode => {
  if (form.driver !== "redis") {
    return "standalone";
  }
  if (
    form.mode === "standalone" ||
    form.mode === "cluster" ||
    form.mode === "sentinel"
  ) {
    return form.mode;
  }
  const seedNodes = normalizeStringList(form.seedNodes);
  if ((seedNodes?.length ?? 0) > 1) {
    return "cluster";
  }
  const host = normalizeTextValue(form.host);
  if (host?.includes(",")) {
    return "cluster";
  }
  return "standalone";
};

export const normalizeConnectionFormInput = (
  raw: ConnectionForm,
): ConnectionForm => {
  const driver = raw.driver;
  const redisMode = getRedisConnectionMode(raw);
  const normalizedHost = normalizeTextValue(raw.host);
  const normalizedPort = normalizePortNumber(raw.port);
  const hostPortNormalized =
    allowsHostWithPort(driver) &&
    normalizedHost &&
    !(driver === "redis" && redisMode !== "standalone")
      ? parseHostEmbeddedPort(normalizedHost, normalizedPort)
      : { host: normalizedHost, port: normalizedPort };
  const seedNodes =
    driver === "redis"
      ? (normalizeStringList(raw.seedNodes) ??
        normalizeRedisNodeListInput(
          redisMode === "cluster"
            ? hostPortNormalized.host
            : hostPortNormalized.host && hostPortNormalized.port
              ? `${hostPortNormalized.host}:${hostPortNormalized.port}`
              : hostPortNormalized.host,
        ))
      : undefined;
  const sentinels =
    driver === "redis" ? normalizeStringList(raw.sentinels) : undefined;
  const normalizedTimeout =
    driver === "redis"
      ? normalizePortNumber(raw.connectTimeoutMs)
      : raw.connectTimeoutMs;
  const fallbackSeedNode =
    driver === "redis" &&
    redisMode === "standalone" &&
    hostPortNormalized.host &&
    hostPortNormalized.port
      ? `${hostPortNormalized.host}:${hostPortNormalized.port}`
      : hostPortNormalized.host;

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
    authMode:
      driver === "elasticsearch"
        ? raw.authMode === "basic" || raw.authMode === "api_key"
          ? raw.authMode
          : "none"
        : driver === "mssql"
          ? raw.authMode === "sql_server" ||
            raw.authMode === "windows" ||
            raw.authMode === "integrated" ||
            raw.authMode === "aad_token"
            ? raw.authMode
            : "sql_server"
          : undefined,
    apiKeyId: normalizeTextValue(raw.apiKeyId),
    apiKeySecret: normalizeTextValue(raw.apiKeySecret, false),
    apiKeyEncoded: normalizeTextValue(raw.apiKeyEncoded, false),
    cloudId: normalizeTextValue(raw.cloudId),
    authSource:
      driver === "mongodb" ? normalizeTextValue(raw.authSource) : undefined,
    filePath: normalizeTextValue(raw.filePath),
    sshHost: normalizeTextValue(raw.sshHost),
    sshPort: normalizePortNumber(raw.sshPort),
    sshUsername: normalizeTextValue(raw.sshUsername),
    sshPassword: normalizeTextValue(raw.sshPassword, false),
    sshKeyPath: normalizeTextValue(raw.sshKeyPath),
    proxy: raw.proxy ? {
      ...raw.proxy,
      host: raw.proxy.host.trim(),
      username: raw.proxy.username.trim(),
    } : undefined,
    mode: driver === "redis" ? redisMode : undefined,
    seedNodes:
      driver === "redis"
        ? redisMode === "standalone"
          ? normalizeStringList([
              ...(seedNodes ?? []),
              ...(fallbackSeedNode ? [fallbackSeedNode] : []),
            ])
          : seedNodes
        : undefined,
    sentinels,
    connectTimeoutMs: normalizedTimeout,
    serviceName:
      driver === "redis"
        ? (normalizeTextValue(raw.serviceName) ?? "")
        : undefined,
    sentinelPassword:
      driver === "redis"
        ? normalizeTextValue(raw.sentinelPassword, false)
        : undefined,
  };
};
