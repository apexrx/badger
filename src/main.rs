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
use sha2::{Digest, Sha256};
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

fn create_fingerprint(
    method: String,
    url: String,
    headers: Option<JsonValue>,
    body: Option<JsonValue>,
) -> String {
    // if headers are present, extract headers - authorization, content-type, idempotency-key(if present)
    // convert keys to lowercase
    // then sort the headers by key and store them in a vec
    let mut headers_vec = Vec::new();
    if let Some(headers) = headers {
        let headers = headers.as_object().unwrap();
        for (key, value) in headers {
            headers_vec.push((key.to_lowercase(), value.clone()));
        }
        headers_vec.sort_by(|a, b| a.0.cmp(&b.0));
    }

    // create a string representation of the header, ex: key1:val1, key2:val2
    let headers_str = headers_vec
        .iter()
        .map(|(key, value)| format!("{}:{}", key, value))
        .collect::<Vec<String>>()
        .join(", ");

    // if body is present convert to string
    let body_str = body.map(|body| body.to_string()).unwrap_or_default();

    // create a string of method, url, headers, body in the format: METHOD + | + URL + | + BODY + | + HEADER_STRING
    let fingerprint = format!("{}|{}|{}|{}", method, url, body_str, headers_str);

    // hashing
    let mut hasher = Sha256::new();
    hasher.update(fingerprint);
    let hash = hasher.finalize();
    let hash_str = hex::encode(hash);

    hash_str
}

async fn create_job(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<JobRequest>,
) -> Result<String, axum::http::StatusCode> {
    let now = Utc::now().naive_utc();

    let url = payload.url.clone();
    let method = payload.method.clone();
    let headers: Option<JsonValue> = payload.headers.clone();
    let body: Option<JsonValue> = payload.body.clone();

    let unique_id = create_fingerprint(method.clone(), url.clone(), headers.clone(), body.clone());

    let new_job = job::ActiveModel {
        unique_id: Set(unique_id.clone()),
        url: Set(url),
        method: Set(method),
        headers: Set(headers.unwrap_or(serde_json::json!({}))),
        body: Set(body.unwrap_or(serde_json::json!(null))),
        retries: Set(0),
        attempts: Set(0),
        next_run_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    match new_job.insert(&state.db).await {
        Ok(model) => {
            // successful insert
            Ok(model.id.to_string() + "\n")
        }

        Err(DbErr::Query(sea_orm::RuntimeErr::SqlxError(e)))
            if e.as_database_error()
                .map(|db_err| db_err.code() == Some("23505".into()))
                .unwrap_or(false) =>
        {
            // unique_id conflict → fetch existing job
            let existing_job = job::Entity::find()
                .filter(job::Column::UniqueId.eq(unique_id))
                .one(&state.db)
                .await
                .map_err(|e| {
                    println!("Database error: {}", e);
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                })?;

            match existing_job {
                Some(job) => Ok(job.id.to_string() + "\n"),
                None => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            }
        }

        Err(e) => {
            println!("Database insertion error: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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
        let max_attempts = 10;
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
