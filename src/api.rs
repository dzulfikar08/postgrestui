use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use crate::app::{App, ConnectionConfig};

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

#[derive(Clone)]
pub struct AppState {
    pub app: Arc<Mutex<App>>,
    pub config: Arc<Mutex<Option<ConnectionConfig>>>,
}

#[derive(Deserialize)]
pub struct PaginationParams {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct ExportParams {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

pub async fn start_server(config: ConnectionConfig, listen_port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::default();
    match app.connect(config.clone()).await {
        Ok(()) => println!("Connected to {}:{}/{}", config.host, config.port, config.database),
        Err(e) => eprintln!("Warning: connection failed: {}. Server will retry on API calls.", e),
    }

    let state = AppState {
        app: Arc::new(Mutex::new(app)),
        config: Arc::new(Mutex::new(Some(config))),
    };

    let app_router = Router::new()
        .route("/api/info", get(api_info))
        .route("/api/tables", get(api_tables))
        .route("/api/table/:schema/:name/data", get(api_table_data))
        .route("/api/table/:schema/:name/schema", get(api_table_schema))
        .route("/api/table/:schema/:name/export/:format", get(api_table_export))
        .route("/api/query", post(api_query))
        .route("/api/script", post(api_script))
        .fallback(serve_assets)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", listen_port);
    println!("postgrestui web UI listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app_router).await?;
    Ok(())
}

async fn ensure_connected(state: &AppState) -> Result<(), String> {
    let app = state.app.lock().await;
    if app.current_db.is_some() {
        return Ok(());
    }
    drop(app);

    let config_guard = state.config.lock().await;
    let config = match config_guard.as_ref() {
        Some(c) => c.clone(),
        None => return Err("No connection config".to_string()),
    };
    drop(config_guard);

    let mut app = state.app.lock().await;
    app.connect(config).await.map_err(|e| e.to_string())
}

async fn api_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    let conn_err = ensure_connected(&state).await.err();
    let app = state.app.lock().await;
    Json(serde_json::json!({
        "server": app.current_db.as_ref().map(|db| format!("{}:{}", db.host, db.port)).unwrap_or_default(),
        "database": app.current_db.as_ref().map(|db| &db.database).unwrap_or(&String::new()),
        "tableCount": app.current_db.as_ref().map(|db| db.tables.len()).unwrap_or(0),
        "viewCount": app.current_db.as_ref().map(|db| db.views.len()).unwrap_or(0),
        "error": conn_err,
    }))
}

async fn api_tables(State(state): State<AppState>) -> Json<serde_json::Value> {
    if let Err(e) = ensure_connected(&state).await {
        return Json(serde_json::json!({"error": e}));
    }
    let app = state.app.lock().await;
    if let Some(db) = &app.current_db {
        Json(serde_json::json!({
            "tables": db.tables,
            "views": db.views,
        }))
    } else {
        Json(serde_json::json!({"tables": [], "views": []}))
    }
}

async fn api_table_data(
    State(state): State<AppState>,
    Path((schema, name)): Path<(String, String)>,
    Query(params): Query<PaginationParams>,
) -> Response {
    if let Err(e) = ensure_connected(&state).await {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": e}))).into_response();
    }
    let app = state.app.lock().await;
    let table = find_table(&app, &schema, &name);
    let Some(table) = table else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Table not found"}))).into_response();
    };
    let total = table.row_count.unwrap_or(0);
    let limit = params.limit.unwrap_or(500);
    let offset = params.offset.unwrap_or(0);

    let result = if total > limit {
        app.select_page(table, offset, limit).await
    } else {
        app.select(table).await
    };

    match result {
        Ok((columns, rows)) => {
            let cell_rows: Vec<Vec<&str>> = rows
                .iter()
                .map(|row| row.iter().map(|c| c.display_text()).collect())
                .collect();
            Json(serde_json::json!({
                "columns": columns,
                "rows": cell_rows,
                "total": total,
                "offset": offset,
                "limit": limit,
            }))
            .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn api_table_schema(
    State(state): State<AppState>,
    Path((schema, name)): Path<(String, String)>,
) -> Response {
    if let Err(e) = ensure_connected(&state).await {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": e}))).into_response();
    }
    let app = state.app.lock().await;
    let table = find_table(&app, &schema, &name);
    let Some(table) = table else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Table not found"}))).into_response();
    };

    let ddl = if table.sql.is_empty() {
        format!("-- Schema definition not available for \"{}\".\"{}\"", table.schema, table.name)
    } else {
        table.sql.clone()
    };

    Json(serde_json::json!({
        "name": table.name,
        "schema": table.schema,
        "sql": ddl,
        "columns": table.columns,
        "rowCount": table.row_count,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct QueryRequest {
    sql: String,
}

async fn api_script(
    State(state): State<AppState>,
    Json(body): Json<QueryRequest>,
) -> Response {
    if let Err(e) = ensure_connected(&state).await {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": e}))).into_response();
    }
    let app = state.app.lock().await;
    match app.execute_script(&body.sql).await {
        Ok(()) => {
            Json(serde_json::json!({"status": "ok", "message": "Script executed successfully."}))
                .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn api_query(
    State(state): State<AppState>,
    Json(body): Json<QueryRequest>,
) -> Response {
    if let Err(e) = ensure_connected(&state).await {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": e}))).into_response();
    }
    let app = state.app.lock().await;
    match app.execute_sql(&body.sql).await {
        Ok(results) => {
            let json_results: Vec<serde_json::Value> = results
                .iter()
                .map(|(columns, rows)| {
                    let cell_rows: Vec<Vec<&str>> = rows
                        .iter()
                        .map(|row| row.iter().map(|c| c.display_text()).collect())
                        .collect();
                    serde_json::json!({
                        "columns": columns,
                        "rows": cell_rows,
                    })
                })
                .collect();
            Json(serde_json::json!({"results": json_results})).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn api_table_export(
    State(state): State<AppState>,
    Path((schema, name, format)): Path<(String, String, String)>,
    Query(params): Query<ExportParams>,
) -> Response {
    if let Err(e) = ensure_connected(&state).await {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": e}))).into_response();
    }
    let app = state.app.lock().await;
    let table = find_table(&app, &schema, &name);
    let Some(table) = table else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Table not found"}))).into_response();
    };

    let limit = params.limit;
    let offset = params.offset.unwrap_or(0);
    let total = table.row_count.unwrap_or(0);

    let result = if limit.is_some() || total > 5000 {
        app.select_page(table, offset, limit.unwrap_or(5000)).await
    } else {
        app.select(table).await
    };

    let (columns, rows) = match result {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    match format.as_str() {
        "csv" => {
            let mut csv = columns.join(",");
            csv.push('\n');
            for row in &rows {
                let cells: Vec<String> = row.iter().map(|c| {
                    let t = c.display_text();
                    if t.contains(',') || t.contains('"') || t.contains('\n') {
                        format!("\"{}\"", t.replace('"', "\"\""))
                    } else {
                        t.to_string()
                    }
                }).collect();
                csv.push_str(&cells.join(","));
                csv.push('\n');
            }
            let filename = format!("{}_{}.csv", schema, name);
            (StatusCode::OK, [
                (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
                (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
            ], csv).into_response()
        }
        "json" => {
            let json_rows: Vec<serde_json::Map<String, serde_json::Value>> = rows.iter().map(|row| {
                let mut map = serde_json::Map::new();
                for (i, col) in columns.iter().enumerate() {
                    let cell = row.get(i).map(|c| c.display_text()).unwrap_or("null");
                    let val: serde_json::Value = if cell == "null" {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String(cell.to_string())
                    };
                    map.insert(col.clone(), val);
                }
                map
            }).collect();
            let filename = format!("{}_{}.json", schema, name);
            (StatusCode::OK, [
                (axum::http::header::CONTENT_TYPE, "application/json".to_string()),
                (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
            ], serde_json::to_string_pretty(&json_rows).unwrap_or_default()).into_response()
        }
        "sql" => {
            let quoted = format!("\"{}\".\"{}\"", schema, name);
            let mut sql = String::new();
            for row in &rows {
                let values: Vec<String> = row.iter().map(|c| {
                    let t = c.display_text();
                    if t == "null" { "NULL".to_string() } else { format!("'{}'", t.replace('\'', "''")) }
                }).collect();
                let cols: Vec<String> = columns.iter().map(|c| format!("\"{}\"", c)).collect();
                sql.push_str(&format!("INSERT INTO {} ({}) VALUES ({});\n", quoted, cols.join(", "), values.join(", ")));
            }
            let filename = format!("{}_{}.sql", schema, name);
            (StatusCode::OK, [
                (axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8".to_string()),
                (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
            ], sql).into_response()
        }
        _ => (StatusCode::BAD_REQUEST, "Unsupported format. Use csv, json, or sql.").into_response(),
    }
}

fn find_table<'a>(app: &'a App, schema: &str, name: &str) -> Option<&'a crate::app::Table> {
    app.current_db.as_ref().and_then(|db| {
        db.tables
            .iter()
            .chain(db.views.iter())
            .find(|t| t.schema == schema && t.name == name)
    })
}

async fn serve_assets(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.is_empty() || path == "index.html" {
        return match Assets::get("index.html") {
            Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()).into_response(),
            None => Html("<h1>postgrestui</h1><p>Run <code>npm run build</code> in web/ first.</p>".to_string()).into_response(),
        };
    }
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], content.data.to_vec()).into_response()
        }
        None => match Assets::get("index.html") {
            Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()).into_response(),
            None => (StatusCode::NOT_FOUND, "Not found").into_response(),
        },
    }
}
