use super::cassandra::CassandraDriver;
use super::clickhouse::ClickHouseDriver;
#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    all(target_os = "macos", target_arch = "x86_64")
))]
use super::db2::Db2Driver;
use super::duckdb::DuckdbDriver;
use super::mongodb::MongoDBDriver;
use super::mssql::MssqlDriver;
use super::mysql::MysqlDriver;
use super::oracle::OracleDriver;
use super::postgres::PostgresDriver;
use super::sqlite::SqliteDriver;
use super::DatabaseDriver;
use crate::error::AppError;
use crate::models::ConnectionForm;

pub fn is_mysql_family_driver(driver: &str) -> bool {
    matches!(driver, "mysql" | "mariadb" | "tidb" | "starrocks" | "doris")
}

pub async fn connect(form: &ConnectionForm) -> Result<Box<dyn DatabaseDriver>, AppError> {
    crate::network::validate_proxy(form)?;
    match form.driver.as_str() {
        "postgres" => {
            let driver = PostgresDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        driver if is_mysql_family_driver(driver) => {
            let driver = MysqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "sqlite" => {
            let driver = SqliteDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "duckdb" => {
            let driver = DuckdbDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "clickhouse" => {
            let driver = ClickHouseDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "mssql" => {
            let driver = MssqlDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "oracle" => {
            let driver = OracleDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        #[cfg(any(
            target_os = "linux",
            target_os = "windows",
            all(target_os = "macos", target_arch = "x86_64")
        ))]
        "db2" => {
            let driver = Db2Driver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "mongodb" => {
            let driver = MongoDBDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        "cassandra" => {
            let driver = CassandraDriver::connect(form).await?;
            Ok(Box::new(driver) as Box<dyn DatabaseDriver>)
        }
        _ => Err(AppError::unsupported(format!(
            "Driver {} not supported",
            form.driver
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::is_mysql_family_driver;

    #[test]
    fn mysql_family_helper_includes_doris_and_starrocks() {
        assert!(is_mysql_family_driver("mysql"));
        assert!(is_mysql_family_driver("starrocks"));
        assert!(is_mysql_family_driver("doris"));
        assert!(!is_mysql_family_driver("postgres"));
    }
}
