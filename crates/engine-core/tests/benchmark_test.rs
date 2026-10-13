use engine_core::BenchmarkEngine;
use inference_runtime::ExecutionBackend;

#[tokio::test]
async fn test_hardware_benchmark_execution() {
    let backend = ExecutionBackend::Cpu { thread_count: 2 };
    let res = BenchmarkEngine::run_system_benchmark(Some(backend)).await;
    assert!(res.is_ok(), "Benchmark run failed: {:?}", res.err());

    let bench = res.unwrap();
    assert!(!bench.device_name.is_empty(), "Device name should be populated");
    assert_eq!(bench.items.len(), 3, "Expected 3 tile tests (512, 768, 1024)");

    for item in &bench.items {
        assert!(item.fps > 0.0, "FPS should be positive");
        assert!(item.latency_ms > 0.0, "Latency should be positive");
        assert!(item.score > 0, "Score should be non-zero");
    }

    assert!(
        bench.recommendation.recommended_tile_size == 512
            || bench.recommendation.recommended_tile_size == 768
            || bench.recommendation.recommended_tile_size == 1024,
        "Recommended tile size should be a valid benchmark size"
    );
    assert!(!bench.recommendation.performance_tier.is_empty());
    assert!(bench.recommendation.overall_score > 0);
}
