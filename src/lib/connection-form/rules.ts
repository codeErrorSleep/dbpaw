import type { ConnectionForm, Driver } from "@/services/api";
import {
  isMysqlFamilyDriver,
  isFileBasedDriver,
} from "@/lib/driver-registry";

export { isMysqlFamilyDriver, isFileBasedDriver };

export const allowsHostWithPort = (driver: Driver) =>
  isMysqlFamilyDriver(driver);

export const requiresPasswordOnCreate = (driver: Driver) =>
  !isMysqlFamilyDriver(driver);

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
