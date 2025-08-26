//! Tests for telemetry metrics collection

use clickhouse::Row;
use indoc::formatdoc;
use integration_tests::{TestServer, telemetry::*};
use serde::Deserialize;

/// Row structure for histogram metrics in ClickHouse
#[derive(Row, Deserialize, Debug)]
struct HistogramMetricRow {
    #[serde(rename = "MetricName")]
    metric_name: String,
    #[serde(rename = "Attributes")]
    attributes: Vec<(String, String)>, // ClickHouse client requires Vec of tuples for maps
    #[serde(rename = "Count")]
    count: u64,
    #[serde(rename = "Sum")]
    sum: f64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_endpoint_metrics() {
    // Generate unique service name for test isolation
    let service_name = unique_service_name("health-metrics");

    let config = formatdoc! {r#"
        [server]
        listen_address = "127.0.0.1:0"

        [server.health]
        enabled = true
        path = "/health"

        [telemetry]
        service_name = "{service_name}"

        [telemetry.resource_attributes]
        environment = "test"

        [telemetry.exporters.otlp]
        enabled = true
        endpoint = "http://localhost:4317"
        protocol = "grpc"

        # Export immediately for testing
        [telemetry.exporters.otlp.batch_export]
        scheduled_delay = "100ms"
        max_export_batch_size = 1

        # Dummy MCP server to ensure server starts
        [mcp]
        enabled = true

        [mcp.servers.dummy]
        cmd = ["echo", "dummy"]
    "#};

    let test_server = TestServer::builder().build(&config).await;

    // Make multiple requests to the health endpoint
    for _ in 0..3 {
        let response = test_server.client.get("/health").await;
        assert_eq!(response.status(), 200);
    }

    // Query ClickHouse for metrics
    let clickhouse = create_clickhouse_client().await;

    // Query duration histogram metrics with retries
    // Note: Histograms automatically include count, so we don't need a separate counter
    let duration_query = format!(
        r#"
        SELECT
            MetricName,
            Attributes,
            Count,
            Sum
        FROM otel_metrics_histogram
        WHERE
            MetricName = 'http.server.request.duration'
            AND ServiceName = '{}'
        ORDER BY TimeUnix DESC
        LIMIT 10
        "#,
        service_name
    );

    let duration_rows = wait_for_metrics::<HistogramMetricRow>(
        &clickhouse,
        &duration_query,
        1,  // At least one histogram entry
        10, // Max retries
    )
    .await;

    // Verify we have histogram data
    assert!(
        !duration_rows.is_empty(),
        "No duration histogram metrics found after retries"
    );

    // Check histogram values are reasonable
    let first_histogram = &duration_rows[0];
    assert!(first_histogram.count > 0, "Histogram count should be positive");
    assert!(first_histogram.sum > 0.0, "Histogram sum should be positive");

    // The histogram count represents the number of requests
    assert!(first_histogram.count >= 3, "Should have recorded at least 3 requests");

    // Calculate average response time
    let avg_response_time_ms = first_histogram.sum / (first_histogram.count as f64);
    assert!(
        avg_response_time_ms < 1000.0,
        "Average response time should be less than 1 second (was {}ms)",
        avg_response_time_ms
    );

    // Snapshot histogram metric name and attributes (use BTreeMap for deterministic ordering)
    insta::assert_debug_snapshot!((
        &first_histogram.metric_name,
        first_histogram.attributes.iter().cloned().collect::<std::collections::BTreeMap<_, _>>()
    ), @r#"
    (
        "http.server.request.duration",
        {
            "http.request.method": "GET",
            "http.response.status_code": "200",
            "http.route": "/health",
        },
    )
    "#);
}
