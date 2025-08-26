//! Telemetry testing utilities for ClickHouse metrics queries

use clickhouse::Row;
use serde::Deserialize;
use std::time::Duration;

/// Helper to create a ClickHouse client for metrics queries
pub async fn create_clickhouse_client() -> clickhouse::Client {
    clickhouse::Client::default()
        .with_url("http://localhost:8123")
        .with_database("otel")
}

/// Generate a unique service name for test isolation
pub fn unique_service_name(test_name: &str) -> String {
    let uuid = uuid::Uuid::new_v4();
    format!("nexus-test-{}-{}", test_name, uuid)
}

/// Wait for metrics with retries to handle timing issues
pub async fn wait_for_metrics<T>(
    clickhouse: &clickhouse::Client,
    query: &str,
    min_expected: usize,
    max_retries: usize,
) -> Vec<T>
where
    T: Row + for<'a> Deserialize<'a>,
{
    for retry in 0..max_retries {
        // Wait before checking (exponential backoff starting at 200ms, cap at 2 seconds)
        let wait_time = Duration::from_millis((200 * (1 << retry)).min(2000));
        tokio::time::sleep(wait_time).await;

        let rows = clickhouse.query(query).fetch_all::<T>().await.unwrap_or_default();

        if rows.len() >= min_expected {
            return rows;
        }
    }

    // Final attempt
    clickhouse.query(query).fetch_all::<T>().await.unwrap_or_default()
}
