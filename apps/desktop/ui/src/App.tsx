import React, { useState } from 'react';
import { SplitPlayer } from './components/SplitPlayer';
import { DeviceSelector } from './components/DeviceSelector';
import { ModelSettings, UpscaleSettings } from './components/ModelSettings';

export const App: React.FC = () => {
  const [selectedDevice, setSelectedDevice] = useState<string>('nvidia-rtx-4060');
  const [isProcessing, setIsProcessing] = useState<boolean>(false);
  const [progress, setProgress] = useState<number>(0);

  const [settings, setSettings] = useState<UpscaleSettings>({
    modelId: 'realesrgan-x4plus',
    scaleFactor: 4,
    tileSize: 512,
    filmGrain: 0.15,
    encoder: 'hevc_nvenc',
    crf: 18,
  });

  const handleStartRender = () => {
    setIsProcessing(true);
    setProgress(0);

    const interval = setInterval(() => {
      setProgress((prev) => {
        if (prev >= 100) {
          clearInterval(interval);
          setIsProcessing(false);
          return 100;
        }
        return prev + 5;
      });
    }, 200);
  };

  return (
    <>
      <header className="app-header">
        <div className="brand-title">
          <span>⚡ VIDEO UPSCALER STUDIO</span>
          <span className="badge">PRO</span>
        </div>
        <div style={{ fontSize: '13px', color: 'var(--text-muted)' }}>
          Active Engine: <strong style={{ color: '#fff' }}>DirectML / TensorRT / CoreML</strong>
        </div>
      </header>

      <div className="app-workspace">
        <main className="viewport-panel">
          <div className="player-container">
            <SplitPlayer
              originalLabel="Input Preview (1080p)"
              upscaledLabel="4K Super Scale Preview"
            />
          </div>

          {/* Timeline / Progress bar */}
          <div
            style={{
              height: '48px',
              backgroundColor: 'var(--bg-secondary)',
              borderTop: '1px solid var(--border-color)',
              display: 'flex',
              alignItems: 'center',
              padding: '0 20px',
              gap: '16px',
            }}
          >
            <span style={{ fontSize: '12px', color: 'var(--text-muted)', minWidth: '80px' }}>
              {isProcessing ? `Processing: ${progress}%` : 'Status: Ready'}
            </span>
            <div
              style={{
                flex: 1,
                height: '6px',
                backgroundColor: 'var(--bg-tertiary)',
                borderRadius: '3px',
                overflow: 'hidden',
              }}
            >
              <div
                style={{
                  width: `${progress}%`,
                  height: '100%',
                  backgroundColor: 'var(--accent-color)',
                  transition: 'width 0.2s ease',
                }}
              />
            </div>
            {isProcessing && (
              <span style={{ fontSize: '12px', color: '#60a5fa' }}>
                Est. Speed: 24.5 fps
              </span>
            )}
          </div>
        </main>

        <aside className="controls-sidebar">
          <div className="sidebar-section">
            <div className="section-title">Hardware Acceleration</div>
            <DeviceSelector
              selectedDeviceId={selectedDevice}
              onSelectDevice={setSelectedDevice}
            />
          </div>

          <ModelSettings settings={settings} onChange={setSettings} />

          <div className="render-action-panel">
            <button
              className="btn-primary"
              disabled={isProcessing}
              onClick={handleStartRender}
            >
              {isProcessing ? 'Processing Queue...' : 'Export 4K Upscale'}
            </button>
          </div>
        </aside>
      </div>
    </>
  );
};
export default App;
