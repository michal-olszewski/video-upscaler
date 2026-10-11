import React from 'react';

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
}

export interface UpscaleSettings {
  modelId: string;
  scaleFactor: number;
  tileSize: number;
  filmGrain: number;
  encoder: string;
  crf: number;
}

interface ModelSettingsProps {
  models: ModelDescriptor[];
  settings: UpscaleSettings;
  onChange: (updated: UpscaleSettings) => void;
}

export const ModelSettings: React.FC<ModelSettingsProps> = ({ models, settings, onChange }) => {
  const update = <K extends keyof UpscaleSettings>(key: K, value: UpscaleSettings[K]) => {
    onChange({ ...settings, [key]: value });
  };

  const selectedModel = models.find((m) => m.id === settings.modelId);

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

  return (
    <div className="sidebar-section">
      <div className="section-title">Enhancement Model</div>

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
          <div style={{ fontSize: '11px', color: 'var(--text-muted)', marginTop: '4px', lineHeight: 1.4 }}>
            {selectedModel.description}
          </div>
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

      <div className="section-title" style={{ marginTop: '20px' }}>Export Encoding</div>

      <div className="form-group">
        <label className="form-label">Hardware Encoder</label>
        <select
          className="select-input"
          value={settings.encoder}
          onChange={(e) => update('encoder', e.target.value)}
        >
          <option value="hevc_amf">AMD AMF H.265 / HEVC (Radeon 890M Hardware)</option>
          <option value="hevc_nvenc">NVIDIA NVENC H.265 / HEVC (RTX 4060 Hardware)</option>
          <option value="hevc_videotoolbox">Apple VideoToolbox H.265 (Metal Hardware)</option>
          <option value="libx265">Software CPU (x265 High Efficiency)</option>
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
