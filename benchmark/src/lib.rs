use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use colored::*;
use log::info;
use serde::{Deserialize, Serialize};

pub mod common;
pub mod scrapy;
pub mod scrapy_rs;

/// Represents a benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Name of the benchmark
    pub name: String,
    /// Framework being benchmarked (scrapy or scrapy-rs)
    pub framework: String,
    /// Start time of the benchmark
    pub start_time: DateTime<Utc>,
    /// End time of the benchmark
    pub end_time: DateTime<Utc>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Number of requests made
    pub request_count: usize,
    /// Number of responses received
    pub response_count: usize,
    /// Number of items scraped
    pub item_count: usize,
    /// Number of errors encountered
    pub error_count: usize,
    /// Requests per second
    pub requests_per_second: f64,
    /// Items per second
    pub items_per_second: f64,
    /// Memory usage in MB
    pub memory_usage_mb: f64,
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Additional metrics
    #[serde(skip)]
    pub additional_metrics: HashMap<String, f64>,
}

impl BenchmarkResult {
    /// Create a new benchmark result
    pub fn new(name: &str, framework: &str) -> Self {
        Self {
            name: name.to_string(),
            framework: framework.to_string(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 0,
            request_count: 0,
            response_count: 0,
            item_count: 0,
            error_count: 0,
            requests_per_second: 0.0,
            items_per_second: 0.0,
            memory_usage_mb: 0.0,
            cpu_usage_percent: 0.0,
            additional_metrics: HashMap::new(),
        }
    }

    /// Calculate derived metrics
    pub fn calculate_metrics(&mut self) {
        let duration_secs = self.duration_ms as f64 / 1000.0;
        if duration_secs > 0.0 {
            self.requests_per_second = self.request_count as f64 / duration_secs;
            self.items_per_second = self.item_count as f64 / duration_secs;
        }
    }

    /// Save the benchmark result to a CSV file
    pub fn save_to_csv(&self, file_path: &str) -> std::io::Result<()> {
        let path = Path::new(file_path);
        let file_exists = path.exists();

        let mut wtr = csv::WriterBuilder::new()
            .has_headers(!file_exists)
            .from_path(path)?;

        wtr.serialize(self)?;
        wtr.flush()?;

        Ok(())
    }
}

/// Trait for benchmark runners
pub trait BenchmarkRunner {
    /// Run the benchmark
    fn run(&mut self) -> BenchmarkResult;

    /// Get the name of the benchmark
    fn name(&self) -> &str;

    /// Get the framework being benchmarked
    fn framework(&self) -> &str;
}

/// Compare two benchmark results and print a comparison
pub fn compare_results(scrapy_result: &BenchmarkResult, scrapy_rs_result: &BenchmarkResult) {
    println!("\n{}", "=== BENCHMARK COMPARISON ===".bold().green());
    println!(
        "{:<20} {:<15} {:<15} {:<15}",
        "Metric".bold(),
        "Scrapy".bold(),
        "Scrapy-RS".bold(),
        "Improvement".bold()
    );

    let duration_ratio = scrapy_result.duration_ms as f64 / scrapy_rs_result.duration_ms as f64;
    println!(
        "{:<20} {:<15.2} {:<15.2} {:<15.2}x",
        "Duration (s)",
        scrapy_result.duration_ms as f64 / 1000.0,
        scrapy_rs_result.duration_ms as f64 / 1000.0,
        duration_ratio
    );

    let rps_ratio = scrapy_rs_result.requests_per_second / scrapy_result.requests_per_second;
    println!(
        "{:<20} {:<15.2} {:<15.2} {:<15.2}x",
        "Requests/sec",
        scrapy_result.requests_per_second,
        scrapy_rs_result.requests_per_second,
        rps_ratio
    );

    let ips_ratio = scrapy_rs_result.items_per_second / scrapy_result.items_per_second;
    println!(
        "{:<20} {:<15.2} {:<15.2} {:<15.2}x",
        "Items/sec", scrapy_result.items_per_second, scrapy_rs_result.items_per_second, ips_ratio
    );

    let memory_ratio = scrapy_result.memory_usage_mb / scrapy_rs_result.memory_usage_mb;
    println!(
        "{:<20} {:<15.2} {:<15.2} {:<15.2}x",
        "Memory (MB)",
        scrapy_result.memory_usage_mb,
        scrapy_rs_result.memory_usage_mb,
        memory_ratio
    );

    let cpu_ratio = scrapy_result.cpu_usage_percent / scrapy_rs_result.cpu_usage_percent;
    println!(
        "{:<20} {:<15.2} {:<15.2} {:<15.2}x",
        "CPU (%)", scrapy_result.cpu_usage_percent, scrapy_rs_result.cpu_usage_percent, cpu_ratio
    );

    println!("\n{}", "=== SUMMARY ===".bold().green());
    println!("Scrapy-RS is:");
    println!("  * {:.2}x faster", duration_ratio);
    println!("  * {:.2}x more requests per second", rps_ratio);
    println!("  * {:.2}x more items per second", ips_ratio);
    println!("  * {:.2}x less memory usage", memory_ratio);
    println!("  * {:.2}x less CPU usage", cpu_ratio);
}

/// Generate a performance report with charts
pub fn generate_report(results: &[BenchmarkResult], output_dir: &str) -> std::io::Result<()> {
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(output_dir)?;

    // Save results to CSV
    let csv_path = format!("{}/benchmark_results.csv", output_dir);
    let mut wtr = csv::Writer::from_path(&csv_path)?;

    for result in results {
        wtr.serialize(result)?;
    }

    wtr.flush()?;

    // Generate HTML report
    let html_path = format!("{}/benchmark_report.html", output_dir);
    let mut html_file = File::create(&html_path)?;

    // Simple HTML template
    let html_content = format!(r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Scrapy-RS vs Scrapy Benchmark Report</title>
        <style>
            body {{ font-family: Arial, sans-serif; margin: 20px; }}
            h1 {{ color: #333; }}
            .chart {{ width: 800px; height: 400px; margin: 20px 0; }}
            table {{ border-collapse: collapse; width: 100%; }}
            th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
            th {{ background-color: #f2f2f2; }}
            tr:nth-child(even) {{ background-color: #f9f9f9; }}
        </style>
        <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    </head>
    <body>
        <h1>Scrapy-RS vs Scrapy Benchmark Report</h1>
        
        <h2>Performance Comparison</h2>
        <div class="chart">
            <canvas id="performanceChart"></canvas>
        </div>
        
        <h2>Memory and CPU Usage</h2>
        <div class="chart">
            <canvas id="resourceChart"></canvas>
        </div>
        
        <h2>Detailed Results</h2>
        <table>
            <tr>
                <th>Benchmark</th>
                <th>Framework</th>
                <th>Duration (s)</th>
                <th>Requests/sec</th>
                <th>Items/sec</th>
                <th>Memory (MB)</th>
                <th>CPU (%)</th>
            </tr>
            {table_rows}
        </table>
        
        <script>
            // Performance chart
            const perfCtx = document.getElementById('performanceChart').getContext('2d');
            new Chart(perfCtx, {{
                type: 'bar',
                data: {{
                    labels: ['Requests/sec', 'Items/sec'],
                    datasets: [
                        {{
                            label: 'Scrapy',
                            data: [{scrapy_rps}, {scrapy_ips}],
                            backgroundColor: 'rgba(54, 162, 235, 0.5)',
                            borderColor: 'rgba(54, 162, 235, 1)',
                            borderWidth: 1
                        }},
                        {{
                            label: 'Scrapy-RS',
                            data: [{scrapy_rs_rps}, {scrapy_rs_ips}],
                            backgroundColor: 'rgba(255, 99, 132, 0.5)',
                            borderColor: 'rgba(255, 99, 132, 1)',
                            borderWidth: 1
                        }}
                    ]
                }},
                options: {{
                    responsive: true,
                    scales: {{
                        y: {{
                            beginAtZero: true,
                            title: {{
                                display: true,
                                text: 'Count per second'
                            }}
                        }}
                    }}
                }}
            }});
            
            // Resource chart
            const resCtx = document.getElementById('resourceChart').getContext('2d');
            new Chart(resCtx, {{
                type: 'bar',
                data: {{
                    labels: ['Memory (MB)', 'CPU (%)'],
                    datasets: [
                        {{
                            label: 'Scrapy',
                            data: [{scrapy_mem}, {scrapy_cpu}],
                            backgroundColor: 'rgba(54, 162, 235, 0.5)',
                            borderColor: 'rgba(54, 162, 235, 1)',
                            borderWidth: 1
                        }},
                        {{
                            label: 'Scrapy-RS',
                            data: [{scrapy_rs_mem}, {scrapy_rs_cpu}],
                            backgroundColor: 'rgba(255, 99, 132, 0.5)',
                            borderColor: 'rgba(255, 99, 132, 1)',
                            borderWidth: 1
                        }}
                    ]
                }},
                options: {{
                    responsive: true,
                    scales: {{
                        y: {{
                            beginAtZero: true,
                            title: {{
                                display: true,
                                text: 'Resource usage'
                            }}
                        }}
                    }}
                }}
            }});
        </script>
    </body>
    </html>
    "#, 
        table_rows = results.iter().map(|r| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td></tr>",
                r.name,
                r.framework,
                r.duration_ms as f64 / 1000.0,
                r.requests_per_second,
                r.items_per_second,
                r.memory_usage_mb,
                r.cpu_usage_percent
            )
        }).collect::<Vec<_>>().join("\n"),
        scrapy_rps = results.iter().find(|r| r.framework == "scrapy").map_or(0.0, |r| r.requests_per_second),
        scrapy_ips = results.iter().find(|r| r.framework == "scrapy").map_or(0.0, |r| r.items_per_second),
        scrapy_rs_rps = results.iter().find(|r| r.framework == "scrapy-rs").map_or(0.0, |r| r.requests_per_second),
        scrapy_rs_ips = results.iter().find(|r| r.framework == "scrapy-rs").map_or(0.0, |r| r.items_per_second),
        scrapy_mem = results.iter().find(|r| r.framework == "scrapy").map_or(0.0, |r| r.memory_usage_mb),
        scrapy_cpu = results.iter().find(|r| r.framework == "scrapy").map_or(0.0, |r| r.cpu_usage_percent),
        scrapy_rs_mem = results.iter().find(|r| r.framework == "scrapy-rs").map_or(0.0, |r| r.memory_usage_mb),
        scrapy_rs_cpu = results.iter().find(|r| r.framework == "scrapy-rs").map_or(0.0, |r| r.cpu_usage_percent)
    );

    html_file.write_all(html_content.as_bytes())?;

    info!("Report generated at {}/benchmark_report.html", output_dir);

    Ok(())
}

/// Measure execution time of a function
pub fn measure_time<F, T>(f: F) -> (T, Duration)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();

    (result, duration)
}

/// Get current memory usage in MB
pub fn get_memory_usage() -> f64 {
    #[cfg(feature = "sysinfo")]
    {
        use sysinfo::{ProcessExt, System, SystemExt};
        let mut system = System::new_all();
        system.refresh_all();

        let pid = std::process::id() as usize;
        if let Some(process) = system.process(sysinfo::Pid::from(pid)) {
            return process.memory() as f64 / 1024.0 / 1024.0;
        }
    }

    // Fallback or if sysinfo feature is not enabled
    0.0
}

/// Get current CPU usage percentage
pub fn get_cpu_usage() -> f64 {
    #[cfg(feature = "sysinfo")]
    {
        use sysinfo::{ProcessExt, System, SystemExt};
        let mut system = System::new_all();
        system.refresh_all();

        let pid = std::process::id() as usize;
        if let Some(process) = system.process(sysinfo::Pid::from(pid)) {
            return process.cpu_usage() as f64;
        }
    }

    // Fallback or if sysinfo feature is not enabled
    0.0
}
