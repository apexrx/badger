use crate::entity::job;
use axum::extract::State;
use axum::{Router, routing::post};
use chrono::{Duration, NaiveDateTime, Utc};
use dotenvy::dotenv;
use rand::{Rng, RngExt};
use reqwest::{Method, RequestBuilder};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Expr, LockBehavior, LockType};
use sea_orm::{
    ActiveModelTrait, ActiveValue, Condition, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
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
    let max_attempts = 10;

    // reuse HTTP client
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    loop {
        let now = Utc::now().naive_utc();

        let job_opt = (&state.db)
            .transaction::<_, Option<job::Model>, DbErr>(|txn| {
                Box::pin(async move {
                    let job = job::Entity::find()
                        .filter(
                            job::Column::Status
                                .eq(entity::sea_orm_active_enums::StatusEnum::Pending),
                        )
                        .filter(
                            Condition::any()
                                .add(job::Column::NextRunAt.lte(now))
                                .add(job::Column::NextRunAt.is_null()),
                        )
                        .order_by_asc(job::Column::CreatedAt)
                        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
                        .one(txn)
                        .await?;

                    if let Some(job) = job {
                        let mut active = job.clone().into_active_model();

                        active.status = Set(entity::sea_orm_active_enums::StatusEnum::Running);
                        active.attempts = Set(job.attempts + 1);
                        active.updated_at = Set(now);
                        active.check_in = Set(Some(now));

                        let updated = active.update(txn).await?;
                        Ok(Some(updated))
                    } else {
                        Ok(None)
                    }
                })
            })
            .await
            .unwrap_or(None);

        let job = match job_opt {
            Some(j) => j,
            None => {
                println!("No pending jobs");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        println!("Processing job: {}", job.id);

        let method = match reqwest::Method::from_bytes(job.method.as_bytes()) {
            Ok(m) => m,
            Err(_) => {
                eprintln!("Invalid HTTP method for job {}", job.id);
                continue;
            }
        };

        let mut request = client.request(method, &job.url);

        if let Some(map) = job.headers.as_object() {
            for (k, v) in map {
                if let Some(val) = v.as_str() {
                    request = request.header(k, val);
                }
            }
        }

        if !job.body.is_null() {
            request = request.json(&job.body);
        }

        let (status, response_body) = match request.send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                (status, text)
            }
            Err(e) => {
                eprintln!("HTTP error for job {}: {}", job.id, e);
                (reqwest::StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
        };

        let _ = (&state.db)
            .transaction::<_, (), DbErr>(|txn| {
                let job = job.clone();
                Box::pin(async move {
                    let mut active = job.clone().into_active_model();

                    if status.is_success() {
                        active.status = Set(entity::sea_orm_active_enums::StatusEnum::Success);
                        active.updated_at = Set(Utc::now().naive_utc());

                        let json: serde_json::Value =
                            serde_json::from_str(&response_body).unwrap_or(JsonValue::Null);

                        active.body = Set(json);
                    } else {
                        let attempts = job.attempts + 1;

                        if attempts >= max_attempts {
                            active.status = Set(entity::sea_orm_active_enums::StatusEnum::Failure);
                        } else {
                            active.status = Set(entity::sea_orm_active_enums::StatusEnum::Pending);

                            let exp = attempts.max(0) as u32;
                            let mut backoff = 1000 * 2i64.pow(exp);
                            let jitter: i64 = rand::rng().random_range(-500..=500);
                            backoff = (backoff + jitter).max(0);

                            let next_time =
                                (Utc::now() + Duration::milliseconds(backoff)).naive_utc();

                            active.next_run_at = Set(next_time);
                        }

                        active.updated_at = Set(Utc::now().naive_utc());
                    }

                    active.update(txn).await?;
                    Ok(())
                })
            })
            .await;
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
