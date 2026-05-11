use serde::Serialize;
use tokio_postgres::{NoTls, Row};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum CellType {
    Text(String),
    Null,
    Blob,
}

impl CellType {
    pub fn display_text(&self) -> &str {
        match self {
            CellType::Text(s) => s,
            CellType::Null => "null",
            CellType::Blob => "[Blob]",
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Db {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub tables: Vec<Table>,
    pub views: Vec<Table>,
}

#[derive(Debug, Default, Serialize)]
pub struct Table {
    pub name: String,
    pub schema: String,
    pub sql: String,
    pub row_count: Option<usize>,
    pub columns: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct App {
    pub current_db: Option<Db>,
    pub config: Option<ConnectionConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

impl App {
    fn connection_string(config: &ConnectionConfig) -> String {
        let mut parts = vec![
            format!("host={}", config.host),
            format!("port={}", config.port),
            format!("dbname={}", config.database),
            format!("user={}", config.username),
        ];
        if !config.password.is_empty() {
            parts.push(format!("password={}", config.password));
        }
        parts.join(" ")
    }

    pub async fn connect(&mut self, config: ConnectionConfig) -> Result<(), Box<dyn std::error::Error>> {
        let conn_str = Self::connection_string(&config);
        let (client, connection) = tokio_postgres::connect(&conn_str, NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        let (mut tables, mut views) = get_tables(&client).await?;

        for table in &mut tables {
            table.row_count = Some(count_rows(&client, &table.schema, &table.name).await);
            table.columns = get_columns(&client, &table.schema, &table.name).await;
        }
        for view in &mut views {
            view.row_count = Some(count_rows(&client, &view.schema, &view.name).await);
            view.columns = get_columns(&client, &view.schema, &view.name).await;
        }

        self.config = Some(config.clone());
        self.current_db = Some(Db {
            host: config.host,
            port: config.port,
            database: config.database,
            tables,
            views,
        });
        Ok(())
    }

    async fn get_connection(
        &self,
    ) -> Result<tokio_postgres::Client, Box<dyn std::error::Error>> {
        let config = self.config.as_ref().ok_or("Not connected")?;
        let conn_str = Self::connection_string(config);
        let (client, connection) = tokio_postgres::connect(&conn_str, NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        Ok(client)
    }

    pub async fn select(
        &self,
        table: &Table,
    ) -> Result<(Vec<String>, Vec<Vec<CellType>>), Box<dyn std::error::Error>> {
        let client = self.get_connection().await?;
        let quoted = format!("\"{}\".\"{}\"", table.schema, table.name);
        let sql = format!("SELECT * FROM {}", quoted);
        let rows = client.query(&sql, &[]).await?;
        let cols: Vec<String> = rows
            .first()
            .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
            .unwrap_or_default();
        let data: Vec<Vec<CellType>> = rows.iter().map(|r| map_row(&cols, r)).collect();
        Ok((cols, data))
    }

    pub async fn select_page(
        &self,
        table: &Table,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<String>, Vec<Vec<CellType>>), Box<dyn std::error::Error>> {
        let client = self.get_connection().await?;
        let quoted = format!("\"{}\".\"{}\"", table.schema, table.name);
        let sql = format!("SELECT * FROM {} LIMIT {} OFFSET {}", quoted, limit, offset);
        let rows = client.query(&sql, &[]).await?;
        let cols: Vec<String> = rows
            .first()
            .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
            .unwrap_or_default();
        let data: Vec<Vec<CellType>> = rows.iter().map(|r| map_row(&cols, r)).collect();
        Ok((cols, data))
    }

    pub async fn execute_sql(
        &self,
        sql: &str,
    ) -> Result<(Vec<String>, Vec<Vec<CellType>>), Box<dyn std::error::Error>> {
        let sql_trimmed = sql.trim();
        if sql_trimmed.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let client = self.get_connection().await?;
        let rows = client.query(sql_trimmed, &[]).await?;
        let cols: Vec<String> = rows
            .first()
            .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
            .unwrap_or_default();
        let data: Vec<Vec<CellType>> = rows.iter().map(|r| map_row(&cols, r)).collect();
        Ok((cols, data))
    }
}

fn map_row(cols: &[String], row: &Row) -> Vec<CellType> {
    let mut data: Vec<CellType> = Vec::new();
    for i in 0..cols.len() {
        let cell = map_cell(row, i);
        data.push(cell);
    }
    data
}

fn map_cell(row: &Row, col_index: usize) -> CellType {
    if let Ok(Some(s)) = row.try_get::<_, Option<&str>>(col_index) {
        return CellType::Text(s.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<String>>(col_index) {
        return CellType::Text(v);
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<i32>>(col_index) {
        return CellType::Text(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<i64>>(col_index) {
        return CellType::Text(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<f32>>(col_index) {
        return CellType::Text(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<f64>>(col_index) {
        return CellType::Text(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<bool>>(col_index) {
        return CellType::Text(v.to_string());
    }
    if let Ok(Some(_)) = row.try_get::<_, Option<&[u8]>>(col_index) {
        return CellType::Blob;
    }
    CellType::Null
}

async fn count_rows(client: &tokio_postgres::Client, schema: &str, table_name: &str) -> usize {
    let quoted = format!("\"{}\".\"{}\"", schema, table_name);
    let sql = format!("SELECT COUNT(*) FROM {}", quoted);
    match client.query_one(&sql, &[]).await {
        Ok(row) => row.get::<_, i64>(0) as usize,
        Err(_) => 0,
    }
}

async fn get_columns(client: &tokio_postgres::Client, schema: &str, table_name: &str) -> Vec<String> {
    let sql = format!(
        "SELECT column_name FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position"
    );
    match client.query(&sql, &[&schema, &table_name]).await {
        Ok(rows) => rows.iter().filter_map(|r| r.try_get::<_, String>(0).ok()).collect(),
        Err(_) => Vec::new(),
    }
}

async fn get_tables(client: &tokio_postgres::Client) -> Result<(Vec<Table>, Vec<Table>), Box<dyn std::error::Error>> {
    let sql = r#"
        SELECT table_type, table_schema, table_name
        FROM information_schema.tables
        WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
          AND table_type IN ('BASE TABLE', 'VIEW')
        ORDER BY table_schema, table_name;
    "#;
    let rows = client.query(sql, &[]).await?;
    let mut tables: Vec<Table> = Vec::new();
    let mut views: Vec<Table> = Vec::new();

    for row in rows {
        let type_id: String = row.get(0);
        let schema_name: String = row.get(1);
        let name: String = row.get(2);

        let table = Table {
            name,
            schema: schema_name,
            sql: String::new(),
            row_count: None,
            columns: Vec::new(),
        };

        if type_id == "BASE TABLE" {
            tables.push(table);
        } else if type_id == "VIEW" {
            views.push(table);
        }
    }
    Ok((tables, views))
}
