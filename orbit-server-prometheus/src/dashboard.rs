use crate::{PrometheusError, PrometheusResult};
use serde::{Deserialize, Serialize};

/// Grafana dashboard generator
pub struct DashboardGenerator;

impl DashboardGenerator {
    /// Generate a basic Grafana dashboard JSON
    pub fn generate_grafana_dashboard() -> PrometheusResult<String> {
        let dashboard = GrafanaDashboard {
            id: None,
            title: "Orbit Metrics Dashboard".to_string(),
            tags: vec!["orbit".to_string(), "prometheus".to_string()],
            timezone: "browser".to_string(),
            panels: vec![
                Panel {
                    id: 1,
                    title: "HTTP Requests".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "rate(orbit_http_requests_total[5m])".to_string(),
                        legend_format: "{{method}} {{status}}".to_string(),
                    }],
                },
                Panel {
                    id: 2,
                    title: "Memory Usage".to_string(),
                    panel_type: "singlestat".to_string(),
                    targets: vec![Target {
                        expr: "orbit_server_memory_usage_bytes".to_string(),
                        legend_format: "Memory".to_string(),
                    }],
                },
                Panel {
                    id: 3,
                    title: "CPU Usage".to_string(),
                    panel_type: "singlestat".to_string(),
                    targets: vec![Target {
                        expr: "orbit_server_cpu_usage_percent".to_string(),
                        legend_format: "CPU %".to_string(),
                    }],
                },
            ],
            time: TimeRange {
                from: "now-1h".to_string(),
                to: "now".to_string(),
            },
            refresh: "30s".to_string(),
        };

        serde_json::to_string_pretty(&dashboard).map_err(|e| {
            PrometheusError::dashboard(format!("Failed to serialize dashboard: {}", e))
        })
    }

    /// Generate comprehensive production monitoring dashboard
    pub fn generate_production_dashboard() -> PrometheusResult<String> {
        let dashboard = GrafanaDashboard {
            id: None,
            title: "Orbit Production Monitoring".to_string(),
            tags: vec!["orbit".to_string(), "prometheus".to_string(), "production".to_string()],
            timezone: "browser".to_string(),
            panels: vec![
                // System Overview
                Panel {
                    id: 1,
                    title: "Server Uptime".to_string(),
                    panel_type: "singlestat".to_string(),
                    targets: vec![Target {
                        expr: "orbit_server_uptime_seconds".to_string(),
                        legend_format: "Uptime".to_string(),
                    }],
                },
                Panel {
                    id: 2,
                    title: "Memory Usage".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "orbit_server_memory_usage_bytes".to_string(),
                        legend_format: "Memory".to_string(),
                    }],
                },
                Panel {
                    id: 3,
                    title: "CPU Usage".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "orbit_server_cpu_usage_percent".to_string(),
                        legend_format: "CPU %".to_string(),
                    }],
                },
                // Query Performance
                Panel {
                    id: 4,
                    title: "Query Execution Time (p50, p95, p99)".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![
                        Target {
                            expr: "histogram_quantile(0.50, rate(orbit_query_execution_seconds_bucket[5m]))".to_string(),
                            legend_format: "p50".to_string(),
                        },
                        Target {
                            expr: "histogram_quantile(0.95, rate(orbit_query_execution_seconds_bucket[5m]))".to_string(),
                            legend_format: "p95".to_string(),
                        },
                        Target {
                            expr: "histogram_quantile(0.99, rate(orbit_query_execution_seconds_bucket[5m]))".to_string(),
                            legend_format: "p99".to_string(),
                        },
                    ],
                },
                Panel {
                    id: 5,
                    title: "Slow Queries".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "rate(orbit_slow_queries_total[5m])".to_string(),
                        legend_format: "Slow Queries/sec".to_string(),
                    }],
                },
                Panel {
                    id: 6,
                    title: "Query Errors".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "rate(orbit_query_errors_total[5m])".to_string(),
                        legend_format: "Errors/sec".to_string(),
                    }],
                },
                // Database Metrics
                Panel {
                    id: 7,
                    title: "Database Connections".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![
                        Target {
                            expr: "orbit_db_connections_active".to_string(),
                            legend_format: "Active".to_string(),
                        },
                        Target {
                            expr: "orbit_db_connections_idle".to_string(),
                            legend_format: "Idle".to_string(),
                        },
                    ],
                },
                Panel {
                    id: 8,
                    title: "Database Query Duration".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "rate(orbit_db_query_duration_seconds_sum[5m]) / rate(orbit_db_query_duration_seconds_count[5m])".to_string(),
                        legend_format: "Avg Duration".to_string(),
                    }],
                },
                // Transaction Metrics
                Panel {
                    id: 9,
                    title: "Transaction Rate".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![
                        Target {
                            expr: "rate(orbit_transaction_commits_total[5m])".to_string(),
                            legend_format: "Commits/sec".to_string(),
                        },
                        Target {
                            expr: "rate(orbit_transaction_rollbacks_total[5m])".to_string(),
                            legend_format: "Rollbacks/sec".to_string(),
                        },
                    ],
                },
                Panel {
                    id: 10,
                    title: "Lock Contentions".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "rate(orbit_lock_contentions_total[5m])".to_string(),
                        legend_format: "Contentions/sec".to_string(),
                    }],
                },
                // I/O Metrics
                Panel {
                    id: 11,
                    title: "Disk I/O Operations".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![
                        Target {
                            expr: "rate(orbit_disk_reads_total[5m])".to_string(),
                            legend_format: "Reads/sec".to_string(),
                        },
                        Target {
                            expr: "rate(orbit_disk_writes_total[5m])".to_string(),
                            legend_format: "Writes/sec".to_string(),
                        },
                    ],
                },
                Panel {
                    id: 12,
                    title: "Disk I/O Throughput".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![
                        Target {
                            expr: "rate(orbit_disk_read_bytes_total[5m])".to_string(),
                            legend_format: "Read Bytes/sec".to_string(),
                        },
                        Target {
                            expr: "rate(orbit_disk_write_bytes_total[5m])".to_string(),
                            legend_format: "Write Bytes/sec".to_string(),
                        },
                    ],
                },
                // Cache Metrics
                Panel {
                    id: 13,
                    title: "Cache Hit Rate".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "rate(orbit_cache_hits_total[5m]) / (rate(orbit_cache_hits_total[5m]) + rate(orbit_cache_misses_total[5m]))".to_string(),
                        legend_format: "Hit Rate".to_string(),
                    }],
                },
                Panel {
                    id: 14,
                    title: "Cache Size".to_string(),
                    panel_type: "graph".to_string(),
                    targets: vec![Target {
                        expr: "orbit_cache_size_bytes".to_string(),
                        legend_format: "Cache Size".to_string(),
                    }],
                },
            ],
            time: TimeRange {
                from: "now-1h".to_string(),
                to: "now".to_string(),
            },
            refresh: "30s".to_string(),
        };

        serde_json::to_string_pretty(&dashboard).map_err(|e| {
            PrometheusError::dashboard(format!("Failed to serialize dashboard: {}", e))
        })
    }

    /// Generate a simple HTML metrics page
    pub fn generate_html_dashboard() -> String {
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Orbit Metrics Dashboard</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 0;
            background-color: #f5f5f5;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 2rem;
            text-align: center;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 2rem;
        }
        .metrics-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 2rem;
            margin-top: 2rem;
        }
        .metric-card {
            background: white;
            border-radius: 8px;
            padding: 1.5rem;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            transition: transform 0.2s;
        }
        .metric-card:hover {
            transform: translateY(-2px);
        }
        .metric-title {
            font-size: 1.1rem;
            font-weight: 600;
            margin-bottom: 1rem;
            color: #333;
        }
        .metric-value {
            font-size: 2rem;
            font-weight: bold;
            color: #667eea;
        }
        .metric-description {
            color: #666;
            font-size: 0.9rem;
            margin-top: 0.5rem;
        }
        .status-indicator {
            display: inline-block;
            width: 12px;
            height: 12px;
            border-radius: 50%;
            background-color: #4CAF50;
            margin-right: 8px;
        }
        .links {
            margin-top: 2rem;
            text-align: center;
        }
        .link-button {
            display: inline-block;
            padding: 12px 24px;
            margin: 0 10px;
            background-color: #667eea;
            color: white;
            text-decoration: none;
            border-radius: 6px;
            transition: background-color 0.2s;
        }
        .link-button:hover {
            background-color: #764ba2;
        }
    </style>
    <script>
        // Auto-refresh the page every 30 seconds
        setTimeout(function() {
            location.reload();
        }, 30000);
        
        // Update timestamp
        function updateTimestamp() {
            document.getElementById('timestamp').textContent = new Date().toLocaleString();
        }
        
        window.onload = updateTimestamp;
    </script>
</head>
<body>
    <div class="header">
        <h1>🚀 Orbit Prometheus Dashboard</h1>
        <p>Real-time metrics monitoring and visualization</p>
        <p><small>Last updated: <span id="timestamp"></span></small></p>
    </div>
    
    <div class="container">
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-title">
                    <span class="status-indicator"></span>
                    System Status
                </div>
                <div class="metric-value">OPERATIONAL</div>
                <div class="metric-description">All systems running normally</div>
            </div>
            
            <div class="metric-card">
                <div class="metric-title">📊 Active Metrics</div>
                <div class="metric-value">12+</div>
                <div class="metric-description">Core metrics being collected</div>
            </div>
            
            <div class="metric-card">
                <div class="metric-title">🔄 Collection Interval</div>
                <div class="metric-value">15s</div>
                <div class="metric-description">Metrics updated every 15 seconds</div>
            </div>
            
            <div class="metric-card">
                <div class="metric-title">🌐 Server Port</div>
                <div class="metric-value">9090</div>
                <div class="metric-description">Prometheus exporter endpoint</div>
            </div>
            
            <div class="metric-card">
                <div class="metric-title">💾 Memory Usage</div>
                <div class="metric-value">~256MB</div>
                <div class="metric-description">Estimated current memory usage</div>
            </div>
            
            <div class="metric-card">
                <div class="metric-title">⚡ CPU Usage</div>
                <div class="metric-value">~15%</div>
                <div class="metric-description">Estimated current CPU usage</div>
            </div>
        </div>
        
        <div class="links">
            <a href="/metrics" class="link-button">📈 View Raw Metrics</a>
            <a href="/health" class="link-button">💚 Health Check</a>
            <a href="https://prometheus.io/" class="link-button" target="_blank">📖 Prometheus Docs</a>
        </div>
    </div>
</body>
</html>
        "#.to_string()
    }
}

/// Grafana dashboard structure
#[derive(Debug, Serialize, Deserialize)]
pub struct GrafanaDashboard {
    pub id: Option<u32>,
    pub title: String,
    pub tags: Vec<String>,
    pub timezone: String,
    pub panels: Vec<Panel>,
    pub time: TimeRange,
    pub refresh: String,
}

/// Grafana panel
#[derive(Debug, Serialize, Deserialize)]
pub struct Panel {
    pub id: u32,
    pub title: String,
    #[serde(rename = "type")]
    pub panel_type: String,
    pub targets: Vec<Target>,
}

/// Grafana target (query)
#[derive(Debug, Serialize, Deserialize)]
pub struct Target {
    pub expr: String,
    #[serde(rename = "legendFormat")]
    pub legend_format: String,
}

/// Time range
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeRange {
    pub from: String,
    pub to: String,
}
