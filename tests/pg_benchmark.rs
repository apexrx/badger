//! PostgreSQL benchmarks for Badger
//! Real-world performance testing with PostgreSQL database

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

    // Truncate table for clean benchmark
    db.execute_unprepared("TRUNCATE TABLE job RESTART IDENTITY CASCADE;")
        .await
        .expect("Failed to truncate table");

    db
}

async fn insert_single_job(db: &DatabaseConnection, _job_num: usize) -> String {
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

fn black_box(val: f64) {
    std::hint::black_box(val);
}

#[cfg(test)]
mod pg_benchmarks {
    use super::*;
    use tokio::sync::Barrier;

    async fn benchmark_single_job_insertion_inner() {
        let db = setup_db().await;
        let iterations = 1000;
        
        let start = Instant::now();
        for i in 0..iterations {
            insert_single_job(&db, i).await;
        }
        let elapsed = start.elapsed();
        
        let jobs_per_sec = iterations as f64 / elapsed.as_secs_f64();
        
        println!("\n=== [PostgreSQL] Single Job Insertion Benchmark ===");
        println!("Iterations: {}", iterations);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
        println!("Latency (avg): {:.2} us", elapsed.as_micros() as f64 / iterations as f64);
    }

    #[tokio::test]
    async fn benchmark_single_job_insertion() {
        benchmark_single_job_insertion_inner().await;
    }

    async fn benchmark_concurrent_single_job_insertion_inner() {
        let db = Arc::new(setup_db().await);
        let iterations = 1000;
        let concurrency = 10;
        let barrier = Arc::new(Barrier::new(concurrency));
        
        let start = Instant::now();
        
        let mut handles = Vec::new();
        for batch in 0..concurrency {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            
            let handle = tokio::spawn(async move {
                barrier.wait().await;
                for i in 0..(iterations / concurrency) {
                    insert_single_job(&db, batch * (iterations / concurrency) + i).await;
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        let elapsed = start.elapsed();
        let jobs_per_sec = iterations as f64 / elapsed.as_secs_f64();
        
        println!("\n=== [PostgreSQL] Concurrent Single Job Insertion Benchmark ===");
        println!("Iterations: {}", iterations);
        println!("Concurrency: {}", concurrency);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_concurrent_single_job_insertion() {
        benchmark_concurrent_single_job_insertion_inner().await;
    }

    async fn benchmark_bulk_job_insertion_inner() {
        let db = setup_db().await;
        let total_jobs = 10000;
        let batch_size = 1000;
        
        let start = Instant::now();
        
        for batch in 0..(total_jobs / batch_size) {
            let now = Utc::now().naive_utc();
            
            // Use batch insert with multiple VALUES
            let mut values: Vec<String> = Vec::with_capacity(batch_size);
            for i in 0..batch_size {
                let unique_id = format!("bulk_{}_{}", batch, i);
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
        }
        
        let elapsed = start.elapsed();
        let jobs_per_sec = total_jobs as f64 / elapsed.as_secs_f64();
        
        println!("\n=== [PostgreSQL] Bulk Job Insertion Benchmark ===");
        println!("Total Jobs: {}", total_jobs);
        println!("Batch Size: {}", batch_size);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_bulk_job_insertion() {
        benchmark_bulk_job_insertion_inner().await;
    }

    async fn benchmark_job_processing_10ms_work_inner() {
        let db = Arc::new(setup_db().await);
        // Use standardized worker count for fair comparison
        // BullMQ/Sidekiq typically use concurrency=10 for benchmarks
        let total_jobs = 100;
        let concurrency = 10;  // Standardized for comparison
        let barrier = Arc::new(Barrier::new(concurrency));
        
        // Pre-populate jobs
        for i in 0..total_jobs {
            insert_single_job(&db, i).await;
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
                    )).await.expect("Query failed");
                    
                    if let Some(row) = result {
                        let unique_id: String = row.try_get("", "unique_id").unwrap();
                        
                        // Simulate 10ms work
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        
                        // Complete job
                        let now = Utc::now().naive_utc();
                        db.execute_unprepared(&format!(
                            "UPDATE job SET status = 'Success', updated_at = '{}' WHERE unique_id = '{}'",
                            now, unique_id
                        )).await.expect("Update failed");
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
        
        println!("\n=== [PostgreSQL] Job Processing Benchmark (10ms work) ===");
        println!("Total Jobs: {}", total_jobs);
        println!("Concurrency: {}", concurrency);
        println!("Work per job: 10ms");
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_job_processing_10ms_work() {
        benchmark_job_processing_10ms_work_inner().await;
    }

    async fn benchmark_pure_queue_overhead_inner() {
        let db = Arc::new(setup_db().await);
        let total_jobs = 500;
        let concurrency = 10;
        let barrier = Arc::new(Barrier::new(concurrency));
        
        // Pre-populate jobs
        for i in 0..total_jobs {
            insert_single_job(&db, i).await;
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
                    )).await.expect("Query failed");
                    
                    if let Some(row) = result {
                        let unique_id: String = row.try_get("", "unique_id").unwrap();
                        
                        let now = Utc::now().naive_utc();
                        db.execute_unprepared(&format!(
                            "UPDATE job SET status = 'Success', updated_at = '{}' WHERE unique_id = '{}'",
                            now, unique_id
                        )).await.expect("Update failed");
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
        
        println!("\n=== [PostgreSQL] Pure Queue Overhead Benchmark ===");
        println!("Total Jobs: {}", total_jobs);
        println!("Concurrency: {}", concurrency);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_pure_queue_overhead() {
        benchmark_pure_queue_overhead_inner().await;
    }

    async fn benchmark_cpu_bound_processing_inner() {
        let db = Arc::new(setup_db().await);
        let total_jobs = 200;
        let concurrency = 10;
        let barrier = Arc::new(Barrier::new(concurrency));
        
        // Pre-populate jobs
        for i in 0..total_jobs {
            insert_single_job(&db, i).await;
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
                    )).await.expect("Query failed");
                    
                    if let Some(row) = result {
                        let unique_id: String = row.try_get("", "unique_id").unwrap();
                        
                        let now = Utc::now().naive_utc();
                        db.execute_unprepared(&format!(
                            "UPDATE job SET status = 'Running', updated_at = '{}' WHERE unique_id = '{}'",
                            now, unique_id
                        )).await.expect("Failed to claim job");
                        
                        // Simulate CPU-bound work (~1ms: sin/cos operations)
                        let mut result: f64 = 0.0;
                        for i in 0..1000 {
                            result += (i as f64).sin() * (i as f64).cos();
                        }
                        black_box(result);
                        
                        db.execute_unprepared(&format!(
                            "UPDATE job SET status = 'Success', updated_at = '{}' WHERE unique_id = '{}'",
                            Utc::now().naive_utc(), unique_id
                        )).await.expect("Failed to complete job");
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
        
        println!("\n=== [PostgreSQL] CPU-Bound Processing Benchmark (~1ms CPU) ===");
        println!("Total Jobs: {}", total_jobs);
        println!("Concurrency: {}", concurrency);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_cpu_bound_processing() {
        benchmark_cpu_bound_processing_inner().await;
    }

    #[tokio::test]
    async fn run_full_pg_benchmark_suite() {
        println!("\n");
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║      BADGER POSTGRESQL BENCHMARK SUITE                   ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║  System: openSUSE Tumbleweed                             ║");
        println!("║  CPU: AMD Ryzen 5 5600H                                  ║");
        println!("║  RAM: 16GB                                               ║");
        println!("║  Database: PostgreSQL 15+ (localhost)                    ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        
        benchmark_single_job_insertion_inner().await;
        benchmark_concurrent_single_job_insertion_inner().await;
        benchmark_bulk_job_insertion_inner().await;
        benchmark_job_processing_10ms_work_inner().await;
        benchmark_pure_queue_overhead_inner().await;
        benchmark_cpu_bound_processing_inner().await;
        
        println!("\n=== PostgreSQL Benchmark Suite Complete ===");
    }
}
