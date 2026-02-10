use crate::entity::job;
use axum::extract::State;
use axum::{Router, routing::post};
use chrono::Utc;
use dotenvy::dotenv;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::Value as JsonValue;

mod entity;

#[derive(Debug, Clone)]
struct AppState {
    db: sea_orm::DatabaseConnection,
}

#[derive(serde::Deserialize)]
struct JobRequest {
    url: String,
    method: String,
    headers: Option<JsonValue>,
    body: Option<JsonValue>,
}

async fn create_job(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<JobRequest>,
) -> Result<String, axum::http::StatusCode> {
    let now = Utc::now().naive_utc();

    let new_job = job::ActiveModel {
        url: Set(payload.url),
        method: Set(payload.method),
        headers: Set(payload
            .headers
            .map(Into::into)
            .unwrap_or(serde_json::json!({}))),
        body: Set(payload
            .body
            .map(Into::into)
            .unwrap_or(serde_json::json!(null))),
        retries: Set(0),
        attempts: Set(0),
        next_run_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let result = new_job.insert(&state.db).await.map_err(|e| {
        println!("Database insertion error: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(result.id.to_string())
}

async fn get_job(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<axum::Json<job::Model>, axum::http::StatusCode> {
    let job = job::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| {
            eprintln!("Database error: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match job {
        Some(job) => Ok(axum::Json(job)),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Connect to database URL
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = sea_orm::Database::connect(db_url).await.unwrap();

    println!("Database connection established");

    // Axum router setup
    let state = AppState { db };

    let app = Router::new()
        .route("/jobs", post(create_job))
        .route("/jobs/{id}", axum::routing::get(get_job))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server listening on port 3000!");
    axum::serve(listener, app).await.unwrap();
}
