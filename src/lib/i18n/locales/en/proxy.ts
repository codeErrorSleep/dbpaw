export const proxy = {
  tlsUnsupported:
    "Network proxy with TLS is not supported for MongoDB or Redis yet.",
  enabled: "Use network proxy",
  description:
    "Connect through a local HTTP or SOCKS5 proxy. SOCKS5 resolves the database hostname through the proxy. Use either a proxy or an SSH tunnel.",
  kind: "Proxy type",
  host: "Proxy host",
  port: "Proxy port",
  username: "Proxy username (optional)",
  password: "Proxy password (optional)",
  passwordHint:
    "Leave blank when editing to keep the saved password for the same proxy.",
  unsupported:
    "Proxy is available for standalone connections; Cassandra and Redis Cluster/Sentinel are not supported yet.",
  hostInvalid: "Enter a proxy hostname or IP address without a URL scheme.",
  portInvalid: "Proxy port must be an integer between 1 and 65535.",
  authInvalid:
    "Enter a proxy username when providing a password. SOCKS5 credentials must not exceed 255 bytes.",
  sshConflict:
    "Use either a network proxy or an SSH tunnel for this connection.",
  mssqlInstance:
    "Use the SQL Server hostname and explicit TCP port instead of a named instance.",
};
