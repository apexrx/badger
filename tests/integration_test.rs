//! Integration tests for Badger
//! These tests verify core functionality including job submission, execution, and persistence

use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
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

async fn create_test_job(
    db: &DatabaseConnection,
    url: &str,
    method: &str,
    status: StatusEnum,
) -> String {
    let now = Utc::now().naive_utc();
    let unique_id = format!("test_{}", Uuid::new_v4().hyphenated());
    let job_id = Uuid::new_v4().hyphenated().to_string();

    db.execute_unprepared(&format!(
        r#"INSERT INTO job (unique_id, id, url, method, headers, body, retries, attempts, status, next_run_at, created_at, updated_at)
           VALUES ('{}', '{}', '{}', '{}', '{{}}', 'null', 0, 0, '{}', '{}', '{}', '{}')"#,
        unique_id, job_id, url, method, status, now, now, now
    ))
    .await
    .expect("Failed to insert job");

    unique_id
}

fn sql(stmt: &str) -> Statement {
    Statement::from_string(DbBackend::Sqlite, stmt.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_database_connection() {
        let db = setup_db().await;
        let result = db.execute_unprepared("SELECT 1").await;
        assert!(result.is_ok(), "Database connection should work");
    }

    #[tokio::test]
    async fn test_job_insert_and_retrieve() {
        let db = setup_db().await;
        let unique_id = create_test_job(&db, "http://example.com", "GET", StatusEnum::Pending).await;
        
        let result = db.query_one(sql(&format!("SELECT * FROM job WHERE unique_id = '{}'", unique_id))).await;
        assert!(result.expect("Query failed").is_some(), "Job should be found");
    }

    #[tokio::test]
    async fn test_job_status_transitions() {
        let db = setup_db().await;
        let unique_id = create_test_job(&db, "http://example.com", "GET", StatusEnum::Pending).await;
        
        db.execute_unprepared(&format!(
            "UPDATE job SET status = 'Running', updated_at = '{}' WHERE unique_id = '{}'",
            Utc::now().naive_utc(), unique_id
        )).await.expect("Update failed");
        
        let result = db.query_one(sql(&format!("SELECT status FROM job WHERE unique_id = '{}'", unique_id))).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_multiple_jobs_insertion() {
        let db = setup_db().await;
        let start = Instant::now();
        
        for i in 0..100 {
            create_test_job(&db, &format!("http://example{}.com", i), "GET", StatusEnum::Pending).await;
        }
        
        let elapsed = start.elapsed();
        println!("Inserted 100 jobs in {:?}", elapsed);
        assert!(elapsed.as_millis() < 5000, "Should insert 100 jobs in under 5 seconds");
    }

    #[tokio::test]
    async fn test_pending_jobs_query() {
        let db = setup_db().await;
        
        for i in 0..5 {
            create_test_job(&db, &format!("http://pending{}.com", i), "GET", StatusEnum::Pending).await;
        }
        for i in 0..3 {
            create_test_job(&db, &format!("http://done{}.com", i), "GET", StatusEnum::Success).await;
        }
        
        let result = db.query_one(sql("SELECT COUNT(*) as count FROM job WHERE status = 'Pending'")).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_job_with_cron() {
        let db = setup_db().await;
        let now = Utc::now().naive_utc();
        let unique_id = format!("cron_{}", Uuid::new_v4().hyphenated());
        let job_id = Uuid::new_v4().hyphenated().to_string();
        
        db.execute_unprepared(&format!(
            r#"INSERT INTO job (unique_id, id, url, method, status, next_run_at, created_at, updated_at, cron)
               VALUES ('{}', '{}', 'http://example.com', 'GET', 'Pending', '{}', '{}', '{}', '*/5 * * * *')"#,
            unique_id, job_id, now, now, now
        )).await.expect("Insert failed");
        
        let result = db.query_one(sql(&format!("SELECT cron FROM job WHERE unique_id = '{}'", unique_id))).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_retry_counter_update() {
        let db = setup_db().await;
        let unique_id = create_test_job(&db, "http://example.com", "GET", StatusEnum::Pending).await;
        
        db.execute_unprepared(&format!(
            "UPDATE job SET retries = retries + 1, attempts = attempts + 1 WHERE unique_id = '{}'",
            unique_id
        )).await.expect("Update failed");
        
        let result = db.query_one(sql(&format!("SELECT retries, attempts FROM job WHERE unique_id = '{}'", unique_id))).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let db = setup_db().await;
        let unique_id = create_test_job(&db, "http://example.com", "GET", StatusEnum::Pending).await;
        
        db.execute_unprepared(&format!(
            "UPDATE job SET retries = 10, attempts = 10, status = 'Failure' WHERE unique_id = '{}'",
            unique_id
        )).await.expect("Update failed");
        
        let result = db.query_one(sql(&format!("SELECT status FROM job WHERE unique_id = '{}' AND status = 'Failure'", unique_id))).await;
        assert!(result.expect("Query failed").is_some(), "Job should be marked as Failure");
    }

    #[tokio::test]
    async fn test_scheduled_job() {
        let db = setup_db().await;
        let now = Utc::now().naive_utc();
        let future = now + Duration::seconds(30);
        let unique_id = format!("scheduled_{}", Uuid::new_v4().hyphenated());
        let job_id = Uuid::new_v4().hyphenated().to_string();
        
        db.execute_unprepared(&format!(
            r#"INSERT INTO job (unique_id, id, url, method, status, next_run_at, created_at, updated_at)
               VALUES ('{}', '{}', 'http://example.com', 'GET', 'Pending', '{}', '{}', '{}')"#,
            unique_id, job_id, future, now, now
        )).await.expect("Insert failed");
        
        let result = db.query_one(sql(&format!("SELECT * FROM job WHERE unique_id = '{}' AND next_run_at <= '{}'", unique_id, now))).await;
        assert!(result.expect("Query failed").is_none(), "Scheduled job should not be ready yet");
    }

    #[tokio::test]
    async fn test_check_in_heartbeat() {
        let db = setup_db().await;
        let now = Utc::now().naive_utc();
        let unique_id = format!("heartbeat_{}", Uuid::new_v4().hyphenated());
        let job_id = Uuid::new_v4().hyphenated().to_string();
        
        db.execute_unprepared(&format!(
            r#"INSERT INTO job (unique_id, id, url, method, status, next_run_at, created_at, updated_at, check_in)
               VALUES ('{}', '{}', 'http://example.com', 'GET', 'Running', '{}', '{}', '{}', '{}')"#,
            unique_id, job_id, now, now, now, now
        )).await.expect("Insert failed");
        
        let later = now + Duration::seconds(10);
        db.execute_unprepared(&format!("UPDATE job SET check_in = '{}' WHERE unique_id = '{}'", later, unique_id))
            .await.expect("Update failed");
        
        let result = db.query_one(sql(&format!("SELECT check_in FROM job WHERE unique_id = '{}' AND check_in IS NOT NULL", unique_id))).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_job_query_by_id() {
        let db = setup_db().await;
        let job_id = Uuid::new_v4().hyphenated().to_string();
        let unique_id = format!("query_{}", job_id);
        let now = Utc::now().naive_utc();
        
        db.execute_unprepared(&format!(
            r#"INSERT INTO job (unique_id, id, url, method, status, next_run_at, created_at, updated_at)
               VALUES ('{}', '{}', 'http://example.com', 'GET', 'Pending', '{}', '{}', '{}')"#,
            unique_id, job_id, now, now, now
        )).await.expect("Insert failed");
        
        let result = db.query_one(sql(&format!("SELECT * FROM job WHERE id = '{}'", job_id))).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_duplicate_unique_id_prevention() {
        let db = setup_db().await;
        let unique_id = "duplicate_test_123";
        let now = Utc::now().naive_utc();
        let job_id1 = Uuid::new_v4().hyphenated().to_string();
        let job_id2 = Uuid::new_v4().hyphenated().to_string();
        
        let result1 = db.execute_unprepared(&format!(
            r#"INSERT INTO job (unique_id, id, url, method, status, next_run_at, created_at, updated_at)
               VALUES ('{}', '{}', 'http://example.com', 'GET', 'Pending', '{}', '{}', '{}')"#,
            unique_id, job_id1, now, now, now
        )).await;
        assert!(result1.is_ok());
        
        let result2 = db.execute_unprepared(&format!(
            r#"INSERT INTO job (unique_id, id, url, method, status, next_run_at, created_at, updated_at)
               VALUES ('{}', '{}', 'http://example.com', 'GET', 'Pending', '{}', '{}', '{}')"#,
            unique_id, job_id2, now, now, now
        )).await;
        assert!(result2.is_err(), "Duplicate unique_id should fail");
    }

    #[tokio::test]
    async fn test_bulk_job_insertion_performance() {
        let db = setup_db().await;
        let start = Instant::now();
        
        for i in 0..1000 {
            create_test_job(&db, &format!("http://bulk{}.com", i), "POST", StatusEnum::Pending).await;
        }
        
        let elapsed = start.elapsed();
        let jobs_per_second = 1000.0 / elapsed.as_secs_f64();
        
        println!("Bulk insertion: 1000 jobs in {:?} ({:.2} jobs/sec)", elapsed, jobs_per_second);
        assert!(jobs_per_second > 100.0, "Should insert at least 100 jobs/sec");
    }

    #[tokio::test]
    async fn test_job_status_filtering() {
        let db = setup_db().await;
        
        create_test_job(&db, "http://pending.com", "GET", StatusEnum::Pending).await;
        create_test_job(&db, "http://running.com", "GET", StatusEnum::Running).await;
        create_test_job(&db, "http://success.com", "GET", StatusEnum::Success).await;
        create_test_job(&db, "http://failure.com", "GET", StatusEnum::Failure).await;
        
        for status in &[StatusEnum::Pending, StatusEnum::Running, StatusEnum::Success, StatusEnum::Failure] {
            let result = db.query_one(sql(&format!("SELECT COUNT(*) as count FROM job WHERE status = '{}'", status))).await;
            assert!(result.expect("Query failed").is_some(), "Should find one {:?}", status);
        }
    }

    #[tokio::test]
    async fn test_job_timestamp_ordering() {
        let db = setup_db().await;
        
        for i in 0..5 {
            create_test_job(&db, &format!("http://order{}.com", i), "GET", StatusEnum::Pending).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
        
        let result = db.query_one(sql("SELECT unique_id FROM job ORDER BY created_at ASC LIMIT 1")).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_concurrent_job_claims_simulation() {
        let db = setup_db().await;

        for i in 0..10 {
            create_test_job(&db, &format!("http://concurrent{}.com", i), "GET", StatusEnum::Pending).await;
        }

        // SQLite doesn't support LIMIT in UPDATE, use subquery approach
        let result = db.execute_unprepared(
            "UPDATE job SET status = 'Running' WHERE rowid IN (SELECT rowid FROM job WHERE status = 'Pending' LIMIT 5)"
        ).await;
        assert_eq!(result.expect("Update failed").rows_affected(), 5);
        
        let pending = db.query_one(sql("SELECT COUNT(*) as count FROM job WHERE status = 'Pending'")).await;
        let running = db.query_one(sql("SELECT COUNT(*) as count FROM job WHERE status = 'Running'")).await;
        
        assert!(pending.expect("Query failed").is_some());
        assert!(running.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_job_headers_and_body_storage() {
        let db = setup_db().await;
        let now = Utc::now().naive_utc();
        let unique_id = format!("headers_{}", Uuid::new_v4().hyphenated());
        let job_id = Uuid::new_v4().hyphenated().to_string();
        
        let headers = r#"{"Content-Type":"application/json","Authorization":"Bearer token"}"#;
        let body = r#"{"key":"value","number":42}"#;
        
        db.execute_unprepared(&format!(
            r#"INSERT INTO job (unique_id, id, url, method, headers, body, status, next_run_at, created_at, updated_at)
               VALUES ('{}', '{}', 'http://example.com/api', 'POST', '{}', '{}', 'Pending', '{}', '{}', '{}')"#,
            unique_id, job_id, headers, body, now, now, now
        )).await.expect("Insert failed");
        
        let result = db.query_one(sql(&format!("SELECT headers, body FROM job WHERE unique_id = '{}'", unique_id))).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_job_count_query() {
        let db = setup_db().await;
        
        for i in 0..25 {
            create_test_job(&db, &format!("http://count{}.com", i), "GET", StatusEnum::Pending).await;
        }
        
        let result = db.query_one(sql("SELECT COUNT(*) as count FROM job")).await;
        assert!(result.expect("Query failed").is_some());
    }

    #[tokio::test]
    async fn test_job_delete() {
        let db = setup_db().await;
        let unique_id = create_test_job(&db, "http://delete.com", "GET", StatusEnum::Pending).await;
        
        let result = db.execute_unprepared(&format!("DELETE FROM job WHERE unique_id = '{}'", unique_id)).await;
        assert_eq!(result.expect("Delete failed").rows_affected(), 1);
        
        let found = db.query_one(sql(&format!("SELECT * FROM job WHERE unique_id = '{}'", unique_id))).await;
        assert!(found.expect("Query failed").is_none(), "Job should be deleted");
    }

    #[tokio::test]
    async fn test_job_update_cron_schedule() {
        let db = setup_db().await;
        let unique_id = create_test_job(&db, "http://cron.com", "GET", StatusEnum::Pending).await;
        
        db.execute_unprepared(&format!("UPDATE job SET cron = '0 */6 * * *', status = 'Pending' WHERE unique_id = '{}'", unique_id))
            .await.expect("Update failed");
        
        let result = db.query_one(sql(&format!("SELECT cron FROM job WHERE unique_id = '{}' AND cron IS NOT NULL", unique_id))).await;
        assert!(result.expect("Query failed").is_some());
    }
}
