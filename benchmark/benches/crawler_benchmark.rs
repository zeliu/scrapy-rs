use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tokio::runtime::Runtime;

use scrapy_rs_core::request::Request;
use scrapy_rs_core::spider::{BasicSpider, Spider};
use scrapy_rs_downloader::{Downloader, DownloaderConfig, HttpDownloader};
use scrapy_rs_engine::{Engine, EngineConfig};
use scrapy_rs_middleware::{
    DefaultHeadersMiddleware, RequestMiddleware, ResponseLoggerMiddleware, ResponseMiddleware,
};
use scrapy_rs_pipeline::{LogPipeline, Pipeline};
use scrapy_rs_scheduler::{MemoryScheduler, Scheduler};

use scrapy_rs_benchmark::common::{get_predefined_scenarios, TestScenario};

fn create_spider(scenario: &TestScenario) -> Arc<dyn Spider> {
    let spider = BasicSpider::new(&scenario.name, scenario.urls.clone());

    Arc::new(spider)
}

fn create_downloader(scenario: &TestScenario) -> Arc<dyn Downloader> {
    let config = DownloaderConfig {
        concurrent_requests: scenario.concurrent_requests,
        user_agent: scenario.user_agent.clone(),
        timeout: 30, // 30 seconds timeout
        ..Default::default()
    };

    let downloader = HttpDownloader::new(config).expect("Failed to create downloader");
    Arc::new(downloader)
}

fn create_scheduler() -> Arc<dyn Scheduler> {
    Arc::new(MemoryScheduler::new())
}

fn create_engine(
    scenario: &TestScenario,
    spider: Arc<dyn Spider>,
    downloader: Arc<dyn Downloader>,
    scheduler: Arc<dyn Scheduler>,
) -> Engine {
    // Create engine configuration
    let config = EngineConfig {
        concurrent_requests: scenario.concurrent_requests,
        concurrent_items: scenario.concurrent_requests,
        download_delay_ms: scenario.download_delay_ms,
        user_agent: scenario.user_agent.clone(),
        respect_robots_txt: scenario.respect_robots_txt,
        follow_redirects: scenario.follow_redirects,
        ..Default::default()
    };

    // Create pipeline
    let pipeline: Arc<dyn Pipeline> = Arc::new(LogPipeline::info());

    // Create request middleware
    let request_middleware: Arc<dyn RequestMiddleware> =
        Arc::new(DefaultHeadersMiddleware::common());

    // Create response middleware
    let response_middleware: Arc<dyn ResponseMiddleware> =
        Arc::new(ResponseLoggerMiddleware::info());

    // Create engine
    Engine::with_components(
        spider,
        scheduler,
        downloader,
        pipeline,
        request_middleware,
        response_middleware,
        config,
    )
}

fn bench_engine(c: &mut Criterion) {
    let scenarios = get_predefined_scenarios();
    let runtime = Runtime::new().expect("Failed to create tokio runtime");

    let mut group = c.benchmark_group("engine");
    group.sample_size(10); // Reduce sample size due to network operations
    group.measurement_time(Duration::from_secs(60)); // Increase measurement time

    for scenario in scenarios {
        // Skip complex scenarios for quick benchmarks
        if scenario.name == "complex" || scenario.name == "real_world" {
            continue;
        }

        // Create a smaller version of the scenario for benchmarking
        let bench_scenario = TestScenario {
            page_limit: 10, // Limit to 10 pages for benchmarking
            ..scenario.clone()
        };

        group.bench_with_input(
            BenchmarkId::new("engine_run", &bench_scenario.name),
            &bench_scenario,
            |b, scenario| {
                b.iter(|| {
                    let spider = create_spider(scenario);
                    let downloader = create_downloader(scenario);
                    let scheduler = create_scheduler();
                    let mut engine = create_engine(scenario, spider, downloader, scheduler);

                    runtime.block_on(async {
                        let _ = engine.run().await;
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_request(c: &mut Criterion) {
    let runtime = Runtime::new().expect("Failed to create tokio runtime");

    let mut group = c.benchmark_group("request");

    // Benchmark creating a request
    group.bench_function("create_request", |b| {
        b.iter(|| {
            let _ = Request::get("https://example.com").unwrap();
        });
    });

    // Benchmark sending a request
    let downloader =
        HttpDownloader::new(DownloaderConfig::default()).expect("Failed to create downloader");

    group.bench_function("send_request", |b| {
        b.iter(|| {
            let request = Request::get("https://example.com").unwrap();

            runtime.block_on(async {
                let _ = downloader.download(request).await;
            });
        });
    });

    group.finish();
}

fn bench_spider(c: &mut Criterion) {
    let runtime = Runtime::new().expect("Failed to create tokio runtime");

    let mut group = c.benchmark_group("spider");

    // Benchmark creating a spider
    group.bench_function("create_spider", |b| {
        b.iter(|| {
            let spider = BasicSpider::new("benchmark", vec!["https://example.com".to_string()]);

            let _ = Arc::new(spider);
        });
    });

    // Benchmark parsing a response
    let downloader =
        HttpDownloader::new(DownloaderConfig::default()).expect("Failed to create downloader");
    let spider = Arc::new(BasicSpider::new(
        "benchmark",
        vec!["https://example.com".to_string()],
    ));

    group.bench_function("parse_response", |b| {
        // First, get a response
        let request = Request::get("https://example.com").unwrap();
        let response = runtime.block_on(async { downloader.download(request).await.unwrap() });

        b.iter(|| {
            runtime.block_on(async {
                let _ = spider.parse(response.clone()).await;
            });
        });
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(50);
    targets = bench_engine, bench_request, bench_spider
);
criterion_main!(benches);
