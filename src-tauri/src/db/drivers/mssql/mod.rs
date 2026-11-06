pub mod connection;
pub mod metadata;
pub mod query;
pub mod table_data;

use super::{
    DatabaseDriver, DriverCapabilities, DriverResult, ForeignKeyDriver, RoutineDriver,
    SynonymDriver,
};
use crate::models::{
    ConnectionForm, QueryResult, RoutineInfo, SchemaForeignKey, SchemaOverview, SynonymInfo,
    TableDataResponse, TableInfo, TableMetadata, TableStructure,
};
use crate::network::NetworkTunnel;
use async_trait::async_trait;
use bb8::Pool;
use connection::MssqlConnectionManager;

pub struct MssqlDriver {
    pub pool: Pool<MssqlConnectionManager>,
    pub network_tunnel: Option<NetworkTunnel>,
    pub supports_for_json: bool,
}

impl MssqlDriver {
    pub async fn connect(form: &ConnectionForm) -> DriverResult<Self> {
        connection::connect(form).await
    }
}

#[async_trait]
impl DatabaseDriver for MssqlDriver {
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::ROUTINES
            | DriverCapabilities::SYNONYMS
            | DriverCapabilities::FOREIGN_KEYS
    }

    async fn close(&self) {}

    async fn test_connection(&self) -> DriverResult<()> {
        let rows = self.fetch_rows("SELECT 1").await?;
        if rows.is_empty() {
            return Err(connection::conn_error("Empty response"));
        }
        Ok(())
    }

    async fn list_databases(&self) -> DriverResult<Vec<String>> {
        self.list_databases_impl().await
    }

    async fn list_schemas(&self) -> DriverResult<Vec<String>> {
        self.list_schemas_impl().await
    }

    async fn list_tables(&self, schema: Option<String>) -> DriverResult<Vec<TableInfo>> {
        self.list_tables_impl(schema).await
    }

    async fn get_table_structure(
        &self,
        schema: String,
        table: String,
    ) -> DriverResult<TableStructure> {
        self.get_table_structure_impl(schema, table).await
    }

    async fn get_table_metadata(
        &self,
        schema: String,
        table: String,
    ) -> DriverResult<TableMetadata> {
        self.get_table_metadata_impl(schema, table).await
    }

    async fn get_table_ddl(&self, schema: String, table: String) -> DriverResult<String> {
        self.get_table_ddl_impl(schema, table).await
    }

    async fn get_table_data(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
        include_total: bool,
    ) -> DriverResult<TableDataResponse> {
        self.get_table_data_impl(
            schema,
            table,
            page,
            limit,
            sort_column,
            sort_direction,
            filter,
            order_by,
            include_total,
        )
        .await
    }

    async fn get_table_data_chunk(
        &self,
        schema: String,
        table: String,
        page: i64,
        limit: i64,
        sort_column: Option<String>,
        sort_direction: Option<String>,
        filter: Option<String>,
        order_by: Option<String>,
    ) -> DriverResult<TableDataResponse> {
        self.get_table_data_chunk_impl(
            schema,
            table,
            page,
            limit,
            sort_column,
            sort_direction,
            filter,
            order_by,
        )
        .await
    }

    async fn execute_query(&self, sql: String) -> DriverResult<QueryResult> {
        self.execute_query_impl(sql).await
    }

    async fn get_schema_overview(&self, schema: Option<String>) -> DriverResult<SchemaOverview> {
        self.get_schema_overview_impl(schema).await
    }
}

#[async_trait]
impl RoutineDriver for MssqlDriver {
    async fn list_routines(&self, schema: Option<String>) -> DriverResult<Vec<RoutineInfo>> {
        self.list_routines_impl(schema).await
    }

    async fn get_routine_ddl(
        &self,
        schema: String,
        name: String,
        routine_type: String,
    ) -> DriverResult<String> {
        self.get_routine_ddl_impl(schema, name, routine_type).await
    }
}

#[async_trait]
impl SynonymDriver for MssqlDriver {
    async fn list_synonyms(&self, schema: Option<String>) -> DriverResult<Vec<SynonymInfo>> {
        self.list_synonyms_impl(schema).await
    }
}

#[async_trait]
impl ForeignKeyDriver for MssqlDriver {
    async fn get_schema_foreign_keys(
        &self,
        database: Option<&str>,
    ) -> DriverResult<Vec<SchemaForeignKey>> {
        self.get_schema_foreign_keys_impl(database).await
    }
}
