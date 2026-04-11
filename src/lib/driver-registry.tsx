import type { ReactNode } from "react";
import { Database, Server } from "lucide-react";
import {
  siMysql,
  siPostgresql,
  siSqlite,
  siClickhouse,
  siDuckdb,
} from "simple-icons";

export type ImportDriverCapability =
  | "supported"
  | "read_only_not_supported"
  | "unsupported";

const DRIVER_IDS = [
  "postgres",
  "mysql",
  "mariadb",
  "tidb",
  "starrocks",
  "sqlite",
  "duckdb",
  "clickhouse",
  "mssql",
  "oracle",
] as const;

export type Driver = (typeof DRIVER_IDS)[number];

const renderSimpleIcon = (icon: { path: string }) => (
  <svg
    viewBox="0 0 24 24"
    width="16"
    height="16"
    aria-hidden="true"
    className="shrink-0"
    role="img"
  >
    <path d={icon.path} fill="currentColor" />
  </svg>
);

export interface DriverConfig {
  id: Driver;
  label: string;
  defaultPort: number | null;
  isFileBased: boolean;
  isMysqlFamily: boolean;
  supportsSSLCA: boolean;
  supportsSchemaBrowsing: boolean;
  supportsCreateDatabase: boolean;
  importCapability: ImportDriverCapability;
  icon: () => ReactNode;
}

export const DRIVER_REGISTRY: DriverConfig[] = [
  {
    id: "postgres",
    label: "PostgreSQL",
    defaultPort: 5432,
    isFileBased: false,
    isMysqlFamily: false,
    supportsSSLCA: true,
    supportsSchemaBrowsing: true,
    supportsCreateDatabase: true,
    importCapability: "supported",
    icon: () => renderSimpleIcon(siPostgresql),
  },
  {
    id: "mysql",
    label: "MySQL",
    defaultPort: 3306,
    isFileBased: false,
    isMysqlFamily: true,
    supportsSSLCA: true,
    supportsSchemaBrowsing: false,
    supportsCreateDatabase: true,
    importCapability: "supported",
    icon: () => renderSimpleIcon(siMysql),
  },
  {
    id: "mariadb",
    label: "MariaDB",
    defaultPort: 3306,
    isFileBased: false,
    isMysqlFamily: true,
    supportsSSLCA: true,
    supportsSchemaBrowsing: false,
    supportsCreateDatabase: true,
    importCapability: "supported",
    icon: () => renderSimpleIcon(siMysql),
  },
  {
    id: "tidb",
    label: "TiDB",
    defaultPort: 4000,
    isFileBased: false,
    isMysqlFamily: true,
    supportsSSLCA: true,
    supportsSchemaBrowsing: false,
    supportsCreateDatabase: true,
    importCapability: "supported",
    icon: () => renderSimpleIcon(siMysql),
  },
  {
    id: "starrocks",
    label: "StarRocks",
    defaultPort: 9030,
    isFileBased: false,
    isMysqlFamily: true,
    supportsSSLCA: true,
    supportsSchemaBrowsing: false,
    supportsCreateDatabase: true,
    importCapability: "unsupported",
    icon: () => <Database className="w-4 h-4" />,
  },
  {
    id: "sqlite",
    label: "SQLite",
    defaultPort: null,
    isFileBased: true,
    isMysqlFamily: false,
    supportsSSLCA: false,
    supportsSchemaBrowsing: false,
    supportsCreateDatabase: false,
    importCapability: "supported",
    icon: () => renderSimpleIcon(siSqlite),
  },
  {
    id: "duckdb",
    label: "DuckDB",
    defaultPort: null,
    isFileBased: true,
    isMysqlFamily: false,
    supportsSSLCA: false,
    supportsSchemaBrowsing: false,
    supportsCreateDatabase: false,
    importCapability: "supported",
    icon: () => renderSimpleIcon(siDuckdb),
  },
  {
    id: "clickhouse",
    label: "ClickHouse",
    defaultPort: 8123,
    isFileBased: false,
    isMysqlFamily: false,
    supportsSSLCA: false,
    supportsSchemaBrowsing: false,
    supportsCreateDatabase: true,
    importCapability: "read_only_not_supported",
    icon: () => renderSimpleIcon(siClickhouse),
  },
  {
    id: "mssql",
    label: "SQL Server",
    defaultPort: 1433,
    isFileBased: false,
    isMysqlFamily: false,
    supportsSSLCA: false,
    supportsSchemaBrowsing: true,
    supportsCreateDatabase: true,
    importCapability: "supported",
    icon: () => <Database className="w-4 h-4" />,
  },
  {
    id: "oracle",
    label: "Oracle",
    defaultPort: 1521,
    isFileBased: false,
    isMysqlFamily: false,
    supportsSSLCA: false,
    supportsSchemaBrowsing: true,
    supportsCreateDatabase: false,
    importCapability: "supported",
    icon: () => <Database className="w-4 h-4" />,
  },
];

export const getDriverConfig = (driver: Driver): DriverConfig =>
  DRIVER_REGISTRY.find((d) => d.id === driver)!;

export const getDefaultPort = (driver: Driver): number | null =>
  getDriverConfig(driver).defaultPort;

export const isFileBasedDriver = (driver: Driver): boolean =>
  getDriverConfig(driver).isFileBased;

export const isMysqlFamilyDriver = (driver: Driver): boolean =>
  getDriverConfig(driver).isMysqlFamily;

export const supportsSSLCA = (driver: Driver): boolean =>
  getDriverConfig(driver).supportsSSLCA;

export const supportsCreateDatabase = (driver: Driver): boolean =>
  getDriverConfig(driver).supportsCreateDatabase;

export const supportsSchemaBrowsing = (driver: Driver): boolean =>
  getDriverConfig(driver).supportsSchemaBrowsing;

export const getConnectionIcon = (
  driver: Driver | string | undefined,
): ReactNode => {
  const config = DRIVER_REGISTRY.find((d) => d.id === driver);
  if (config) return config.icon();
  const normalized = String(driver || "")
    .trim()
    .toLowerCase();
  if (normalized === "postgresql" || normalized === "pgsql")
    return getConnectionIcon("postgres");
  if (normalized === "sqlite3") return getConnectionIcon("sqlite");
  return <Server className="w-4 h-4" />;
};
