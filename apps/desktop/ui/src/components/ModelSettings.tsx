import React, { useState } from 'react';

export interface ModelDescriptor {
  id: string;
  name: string;
  description: string;
  category: string;
  scale_factor: number;
  temporal_window_frames: number;
  default_tile_size: number;
  default_padding: number;
  supports_fp16: boolean;
  filename: string;
  download_url?: string;
  is_cached?: boolean;
}

export interface UpscaleSettings {
  modelId: string;
  scaleFactor: number;
  tileSize: number;
  filmGrain: number;
  temporalMode: 'Disabled' | 'Fast3Frame' | 'Quality5Frame';
  encoder: string;
  crf: number;
  colorFormat: 'sdr_8bit' | 'hdr10_10bit' | 'sdr_10bit' | 'prores_hq';
  interpolationMode: 'Disabled' | 'DoubleFps' | 'QuadrupleFps' | 'SlowMotion2x' | 'SlowMotion4x';
  deinterlaceMode: 'Disabled' | 'Auto' | 'BwdifSameRate' | 'BwdifBob' | 'Yadif';
  denoiseStrength: number;
}

interface ModelSettingsProps {
  models: ModelDescriptor[];
  settings: UpscaleSettings;
  onChange: (updated: UpscaleSettings) => void;
  onModelDownloaded?: () => void;
}

export const ModelSettings: React.FC<ModelSettingsProps> = ({
  models,
  settings,
  onChange,
  onModelDownloaded,
}) => {
  const [downloadingModelId, setDownloadingModelId] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<{
    percentage: number;
    downloaded: number;
    total: number;
  } | null>(null);

  const update = <K extends keyof UpscaleSettings>(key: K, value: UpscaleSettings[K]) => {
    onChange({ ...settings, [key]: value });
  };

  const selectedModel = models.find((m) => m.id === settings.modelId);

  const handleDownload = async (modelId: string) => {
    try {
      setDownloadingModelId(modelId);
      setDownloadProgress({ percentage: 0, downloaded: 0, total: 0 });
      await fetch('/api/models/download', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model_id: modelId }),
      });

      const timer = setInterval(async () => {
        try {
          const res = await fetch('/api/models/downloads');
          if (res.ok) {
            const data = await res.json();
            const prog = data[modelId];
            if (prog) {
              setDownloadProgress({
                percentage: prog.percentage,
                downloaded: prog.downloaded_bytes,
                total: prog.total_bytes,
              });
              if (prog.is_completed) {
                clearInterval(timer);
                setDownloadingModelId(null);
                setDownloadProgress(null);
                if (onModelDownloaded) onModelDownloaded();
              }
            }
          }
        } catch (e) {
          console.error(e);
        }
      }, 500);
    } catch (e) {
      console.error('Download error:', e);
      setDownloadingModelId(null);
    }
  };

  const handleModelChange = (id: string) => {
    const found = models.find((m) => m.id === id);
    if (found) {
      onChange({
        ...settings,
        modelId: id,
        scaleFactor: found.scale_factor,
        tileSize: found.default_tile_size,
      });
    } else {
      update('modelId', id);
    }
  };

  const applyPreset = (preset: string) => {
    switch (preset) {
      case 'web':
        onChange({
          ...settings,
          modelId: 'compact-span-x2',
          scaleFactor: 2,
          tileSize: 768,
          filmGrain: 0.08,
          temporalMode: 'Fast3Frame',
          encoder: 'av1_amf',
          colorFormat: 'sdr_8bit',
          interpolationMode: 'Disabled',
          deinterlaceMode: 'Disabled',
          denoiseStrength: 0.0,
          crf: 20,
        });
        break;
      case 'cinematic':
        onChange({
          ...settings,
          modelId: 'realesrgan-x4plus',
          scaleFactor: 4,
          tileSize: 512,
          filmGrain: 0.15,
          temporalMode: 'Quality5Frame',
          encoder: 'hevc_nvenc',
          colorFormat: 'hdr10_10bit',
          interpolationMode: 'DoubleFps',
          deinterlaceMode: 'Disabled',
          denoiseStrength: 0.0,
          crf: 16,
        });
        break;
      case 'anime':
        onChange({
          ...settings,
          modelId: 'realesrgan-anime-6b',
          scaleFactor: 4,
          tileSize: 512,
          filmGrain: 0.02,
          temporalMode: 'Fast3Frame',
          encoder: 'hevc_nvenc',
          colorFormat: 'sdr_8bit',
          interpolationMode: 'Disabled',
          deinterlaceMode: 'Disabled',
          denoiseStrength: 0.0,
          crf: 18,
        });
        break;
      case 'archival':
        onChange({
          ...settings,
          modelId: 'compact-span-x2',
          scaleFactor: 2,
          tileSize: 768,
          filmGrain: 0.15,
          temporalMode: 'Quality5Frame',
          encoder: 'prores_ks',
          colorFormat: 'prores_hq',
          interpolationMode: 'DoubleFps',
          deinterlaceMode: 'Auto',
          denoiseStrength: 0.35,
          crf: 16,
        });
        break;
    }
  };

  return (
    <div className="sidebar-section">
      <div className="section-title">Workflow Presets</div>
      <div className="form-group">
        <label className="form-label">Quick Profile</label>
        <select
          className="select-input"
          style={{ borderColor: '#3b82f6', background: 'rgba(59, 130, 246, 0.1)' }}
          onChange={(e) => applyPreset(e.target.value)}
          defaultValue="custom"
        >
          <option value="custom">⚙ Custom Profile</option>
          <option value="web">⚡ Web & Social (2x Fast, AMF/NVENC, CRF 20, 3-Frame)</option>
          <option value="cinematic">🎬 Cinematic 4K Master (4x Real-ESRGAN, Grain 15%, CRF 16, 5-Frame)</option>
          <option value="anime">✨ Anime & Animation (4x Crisp Lines, Low Grain, Fast)</option>
          <option value="archival">📼 Archival Restoration (2x Compact, Grain 20%, ProRes 422)</option>
        </select>
      </div>

      <div className="section-title" style={{ marginTop: '16px' }}>Enhancement Model</div>


      <div className="form-group">
        <label className="form-label">Neural Model Architecture</label>
        <select
          className="select-input"
          value={settings.modelId}
          onChange={(e) => handleModelChange(e.target.value)}
        >
          {models.length > 0 ? (
            models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name} ({m.scale_factor}x)
              </option>
            ))
          ) : (
            <option value="compact-span-x2">Compact SPAN (x2 Fast)</option>
          )}
        </select>
        {selectedModel && (
          <>
            <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginTop: '4px', lineHeight: 1.4 }}>
              {selectedModel.description}
            </div>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginTop: '8px' }}>
              <span style={{ fontSize: '11px', color: selectedModel.is_cached ? '#10b981' : '#f59e0b', fontWeight: 600 }}>
                {selectedModel.is_cached ? '● Ready (Cached in models/cache)' : '○ Weights Not Cached'}
              </span>
              {!selectedModel.is_cached && selectedModel.download_url && (
                <button
                  className="btn-secondary"
                  style={{ fontSize: '11px', padding: '3px 8px' }}
                  disabled={downloadingModelId === selectedModel.id}
                  onClick={() => handleDownload(selectedModel.id)}
                >
                  {downloadingModelId === selectedModel.id
                    ? `Downloading (${downloadProgress?.percentage.toFixed(0) || 0}%)`
                    : '⬇ Download Weights'}
                </button>
              )}
            </div>
            {downloadingModelId === selectedModel.id && downloadProgress && (
              <div style={{ marginTop: '6px' }}>
                <div style={{ height: '4px', backgroundColor: 'var(--bg-tertiary)', borderRadius: '2px', overflow: 'hidden' }}>
                  <div
                    style={{
                      width: `${downloadProgress.percentage}%`,
                      height: '100%',
                      backgroundColor: 'var(--accent-color)',
                      transition: 'width 0.2s ease',
                    }}
                  />
                </div>
                {downloadProgress.total > 0 && (
                  <div style={{ fontSize: '10px', color: 'var(--text-muted)', marginTop: '2px', textAlign: 'right' }}>
                    {(downloadProgress.downloaded / (1024 * 1024)).toFixed(1)} MB / {(downloadProgress.total / (1024 * 1024)).toFixed(1)} MB
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>

      <div className="form-group">
        <label className="form-label">Scale Factor</label>
        <select
          className="select-input"
          value={settings.scaleFactor}
          onChange={(e) => update('scaleFactor', Number(e.target.value))}
        >
          <option value={2}>2x Super Resolution (e.g. 1080p → 4K UHD)</option>
          <option value={4}>4x Super Resolution (e.g. 720p/1080p → 4K/8K)</option>
        </select>
      </div>

      <div className="form-group">
        <label className="form-label">Temporal Flicker Suppression</label>
        <select
          className="select-input"
          value={settings.temporalMode}
          onChange={(e) => update('temporalMode', e.target.value as any)}
        >
          <option value="Disabled">Disabled (Single Frame / Fast)</option>
          <option value="Fast3Frame">Fast Temporal (3-Frame Window — Real-Time)</option>
          <option value="Quality5Frame">Quality Temporal (5-Frame Window — Maximum Stability)</option>
        </select>
        <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginTop: '4px', lineHeight: 1.4 }}>
          {settings.temporalMode === 'Disabled' && 'Processes frames independently without temporal context.'}
          {settings.temporalMode === 'Fast3Frame' && 'Bilateral 3-frame sliding window eliminates high-frequency texture flicker.'}
          {settings.temporalMode === 'Quality5Frame' && 'Full 5-frame recurrent temporal fusion for maximum film-grade stability.'}
        </div>
      </div>

      <div className="form-group">
        <label className="form-label">Motion Interpolation & Slow-Motion (RIFE / Chronos)</label>
        <select
          className="select-input"
          value={settings.interpolationMode}
          onChange={(e) => update('interpolationMode', e.target.value as any)}
        >
          <option value="Disabled">Native Frame Rate (No Retiming)</option>
          <option value="DoubleFps">2× Motion Interpolation (Smooth 60fps / HFR)</option>
          <option value="QuadrupleFps">4× Motion Interpolation (Ultra 120fps HFR)</option>
          <option value="SlowMotion2x">2× Filmic Slow-Motion (50% Half-Speed Playback)</option>
          <option value="SlowMotion4x">4× Filmic Slow-Motion (25% Quarter-Speed Playback)</option>
        </select>
        <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginTop: '4px', lineHeight: 1.4 }}>
          {settings.interpolationMode === 'Disabled' && 'Preserves native video framerate without synthesizing new intermediate frames.'}
          {settings.interpolationMode === 'DoubleFps' && 'Bi-directional optical flow motion synthesis doubles framerate for buttery-smooth 60fps playback.'}
          {settings.interpolationMode === 'QuadrupleFps' && 'Hierarchical 4x optical flow interpolation targets high-refresh 120fps displays.'}
          {settings.interpolationMode === 'SlowMotion2x' && 'Synthesizes 2x intermediate frames while keeping native playback rate for smooth half-speed slow motion.'}
          {settings.interpolationMode === 'SlowMotion4x' && 'Synthesizes 4x intermediate frames for ultra-smooth 25% quarter-speed action playback.'}
        </div>
      </div>

      <div className="section-title" style={{ marginTop: '20px' }}>Archival Broadcast & Pre-Pass</div>

      <div className="form-group">
        <label className="form-label">Deinterlacing Engine</label>
        <select
          className="select-input"
          value={settings.deinterlaceMode}
          onChange={(e) => update('deinterlaceMode', e.target.value as any)}
        >
          <option value="Disabled">Disabled (Progressive Footage)</option>
          <option value="Auto">Auto-Detect Interlaced (bwdif)</option>
          <option value="BwdifSameRate">Motion-Adaptive Same-Rate (bwdif mode 0 — 1:1 fps)</option>
          <option value="BwdifBob">Fluid Field Bob (bwdif mode 1 — 50fps/60fps 2× fluid motion)</option>
          <option value="Yadif">YADIF Adaptive (mode 0)</option>
        </select>
        <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginTop: '4px', lineHeight: 1.4 }}>
          {settings.deinterlaceMode === 'Disabled' && 'No deinterlacing applied. Assumes input is progressive.'}
          {settings.deinterlaceMode === 'Auto' && 'Inspects container stream metadata and applies bwdif only if interlaced fields are detected.'}
          {settings.deinterlaceMode === 'BwdifSameRate' && 'Bob-Weaver motion-adaptive deinterlacing preserving original container framerate and timeline duration.'}
          {settings.deinterlaceMode === 'BwdifBob' && 'Doubles framerate by converting both top & bottom fields into full frames for broadcast fluid motion.'}
          {settings.deinterlaceMode === 'Yadif' && 'Standard motion-adaptive Yadif filter for legacy broadcast interlacing.'}
        </div>
      </div>

      <div className="form-group">
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
          <label className="form-label" style={{ margin: 0 }}>Archival Denoise Pass</label>
          <span className="slider-value">{Math.round(settings.denoiseStrength * 100)}%</span>
        </div>
        <div className="slider-container">
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            className="range-slider"
            value={settings.denoiseStrength}
            onChange={(e) => update('denoiseStrength', parseFloat(e.target.value))}
          />
        </div>
        <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginTop: '4px', lineHeight: 1.4 }}>
          {settings.denoiseStrength === 0
            ? 'No denoising. Raw frames directly processed.'
            : 'Applies 3D spatio-temporal noise reduction (hqdn3d) to eliminate VHS, DVD and analog sensor grain before upscaling.'}
        </div>
      </div>


      <div className="form-group">
        <label className="form-label">VRAM Tile Dimension</label>
        <select
          className="select-input"
          value={settings.tileSize}
          onChange={(e) => update('tileSize', Number(e.target.value))}
        >
          <option value={384}>384 × 384 (Low VRAM / 4GB-6GB)</option>
          <option value={512}>512 × 512 (Recommended / 8GB VRAM)</option>
          <option value={768}>768 × 768 (High VRAM / Fast)</option>
          <option value={0}>Auto / Full Frame</option>
        </select>
      </div>

      <div className="form-group">
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
          <label className="form-label" style={{ margin: 0 }}>Natural Film Grain</label>
          <span className="slider-value">{Math.round(settings.filmGrain * 100)}%</span>
        </div>
        <div className="slider-container">
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            className="range-slider"
            value={settings.filmGrain}
            onChange={(e) => update('filmGrain', parseFloat(e.target.value))}
          />
        </div>
      </div>

      <div className="section-title" style={{ marginTop: '20px' }}>Export Encoding & Color</div>

      <div className="form-group">
        <label className="form-label">Hardware Encoder</label>
        <select
          className="select-input"
          value={settings.encoder}
          onChange={(e) => update('encoder', e.target.value)}
        >
          <option value="hevc_amf">AMD AMF H.265 / HEVC (Radeon 890M Hardware)</option>
          <option value="av1_amf">AMD AMF AV1 Next-Gen (Radeon 890M Hardware)</option>
          <option value="hevc_nvenc">NVIDIA NVENC H.265 / HEVC (RTX 4060 Hardware)</option>
          <option value="av1_nvenc">NVIDIA NVENC AV1 Next-Gen (RTX 4060 Hardware)</option>
          <option value="hevc_videotoolbox">Apple VideoToolbox H.265 (Metal Hardware)</option>
          <option value="av1_svt">SVT-AV1 Open Source (High-Efficiency CPU)</option>
          <option value="libx265">Software CPU (x265 High Efficiency)</option>
          <option value="prores_ks">Apple ProRes 422 HQ (10-Bit Master Quality)</option>
        </select>
      </div>

      <div className="form-group">
        <label className="form-label">Color Space & Bit Depth</label>
        <select
          className="select-input"
          value={settings.colorFormat}
          onChange={(e) => update('colorFormat', e.target.value as any)}
        >
          <option value="sdr_8bit">8-bit Standard SDR (Rec.709 — Universal Compatibility)</option>
          <option value="hdr10_10bit">10-bit High Dynamic Range (HDR10 / Rec.2020 PQ)</option>
          <option value="sdr_10bit">10-bit Deep Color SDR (Rec.709 — Zero Banding Gradients)</option>
          <option value="prores_hq">10-bit ProRes 422 HQ (DaVinci Resolve Master Intermediate)</option>
        </select>
        <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginTop: '4px', lineHeight: 1.4 }}>
          {settings.colorFormat === 'sdr_8bit' && 'Standard 8-bit YUV420p SDR for web and universal playback.'}
          {settings.colorFormat === 'hdr10_10bit' && 'Preserves HDR10 PQ transfer and Wide Color Gamut BT.2020 without color clipping.'}
          {settings.colorFormat === 'sdr_10bit' && '10-bit YUV420p10le precision eliminates sky/shadow banding in SDR video.'}
          {settings.colorFormat === 'prores_hq' && 'Uncompressed 10-bit 4:2:2 mastering format matching DaVinci Resolve Studio.'}
        </div>
      </div>

      <div className="form-group">
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
          <label className="form-label" style={{ margin: 0 }}>Constant Rate Factor (CRF)</label>
          <span className="slider-value">{settings.crf}</span>
        </div>
        <div className="slider-container">
          <input
            type="range"
            min="12"
            max="28"
            step="1"
            className="range-slider"
            value={settings.crf}
            onChange={(e) => update('crf', parseInt(e.target.value))}
          />
        </div>
      </div>
    </div>
  );
};
