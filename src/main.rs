use crate::entity::job;
use axum::extract::State;
use axum::{Router, routing::post};
use chrono::{Duration, NaiveDateTime, Utc};
use dotenvy::dotenv;
use rand::{Rng, RngExt};
use reqwest::{Method, RequestBuilder};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, Condition, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

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

    Ok(result.id.to_string() + "\n")
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

async fn worker_task(state: AppState) {
    loop {
        let max_attempts = 8;
        let now = chrono::Utc::now();

        let job = job::Entity::find()
            .filter(
                job::Column::Status.eq(crate::entity::sea_orm_active_enums::StatusEnum::Pending),
            )
            .filter(
                Condition::any()
                    .add(job::Column::NextRunAt.lte(now))
                    .add(job::Column::NextRunAt.is_null()),
            )
            .order_by_asc(job::Column::CreatedAt)
            .one(&state.db)
            .await;

        // Reusable client for HTTP requests
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();

        match job {
            Ok(Some(job)) => {
                println!("Processing job: {}", job.id);

                let next_attempts = job.attempts + 1;
                let body: serde_json::Value = job.body.clone();
                let headers = job.headers.clone();
                let url = job.url.clone();
                let method = job.method.clone();

                let mut active_job = job.into_active_model();
                active_job.status = Set(entity::sea_orm_active_enums::StatusEnum::Running);
                active_job.attempts = Set(next_attempts);
                active_job.updated_at = Set(Utc::now().naive_utc());

                let updated_job = match active_job.update(&state.db).await {
                    Ok(model) => model,
                    Err(e) => {
                        eprintln!("Error while updating job: {}", e);
                        continue;
                    }
                };

                let method = match Method::from_bytes(method.as_bytes()) {
                    Ok(m) => m,
                    Err(_) => {
                        eprintln!("Invalid HTTP method");
                        continue;
                    }
                };

                let mut request = client.request(method, &url);

                if let Some(map) = headers.as_object() {
                    for (key, value) in map {
                        if let Some(val) = value.as_str() {
                            request = request.header(key, val);
                        }
                    }
                }

                if !body.is_null() {
                    request = request.json(&body);
                }

                let (status, body) = match request.send().await {
                    Ok(response) => {
                        let status = response.status();

                        let body = match response.text().await {
                            Ok(t) => t,
                            Err(e) => {
                                eprintln!("Failed reading body: {}", e);
                                String::new()
                            }
                        };

                        (status, body)
                    }

                    Err(e) => {
                        eprintln!("HTTP request failed: {}", e);
                        (reqwest::StatusCode::INTERNAL_SERVER_ERROR, String::new())
                    }
                };

                if status.is_success() {
                    let mut completed_job = updated_job.into_active_model();
                    completed_job.status = Set(entity::sea_orm_active_enums::StatusEnum::Success);
                    completed_job.updated_at = Set(Utc::now().naive_utc());

                    let json: JsonValue = serde_json::from_str(&body).unwrap_or(JsonValue::Null);
                    completed_job.body = Set(json);

                    if let Err(e) = completed_job.update(&state.db).await {
                        eprintln!("Error while processing job: {}", e);
                    }
                } else {
                    let attempts = updated_job.attempts;
                    let exp = attempts.max(0) as u32;
                    let mut backoff: i64 = 1000 * 2i64.pow(exp);
                    // adding jitter/randomness to prevent thundering herd problem
                    let jitter: i64 = rand::rng().random_range(-500..=500);
                    backoff = (backoff + jitter).max(0);

                    let mut failed_job = updated_job.into_active_model();

                    if attempts >= max_attempts {
                        failed_job.status = Set(entity::sea_orm_active_enums::StatusEnum::Failure);
                        failed_job.updated_at = Set(Utc::now().naive_utc());
                    } else {
                        failed_job.status = Set(entity::sea_orm_active_enums::StatusEnum::Pending);
                        failed_job.updated_at = Set(Utc::now().naive_utc());

                        let next_time = (Utc::now() + Duration::milliseconds(backoff)).naive_utc();
                        failed_job.next_run_at = Set(next_time);
                    }

                    if let Err(e) = failed_job.update(&state.db).await {
                        eprintln!("Error while processing job: {}", e);
                    }
                }
            }
            Ok(None) => {
                println!("No pending jobs");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            Err(e) => {
                eprintln!("Error processing job: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

async fn monitor_task(state: AppState) {
    loop {
        let cutoff = Utc::now().naive_utc() - Duration::seconds(30);

        let job = job::Entity::find()
            .filter(
                job::Column::Status.eq(crate::entity::sea_orm_active_enums::StatusEnum::Running),
            )
            .filter(
                Condition::any().add(job::Column::CheckIn.lte(cutoff)).add(
                    Condition::all()
                        .add(job::Column::CheckIn.is_null())
                        .add(job::Column::UpdatedAt.lte(cutoff)),
                ),
            )
            .order_by_asc(job::Column::UpdatedAt)
            .one(&state.db)
            .await;

        match job {
            Ok(Some(job)) => {
                let mut active_job = job.into_active_model();
                active_job.check_in = Set(Some(Utc::now().naive_utc()));
                active_job.status = Set(entity::sea_orm_active_enums::StatusEnum::Pending);

                if let Err(e) = active_job.update(&state.db).await {
                    eprintln!("Error while processing job: {}", e);
                }
            }
            Ok(None) => {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            Err(e) => {
                eprintln!("Error fetching job: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
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

    // worker
    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        worker_task(worker_state).await;
    });

    let monitor_state = state.clone();
    let monitor = tokio::spawn(async move {
        monitor_task(monitor_state).await;
    });

    let app = Router::new()
        .route("/jobs", post(create_job))
        .route("/jobs/{id}", axum::routing::get(get_job))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server listening on port 3000!");
    axum::serve(listener, app).await.unwrap();
}
