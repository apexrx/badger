//! Comprehensive benchmarks for Badger
//! Aligned with BullMQ benchmark methodology for fair comparison

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
    Statement::from_string(DbBackend::Sqlite, stmt.to_string())
}

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");
    
    db.execute_unprepared(
        r#"CREATE TABLE job (
            unique_id TEXT PRIMARY KEY NOT NULL,
            id TEXT NOT NULL,
            url TEXT NOT NULL,
            method TEXT NOT NULL,
            headers TEXT NOT NULL DEFAULT '{}',
            body TEXT NOT NULL DEFAULT 'null',
            retries INTEGER NOT NULL DEFAULT 0,
            attempts INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'Pending',
            next_run_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            check_in TEXT,
            cron TEXT
        )"#
    )
    .await
    .expect("Failed to create table");
    
    db
}

async fn insert_single_job(db: &DatabaseConnection, job_num: usize) -> String {
    let now = Utc::now().naive_utc();
    let unique_id = format!("test_{}", Uuid::new_v4().hyphenated());
    let job_id = Uuid::new_v4().hyphenated().to_string();

    db.execute_unprepared(&format!(
        r#"INSERT INTO job (unique_id, id, url, method, headers, body, retries, attempts, status, next_run_at, created_at, updated_at)
           VALUES ('{}', '{}', 'http://example.com/job/{}', 'GET', '{{}}', 'null', 0, 0, 'Pending', '{}', '{}', '{}')"#,
        unique_id, job_id, job_num, now, now, now
    ))
    .await
    .expect("Failed to insert job");

    unique_id
}

fn black_box(val: f64) {
    std::hint::black_box(val);
}

#[cfg(test)]
mod benchmarks {
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
        
        println!("\n=== Single Job Insertion Benchmark ===");
        println!("Iterations: {}", iterations);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
        println!("Latency (avg): {:.2} µs", elapsed.as_micros() as f64 / iterations as f64);
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
        
        println!("\n=== Concurrent Single Job Insertion Benchmark ===");
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
            let mut insert_sql = String::from("INSERT INTO job (unique_id, id, url, method, headers, body, retries, attempts, status, next_run_at, created_at, updated_at) VALUES ");
            let now = Utc::now().naive_utc();
            
            for i in 0..batch_size {
                let job_num = batch * batch_size + i;
                let unique_id = format!("bulk_{}", Uuid::new_v4().hyphenated());
                let job_id = Uuid::new_v4().hyphenated().to_string();
                
                if i > 0 {
                    insert_sql.push_str(", ");
                }
                insert_sql.push_str(&format!(
                    "('{}', '{}', 'http://example.com/bulk/{}', 'GET', '{{}}', 'null', 0, 0, 'Pending', '{}', '{}', '{}')",
                    unique_id, job_id, job_num, now, now, now
                ));
            }
            
            db.execute_unprepared(&insert_sql).await.expect("Bulk insert failed");
        }
        
        let elapsed = start.elapsed();
        let jobs_per_sec = total_jobs as f64 / elapsed.as_secs_f64();
        
        println!("\n=== Bulk Job Insertion Benchmark ===");
        println!("Total Jobs: {}", total_jobs);
        println!("Batch Size: {}", batch_size);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_bulk_job_insertion() {
        benchmark_bulk_job_insertion_inner().await;
    }

    async fn benchmark_concurrent_bulk_insertion_inner() {
        let db = Arc::new(setup_db().await);
        let total_jobs = 10000;
        let concurrency = 10;
        let jobs_per_inserter = total_jobs / concurrency;
        let batch_size = 100;
        let barrier = Arc::new(Barrier::new(concurrency));
        
        let start = Instant::now();
        
        let mut handles = Vec::new();
        for batch in 0..concurrency {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            
            let handle = tokio::spawn(async move {
                barrier.wait().await;
                
                for batch_num in 0..(jobs_per_inserter / batch_size) {
                    let mut insert_sql = String::from("INSERT INTO job (unique_id, id, url, method, headers, body, retries, attempts, status, next_run_at, created_at, updated_at) VALUES ");
                    let now = Utc::now().naive_utc();
                    
                    for i in 0..batch_size {
                        let job_num = batch * jobs_per_inserter + batch_num * batch_size + i;
                        let unique_id = format!("concurrent_bulk_{}", Uuid::new_v4().hyphenated());
                        let job_id = Uuid::new_v4().hyphenated().to_string();
                        
                        if i > 0 {
                            insert_sql.push_str(", ");
                        }
                        insert_sql.push_str(&format!(
                            "('{}', '{}', 'http://example.com/concurrent/{}', 'GET', '{{}}', 'null', 0, 0, 'Pending', '{}', '{}', '{}')",
                            unique_id, job_id, job_num, now, now, now
                        ));
                    }
                    
                    db.execute_unprepared(&insert_sql).await.expect("Bulk insert failed");
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        let elapsed = start.elapsed();
        let jobs_per_sec = total_jobs as f64 / elapsed.as_secs_f64();
        
        println!("\n=== Concurrent Bulk Insertion Benchmark ===");
        println!("Total Jobs: {}", total_jobs);
        println!("Concurrency: {}", concurrency);
        println!("Batch Size: {}", batch_size);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_concurrent_bulk_insertion() {
        benchmark_concurrent_bulk_insertion_inner().await;
    }

    async fn benchmark_job_processing_10ms_work_inner() {
        let db = Arc::new(setup_db().await);
        let total_jobs = 100;
        let concurrency = 10;
        let barrier = Arc::new(Barrier::new(concurrency));
        
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
                        "SELECT unique_id FROM job WHERE status = 'Pending' LIMIT 1"
                    )).await.expect("Query failed");
                    
                    if let Some(row) = result {
                        let unique_id: String = row.try_get("", "unique_id").unwrap();
                        tokio::time::sleep(Duration::from_millis(10)).await;
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
        
        println!("\n=== Job Processing Benchmark (10ms work) ===");
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
                        "SELECT unique_id FROM job WHERE status = 'Pending' LIMIT 1"
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
        
        println!("\n=== Pure Queue Overhead Benchmark ===");
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
                        "SELECT unique_id FROM job WHERE status = 'Pending' LIMIT 1"
                    )).await.expect("Query failed");
                    
                    if let Some(row) = result {
                        let unique_id: String = row.try_get("", "unique_id").unwrap();
                        
                        let now = Utc::now().naive_utc();
                        db.execute_unprepared(&format!(
                            "UPDATE job SET status = 'Running', updated_at = '{}' WHERE unique_id = '{}'",
                            now, unique_id
                        )).await.expect("Failed to claim job");
                        
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
        
        println!("\n=== CPU-Bound Processing Benchmark (~1ms CPU) ===");
        println!("Total Jobs: {}", total_jobs);
        println!("Concurrency: {}", concurrency);
        println!("Duration: {:?}", elapsed);
        println!("Throughput: {:.2} jobs/sec", jobs_per_sec);
    }

    #[tokio::test]
    async fn benchmark_cpu_bound_processing() {
        benchmark_cpu_bound_processing_inner().await;
    }

    async fn benchmark_batch_sizes_inner() {
        let batch_sizes = vec![100, 250, 500, 1000, 2000];
        
        println!("\n=== Batch Size Comparison Benchmark ===");
        println!("{:<15} {:<20} {:<20}", "Batch Size", "Throughput (jobs/sec)", "Duration");
        println!("{:-<60}", "");
        
        for batch_size in batch_sizes {
            let db = setup_db().await;
            let total_jobs = batch_size * 10;
            
            let start = Instant::now();
            
            for batch in 0..(total_jobs / batch_size) {
                let mut insert_sql = String::from("INSERT INTO job (unique_id, id, url, method, headers, body, retries, attempts, status, next_run_at, created_at, updated_at) VALUES ");
                let now = Utc::now().naive_utc();
                
                for i in 0..batch_size {
                    let job_num = batch * batch_size + i;
                    let unique_id = format!("batch_size_{}_{}", batch_size, Uuid::new_v4().hyphenated());
                    let job_id = Uuid::new_v4().hyphenated().to_string();
                    
                    if i > 0 {
                        insert_sql.push_str(", ");
                    }
                    insert_sql.push_str(&format!(
                        "('{}', '{}', 'http://example.com/batch{}/{}', 'GET', '{{}}', 'null', 0, 0, 'Pending', '{}', '{}', '{}')",
                        unique_id, job_id, batch_size, job_num, now, now, now
                    ));
                }
                
                db.execute_unprepared(&insert_sql).await.expect("Bulk insert failed");
            }
            
            let elapsed = start.elapsed();
            let jobs_per_sec = total_jobs as f64 / elapsed.as_secs_f64();
            
            println!("{:<15} {:<20.2} {:?}", batch_size, jobs_per_sec, elapsed);
        }
    }

    #[tokio::test]
    async fn benchmark_batch_sizes() {
        benchmark_batch_sizes_inner().await;
    }

    #[tokio::test]
    async fn run_full_benchmark_suite() {
        println!("\n");
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║         BADGER COMPREHENSIVE BENCHMARK SUITE             ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        println!("║  System: openSUSE Tumbleweed                             ║");
        println!("║  CPU: AMD Ryzen 5 5600H                                  ║");
        println!("║  RAM: 16GB                                               ║");
        println!("║  Database: SQLite (in-memory)                            ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        
        benchmark_single_job_insertion_inner().await;
        benchmark_concurrent_single_job_insertion_inner().await;
        benchmark_bulk_job_insertion_inner().await;
        benchmark_concurrent_bulk_insertion_inner().await;
        benchmark_job_processing_10ms_work_inner().await;
        benchmark_pure_queue_overhead_inner().await;
        benchmark_cpu_bound_processing_inner().await;
        benchmark_batch_sizes_inner().await;
        
        println!("\n=== Benchmark Suite Complete ===");
    }
}
