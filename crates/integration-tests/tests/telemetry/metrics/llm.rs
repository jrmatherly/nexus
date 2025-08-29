//! LLM endpoint metrics tests

use indoc::formatdoc;
use integration_tests::{TestServer, llms::OpenAIMock, telemetry::*};

use crate::telemetry::metrics::HistogramMetricRow;

#[tokio::test]
async fn llm_endpoint_metrics() {
    // Generate unique service name for test isolation
    let service_name = unique_service_name("llm-metrics");

    // Record the start time for filtering metrics
    let start_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let config = formatdoc! {r#"
        [server]
        listen_address = "127.0.0.1:0"

        [telemetry]
        service_name = "{service_name}"

        [telemetry.resource_attributes]
        environment = "test"

        [telemetry.exporters.otlp]
        enabled = true
        endpoint = "http://localhost:4317"
        protocol = "grpc"

        # Export immediately for testing - use minimal delay
        [telemetry.exporters.otlp.batch_export]
        scheduled_delay = "1ms"
        max_export_batch_size = 1

        [llm]
        enabled = true
        path = "/llm"
    "#};

    // Setup mock LLM provider
    let mut builder = TestServer::builder();
    builder.spawn_llm(OpenAIMock::new("test_openai")).await;
    let test_server = builder.build(&config).await;

    // Make multiple requests to the LLM endpoint
    let request = serde_json::json!({
        "model": "test_openai/gpt-3.5-turbo",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 10
    });

    for _ in 0..2 {
        let response = test_server
            .client
            .request(reqwest::Method::POST, "/llm/v1/chat/completions")
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }

    // Drop the test server to force flush metrics
    drop(test_server);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Query ClickHouse for metrics
    let clickhouse = create_clickhouse_client().await;

    // Query for duration metrics - filter by start time to avoid previous test runs
    let duration_query = formatdoc! {r#"
        SELECT
            MetricName,
            Attributes,
            Count
        FROM otel_metrics_histogram
        WHERE
            MetricName = 'http.server.request.duration'
            AND ServiceName = '{service_name}'
            AND Attributes['http.route'] = '/llm/v1/chat/completions'
            AND Attributes['http.request.method'] = 'POST'
            AND StartTimeUnix >= {start_time}
        ORDER BY TimeUnix DESC
    "#};

    // Wait for metrics
    // Expected at least 2 HTTP POST requests: we made 2 explicit POST requests to /llm/v1/chat/completions
    // Using >= because metrics export can result in partial batches being visible during the wait period
    let llm_histograms = wait_for_metrics_matching::<HistogramMetricRow, _>(&clickhouse, duration_query, |rows| {
        // Sum all counts across batches (SQL already filtered by route and method)
        let total_count: u64 = rows.iter().map(|row| row.count).sum();
        total_count >= 2
    })
    .await
    .expect("Failed to get LLM metrics");

    // Snapshot the first metric's attributes (they should all be the same for /llm/v1/chat/completions route)
    let first_histogram = &llm_histograms[0];

    insta::assert_debug_snapshot!((
        &first_histogram.metric_name,
        first_histogram.attributes.iter().cloned().collect::<std::collections::BTreeMap<_, _>>()
    ), @r#"
    (
        "http.server.request.duration",
        {
            "http.request.method": "POST",
            "http.response.status_code": "200",
            "http.route": "/llm/v1/chat/completions",
        },
    )
    "#);
}
