export const proxy = {
  tlsUnsupported: "暂不支持 MongoDB 或 Redis 同时使用网络代理与 TLS。",
  enabled: "使用网络代理",
  description:
    "通过本地 HTTP 或 SOCKS5 代理连接。SOCKS5 会通过代理解析数据库域名。网络代理与 SSH 隧道请选择其一。",
  kind: "代理类型",
  host: "代理主机",
  port: "代理端口",
  username: "代理用户名（可选）",
  password: "代理密码（可选）",
  passwordHint: "编辑时留空可保留同一代理已保存的密码。",
  unsupported:
    "代理支持单节点连接，暂不支持 Cassandra 和 Redis 集群 / 哨兵模式。",
  hostInvalid: "请输入代理主机名或 IP 地址，不要包含协议前缀。",
  portInvalid: "代理端口必须是 1 到 65535 之间的整数。",
  authInvalid:
    "填写代理密码时需提供用户名。SOCKS5 用户名和密码均不得超过 255 字节。",
  sshConflict: "网络代理与 SSH 隧道只能启用一个。",
  mssqlInstance:
    "使用网络代理时，请填写 SQL Server 主机名和明确的 TCP 端口，不要使用命名实例。",
};
