import type { ConnectionForm, NetworkProxy } from "@/services/api";
import { getRedisConnectionMode, isFileBasedDriver } from "./rules";

export const DEFAULT_NETWORK_PROXY: NetworkProxy = {
  enabled: false,
  kind: "socks5",
  host: "127.0.0.1",
  port: 1080,
  username: "",
  password: "",
};

export const supportsNetworkProxy = (form: ConnectionForm) =>
  !isFileBasedDriver(form.driver) &&
  form.driver !== "cassandra" &&
  (form.driver !== "redis" || getRedisConnectionMode(form) === "standalone");

export function validateNetworkProxy(form: ConnectionForm): { key: string }[] {
  const proxy = form.proxy;
  if (!proxy?.enabled) return [];
  const issues: { key: string }[] = [];
  const add = (key: string) =>
    issues.push({ key: `connection.dialog.proxy.${key}` });
  if (!supportsNetworkProxy(form)) add("unsupported");
  if ((form.driver === "mongodb" || form.driver === "redis") && form.ssl)
    add("tlsUnsupported");
  if (form.sshEnabled) add("sshConflict");
  if (
    !proxy.host.trim() ||
    /[\s/@\\?#]/.test(proxy.host) ||
    proxy.host.includes("://")
  )
    add("hostInvalid");
  if (!Number.isInteger(proxy.port) || proxy.port < 1 || proxy.port > 65535)
    add("portInvalid");
  const bytes = new TextEncoder();
  if (
    (!proxy.username && proxy.password) ||
    (proxy.kind === "socks5" &&
      (bytes.encode(proxy.username).length > 255 ||
        bytes.encode(proxy.password).length > 255))
  )
    add("authInvalid");
  if (form.driver === "mssql" && form.host?.includes("\\"))
    add("mssqlInstance");
  return issues;
}
