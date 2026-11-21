use serde::Serialize;
use tokio_postgres::{Client, NoTls, SimpleQueryMessage};

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
        Ok(text_query(&client, &sql).await?)
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
        Ok(text_query(&client, &sql).await?)
    }

    pub async fn execute_sql(
        &self,
        sql: &str,
    ) -> Result<Vec<(Vec<String>, Vec<Vec<CellType>>)>, Box<dyn std::error::Error>> {
        let sql_trimmed = sql.trim();
        if sql_trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let client = self.get_connection().await?;
        let stmts = split_sql(sql_trimmed);

        if stmts.is_empty() {
            return Ok(Vec::new());
        }

        let total = stmts.len();
        let mut results: Vec<(Vec<String>, Vec<Vec<CellType>>)> = Vec::new();

        for (i, stmt) in stmts.iter().enumerate() {
            let result = match text_query(&client, stmt).await {
                Ok(result) => result,
                Err(e) => {
                    let snippet: String = stmt.chars().take(120).collect();
                    return Err(format!(
                        "Statement {}/{} failed:\n---\n{}...\n---\n{}",
                        i + 1,
                        total,
                        snippet,
                        e,
                    )
                    .into());
                }
            };
            results.push(result);
        }

        Ok(results)
    }

    pub async fn execute_script(
        &self,
        sql: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let sql_trimmed = sql.trim();
        if sql_trimmed.is_empty() {
            return Ok(());
        }
        let client = self.get_connection().await?;
        let stmts = split_sql(sql_trimmed);
        if stmts.is_empty() {
            return Ok(());
        }
        let total = stmts.len();
        for (i, stmt) in stmts.iter().enumerate() {
            if let Err(e) = client.execute(stmt, &[]).await {
                let snippet: String = stmt.chars().take(120).collect();
                return Err(format!(
                    "Statement {}/{} failed:\n---\n{}...\n---\n{}",
                    i + 1,
                    total,
                    snippet,
                    e,
                )
                .into());
            }
        }
        Ok(())
    }
}

fn split_sql(sql: &str) -> Vec<String> {
    let mut statements: Vec<String> = Vec::new();
    let mut cur: Vec<char> = Vec::new();
    let s: Vec<char> = sql.chars().collect();
    let len = s.len();
    let mut i = 0;

    while i < len {
        let c = s[i];

        if c == '\'' {
            cur.push(c);
            i += 1;
            while i < len {
                cur.push(s[i]);
                if s[i] == '\'' {
                    if i + 1 < len && s[i + 1] == '\'' {
                        i += 1;
                        cur.push(s[i]);
                    } else {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            continue;
        }

        if c == '"' {
            cur.push(c);
            i += 1;
            while i < len {
                cur.push(s[i]);
                if s[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        if c == '-' && i + 1 < len && s[i + 1] == '-' {
            cur.push('-');
            cur.push('-');
            i += 2;
            while i < len && s[i] != '\n' {
                cur.push(s[i]);
                i += 1;
            }
            continue;
        }

        if c == '/' && i + 1 < len && s[i + 1] == '*' {
            cur.push('/');
            cur.push('*');
            i += 2;
            while i < len {
                cur.push(s[i]);
                if s[i] == '*' && i + 1 < len && s[i + 1] == '/' {
                    i += 1;
                    cur.push(s[i]);
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        if c == '$' {
            cur.push('$');
            i += 1;
            let mut tag: Vec<char> = Vec::new();
            while i < len && s[i] != '$' {
                tag.push(s[i]);
                cur.push(s[i]);
                i += 1;
            }
            if i < len && s[i] == '$' {
                cur.push('$');
                i += 1;
                let close: String = {
                    let mut m = String::from("$");
                    m.extend(tag.iter());
                    m.push('$');
                    m
                };
                let close_chars: Vec<char> = close.chars().collect();
                let cc = close_chars.len();
                'find: while i < len {
                    if i + cc <= len && s[i..i + cc] == close_chars[..] {
                        cur.extend(&close_chars);
                        i += cc;
                        break 'find;
                    }
                    cur.push(s[i]);
                    i += 1;
                }
            }
            continue;
        }

        if c == ';' {
            let trimmed: String = cur.iter().collect();
            if !trimmed.trim().is_empty() {
                statements.push(trimmed);
            }
            cur.clear();
            i += 1;
            continue;
        }

        cur.push(c);
        i += 1;
    }

    let trimmed: String = cur.iter().collect();
    if !trimmed.trim().is_empty() {
        statements.push(trimmed);
    }

    statements
}

/// Run one statement over the simple query protocol, which returns every value as
/// Postgres's own text output. Values of any type (numeric, timestamptz, uuid, json,
/// arrays, intervals...) display as psql would show them; only SQL NULL is Null.
/// The binary protocol needed a Rust type per column and showed unmapped types as null.
async fn text_query(
    client: &Client,
    sql: &str,
) -> Result<(Vec<String>, Vec<Vec<CellType>>), tokio_postgres::Error> {
    let mut cols: Vec<String> = Vec::new();
    let mut data: Vec<Vec<CellType>> = Vec::new();
    for msg in client.simple_query(sql).await? {
        match msg {
            SimpleQueryMessage::RowDescription(desc) => {
                cols = desc.iter().map(|c| c.name().to_string()).collect();
            }
            SimpleQueryMessage::Row(row) => {
                if cols.is_empty() {
                    cols = row.columns().iter().map(|c| c.name().to_string()).collect();
                }
                data.push(
                    (0..row.len())
                        .map(|i| match row.get(i) {
                            Some(v) => CellType::Text(v.to_string()),
                            None => CellType::Null,
                        })
                        .collect(),
                );
            }
            _ => {}
        }
    }
    Ok((cols, data))
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

#[cfg(test)]
mod text_query_tests {
    use super::*;

    /// Needs a Postgres server: CI provides one through POSTGRES_TEST_URL; skipped without it.
    #[tokio::test]
    async fn every_type_renders_and_only_null_is_null() {
        let Ok(url) = std::env::var("POSTGRES_TEST_URL") else {
            eprintln!("POSTGRES_TEST_URL not set, skipping");
            return;
        };
        let (client, conn) = tokio_postgres::connect(&url, NoTls).await.unwrap();
        tokio::spawn(conn);
        let (cols, rows) = text_query(
            &client,
            "SELECT 1234.50::numeric(10,2) AS total, -1.5::numeric AS neg, \
             '2026-11-21 08:30:00+00'::timestamptz AT TIME ZONE 'UTC' AS ordered_at, \
             '2026-11-21'::date AS day, '6a1f0c2e-9b3d-4f8a-a1b2-c3d4e5f60718'::uuid AS id, \
             '{\"a\":1}'::jsonb AS doc, 7::smallint AS qty, NULL::numeric AS missing",
        )
        .await
        .unwrap();
        assert_eq!(cols, ["total", "neg", "ordered_at", "day", "id", "doc", "qty", "missing"]);
        let shown: Vec<&str> = rows[0].iter().map(|c| c.display_text()).collect();
        assert_eq!(
            shown,
            [
                "1234.50",
                "-1.5",
                "2026-11-21 08:30:00",
                "2026-11-21",
                "6a1f0c2e-9b3d-4f8a-a1b2-c3d4e5f60718",
                "{\"a\": 1}",
                "7",
                "null"
            ]
        );
    }

    #[tokio::test]
    async fn empty_result_keeps_column_names() {
        let Ok(url) = std::env::var("POSTGRES_TEST_URL") else {
            return;
        };
        let (client, conn) = tokio_postgres::connect(&url, NoTls).await.unwrap();
        tokio::spawn(conn);
        let (cols, rows) = text_query(&client, "SELECT 1 AS a, 'x' AS b WHERE false").await.unwrap();
        assert_eq!(cols, ["a", "b"]);
        assert!(rows.is_empty());
    }
}
