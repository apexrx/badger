//! Normalized benchmarks for fair comparison
//! Adjusts for batch size, concurrency, and work per job

use chrono::Utc;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusEnum {
    Pending,
    Running,
    Success,
    Failure,
}

impl std::fmt::Display for StatusEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusEnum::Pending => write!(f, "Pending"),
            StatusEnum::Running => write!(f, "Running"),
            StatusEnum::Success => write!(f, "Success"),
            StatusEnum::Failure => write!(f, "Failure"),
        }
    }
}

fn sql(stmt: &str) -> Statement {
    Statement::from_string(DbBackend::Postgres, stmt.to_string())
}

async fn setup_db() -> DatabaseConnection {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://user:pass@localhost:5432/badger_db".to_string());

    let db = Database::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");

    db.execute_unprepared("TRUNCATE TABLE job RESTART IDENTITY CASCADE;")
        .await
        .expect("Failed to truncate table");

    db
}

async fn insert_single_job(db: &DatabaseConnection) -> String {
    let now = Utc::now().naive_utc();
    let unique_id = format!("test_{}", Uuid::new_v4().hyphenated());
    let job_id = Uuid::new_v4().hyphenated().to_string();

    db.execute_unprepared(&format!(
        r#"INSERT INTO job (unique_id, id, url, method, headers, body, retries, attempts, status, next_run_at, created_at, updated_at)
           VALUES ('{}', '{}', 'http://example.com/job', 'GET', '{{}}', 'null', 0, 0, 'Pending', '{}', '{}', '{}')"#,
        unique_id, job_id, now, now, now
    ))
    .await
    .expect("Failed to insert job");

    unique_id
}

#[cfg(test)]
mod normalized_benchmarks {
    use super::*;
    use tokio::sync::Barrier;

    /// Normalized metric: Jobs per second per worker
    fn calc_per_worker(jobs: f64, workers: f64) -> f64 {
        jobs / workers
    }

    /// Normalized metric: Latency per job in milliseconds
    fn calc_latency_ms(total_ms: f64, jobs: f64) -> f64 {
        total_ms / jobs
    }

    #[tokio::test]
    async fn run_normalized_benchmark_suite() {
        println!("\n");
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║      BADGER NORMALIZED BENCHMARK SUITE                   ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║  Database: PostgreSQL (localhost)                        ║");
        println!("║  Normalized for: concurrency, batch size, work load      ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();

        // Test 1: Single worker, no work (pure overhead)
        let db = setup_db().await;
        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            insert_single_job(&db).await;
        }
        let elapsed = start.elapsed();
        let jobs_per_sec = iterations as f64 / elapsed.as_secs_f64();
        let latency_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
        
        println!("=== Single Worker, No Work (Pure Overhead) ===");
        println!("  Jobs: {} | Workers: 1 | Work: 0ms", iterations);
        println!("  Throughput: {:.1} jobs/sec", jobs_per_sec);
        println!("  Latency: {:.2} ms/job", latency_ms);
        println!("  Normalized: {:.1} jobs/sec/worker", calc_per_worker(jobs_per_sec, 1.0));
        println!();

        // Test 2: Single worker, 10ms work
        let db = Arc::new(setup_db().await);
        let iterations = 50;
        let work_ms = 10;
        
        // Pre-populate
        for _ in 0..iterations {
            insert_single_job(&db).await;
        }
        
        let start = Instant::now();
        let result = db.query_one(sql(
            "SELECT unique_id FROM job WHERE status = 'Pending' ORDER BY created_at ASC LIMIT 1"
        )).await;
        
        if let Some(row) = result.ok().flatten() {
            let unique_id: String = row.try_get("", "unique_id").unwrap();
            tokio::time::sleep(Duration::from_millis(work_ms)).await;
            db.execute_unprepared(&format!(
                "UPDATE job SET status = 'Success' WHERE unique_id = '{}'", unique_id
            )).await.unwrap();
        }
        let elapsed = start.elapsed();
        
        // Extrapolate for full iterations
        let estimated_total_ms = elapsed.as_secs_f64() * 1000.0 * iterations as f64;
        let estimated_jobs_per_sec = 1000.0 / (estimated_total_ms / iterations as f64);
        
        println!("=== Single Worker, 10ms Work ===");
        println!("  Jobs: {} | Workers: 1 | Work: {}ms", iterations, work_ms);
        println!("  Sample Duration: {:?}", elapsed);
        println!("  Estimated Throughput: {:.1} jobs/sec", estimated_jobs_per_sec);
        println!("  Estimated Latency: {:.2} ms/job", estimated_total_ms / iterations as f64);
        println!();

        // Test 3: 10 workers, 10ms work
        let db = Arc::new(setup_db().await);
        let total_jobs = 100;
        let concurrency = 10;
        let barrier = Arc::new(Barrier::new(concurrency));
        
        // Pre-populate
        for _ in 0..total_jobs {
            insert_single_job(&db).await;
        }
        
        let start = Instant::now();
        let mut handles = Vec::new();
        
        for _ in 0..concurrency {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            
            let handle = tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..(total_jobs / concurrency) {
                    let result = db.query_one(sql(
                        "SELECT unique_id FROM job WHERE status = 'Pending' ORDER BY created_at ASC LIMIT 1"
                    )).await;
                    
                    if let Ok(Some(row)) = result {
                        let unique_id: String = row.try_get("", "unique_id").unwrap();
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        db.execute_unprepared(&format!(
                            "UPDATE job SET status = 'Success' WHERE unique_id = '{}'", unique_id
                        )).await.unwrap();
                    }
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        let elapsed = start.elapsed();
        let jobs_per_sec = total_jobs as f64 / elapsed.as_secs_f64();
        let per_worker = calc_per_worker(jobs_per_sec, concurrency as f64);
        let latency = calc_latency_ms(elapsed.as_secs_f64() * 1000.0, total_jobs as f64);
        
        println!("=== 10 Workers, 10ms Work ===");
        println!("  Jobs: {} | Workers: {} | Work: 10ms", total_jobs, concurrency);
        println!("  Duration: {:?}", elapsed);
        println!("  Throughput: {:.1} jobs/sec (total)", jobs_per_sec);
        println!("  Throughput: {:.1} jobs/sec/worker", per_worker);
        println!("  Latency: {:.2} ms/job (avg)", latency);
        println!();

        // Test 4: Bulk insert normalized (per-job overhead)
        let db = setup_db().await;
        let batch_size = 1000;
        let start = Instant::now();
        
        let mut values: Vec<String> = Vec::with_capacity(batch_size);
        let now = Utc::now().naive_utc();
        for i in 0..batch_size {
            let unique_id = format!("norm_{}", i);
            let job_id = Uuid::new_v4().hyphenated().to_string();
            values.push(format!(
                "('{}', '{}', 'http://example.com/bulk', 'GET', '{{}}', 'null', 0, 0, 'Pending', '{}', '{}', '{}')",
                unique_id, job_id, now, now, now
            ));
        }
        
        let insert_sql = format!(
            "INSERT INTO job (unique_id, id, url, method, headers, body, retries, attempts, status, next_run_at, created_at, updated_at) VALUES {}",
            values.join(", ")
        );
        db.execute_unprepared(&insert_sql).await.expect("Bulk insert failed");
        
        let elapsed = start.elapsed();
        let jobs_per_sec = batch_size as f64 / elapsed.as_secs_f64();
        let latency_us = elapsed.as_secs_f64() * 1_000_000.0 / batch_size as f64;
        
        println!("=== Bulk Insert (1000 jobs, single transaction) ===");
        println!("  Jobs: {} | Batch: 1 transaction", batch_size);
        println!("  Duration: {:?}", elapsed);
        println!("  Throughput: {:.0} jobs/sec", jobs_per_sec);
        println!("  Latency: {:.1} µs/job (marginal cost)", latency_us);
        println!();

        // Summary
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║                    SUMMARY                               ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║  Metric                          │ Value                 ║");
        println!("╠══════════════════════════════════╪═══════════════════════╣");
        println!("║  Single insert (no work)         │ {:.0} jobs/sec        ║", jobs_per_sec);
        println!("║  Single worker (10ms work)       │ ~{:.0} jobs/sec        ║", 1000.0 / (10.0 + latency));
        println!("║  Per-worker throughput           │ {:.1} jobs/sec/worker  ║", per_worker);
        println!("║  Bulk insert marginal cost       │ {:.1} µs/job          ║", latency_us);
        println!("╚══════════════════════════════════════════════════════════╝");
    }
}
