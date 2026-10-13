import React, { useState } from 'react';

export interface BenchmarkScoreboardItem {
  test_name: string;
  tile_size: number;
  latency_ms: number;
  fps: number;
  vram_mb: number;
  score: number;
}

export interface HardwareOptimizationRecommendation {
  recommended_tile_size: number;
  recommended_batch_concurrency: number;
  recommended_backend: string;
  performance_tier: string;
  overall_score: number;
}

export interface BenchmarkRunResponse {
  device_name: string;
  vendor: string;
  is_gpu: boolean;
  items: BenchmarkScoreboardItem[];
  recommendation: HardwareOptimizationRecommendation;
}

interface BenchmarkModalProps {
  isOpen: boolean;
  onClose: () => void;
  onApplyRecommendation: (recommendedTileSize: number) => void;
}

export const BenchmarkModal: React.FC<BenchmarkModalProps> = ({
  isOpen,
  onClose,
  onApplyRecommendation,
}) => {
  const [isRunning, setIsRunning] = useState<boolean>(false);
  const [benchmarkResult, setBenchmarkResult] = useState<BenchmarkRunResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [applied, setApplied] = useState<boolean>(false);

  if (!isOpen) return null;

  const handleRunBenchmark = async () => {
    setIsRunning(true);
    setError(null);
    setApplied(false);

    try {
      const res = await fetch('/api/benchmark/run', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || 'Benchmark run failed');
      }

      const data: BenchmarkRunResponse = await res.json();
      setBenchmarkResult(data);
    } catch (err: any) {
      setError(err.message || 'Failed to complete benchmark');
    } finally {
      setIsRunning(false);
    }
  };

  const handleApply = () => {
    if (benchmarkResult) {
      onApplyRecommendation(benchmarkResult.recommendation.recommended_tile_size);
      setApplied(true);
      setTimeout(() => {
        onClose();
      }, 1200);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal-container benchmark-modal"
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: '680px', width: '92%' }}
      >
        <div className="modal-header">
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{ fontSize: '18px' }}>⚡</span>
            <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>
              Hardware Benchmark & Auto-Optimizer
            </h3>
          </div>
          <button className="modal-close-btn" onClick={onClose}>
            ✕
          </button>
        </div>

        <div className="modal-body" style={{ padding: '16px 20px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <div style={{ fontSize: '13px', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
            Measures neural super-resolution inference throughput (FPS, latency, and memory footprint) across tile sizes to optimize your hardware configuration (Topaz Video AI benchmark parity).
          </div>

          {error && (
            <div style={{ background: 'rgba(239, 68, 68, 0.15)', border: '1px solid rgba(239, 68, 68, 0.4)', borderRadius: '6px', padding: '10px 14px', color: '#f87171', fontSize: '12px' }}>
              <strong>Error:</strong> {error}
            </div>
          )}

          {!benchmarkResult && !isRunning && (
            <div style={{ textAlign: 'center', padding: '24px 16px', background: 'rgba(255, 255, 255, 0.02)', borderRadius: '8px', border: '1px dashed rgba(255, 255, 255, 0.1)' }}>
              <div style={{ fontSize: '32px', marginBottom: '8px' }}>🏁</div>
              <div style={{ fontSize: '14px', fontWeight: 600, marginBottom: '6px' }}>Ready to Benchmark System</div>
              <div style={{ fontSize: '12px', color: 'var(--text-muted)', marginBottom: '16px' }}>
                Runs high-frequency synthetic neural tensor inference on DirectML / GPU.
              </div>
              <button
                type="button"
                className="btn-primary"
                style={{ padding: '8px 24px', fontSize: '13px' }}
                onClick={handleRunBenchmark}
              >
                ▶ Run Benchmark Test
              </button>
            </div>
          )}

          {isRunning && (
            <div style={{ textAlign: 'center', padding: '32px 16px', background: 'rgba(255, 255, 255, 0.02)', borderRadius: '8px' }}>
              <div className="spinner" style={{ margin: '0 auto 16px auto', width: '32px', height: '32px' }} />
              <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--accent-color)' }}>
                Executing Neural Tile Benchmarks...
              </div>
              <div style={{ fontSize: '12px', color: 'var(--text-muted)', marginTop: '6px' }}>
                Testing 512px, 768px, and 1024px tensor tiling configurations...
              </div>
            </div>
          )}

          {benchmarkResult && (
            <>
              {/* Device and Recommendation Card */}
              <div style={{ background: 'rgba(56, 189, 248, 0.08)', border: '1px solid rgba(56, 189, 248, 0.25)', borderRadius: '8px', padding: '14px 16px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                  <div>
                    <div style={{ fontSize: '11px', textTransform: 'uppercase', color: '#38bdf8', fontWeight: 700, letterSpacing: '0.05em' }}>
                      Benchmark Score & Performance Tier
                    </div>
                    <div style={{ fontSize: '18px', fontWeight: 700, color: '#f8fafc', marginTop: '2px' }}>
                      {benchmarkResult.recommendation.performance_tier}
                    </div>
                    <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                      Device: <strong>{benchmarkResult.device_name}</strong> ({benchmarkResult.vendor})
                    </div>
                  </div>
                  <div style={{ textAlign: 'right' }}>
                    <div style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Studio Score</div>
                    <div style={{ fontSize: '26px', fontWeight: 800, color: '#38bdf8' }}>
                      {benchmarkResult.recommendation.overall_score.toLocaleString()} <span style={{ fontSize: '13px', fontWeight: 600 }}>PTS</span>
                    </div>
                  </div>
                </div>

                <div style={{ display: 'flex', gap: '16px', marginTop: '12px', paddingTop: '10px', borderTop: '1px solid rgba(56, 189, 248, 0.15)', fontSize: '12px' }}>
                  <div>
                    <span style={{ color: 'var(--text-muted)' }}>Optimal Tile Size: </span>
                    <strong style={{ color: '#38bdf8' }}>{benchmarkResult.recommendation.recommended_tile_size} px</strong>
                  </div>
                  <div>
                    <span style={{ color: 'var(--text-muted)' }}>Concurrency: </span>
                    <strong>{benchmarkResult.recommendation.recommended_batch_concurrency} Job(s)</strong>
                  </div>
                  <div>
                    <span style={{ color: 'var(--text-muted)' }}>Backend: </span>
                    <span>{benchmarkResult.recommendation.recommended_backend}</span>
                  </div>
                </div>
              </div>

              {/* Scoreboard Table */}
              <div>
                <div style={{ fontSize: '12px', fontWeight: 600, marginBottom: '6px', color: 'var(--text-secondary)' }}>
                  Detailed Benchmark Metrics
                </div>
                <table className="benchmark-table" style={{ width: '100%', borderCollapse: 'collapse', fontSize: '12px', textAlign: 'left' }}>
                  <thead>
                    <tr style={{ borderBottom: '1px solid rgba(255, 255, 255, 0.1)', color: 'var(--text-muted)' }}>
                      <th style={{ padding: '6px 8px' }}>Tile Configuration</th>
                      <th style={{ padding: '6px 8px' }}>Latency</th>
                      <th style={{ padding: '6px 8px' }}>Throughput (FPS)</th>
                      <th style={{ padding: '6px 8px' }}>VRAM Footprint</th>
                      <th style={{ padding: '6px 8px', textAlign: 'right' }}>Score</th>
                    </tr>
                  </thead>
                  <tbody>
                    {benchmarkResult.items.map((item, idx) => {
                      const isBest = item.tile_size === benchmarkResult.recommendation.recommended_tile_size;
                      return (
                        <tr
                          key={idx}
                          style={{
                            borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
                            background: isBest ? 'rgba(56, 189, 248, 0.08)' : undefined,
                          }}
                        >
                          <td style={{ padding: '8px' }}>
                            <strong>{item.tile_size} px</strong>
                            {isBest && (
                              <span style={{ marginLeft: '6px', fontSize: '10px', background: '#38bdf8', color: '#000', padding: '1px 5px', borderRadius: '3px', fontWeight: 700 }}>
                                RECOMMENDED
                              </span>
                            )}
                          </td>
                          <td style={{ padding: '8px' }}>{item.latency_ms.toFixed(1)} ms</td>
                          <td style={{ padding: '8px', color: '#38bdf8', fontWeight: 600 }}>{item.fps.toFixed(1)} FPS</td>
                          <td style={{ padding: '8px' }}>~{item.vram_mb} MB</td>
                          <td style={{ padding: '8px', textAlign: 'right', fontWeight: 600 }}>{item.score.toLocaleString()}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>

        <div className="modal-footer" style={{ padding: '12px 20px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderTop: '1px solid rgba(255, 255, 255, 0.08)' }}>
          {benchmarkResult && (
            <button
              type="button"
              className="btn-secondary"
              style={{ fontSize: '12px' }}
              onClick={handleRunBenchmark}
              disabled={isRunning}
            >
              🔄 Re-run Benchmark
            </button>
          )}
          {!benchmarkResult && <div />}

          <div style={{ display: 'flex', gap: '8px' }}>
            <button type="button" className="btn-secondary" onClick={onClose}>
              Close
            </button>
            {benchmarkResult && (
              <button
                type="button"
                className="btn-primary"
                onClick={handleApply}
                disabled={applied}
                style={{ background: applied ? '#22c55e' : undefined }}
              >
                {applied ? '✓ Settings Applied!' : '⚡ Apply Optimal Settings'}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
