import React, { useState, useEffect, useRef } from 'react';
import { SplitPlayer } from './components/SplitPlayer';
import { DeviceSelector, DeviceInfo } from './components/DeviceSelector';
import { ModelSettings, ModelDescriptor, UpscaleSettings } from './components/ModelSettings';

interface VideoStreamInfo {
  width: number;
  height: number;
  frame_rate: number;
  total_frames?: number;
  duration_seconds: number;
  codec_name: string;
}

interface MediaMetadata {
  file_path: string;
  format_name: string;
  duration_seconds: number;
  video_stream: VideoStreamInfo;
}

interface PreviewComparisonResult {
  original_data_url: string;
  upscaled_data_url: string;
  in_width: number;
  in_height: number;
  out_width: number;
  out_height: number;
  timestamp_seconds: number;
}

interface JobProgress {
  job_id: string;
  state: 'Queued' | 'Analyzing' | 'Processing' | 'Paused' | 'Completed' | 'Failed' | 'Cancelled';
  current_frame: number;
  total_frames: number;
  percentage: number;
  current_fps: number;
  eta_seconds: number;
  error_message?: string;
}

export const App: React.FC = () => {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<number>(0);
  const [models, setModels] = useState<ModelDescriptor[]>([]);
  
  const [inputPath, setInputPath] = useState<string>('samples/sample_test.mp4');
  const [outputPath, setOutputPath] = useState<string>('samples/sample_test_upscaled.mp4');
  const [metadata, setMetadata] = useState<MediaMetadata | null>(null);
  
  const [previewTime, setPreviewTime] = useState<number>(0.5);
  const [previewData, setPreviewData] = useState<PreviewComparisonResult | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState<boolean>(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  const [isProcessing, setIsProcessing] = useState<boolean>(false);
  const [progress, setProgress] = useState<JobProgress | null>(null);

  const [settings, setSettings] = useState<UpscaleSettings>({
    modelId: 'compact-span-x2',
    scaleFactor: 2,
    tileSize: 768,
    filmGrain: 0.12,
    temporalMode: 'Fast3Frame',
    encoder: 'hevc_amf',
    crf: 18,
  });


  // Load hardware devices and models on startup
  useEffect(() => {
    fetch('/api/devices')
      .then((r) => r.json())
      .then((data: DeviceInfo[]) => {
        setDevices(data);
        if (data.length > 0) {
          setSelectedDevice(data[0].id);
        }
      })
      .catch((err) => console.error('Failed to query devices:', err));

    fetch('/api/models')
      .then((r) => r.json())
      .then((data: ModelDescriptor[]) => {
        setModels(data);
        const compact = data.find((m) => m.id === 'compact-span-x2');
        if (compact) {
          setSettings((prev) => ({
            ...prev,
            modelId: compact.id,
            scaleFactor: compact.scale_factor,
            tileSize: compact.default_tile_size,
          }));
        }
      })
      .catch((err) => console.error('Failed to query models:', err));

    // Auto-probe initial sample if present
    handleProbeMedia('samples/sample_test.mp4');
  }, []);

  const handleProbeMedia = async (path: string) => {
    try {
      const res = await fetch('/api/probe', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path }),
      });
      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || 'Failed to probe video');
      }
      const data: MediaMetadata = await res.json();
      setMetadata(data);
      // Auto-set output path
      const stem = path.replace(/\.[^/.]+$/, '');
      setOutputPath(`${stem}_upscaled.mp4`);
      setPreviewError(null);
    } catch (err: any) {
      console.warn('Probe error:', err.message);
    }
  };

  const handleGeneratePreview = async () => {
    if (!inputPath) return;
    setIsPreviewLoading(true);
    setPreviewError(null);

    try {
      const res = await fetch('/api/preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          path: inputPath,
          timestamp_seconds: previewTime,
          model_id: settings.modelId,
          device_id: selectedDevice,
          film_grain: settings.filmGrain,
        }),
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || 'Preview generation failed');
      }

      const data: PreviewComparisonResult = await res.json();
      setPreviewData(data);
    } catch (err: any) {
      setPreviewError(err.message);
    } finally {
      setIsPreviewLoading(false);
    }
  };

  // Status polling during active upscale
  const pollTimerRef = useRef<number | null>(null);

  const startStatusPolling = () => {
    if (pollTimerRef.current) clearInterval(pollTimerRef.current);
    pollTimerRef.current = window.setInterval(async () => {
      try {
        const res = await fetch('/api/status');
        if (res.ok) {
          const prog: JobProgress = await res.json();
          setProgress(prog);

          if (prog.state === 'Completed' || prog.state === 'Failed' || prog.state === 'Cancelled') {
            setIsProcessing(false);
            if (pollTimerRef.current) {
              clearInterval(pollTimerRef.current);
              pollTimerRef.current = null;
            }
          }
        }
      } catch (err) {
        console.error('Polling error:', err);
      }
    }, 400);
  };

  const handleStartRender = async () => {
    setIsProcessing(true);
    setProgress(null);

    let encoderEnum: any = { AmdAmf: 'H265Hevc' };
    if (settings.encoder === 'hevc_nvenc') {
      encoderEnum = { NvidiaNvenc: 'H265Hevc' };
    } else if (settings.encoder === 'hevc_videotoolbox') {
      encoderEnum = { AppleVideoToolbox: 'H265Hevc' };
    } else if (settings.encoder === 'libx265') {
      encoderEnum = { SoftwareCpu: 'H265Hevc' };
    } else if (settings.encoder === 'prores_ks') {
      encoderEnum = { SoftwareCpu: 'ProRes' };
    }

    const payload = {
      id: `job-${Date.now()}`,
      input_path: inputPath,
      output_path: outputPath,
      model_id: settings.modelId,
      backend: { DirectMl: { device_id: selectedDevice } },
      tile_size: settings.tileSize > 0 ? settings.tileSize : null,
      tile_padding: null,
      film_grain: settings.filmGrain,
      temporal_mode: settings.temporalMode,
      encoding_profile: {

        encoder: encoderEnum,
        crf: settings.crf,
        bitrate_kbps: null,
        preset: 'medium',
        pixel_format: 'yuv420p',
        copy_audio: true,
        copy_subtitles: true,
      },
    };

    try {
      const res = await fetch('/api/upscale', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || 'Failed to start upscale job');
      }

      startStatusPolling();
    } catch (err: any) {
      alert(`Upscale start error: ${err.message}`);
      setIsProcessing(false);
    }
  };

  const handleCancelRender = async () => {
    try {
      await fetch('/api/cancel', { method: 'POST' });
    } catch (err) {
      console.error('Cancel error:', err);
    }
  };

  const activeDevice = devices.find((d) => d.id === selectedDevice);
  const activeDeviceName = activeDevice ? activeDevice.name : 'DirectML Device';

  const origLabel = previewData
    ? `Input (${previewData.in_width}×${previewData.in_height})`
    : metadata
    ? `Input (${metadata.video_stream.width}×${metadata.video_stream.height})`
    : 'Input Frame';

  const upscaledLabel = previewData
    ? `AI Super Scale (${previewData.out_width}×${previewData.out_height})`
    : metadata
    ? `Super Scale (${metadata.video_stream.width * settings.scaleFactor}×${metadata.video_stream.height * settings.scaleFactor})`
    : 'Super Scale Output';

  return (
    <>
      <header className="app-header">
        <div className="brand-title">
          <span>⚡ VIDEO UPSCALER STUDIO</span>
          <span className="badge">PRO</span>
        </div>
        <div style={{ fontSize: '13px', color: 'var(--text-muted)' }}>
          Active Engine:{' '}
          <strong style={{ color: '#60a5fa' }}>{activeDeviceName} (DirectX 12 Compute)</strong>
        </div>
      </header>

      <div className="app-workspace">
        <main className="viewport-panel">
          {/* File input and metadata header */}
          <div
            style={{
              padding: '12px 20px',
              backgroundColor: 'var(--bg-secondary)',
              borderBottom: '1px solid var(--border-color)',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
            }}
          >
            <span style={{ fontSize: '12px', color: 'var(--text-muted)', fontWeight: 600 }}>SOURCE:</span>
            <input
              type="text"
              className="text-input"
              style={{ flex: 1, padding: '6px 10px', fontSize: '12px' }}
              value={inputPath}
              onChange={(e) => setInputPath(e.target.value)}
              placeholder="e.g. C:/videos/clip.mp4 or samples/sample_test.mp4"
            />
            <button
              className="btn-secondary"
              onClick={() => handleProbeMedia(inputPath)}
            >
              Probe Info
            </button>
            {metadata && (
              <div style={{ display: 'flex', gap: '8px' }}>
                <span className="meta-chip">
                  <strong>{metadata.video_stream.width}×{metadata.video_stream.height}</strong>
                </span>
                <span className="meta-chip">
                  <strong>{metadata.video_stream.frame_rate.toFixed(1)} fps</strong>
                </span>
                <span className="meta-chip">
                  <strong>{metadata.duration_seconds.toFixed(1)}s</strong>
                </span>
                <span className="meta-chip">
                  <strong>{metadata.video_stream.codec_name.toUpperCase()}</strong>
                </span>
              </div>
            )}
          </div>

          {/* Center viewport with SplitPlayer */}
          <div className="player-container">
            <SplitPlayer
              originalSrc={previewData?.original_data_url}
              upscaledSrc={previewData?.upscaled_data_url}
              originalLabel={origLabel}
              upscaledLabel={upscaledLabel}
              isLoading={isPreviewLoading}
              loadingMessage={`DirectML neural upscaling frame at ${previewTime.toFixed(2)}s...`}
            />
            {previewError && (
              <div
                style={{
                  position: 'absolute',
                  top: '16px',
                  backgroundColor: '#ef4444dd',
                  color: '#fff',
                  padding: '8px 16px',
                  borderRadius: '6px',
                  fontSize: '12px',
                  fontWeight: 500,
                  zIndex: 40,
                }}
              >
                Error: {previewError}
              </div>
            )}
          </div>

          {/* Frame Preview Controls & Timeline */}
          <div
            style={{
              padding: '10px 20px',
              backgroundColor: 'var(--bg-secondary)',
              borderTop: '1px solid var(--border-color)',
              display: 'flex',
              alignItems: 'center',
              gap: '16px',
            }}
          >
            <span style={{ fontSize: '12px', color: 'var(--text-muted)', minWidth: '85px' }}>
              Preview Time:
            </span>
            <input
              type="range"
              min="0"
              max={metadata ? metadata.duration_seconds : 10}
              step="0.05"
              className="range-slider"
              value={previewTime}
              onChange={(e) => setPreviewTime(parseFloat(e.target.value))}
            />
            <span style={{ fontSize: '13px', fontVariantNumeric: 'tabular-nums', minWidth: '50px' }}>
              {previewTime.toFixed(2)}s
            </span>
            <button
              className="btn-secondary"
              disabled={isPreviewLoading || !inputPath}
              onClick={handleGeneratePreview}
            >
              {isPreviewLoading ? 'Generating...' : '🔍 Extract Frame Preview'}
            </button>
          </div>

          {/* Live Upscale Progress Bar */}
          <div
            style={{
              height: '52px',
              backgroundColor: 'var(--bg-primary)',
              borderTop: '1px solid var(--border-color)',
              display: 'flex',
              alignItems: 'center',
              padding: '0 20px',
              gap: '16px',
            }}
          >
            <span style={{ fontSize: '12px', color: 'var(--text-muted)', minWidth: '100px' }}>
              {isProcessing
                ? `Exporting: ${progress?.percentage?.toFixed(1) || 0}%`
                : progress?.state === 'Completed'
                ? 'Export Complete! 🎉'
                : 'Status: Idle'}
            </span>
            <div
              style={{
                flex: 1,
                height: '8px',
                backgroundColor: 'var(--bg-tertiary)',
                borderRadius: '4px',
                overflow: 'hidden',
              }}
            >
              <div
                style={{
                  width: `${progress?.percentage || 0}%`,
                  height: '100%',
                  backgroundColor:
                    progress?.state === 'Completed'
                      ? 'var(--success-color)'
                      : progress?.state === 'Failed'
                      ? '#ef4444'
                      : 'var(--accent-color)',
                  transition: 'width 0.2s ease',
                }}
              />
            </div>

            {isProcessing && progress && (
              <div style={{ display: 'flex', alignItems: 'center', gap: '14px', fontSize: '12px' }}>
                <span style={{ color: '#60a5fa' }}>
                  Frame {progress.current_frame} / {progress.total_frames}
                </span>
                <span style={{ color: 'var(--text-muted)' }}>
                  Speed: {progress.current_fps.toFixed(1)} fps
                </span>
                <span style={{ color: 'var(--text-muted)' }}>
                  ETA: {progress.eta_seconds}s
                </span>
                <button className="btn-danger" onClick={handleCancelRender}>
                  Cancel
                </button>
              </div>
            )}
          </div>
        </main>

        <aside className="controls-sidebar">
          <div className="sidebar-section">
            <div className="section-title">Hardware Acceleration</div>
            <DeviceSelector
              devices={devices}
              selectedDeviceId={selectedDevice}
              onSelectDevice={setSelectedDevice}
            />
          </div>

          <ModelSettings
            models={models}
            settings={settings}
            onChange={setSettings}
          />

          <div className="sidebar-section">
            <div className="section-title">Output Destination</div>
            <div className="form-group">
              <label className="form-label">Export File Path</label>
              <input
                type="text"
                className="text-input"
                value={outputPath}
                onChange={(e) => setOutputPath(e.target.value)}
              />
            </div>
          </div>

          <div className="render-action-panel">
            <button
              className="btn-primary"
              disabled={isProcessing}
              onClick={handleStartRender}
            >
              {isProcessing ? 'Processing Video Queue...' : `Export ${settings.scaleFactor}x Super Resolution`}
            </button>
          </div>
        </aside>
      </div>
    </>
  );
};

export default App;
