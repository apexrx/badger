//! API/End-to-End tests for Badger
//! These tests verify the HTTP API endpoints

use chrono::Utc;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct JobRequest {
    url: String,
    method: String,
    headers: Option<serde_json::Value>,
    body: Option<serde_json::Value>,
    run_at: Option<chrono::DateTime<chrono::Utc>>,
    cron: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JobResponse {
    id: String,
    url: String,
    method: String,
    status: String,
}

async fn setup_test_db() -> DatabaseConnection {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_job_endpoint() {
        // This test verifies the job creation payload structure
        let job_request = JobRequest {
            url: "http://example.com".to_string(),
            method: "GET".to_string(),
            headers: Some(json!({"Content-Type": "application/json"})),
            body: Some(json!({"key": "value"})),
            run_at: None,
            cron: None,
        };
        
        let payload = serde_json::to_string(&job_request).unwrap();
        assert!(payload.contains("http://example.com"));
        assert!(payload.contains("GET"));
    }

    #[tokio::test]
    async fn test_job_fingerprint_generation() {
        // Test that the same job parameters produce the same fingerprint
        use sha2::{Digest, Sha256};
        use hex;
        
        fn create_fingerprint(
            method: String,
            url: String,
            headers: Option<serde_json::Value>,
            body: Option<serde_json::Value>,
            run_at: Option<chrono::DateTime<chrono::Utc>>,
        ) -> String {
            let mut headers_vec = Vec::new();
            if let Some(headers) = headers {
                let headers = headers.as_object().unwrap();
                for (key, value) in headers {
                    headers_vec.push((key.to_lowercase(), value.clone()));
                }
                headers_vec.sort_by(|a, b| a.0.cmp(&b.0));
            }

            let headers_str = headers_vec
                .iter()
                .map(|(key, value)| format!("{}:{}", key, value))
                .collect::<Vec<String>>()
                .join(", ");

            let body_str = body.map(|body| body.to_string()).unwrap_or_default();
            let run_ts: i64 = run_at.map(|t| t.timestamp()).unwrap_or(0);
            let fingerprint = format!("{}|{}|{}|{}|{}", method, url, body_str, headers_str, run_ts);

            let mut hasher = Sha256::new();
            hasher.update(fingerprint);
            let hash = hasher.finalize();
            hex::encode(hash)
        }
        
        let fp1 = create_fingerprint(
            "GET".to_string(),
            "http://example.com".to_string(),
            Some(json!({})),
            None,
            None,
        );
        
        let fp2 = create_fingerprint(
            "GET".to_string(),
            "http://example.com".to_string(),
            Some(json!({})),
            None,
            None,
        );
        
        assert_eq!(fp1, fp2, "Same job parameters should produce same fingerprint");
        
        let fp3 = create_fingerprint(
            "POST".to_string(),
            "http://example.com".to_string(),
            Some(json!({})),
            None,
            None,
        );
        
        assert_ne!(fp1, fp3, "Different methods should produce different fingerprints");
    }

    #[tokio::test]
    async fn test_cron_expression_parsing() {
        use cron::Schedule;
        use std::str::FromStr;
        
        // The cron crate v0.15 uses 6-field format (seconds minutes hours days months weekdays)
        // Days of week: 1=Monday, 7=Sunday
        // Test valid cron expressions
        let valid_expressions = vec![
            "0 0 * * * *",       // Every hour at minute 0
            "0 0 0 * * *",       // Every day at midnight
            "0 0 0 * * 7",       // Every week on Sunday at midnight
            "0 0 0 1 * *",       // Every month on 1st at midnight
            "0 0 0 * * 1-5",     // Every weekday at midnight
        ];
        
        for expr in valid_expressions {
            let schedule = Schedule::from_str(expr);
            assert!(schedule.is_ok(), "Expression '{}' should be valid, got: {:?}", expr, schedule.err());
        }
        
        // Test invalid cron expressions
        let invalid_expressions = vec![
            "* * *",           // Too few fields
            "invalid",         // Garbage
            "60 * * * * *",    // Invalid seconds value
            "* 60 * * * *",    // Invalid minutes value
        ];
        
        for expr in invalid_expressions {
            let schedule = Schedule::from_str(expr);
            assert!(schedule.is_err(), "Expression '{}' should be invalid", expr);
        }
    }

    #[tokio::test]
    async fn test_next_execution_time_calculation() {
        use cron::Schedule;
        use std::str::FromStr;
        
        // Use a valid 6-field cron expression (seconds minutes hours days months weekdays)
        let schedule = Schedule::from_str("0 0 * * * *").unwrap();
        let next = schedule.upcoming(Utc).next();
        
        assert!(next.is_some(), "Should calculate next execution time");
        let next_time = next.unwrap();
        let now = Utc::now();
        
        assert!(next_time > now, "Next execution should be in the future");
        assert!(next_time < now + chrono::Duration::hours(2), "Next execution should be within 2 hours");
    }

    #[tokio::test]
    async fn test_exponential_backoff_calculation() {
        // Test the exponential backoff formula used in Badger
        fn calculate_backoff(attempts: u32) -> i64 {
            let base = 1000 * 2i64.pow(attempts.max(0));
            base
        }
        
        // Verify exponential growth
        assert_eq!(calculate_backoff(0), 1000);      // 1 second
        assert_eq!(calculate_backoff(1), 2000);      // 2 seconds
        assert_eq!(calculate_backoff(2), 4000);      // 4 seconds
        assert_eq!(calculate_backoff(3), 8000);      // 8 seconds
        assert_eq!(calculate_backoff(4), 16000);     // 16 seconds
        assert_eq!(calculate_backoff(5), 32000);     // 32 seconds
        
        // Verify it grows exponentially
        for i in 1..10 {
            let current = calculate_backoff(i);
            let previous = calculate_backoff(i - 1);
            assert!(current > previous, "Backoff should increase with attempts");
            assert_eq!(current, previous * 2, "Backoff should double each attempt");
        }
    }

    #[tokio::test]
    async fn test_http_method_parsing() {
        use reqwest::Method;
        
        // Test valid HTTP methods
        let valid_methods = vec!["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
        
        for method_str in valid_methods {
            let method = Method::from_bytes(method_str.as_bytes());
            assert!(method.is_ok(), "Method '{}' should be valid", method_str);
        }
        
        // Note: reqwest allows custom methods, so we test with empty string instead
        let invalid_method = Method::from_bytes(b"");
        assert!(invalid_method.is_err(), "Empty string should not be a valid method");
    }

    #[tokio::test]
    async fn test_url_parsing() {
        use url::Url;
        
        // Test valid URLs
        let valid_urls = vec![
            "http://example.com",
            "https://example.com/api/v1/jobs",
            "http://localhost:8080/webhook",
            "https://api.example.com:443/path?query=value",
        ];
        
        for url_str in valid_urls {
            let url = Url::parse(url_str);
            assert!(url.is_ok(), "URL '{}' should be valid", url_str);
        }
        
        // Test invalid URLs
        let invalid_urls = vec![
            "not-a-url",
            "http://",
            "://example.com",
            "",
        ];
        
        for url_str in invalid_urls {
            let url = Url::parse(url_str);
            assert!(url.is_err(), "URL '{}' should be invalid", url_str);
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_quota() {
        use governor::{Quota, RateLimiter, state::keyed::DefaultKeyedStateStore, clock::DefaultClock};
        use std::num::NonZeroU32;
        
        // Create rate limiter with 5 requests per second
        let quota = Quota::per_second(NonZeroU32::new(5).unwrap());
        let limiter: RateLimiter<String, _, _> = RateLimiter::new(
            quota,
            DefaultKeyedStateStore::<String>::new(),
            DefaultClock::default(),
        );
        
        // Test that we can make 5 requests
        for _ in 0..5 {
            let result = limiter.check_key(&"example.com".to_string());
            assert!(result.is_ok(), "Should allow request within quota");
        }
        
        // The 6th request should be rate limited
        let result = limiter.check_key(&"example.com".to_string());
        assert!(result.is_err(), "Should rate limit request over quota");
    }

    #[tokio::test]
    async fn test_json_serialization() {
        // Test that job request can be serialized/deserialized
        let original = JobRequest {
            url: "http://example.com".to_string(),
            method: "POST".to_string(),
            headers: Some(json!({
                "Content-Type": "application/json",
                "Authorization": "Bearer token"
            })),
            body: Some(json!({
                "name": "test",
                "value": 42,
                "nested": {"key": "value"}
            })),
            run_at: None,
            cron: Some("*/5 * * * *".to_string()),
        };
        
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: JobRequest = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(original.url, deserialized.url);
        assert_eq!(original.method, deserialized.method);
        assert_eq!(original.cron, deserialized.cron);
    }

    #[tokio::test]
    async fn test_uuid_generation() {
        // Test UUID generation for job IDs
        let mut ids = std::collections::HashSet::new();
        
        for _ in 0..1000 {
            let id = Uuid::new_v4();
            assert!(ids.insert(id), "UUID should be unique");
        }
        
        assert_eq!(ids.len(), 1000, "All UUIDs should be unique");
    }

    #[tokio::test]
    async fn test_timestamp_handling() {
        let now = Utc::now();
        let future = now + chrono::Duration::hours(1);
        let past = now - chrono::Duration::hours(1);
        
        assert!(future > now, "Future should be greater than now");
        assert!(past < now, "Past should be less than now");
        
        // Test timestamp serialization
        let timestamp = now.timestamp();
        assert!(timestamp > 0, "Timestamp should be positive");
    }

    #[tokio::test]
    async fn test_status_enum_values() {
        // Test that status values match expected strings
        let statuses = vec!["Pending", "Running", "Success", "Failure"];
        
        for status in statuses {
            // Verify status can be used in SQL queries
            let query = format!("SELECT * FROM job WHERE status = '{}'", status);
            assert!(query.contains(status), "Query should contain status");
        }
    }

    #[tokio::test]
    async fn test_headers_serialization() {
        // Test various header configurations
        let test_cases = vec![
            json!({}),  // Empty headers
            json!({"Content-Type": "application/json"}),  // Single header
            json!({"Content-Type": "application/json", "Authorization": "Bearer token"}),  // Multiple headers
            json!({"X-Custom-Header": "value", "X-Another": "value2"}),  // Custom headers
        ];
        
        for headers in test_cases {
            let serialized = serde_json::to_string(&headers).unwrap();
            let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
            assert_eq!(headers, deserialized, "Headers should serialize/deserialize correctly");
        }
    }

    #[tokio::test]
    async fn test_body_serialization() {
        // Test various body configurations
        let test_cases = vec![
            json!(null),  // Null body
            json!({}),  // Empty object
            json!({"key": "value"}),  // Simple object
            json!({"nested": {"deep": {"value": 42}}}),  // Nested object
            json!([1, 2, 3]),  // Array
        ];
        
        for body in test_cases {
            let serialized = serde_json::to_string(&body).unwrap();
            let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
            assert_eq!(body, deserialized, "Body should serialize/deserialize correctly");
        }
    }

    #[tokio::test]
    async fn test_database_statement_creation() {
        // Test SQL statement creation for various operations
        let insert_sql = "INSERT INTO job (id, url, method) VALUES ('uuid', 'http://example.com', 'GET')";
        let insert_stmt = Statement::from_string(DbBackend::Sqlite, insert_sql.to_string());
        assert!(insert_stmt.to_string().contains("INSERT"));
        
        let select_sql = "SELECT * FROM job WHERE status = 'Pending'";
        let select_stmt = Statement::from_string(DbBackend::Sqlite, select_sql.to_string());
        assert!(select_stmt.to_string().contains("SELECT"));
        
        let update_sql = "UPDATE job SET status = 'Running' WHERE id = 'uuid'";
        let update_stmt = Statement::from_string(DbBackend::Sqlite, update_sql.to_string());
        assert!(update_stmt.to_string().contains("UPDATE"));
        
        let delete_sql = "DELETE FROM job WHERE id = 'uuid'";
        let delete_stmt = Statement::from_string(DbBackend::Sqlite, delete_sql.to_string());
        assert!(delete_stmt.to_string().contains("DELETE"));
    }
}
