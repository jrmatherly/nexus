//! Tests for telemetry metrics collection

use clickhouse::Row;
use serde::Deserialize;

mod llm;
mod mcp;

/// Row structure for histogram metrics in ClickHouse
#[derive(Row, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct HistogramMetricRow {
    pub metric_name: String,
    pub attributes: Vec<(String, String)>,
    pub count: u64,
}

/// Row structure for sum/counter metrics in ClickHouse
#[derive(Row, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SumMetricRow {
    pub metric_name: String,
    pub attributes: Vec<(String, String)>,
    pub value: f64,
}
