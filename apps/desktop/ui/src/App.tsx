import React, { useState, useEffect, useRef } from 'react';
import { SplitPlayer } from './components/SplitPlayer';
import { DeviceSelector, DeviceInfo } from './components/DeviceSelector';
import { ModelSettings, ModelDescriptor, UpscaleSettings } from './components/ModelSettings';
import { BatchQueue, BatchJobItem } from './components/BatchQueue';

interface VideoStreamInfo {
  width: number;
  height: number;
  frame_rate: number;
  total_frames?: number;
  duration_seconds: number;
  codec_name: string;
  pixel_format?: string;
  color_space?: string;
  color_primaries?: string;
  color_transfer?: string;
  field_order?: string;
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
  const [activeTab, setActiveTab] = useState<'preview' | 'queue'>('preview');
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

  const [trimStart, setTrimStart] = useState<number | null>(null);
  const [trimEnd, setTrimEnd] = useState<number | null>(null);

  const [isProcessing, setIsProcessing] = useState<boolean>(false);
  const [progress, setProgress] = useState<JobProgress | null>(null);

  // Batch queue state
  const [queueJobs, setQueueJobs] = useState<BatchJobItem[]>([]);
  const [isQueueActive, setIsQueueActive] = useState<boolean>(false);

  const stepTime = (delta: number) => {
    const maxDur = metadata ? metadata.duration_seconds : 1000;
    setPreviewTime((prev) => Math.max(0, Math.min(Number((prev + delta).toFixed(3)), maxDur)));
  };

  const handleSetIn = () => {
    setTrimStart(previewTime);
    if (trimEnd !== null && previewTime > trimEnd) {
      setTrimEnd(null);
    }
  };

  const handleSetOut = () => {
    setTrimEnd(previewTime);
    if (trimStart !== null && previewTime < trimStart) {
      setTrimStart(null);
    }
  };

  const handleResetTrim = () => {
    setTrimStart(null);
    setTrimEnd(null);
  };

  const formatTimecode = (secs: number, fps: number = 30) => {
    const safeFps = fps > 0 ? fps : 30;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    const f = Math.floor((secs % 1) * safeFps);
    const pad = (n: number) => n.toString().padStart(2, '0');
    return `${pad(h)}:${pad(m)}:${pad(s)}:${pad(f)}`;
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT')) {
        return;
      }
      if (e.key === 'i' || e.key === 'I') {
        e.preventDefault();
        setTrimStart(previewTime);
        if (trimEnd !== null && previewTime > trimEnd) {
          setTrimEnd(null);
        }
      } else if (e.key === 'o' || e.key === 'O') {
        e.preventDefault();
        setTrimEnd(previewTime);
        if (trimStart !== null && previewTime < trimStart) {
          setTrimStart(null);
        }
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        const fps = metadata?.video_stream?.frame_rate || 30;
        const delta = e.shiftKey ? -1.0 : -1.0 / fps;
        stepTime(delta);
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        const fps = metadata?.video_stream?.frame_rate || 30;
        const delta = e.shiftKey ? 1.0 : 1.0 / fps;
        stepTime(delta);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [previewTime, trimStart, trimEnd, metadata]);

  const [settings, setSettings] = useState<UpscaleSettings>({
    modelId: 'compact-span-x2',
    scaleFactor: 2,
    tileSize: 768,
    filmGrain: 0.12,
    temporalMode: 'Fast3Frame',
    encoder: 'hevc_amf',
    colorFormat: 'sdr_8bit',
    interpolationMode: 'Disabled',
    deinterlaceMode: 'Disabled',
    denoiseStrength: 0.0,
    crf: 18,
    audioCodec: 'Copy',
    audioBitrateKbps: 192,
    loudnessNormalization: false,
    targetLufs: -14.0,
    targetResolution: 'NativeScale',
    fitMode: 'Letterbox',
    matteColor: 'black',
  });

  const loadModels = () => {
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
  };

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

    loadModels();

    // Auto-probe initial sample if present
    handleProbeMedia('samples/sample_test.mp4');
    fetchQueueStatus();
  }, []);

  // Periodic polling for batch queue
  useEffect(() => {
    const timer = setInterval(() => {
      fetchQueueStatus();
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  const fetchQueueStatus = async () => {
    try {
      const res = await fetch('/api/queue');
      if (res.ok) {
        const data = await res.json();
        setQueueJobs(data.jobs || []);
        setIsQueueActive(data.is_active || false);
      }
    } catch (err) {
      console.error('Failed to query queue status:', err);
    }
  };

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
      // Auto-detect HDR10 or 10-bit source video
      if (data.video_stream.color_transfer === 'smpte2084' || data.video_stream.color_primaries === 'bt2020') {
        setSettings((prev) => ({ ...prev, colorFormat: 'hdr10_10bit' }));
      } else if (data.video_stream.pixel_format?.includes('10')) {
        setSettings((prev) => ({ ...prev, colorFormat: 'sdr_10bit' }));
      }
      // Auto-set output path
      const stem = path.replace(/\.[^/.]+$/, '');
      setOutputPath(`${stem}_upscaled.mp4`);
      setPreviewError(null);
    } catch (err: any) {
      console.warn('Probe error:', err.message);
    }
  };

  const handleBrowseInput = async () => {
    try {
      const res = await fetch('/api/dialog/open-file', { method: 'POST' });
      if (res.ok) {
        const data = await res.json();
        if (data.path) {
          setInputPath(data.path);
          handleProbeMedia(data.path);
        }
      }
    } catch (err) {
      console.error('File dialog error:', err);
    }
  };

  const handleBrowseOutput = async () => {
    try {
      const defaultName = outputPath ? outputPath.split(/[\\/]/).pop() : 'upscaled.mp4';
      const res = await fetch('/api/dialog/save-file', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ default_name: defaultName }),
      });
      if (res.ok) {
        const data = await res.json();
        if (data.path) {
          setOutputPath(data.path);
        }
      }
    } catch (err) {
      console.error('Save dialog error:', err);
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
          deinterlace_mode: settings.deinterlaceMode,
          denoise_strength: settings.denoiseStrength,
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

  // Status polling during active single upscale
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

  const getEncoderEnum = (encoderStr: string) => {
    switch (encoderStr) {
      case 'hevc_nvenc':
        return { NvidiaNvenc: 'H265Hevc' };
      case 'av1_nvenc':
        return { NvidiaNvenc: 'Av1' };
      case 'hevc_amf':
        return { AmdAmf: 'H265Hevc' };
      case 'av1_amf':
        return { AmdAmf: 'Av1' };
      case 'hevc_videotoolbox':
        return { AppleVideoToolbox: 'H265Hevc' };
      case 'libx265':
        return { SoftwareCpu: 'H265Hevc' };
      case 'av1_svt':
        return { SoftwareCpu: 'Av1' };
      case 'prores_ks':
        return { SoftwareCpu: 'ProRes' };
      default:
        return { AmdAmf: 'H265Hevc' };
    }
  };

  const buildEncodingProfile = () => {
    const encoderEnum = getEncoderEnum(settings.encoder);
    let pixel_format = 'yuv420p';
    let high_bit_depth = false;
    let color_primaries: string | null = null;
    let color_transfer: string | null = null;
    let color_space: string | null = null;

    if (settings.colorFormat === 'hdr10_10bit') {
      pixel_format = 'yuv420p10le';
      high_bit_depth = true;
      color_primaries = 'bt2020';
      color_transfer = 'smpte2084';
      color_space = 'bt2020nc';
    } else if (settings.colorFormat === 'sdr_10bit') {
      pixel_format = 'yuv420p10le';
      high_bit_depth = true;
      color_primaries = 'bt709';
      color_transfer = 'bt709';
      color_space = 'bt709';
    } else if (settings.colorFormat === 'prores_hq') {
      pixel_format = 'yuv422p10le';
      high_bit_depth = true;
      color_primaries = 'bt709';
      color_transfer = 'bt709';
      color_space = 'bt709';
    }

    const audio_profile = {
      codec: settings.audioCodec,
      bitrate_kbps: settings.audioBitrateKbps,
      loudness_normalization: settings.loudnessNormalization,
      target_lufs: settings.targetLufs,
    };

    const geometry_profile = {
      resolution: settings.targetResolution === 'Custom'
        ? { Custom: { width: settings.customWidth || 1920, height: settings.customHeight || 1080 } }
        : settings.targetResolution,
      fit_mode: settings.fitMode,
      matte_color: settings.matteColor || 'black',
    };

    return {
      encoder: encoderEnum,
      crf: settings.crf,
      bitrate_kbps: null,
      preset: 'medium',
      pixel_format,
      copy_audio: settings.audioCodec === 'Copy',
      copy_subtitles: true,
      color_primaries,
      color_transfer,
      color_space,
      high_bit_depth,
      audio_profile,
      geometry_profile,
    };
  };

  const handleStartRender = async () => {
    setIsProcessing(true);
    setProgress(null);

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
      interpolation_mode: settings.interpolationMode,
      deinterlace_mode: settings.deinterlaceMode,
      denoise_strength: settings.denoiseStrength,
      encoding_profile: buildEncodingProfile(),
      trim_start: trimStart,
      trim_end: trimEnd,
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

  // Batch queue handlers
  const handleQueueCurrentJob = async () => {
    if (!inputPath || !outputPath) return;
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
      interpolation_mode: settings.interpolationMode,
      deinterlace_mode: settings.deinterlaceMode,
      denoise_strength: settings.denoiseStrength,
      encoding_profile: buildEncodingProfile(),
      trim_start: trimStart,
      trim_end: trimEnd,
    };

    try {
      const res = await fetch('/api/queue/add', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (res.ok) {
        await fetchQueueStatus();
        setActiveTab('queue');
      }
    } catch (err) {
      console.error('Error queuing current job:', err);
    }
  };

  const handleAddFilesToQueue = async () => {
    try {
      const res = await fetch('/api/dialog/open-files', { method: 'POST' });
      if (!res.ok) return;
      const data = await res.json();
      if (!data.paths || data.paths.length === 0) return;

      const profile = buildEncodingProfile();

      for (const path of data.paths) {
        const stem = path.replace(/\.[^/.]+$/, '');
        const outPath = `${stem}_${settings.scaleFactor}x_upscaled.mp4`;
        const payload = {
          id: `job-${Date.now()}-${Math.floor(Math.random() * 10000)}`,
          input_path: path,
          output_path: outPath,
          model_id: settings.modelId,
          backend: { DirectMl: { device_id: selectedDevice } },
          tile_size: settings.tileSize > 0 ? settings.tileSize : null,
          tile_padding: null,
          film_grain: settings.filmGrain,
          temporal_mode: settings.temporalMode,
          interpolation_mode: settings.interpolationMode,
          deinterlace_mode: settings.deinterlaceMode,
          denoise_strength: settings.denoiseStrength,
          encoding_profile: profile,
          trim_start: null,
          trim_end: null,
        };
        await fetch('/api/queue/add', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
      }
      await fetchQueueStatus();
      setActiveTab('queue');
    } catch (e) {
      console.error('Error adding batch files:', e);
    }
  };

  const handleStartQueue = async () => {
    try {
      await fetch('/api/queue/start', { method: 'POST' });
      fetchQueueStatus();
    } catch (err) {
      console.error('Queue start error:', err);
    }
  };

  const handlePauseQueue = async () => {
    try {
      await fetch('/api/queue/pause', { method: 'POST' });
      fetchQueueStatus();
    } catch (err) {
      console.error('Queue pause error:', err);
    }
  };

  const handleClearCompleted = async () => {
    try {
      await fetch('/api/queue/clear', { method: 'POST' });
      fetchQueueStatus();
    } catch (err) {
      console.error('Queue clear error:', err);
    }
  };

  const handleRemoveJob = async (jobId: string) => {
    try {
      await fetch('/api/queue/remove', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ job_id: jobId }),
      });
      fetchQueueStatus();
    } catch (err) {
      console.error('Queue remove error:', err);
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

  const queuedCount = queueJobs.filter((j) => j.state === 'Queued').length;

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

      {/* Tab Bar for View Modes */}
      <div
        style={{
          display: 'flex',
          gap: '8px',
          padding: '0 20px',
          backgroundColor: 'var(--bg-secondary)',
          borderBottom: '1px solid var(--border-color)',
        }}
      >
        <button
          className={`tab-btn ${activeTab === 'preview' ? 'active' : ''}`}
          onClick={() => setActiveTab('preview')}
        >
          🎬 Studio Preview & Compare
        </button>
        <button
          className={`tab-btn ${activeTab === 'queue' ? 'active' : ''}`}
          onClick={() => setActiveTab('queue')}
        >
          📋 Batch Queue {queueJobs.length > 0 && `(${queueJobs.length})`}
          {isQueueActive && (
            <span
              style={{
                display: 'inline-block',
                width: '7px',
                height: '7px',
                borderRadius: '50%',
                backgroundColor: '#10b981',
                marginLeft: '6px',
              }}
            />
          )}
        </button>
      </div>

      <div className="app-workspace">
        <main className="viewport-panel">
          {activeTab === 'preview' ? (
            <>
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
                <button className="btn-secondary" onClick={handleBrowseInput}>
                  📂 Browse...
                </button>
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
                      {settings.interpolationMode !== 'Disabled' && (
                        <span style={{ color: '#a78bfa', marginLeft: '5px' }}>
                          → {settings.interpolationMode === 'DoubleFps'
                            ? `${(metadata.video_stream.frame_rate * 2).toFixed(1)} fps (2× Motion)`
                            : settings.interpolationMode === 'QuadrupleFps'
                            ? `${(metadata.video_stream.frame_rate * 4).toFixed(1)} fps (4× Motion)`
                            : settings.interpolationMode === 'SlowMotion2x'
                            ? `(2× Slow-Mo)`
                            : `(4× Slow-Mo)`}
                        </span>
                      )}
                    </span>
                    <span className="meta-chip">
                      <strong>{metadata.duration_seconds.toFixed(1)}s</strong>
                    </span>
                    <span className="meta-chip">
                      <strong>{metadata.video_stream.codec_name.toUpperCase()}</strong>
                    </span>
                    <span
                      className="meta-chip"
                      style={{
                        backgroundColor: (metadata.video_stream.color_transfer === 'smpte2084' || metadata.video_stream.color_primaries === 'bt2020')
                          ? 'rgba(234, 179, 8, 0.2)'
                          : 'rgba(59, 130, 246, 0.15)',
                        borderColor: (metadata.video_stream.color_transfer === 'smpte2084' || metadata.video_stream.color_primaries === 'bt2020')
                          ? '#eab308'
                          : '#3b82f6',
                        color: (metadata.video_stream.color_transfer === 'smpte2084' || metadata.video_stream.color_primaries === 'bt2020')
                          ? '#facc15'
                          : '#60a5fa',
                      }}
                    >
                      <strong>
                        {metadata.video_stream.pixel_format?.includes('10') ? '10-BIT' : '8-BIT'}{' '}
                        {metadata.video_stream.color_transfer === 'smpte2084' ? 'HDR10' : 'SDR'}
                      </strong>
                    </span>
                    {metadata.video_stream.field_order &&
                      metadata.video_stream.field_order !== 'progressive' &&
                      metadata.video_stream.field_order !== 'unknown' && (
                        <span
                          className="meta-chip"
                          style={{
                            backgroundColor: 'rgba(239, 68, 68, 0.2)',
                            borderColor: '#ef4444',
                            color: '#f87171',
                          }}
                        >
                          <strong>INTERLACED ({metadata.video_stream.field_order.toUpperCase()})</strong>
                        </span>
                      )}
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

              {/* Frame Preview Controls & Interactive Timeline Trimmer */}
              <div
                style={{
                  padding: '12px 20px',
                  backgroundColor: 'var(--bg-secondary)',
                  borderTop: '1px solid var(--border-color)',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '8px',
                }}
              >
                {/* Upper Row: Timecode, Slider scrubber with Trim Range Bar, Preview Button */}
                <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
                  <div style={{ minWidth: '150px', display: 'flex', flexDirection: 'column' }}>
                    <span style={{ fontSize: '11px', color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.5px' }}>
                      Playhead / Duration
                    </span>
                    <span style={{ fontSize: '13px', fontWeight: 600, color: '#f3f4f6', fontVariantNumeric: 'tabular-nums' }}>
                      {formatTimecode(previewTime, metadata?.video_stream?.frame_rate || 30)} / {formatTimecode(metadata?.duration_seconds || 0, metadata?.video_stream?.frame_rate || 30)}
                    </span>
                  </div>

                  <div style={{ flex: 1, display: 'flex', flexDirection: 'column', position: 'relative' }}>
                    <input
                      type="range"
                      min="0"
                      max={metadata ? metadata.duration_seconds : 10}
                      step="0.01"
                      className="range-slider"
                      value={previewTime}
                      onChange={(e) => setPreviewTime(parseFloat(e.target.value))}
                    />
                    <div className="trim-bar-container">
                      {(trimStart !== null || trimEnd !== null) && (
                        <div
                          className="trim-range-highlight"
                          style={{
                            left: `${((trimStart ?? 0) / (metadata?.duration_seconds || 10)) * 100}%`,
                            width: `${Math.max(
                              0.5,
                              (((trimEnd ?? (metadata?.duration_seconds || 10)) - (trimStart ?? 0)) /
                                (metadata?.duration_seconds || 10)) *
                                100
                            )}%`,
                          }}
                        />
                      )}
                    </div>
                  </div>

                  <button
                    className="btn-secondary"
                    disabled={isPreviewLoading || !inputPath}
                    onClick={handleGeneratePreview}
                    style={{ whiteSpace: 'nowrap' }}
                  >
                    {isPreviewLoading ? 'Generating...' : '🔍 Extract Frame Preview'}
                  </button>
                </div>

                {/* Lower Row: Frame Stepping, In/Out Trimming Controls & Segment Status */}
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '12px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                    <span style={{ fontSize: '11px', color: 'var(--text-muted)', marginRight: '4px' }}>Step:</span>
                    <button className="step-btn" onClick={() => stepTime(-1.0)} title="Step Backward 1 Second (Shift+Left)">
                      ◀ -1s
                    </button>
                    <button
                      className="step-btn"
                      onClick={() => stepTime(-(1.0 / (metadata?.video_stream?.frame_rate || 30)))}
                      title="Step Backward 1 Frame (Left Arrow)"
                    >
                      ◀ -1f
                    </button>
                    <button
                      className="step-btn"
                      onClick={() => stepTime(1.0 / (metadata?.video_stream?.frame_rate || 30))}
                      title="Step Forward 1 Frame (Right Arrow)"
                    >
                      +1f ▶
                    </button>
                    <button className="step-btn" onClick={() => stepTime(1.0)} title="Step Forward 1 Second (Shift+Right)">
                      +1s ▶
                    </button>

                    <div style={{ width: '1px', height: '16px', backgroundColor: 'var(--border-color)', margin: '0 6px' }} />

                    <button
                      className={`step-btn ${trimStart !== null ? 'active' : ''}`}
                      onClick={handleSetIn}
                      style={{
                        backgroundColor: trimStart !== null ? 'rgba(56, 189, 248, 0.2)' : undefined,
                        borderColor: trimStart !== null ? '#38bdf8' : undefined,
                        color: trimStart !== null ? '#38bdf8' : undefined,
                      }}
                      title="Set In Point (Key: I)"
                    >
                      [ Set In (I)
                    </button>
                    <button
                      className={`step-btn ${trimEnd !== null ? 'active' : ''}`}
                      onClick={handleSetOut}
                      style={{
                        backgroundColor: trimEnd !== null ? 'rgba(56, 189, 248, 0.2)' : undefined,
                        borderColor: trimEnd !== null ? '#38bdf8' : undefined,
                        color: trimEnd !== null ? '#38bdf8' : undefined,
                      }}
                      title="Set Out Point (Key: O)"
                    >
                      ] Set Out (O)
                    </button>
                    {(trimStart !== null || trimEnd !== null) && (
                      <button
                        className="step-btn"
                        onClick={handleResetTrim}
                        title="Clear In/Out Trim (Export Entire Clip)"
                      >
                        ⟲ Full Clip
                      </button>
                    )}
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    {trimStart !== null || trimEnd !== null ? (
                      <span
                        className="meta-chip"
                        style={{
                          backgroundColor: 'rgba(6, 182, 212, 0.15)',
                          borderColor: '#06b6d4',
                          color: '#22d3ee',
                          fontSize: '11px',
                        }}
                      >
                        ✂ Trim Range: [
                        {formatTimecode(trimStart ?? 0, metadata?.video_stream?.frame_rate || 30)} →{' '}
                        {formatTimecode(trimEnd ?? (metadata?.duration_seconds || 0), metadata?.video_stream?.frame_rate || 30)}
                        ]{' '}
                        (
                        {(
                          (trimEnd ?? (metadata?.duration_seconds || 0)) - (trimStart ?? 0)
                        ).toFixed(2)}
                        s /{' '}
                        {Math.round(
                          ((trimEnd ?? (metadata?.duration_seconds || 0)) - (trimStart ?? 0)) *
                            (metadata?.video_stream?.frame_rate || 30)
                        )}{' '}
                        frames)
                      </span>
                    ) : (
                      <span
                        className="meta-chip"
                        style={{
                          fontSize: '11px',
                          color: 'var(--text-muted)',
                        }}
                      >
                        🎞 Export Mode: Entire Video (
                        {metadata?.duration_seconds ? `${metadata.duration_seconds.toFixed(1)}s` : 'Full'}
                        )
                      </span>
                    )}
                  </div>
                </div>
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
            </>
          ) : (
            <BatchQueue
              jobs={queueJobs}
              isActive={isQueueActive}
              onStartQueue={handleStartQueue}
              onPauseQueue={handlePauseQueue}
              onClearCompleted={handleClearCompleted}
              onRemoveJob={handleRemoveJob}
              onAddFiles={handleAddFilesToQueue}
            />
          )}
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
            onModelDownloaded={loadModels}
          />

          <div className="sidebar-section">
            <div className="section-title">Output Destination</div>
            <div className="form-group">
              <label className="form-label">Export File Path</label>
              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="text"
                  className="text-input"
                  style={{ flex: 1 }}
                  value={outputPath}
                  onChange={(e) => setOutputPath(e.target.value)}
                />
                <button className="btn-secondary" onClick={handleBrowseOutput}>
                  Browse...
                </button>
              </div>
            </div>
          </div>

          <div className="render-action-panel" style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <button
              className="btn-primary"
              disabled={isProcessing}
              onClick={handleStartRender}
            >
              {isProcessing
                ? 'Processing Video...'
                : trimStart !== null || trimEnd !== null
                ? `Export Trimmed Range (${settings.scaleFactor}x Super Scale)`
                : `Export ${settings.scaleFactor}x Super Resolution`}
            </button>
            <button
              className="btn-secondary"
              onClick={handleQueueCurrentJob}
              disabled={!inputPath || !outputPath}
              title="Add current file and profile settings to background batch queue"
            >
              ➕ Add to Batch Queue ({queuedCount} Pending)
            </button>
          </div>
        </aside>
      </div>
    </>
  );
};

export default App;
