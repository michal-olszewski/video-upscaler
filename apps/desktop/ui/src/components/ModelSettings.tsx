import React from 'react';

export interface UpscaleSettings {
  modelId: string;
  scaleFactor: number;
  tileSize: number;
  filmGrain: number;
  encoder: string;
  crf: number;
}

interface ModelSettingsProps {
  settings: UpscaleSettings;
  onChange: (updated: UpscaleSettings) => void;
}

export const ModelSettings: React.FC<ModelSettingsProps> = ({ settings, onChange }) => {
  const update = <K extends keyof UpscaleSettings>(key: K, value: UpscaleSettings[K]) => {
    onChange({ ...settings, [key]: value });
  };

  return (
    <div className="sidebar-section">
      <div className="section-title">Enhancement Model</div>

      <div className="form-group">
        <label className="form-label">Neural Model Architecture</label>
        <select
          className="select-input"
          value={settings.modelId}
          onChange={(e) => update('modelId', e.target.value)}
        >
          <option value="realesrgan-x4plus">Real-ESRGAN x4 Plus (Universal Cinematic)</option>
          <option value="basicvsr-plusplus">BasicVSR++ (Temporal Consistent VSR)</option>
          <option value="compact-span-x2">Compact SPAN (2x Real-Time Fast)</option>
          <option value="realesrgan-anime-6b">Real-ESRGAN Anime 6B (Clean Lines)</option>
        </select>
      </div>

      <div className="form-group">
        <label className="form-label">Scale Factor</label>
        <select
          className="select-input"
          value={settings.scaleFactor}
          onChange={(e) => update('scaleFactor', Number(e.target.value))}
        >
          <option value={2}>2x Upscale (e.g. 1080p → 4K)</option>
          <option value={4}>4x Upscale (e.g. 720p/1080p → 4K/8K)</option>
        </select>
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
          <option value={768}>768 × 768 (High VRAM / 12GB-16GB)</option>
          <option value={0}>Auto / Full Frame (Apple Silicon Unified Memory)</option>
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

      <div className="section-title" style={{ marginTop: '20px' }}>Export Encoding</div>

      <div className="form-group">
        <label className="form-label">Hardware Encoder</label>
        <select
          className="select-input"
          value={settings.encoder}
          onChange={(e) => update('encoder', e.target.value)}
        >
          <option value="hevc_nvenc">NVIDIA NVENC H.265 / HEVC</option>
          <option value="hevc_amf">AMD AMF H.265 / HEVC</option>
          <option value="hevc_videotoolbox">Apple VideoToolbox H.265 / HEVC</option>
          <option value="libx265">Software CPU (x265 - Maximum Compression)</option>
          <option value="prores_ks">Apple ProRes 422 HQ (Master Quality)</option>
        </select>
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
