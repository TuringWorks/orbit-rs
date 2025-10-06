//! # Orbit Time Series Database Demo
//!
//! This example demonstrates the comprehensive time series functionality of orbit-rs:
//! - Creating time series with metadata and labels
//! - High-speed data ingestion with batching
//! - Querying with time ranges and aggregations
//! - Memory storage with automatic retention
//! - Performance monitoring and statistics

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use orbit_shared::timeseries::*;
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("🚀 Starting Orbit Time Series Database Demo");

    // Create time series engine with in-memory storage
    let config = TimeSeriesConfig {
        storage_backend: StorageBackend::Memory,
        memory_limit_mb: 256, // 256MB limit
        default_retention_policy: RetentionPolicy {
            duration_seconds: 86400 * 7, // 7 days
            downsampling_rules: vec![],
        },
        default_compression_policy: CompressionPolicy {
            compression_type: CompressionType::Delta,
            chunk_size: 1024,
            compress_after_seconds: 3600, // 1 hour
        },
        enable_metrics: true,
        batch_size: 1000,
        flush_interval_ms: 1000,
        ..Default::default()
    };

    let engine = core::TimeSeriesEngine::new(config).await?;
    info!("✅ Time series engine initialized");

    // Demo 1: Create multiple time series with different metrics
    info!("📊 Demo 1: Creating time series with labels");

    let cpu_series = create_cpu_series(&engine).await?;
    let memory_series = create_memory_series(&engine).await?;
    let network_series = create_network_series(&engine).await?;

    // Demo 2: Simulate high-speed data ingestion
    info!("⚡ Demo 2: High-speed data ingestion");

    let start_time = Utc::now();
    ingest_sample_data(&engine, &[cpu_series, memory_series, network_series], 1000).await?;
    let ingestion_time = Utc::now().signed_duration_since(start_time);

    info!(
        "✅ Ingested 3000 data points in {}ms",
        ingestion_time.num_milliseconds()
    );

    // Demo 3: Query and aggregation examples
    info!("🔍 Demo 3: Querying and aggregations");

    demonstrate_queries(&engine, cpu_series).await?;

    // Demo 4: Storage statistics and performance
    info!("📈 Demo 4: Storage statistics");

    show_storage_statistics(&engine).await?;

    // Demo 5: Query builder demonstration
    info!("🛠️  Demo 5: Advanced query builder");

    demonstrate_query_builder().await?;

    info!("🎉 Orbit Time Series Demo completed successfully!");
    Ok(())
}

/// Create a CPU utilization time series
async fn create_cpu_series(engine: &core::TimeSeriesEngine) -> Result<SeriesId> {
    let mut labels = HashMap::new();
    labels.insert("metric".to_string(), "cpu_usage".to_string());
    labels.insert("host".to_string(), "server-001".to_string());
    labels.insert("datacenter".to_string(), "us-west-1".to_string());

    let series_id = engine
        .create_series("cpu.usage.percent".to_string(), labels)
        .await?;
    info!("📊 Created CPU series: {}", series_id);
    Ok(series_id)
}

/// Create a memory utilization time series
async fn create_memory_series(engine: &core::TimeSeriesEngine) -> Result<SeriesId> {
    let mut labels = HashMap::new();
    labels.insert("metric".to_string(), "memory_usage".to_string());
    labels.insert("host".to_string(), "server-001".to_string());
    labels.insert("datacenter".to_string(), "us-west-1".to_string());

    let series_id = engine
        .create_series("memory.usage.percent".to_string(), labels)
        .await?;
    info!("💾 Created Memory series: {}", series_id);
    Ok(series_id)
}

/// Create a network throughput time series
async fn create_network_series(engine: &core::TimeSeriesEngine) -> Result<SeriesId> {
    let mut labels = HashMap::new();
    labels.insert("metric".to_string(), "network_throughput".to_string());
    labels.insert("host".to_string(), "server-001".to_string());
    labels.insert("datacenter".to_string(), "us-west-1".to_string());

    let series_id = engine
        .create_series("network.throughput.mbps".to_string(), labels)
        .await?;
    info!("🌐 Created Network series: {}", series_id);
    Ok(series_id)
}

/// Simulate high-speed data ingestion
async fn ingest_sample_data(
    engine: &core::TimeSeriesEngine,
    series_ids: &[SeriesId],
    points_per_series: usize,
) -> Result<()> {
    let mut rng = rand::thread_rng();
    let start_time = Utc::now() - ChronoDuration::hours(1);

    for &series_id in series_ids {
        let mut data_points = Vec::new();

        for i in 0..points_per_series {
            let timestamp =
                datetime_to_timestamp(start_time + ChronoDuration::seconds(i as i64 * 3));
            let value = match rng.gen_range(0..3) {
                0 => TimeSeriesValue::Float(rng.gen_range(0.0..100.0)), // CPU/Memory percentage
                1 => TimeSeriesValue::Float(rng.gen_range(0.0..1000.0)), // Network throughput
                _ => TimeSeriesValue::Float(rng.gen_range(20.0..80.0)), // Stable metric
            };

            data_points.push(DataPoint {
                timestamp,
                value,
                labels: HashMap::new(),
            });
        }

        engine.insert_batch(series_id, data_points).await?;
    }

    Ok(())
}

/// Demonstrate various query operations
async fn demonstrate_queries(engine: &core::TimeSeriesEngine, series_id: SeriesId) -> Result<()> {
    let end_time = Utc::now();
    let start_time = end_time - ChronoDuration::minutes(30);

    // Raw data query
    info!("🔍 Querying raw data...");
    let time_range = TimeRange {
        start: datetime_to_timestamp(start_time),
        end: datetime_to_timestamp(end_time),
    };

    let result = engine.query(series_id, time_range.clone()).await?;
    info!("📊 Retrieved {} raw data points", result.total_points);
    info!("⏱️  Query executed in {}ms", result.execution_time_ms);

    // Aggregated query - 5-minute average
    info!("📈 Querying 5-minute averages...");
    let agg_result = engine
        .query_aggregate(
            series_id,
            time_range,
            AggregationType::Average,
            300, // 5 minutes in seconds
        )
        .await?;

    info!("📊 Retrieved {} aggregated points", agg_result.total_points);
    info!(
        "⏱️  Aggregation query executed in {}ms",
        agg_result.execution_time_ms
    );

    // Show sample data points
    if !agg_result.data_points.is_empty() {
        info!("📋 Sample aggregated data points:");
        for (i, point) in agg_result.data_points.iter().take(5).enumerate() {
            let timestamp = timestamp_to_datetime(point.timestamp);
            info!(
                "  {}. {} -> {:?}",
                i + 1,
                timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                point.value
            );
        }
    }

    Ok(())
}

/// Show storage statistics and performance metrics
async fn show_storage_statistics(engine: &core::TimeSeriesEngine) -> Result<()> {
    let stats = engine.get_storage_stats().await?;

    info!("📊 Storage Statistics:");
    info!("  📈 Total series: {}", stats.total_series);
    info!("  📊 Total data points: {}", stats.total_data_points);
    info!(
        "  💾 Storage size: {} bytes ({:.2} MB)",
        stats.storage_size_bytes,
        stats.storage_size_bytes as f64 / 1_048_576.0
    );
    info!("  🗜️  Compression ratio: {:.2}x", stats.compression_ratio);

    if stats.ingestion_rate_points_per_second > 0.0 {
        info!(
            "  ⚡ Ingestion rate: {:.1} points/sec",
            stats.ingestion_rate_points_per_second
        );
    }

    if stats.query_rate_per_second > 0.0 {
        info!(
            "  🔍 Query rate: {:.1} queries/sec",
            stats.query_rate_per_second
        );
    }

    // List all series
    info!("📋 Series List:");
    let series_list = engine.list_series().await?;
    for (i, series) in series_list.iter().enumerate() {
        info!(
            "  {}. {} (ID: {}) - {} labels",
            i + 1,
            series.name,
            series.series_id,
            series.labels.len()
        );
    }

    Ok(())
}

/// Demonstrate query builder functionality
async fn demonstrate_query_builder() -> Result<()> {
    info!("🛠️  Query Builder Examples:");

    // Example 1: Simple time range query
    let query1 = query::TimeSeriesQuery::new()
        .select_series(query::SeriesSelector::ByName(
            "cpu.usage.percent".to_string(),
        ))
        .time_range(
            datetime_to_timestamp(Utc::now() - ChronoDuration::hours(1)),
            datetime_to_timestamp(Utc::now()),
        )
        .limit(100);

    info!("  📊 Query 1: CPU data for last hour (limit 100)");
    info!("     Series: {:?}", query1.series_selector);
    info!(
        "     Time range: {} points",
        (query1.time_range.end - query1.time_range.start) / 1_000_000_000 / 60
    );

    // Example 2: Aggregated query with filtering
    let query2 = query::TimeSeriesQuery::new()
        .select_series(query::SeriesSelector::ByLabels({
            let mut labels = HashMap::new();
            labels.insert("host".to_string(), "server-001".to_string());
            labels
        }))
        .time_range(
            datetime_to_timestamp(Utc::now() - ChronoDuration::days(1)),
            datetime_to_timestamp(Utc::now()),
        )
        .aggregate(AggregationType::Average, Duration::from_secs(3600)) // 1-hour buckets
        .filter(query::Filter::ValueGreaterThan(50.0));

    info!("  📈 Query 2: Daily averages for server-001, values > 50");
    info!("     Aggregation: 1-hour windows");
    info!("     Filters: {} applied", query2.filters.len());

    // Example 3: Multi-series label-based query
    let query3 = query::TimeSeriesQuery::new()
        .select_series(query::SeriesSelector::ByLabels({
            let mut labels = HashMap::new();
            labels.insert("datacenter".to_string(), "us-west-1".to_string());
            labels
        }))
        .time_range(
            datetime_to_timestamp(Utc::now() - ChronoDuration::hours(6)),
            datetime_to_timestamp(Utc::now()),
        )
        .filter(query::Filter::ValueBetween(10.0, 90.0))
        .offset(50)
        .limit(500);

    info!("  🌐 Query 3: All metrics from us-west-1 datacenter (last 6h)");
    info!("     Value range: 10.0 - 90.0");
    info!(
        "     Pagination: offset {}, limit {}",
        query3.offset.unwrap_or(0),
        query3.limit.unwrap_or(0)
    );

    Ok(())
}
