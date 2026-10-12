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
  // Phase 19: Pro Audio Suite & Loudness Normalization
  audioCodec: 'Copy' | 'Aac' | 'Opus' | 'Flac' | 'PcmS16le' | 'PcmS24le' | 'Mute';
  audioBitrateKbps: number;
  loudnessNormalization: boolean;
  targetLufs: number;
  // Phase 20: Target Resolution & Crop / Aspect Ratio
  targetResolution: 'NativeScale' | 'Preset720p' | 'Preset1080p' | 'Preset1440p' | 'Preset4KUHD' | 'Preset8KUHD' | 'SocialVertical9_16' | 'SocialSquare1_1' | 'CinemaWidescreen' | 'Custom';
  customWidth?: number;
  customHeight?: number;
  fitMode: 'Letterbox' | 'CenterCrop' | 'Stretch';
  matteColor: string;
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
          targetResolution: 'NativeScale',
          fitMode: 'Letterbox',
          audioCodec: 'Aac',
          audioBitrateKbps: 192,
          loudnessNormalization: true,
          targetLufs: -14.0,
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
          targetResolution: 'Preset4KUHD',
          fitMode: 'Letterbox',
          audioCodec: 'Aac',
          audioBitrateKbps: 320,
          loudnessNormalization: true,
          targetLufs: -14.0,
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
          targetResolution: 'NativeScale',
          fitMode: 'Letterbox',
          audioCodec: 'Copy',
          audioBitrateKbps: 192,
          loudnessNormalization: false,
          targetLufs: -14.0,
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
          targetResolution: 'Preset1080p',
          fitMode: 'Letterbox',
          audioCodec: 'PcmS24le',
          audioBitrateKbps: 0,
          loudnessNormalization: false,
          targetLufs: -14.0,
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

      <div className="section-title" style={{ marginTop: '20px' }}>Target Resolution & Framing</div>

      <div className="form-group">
        <label className="form-label">Target Resolution</label>
        <select
          className="select-input"
          value={settings.targetResolution}
          onChange={(e) => update('targetResolution', e.target.value as any)}
        >
          <option value="NativeScale">Original (Neural Scale x{settings.scaleFactor})</option>
          <option value="Preset1080p">1080p Full HD (1920x1080)</option>
          <option value="Preset1440p">1440p 2K QHD (2560x1440)</option>
          <option value="Preset4KUHD">4K UHD Master (3840x2160)</option>
          <option value="Preset8KUHD">8K UHD Master (7680x4320)</option>
          <option value="Preset720p">720p HD (1280x720)</option>
          <option value="SocialVertical9_16">9:16 Vertical (1080x1920 — TikTok / Reels / Shorts)</option>
          <option value="SocialSquare1_1">1:1 Square (1080x1080 — Instagram Feed)</option>
          <option value="CinemaWidescreen">2.39:1 Cinema Scope (3840x1606)</option>
          <option value="Custom">Custom Dimensions...</option>
        </select>
      </div>

      {settings.targetResolution === 'Custom' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px', marginBottom: '12px' }}>
          <div className="form-group" style={{ margin: 0 }}>
            <label className="form-label">Width (px)</label>
            <input
              type="number"
              className="select-input"
              value={settings.customWidth || 1920}
              onChange={(e) => update('customWidth', parseInt(e.target.value) || 1920)}
            />
          </div>
          <div className="form-group" style={{ margin: 0 }}>
            <label className="form-label">Height (px)</label>
            <input
              type="number"
              className="select-input"
              value={settings.customHeight || 1080}
              onChange={(e) => update('customHeight', parseInt(e.target.value) || 1080)}
            />
          </div>
        </div>
      )}

      {settings.targetResolution !== 'NativeScale' && (
        <>
          <div className="form-group">
            <label className="form-label">Aspect Ratio Fitting Mode</label>
            <select
              className="select-input"
              value={settings.fitMode}
              onChange={(e) => update('fitMode', e.target.value as any)}
            >
              <option value="Letterbox">Letterbox / Matte (Preserve Full Picture with Borders)</option>
              <option value="CenterCrop">Center Crop Fill (Fill Canvas, Crop Excess Edges)</option>
              <option value="Stretch">Stretch to Canvas (Disregard Aspect Ratio)</option>
            </select>
          </div>

          {settings.fitMode === 'Letterbox' && (
            <div className="form-group">
              <label className="form-label">Matte Bar Color</label>
              <select
                className="select-input"
                value={settings.matteColor}
                onChange={(e) => update('matteColor', e.target.value)}
              >
                <option value="black">Deep Black (Pure #000000)</option>
                <option value="#111111">Studio Charcoal (#111111)</option>
                <option value="white">Pristine White (#FFFFFF)</option>
              </select>
            </div>
          )}
        </>
      )}

      <div className="section-title" style={{ marginTop: '20px' }}>Pro Audio Suite & Mastering</div>

      <div className="form-group">
        <label className="form-label">Audio Codec</label>
        <select
          className="select-input"
          value={settings.audioCodec}
          onChange={(e) => update('audioCodec', e.target.value as any)}
        >
          <option value="Copy">Passthrough Copy (Original Pristine Stream)</option>
          <option value="Aac">AAC High-Efficiency (Broadcast & Web Master)</option>
          <option value="Opus">Opus Low-Latency (Next-Gen Audiophile)</option>
          <option value="Flac">FLAC Lossless Audio (Bit-Perfect Archive)</option>
          <option value="PcmS24le">PCM 24-Bit Linear (DaVinci Resolve Studio Standard)</option>
          <option value="PcmS16le">PCM 16-Bit Linear (CD Quality Master)</option>
          <option value="Mute">Mute Audio (Mute / Strip All Audio Tracks)</option>
        </select>
      </div>

      {(settings.audioCodec === 'Aac' || settings.audioCodec === 'Opus') && (
        <div className="form-group">
          <label className="form-label">Audio Bitrate</label>
          <select
            className="select-input"
            value={settings.audioBitrateKbps}
            onChange={(e) => update('audioBitrateKbps', parseInt(e.target.value))}
          >
            <option value="128">128 kbps (Efficient Voice & Streaming)</option>
            <option value="192">192 kbps (Standard High Fidelity)</option>
            <option value="256">256 kbps (Premium Audio Grade)</option>
            <option value="320">320 kbps (Maximum Studio Bitrate)</option>
          </select>
        </div>
      )}

      {settings.audioCodec !== 'Copy' && settings.audioCodec !== 'Mute' && (
        <div className="form-group" style={{ background: 'rgba(255, 255, 255, 0.03)', padding: '10px', borderRadius: '6px' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
            <label className="form-label" style={{ margin: 0, fontWeight: 600 }}>EBU R128 Loudness Normalization</label>
            <input
              type="checkbox"
              checked={settings.loudnessNormalization}
              onChange={(e) => update('loudnessNormalization', e.target.checked)}
              style={{ cursor: 'pointer', transform: 'scale(1.2)' }}
            />
          </div>
          <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginBottom: '8px', lineHeight: 1.4 }}>
            Applies true-peak multi-pass loudness normalization preventing clipping and ensuring compliant streaming loudness.
          </div>

          {settings.loudnessNormalization && (
            <>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Target Loudness</span>
                <span className="slider-value" style={{ color: '#38bdf8', fontWeight: 600 }}>{settings.targetLufs.toFixed(1)} LUFS</span>
              </div>
              <div className="slider-container">
                <input
                  type="range"
                  min="-24"
                  max="-9"
                  step="0.5"
                  className="range-slider"
                  value={settings.targetLufs}
                  onChange={(e) => update('targetLufs', parseFloat(e.target.value))}
                />
              </div>
              <div style={{ display: 'flex', gap: '4px', marginTop: '6px', flexWrap: 'wrap' }}>
                <button
                  type="button"
                  className="btn-secondary"
                  style={{ fontSize: '10px', padding: '2px 6px', background: settings.targetLufs === -14.0 ? 'var(--accent-color)' : undefined }}
                  onClick={() => update('targetLufs', -14.0)}
                >
                  -14.0 LUFS (YouTube/Spotify)
                </button>
                <button
                  type="button"
                  className="btn-secondary"
                  style={{ fontSize: '10px', padding: '2px 6px', background: settings.targetLufs === -16.0 ? 'var(--accent-color)' : undefined }}
                  onClick={() => update('targetLufs', -16.0)}
                >
                  -16.0 LUFS (Apple Music)
                </button>
                <button
                  type="button"
                  className="btn-secondary"
                  style={{ fontSize: '10px', padding: '2px 6px', background: settings.targetLufs === -23.0 ? 'var(--accent-color)' : undefined }}
                  onClick={() => update('targetLufs', -23.0)}
                >
                  -23.0 LUFS (EBU R128)
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
};
