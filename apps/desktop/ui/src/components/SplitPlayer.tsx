import React, { useState, useRef, useCallback } from 'react';

interface SplitPlayerProps {
  originalSrc?: string;
  upscaledSrc?: string;
  originalLabel?: string;
  upscaledLabel?: string;
  isLoading?: boolean;
  loadingMessage?: string;
}

export const SplitPlayer: React.FC<SplitPlayerProps> = ({
  originalSrc,
  upscaledSrc,
  originalLabel = 'Input Source Frame',
  upscaledLabel = 'Super Scale Output',
  isLoading = false,
  loadingMessage = 'Processing DirectML Super-Resolution...',
}) => {
  const [sliderPos, setSliderPos] = useState<number>(50); // percentage 0..100
  const [zoom, setZoom] = useState<number>(1.0); // 0.5 to 8.0
  const [pan, setPan] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  const [isDraggingSlider, setIsDraggingSlider] = useState<boolean>(false);
  const [isPanning, setIsPanning] = useState<boolean>(false);
  const [dragStart, setDragStart] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [panStart, setPanStart] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  const containerRef = useRef<HTMLDivElement>(null);

  // Wheel zoom centered on mouse cursor
  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    e.preventDefault();
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;
    const centerX = rect.width / 2;
    const centerY = rect.height / 2;

    const factor = e.deltaY < 0 ? 1.2 : 0.833;
    const newZoom = Math.min(Math.max(zoom * factor, 0.5), 8.0);

    if (newZoom <= 1.0) {
      setZoom(1.0);
      setPan({ x: 0, y: 0 });
    } else {
      const pX = (mouseX - centerX - pan.x) / zoom;
      const pY = (mouseY - centerY - pan.y) / zoom;
      const newPanX = mouseX - centerX - pX * newZoom;
      const newPanY = mouseY - centerY - pY * newZoom;
      setZoom(newZoom);
      setPan({ x: newPanX, y: newPanY });
    }
  };

  // Slider dragging
  const handleSliderPointerDown = (e: React.PointerEvent) => {
    e.stopPropagation();
    setIsDraggingSlider(true);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };

  // Canvas pan dragging
  const handleCanvasPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button === 0 && (zoom > 1.0 || e.altKey)) {
      setIsPanning(true);
      setDragStart({ x: e.clientX, y: e.clientY });
      setPanStart({ ...pan });
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    } else if (e.button === 1) {
      // Middle click always pans
      setIsPanning(true);
      setDragStart({ x: e.clientX, y: e.clientY });
      setPanStart({ ...pan });
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    }
  };

  const handlePointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (isDraggingSlider && containerRef.current) {
        const rect = containerRef.current.getBoundingClientRect();
        const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
        const percent = (x / rect.width) * 100;
        setSliderPos(percent);
      } else if (isPanning) {
        const dx = e.clientX - dragStart.x;
        const dy = e.clientY - dragStart.y;
        setPan({
          x: panStart.x + dx,
          y: panStart.y + dy,
        });
      }
    },
    [isDraggingSlider, isPanning, dragStart, panStart]
  );

  const handlePointerUp = (e: React.PointerEvent) => {
    if (isDraggingSlider) {
      setIsDraggingSlider(false);
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
    if (isPanning) {
      setIsPanning(false);
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
  };

  // Double click toggles between Fit and 200% zoom
  const handleDoubleClick = (e: React.WheelEvent<HTMLDivElement> | React.MouseEvent<HTMLDivElement>) => {
    if (zoom > 1.0) {
      setZoom(1.0);
      setPan({ x: 0, y: 0 });
    } else if (containerRef.current) {
      const rect = containerRef.current.getBoundingClientRect();
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;
      const centerX = rect.width / 2;
      const centerY = rect.height / 2;
      const targetZoom = 2.0;

      const pX = (mouseX - centerX) / 1.0;
      const pY = (mouseY - centerY) / 1.0;
      setZoom(targetZoom);
      setPan({
        x: mouseX - centerX - pX * targetZoom,
        y: mouseY - centerY - pY * targetZoom,
      });
    }
  };

  const resetZoom = () => {
    setZoom(1.0);
    setPan({ x: 0, y: 0 });
  };

  const setFixedZoom = (targetZoom: number) => {
    setZoom(targetZoom);
    if (targetZoom <= 1.0) {
      setPan({ x: 0, y: 0 });
    } else {
      setPan((prev) => ({
        x: (prev.x / zoom) * targetZoom,
        y: (prev.y / zoom) * targetZoom,
      }));
    }
  };

  const hasImages = Boolean(originalSrc && upscaledSrc);

  return (
    <div
      ref={containerRef}
      className="split-viewer"
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onWheel={handleWheel}
      style={{ userSelect: 'none', position: 'relative' }}
    >
      {isLoading && (
        <div
          style={{
            position: 'absolute',
            inset: 0,
            zIndex: 30,
            backgroundColor: 'rgba(15, 17, 23, 0.85)',
            backdropFilter: 'blur(6px)',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '16px',
          }}
        >
          <div className="spinner" />
          <span style={{ fontSize: '14px', fontWeight: 500, color: '#f3f4f6' }}>
            {loadingMessage}
          </span>
        </div>
      )}

      {hasImages ? (
        <>
          {/* Zoom and Pan Viewport Canvas */}
          <div
            style={{
              position: 'absolute',
              inset: 0,
              overflow: 'hidden',
              cursor: isDraggingSlider
                ? 'ew-resize'
                : isPanning
                ? 'grabbing'
                : zoom > 1.0
                ? 'grab'
                : 'default',
            }}
            onPointerDown={handleCanvasPointerDown}
            onDoubleClick={handleDoubleClick}
          >
            {/* Background layer: Original input */}
            <div
              className="split-layer layer-original"
              style={{
                backgroundColor: '#0b0c10',
                overflow: 'hidden',
              }}
            >
              <div
                style={{
                  width: '100%',
                  height: '100%',
                  transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
                  transformOrigin: 'center center',
                  transition: isPanning ? 'none' : 'transform 0.05s ease-out',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <img
                  src={originalSrc}
                  alt="Original Input"
                  style={{
                    width: '100%',
                    height: '100%',
                    objectFit: 'contain',
                    pointerEvents: 'none',
                  }}
                />
              </div>
            </div>

            {/* Foreground layer: Upscaled output (clipped by slider position in viewport space) */}
            <div
              className="split-layer layer-upscaled"
              style={{
                clipPath: `inset(0 0 0 ${sliderPos}%)`,
                backgroundColor: 'transparent',
                overflow: 'hidden',
              }}
            >
              <div
                style={{
                  width: '100%',
                  height: '100%',
                  transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
                  transformOrigin: 'center center',
                  transition: isPanning ? 'none' : 'transform 0.05s ease-out',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <img
                  src={upscaledSrc}
                  alt="Neural Upscaled"
                  style={{
                    width: '100%',
                    height: '100%',
                    objectFit: 'contain',
                    pointerEvents: 'none',
                  }}
                />
              </div>
            </div>

            {/* Vertical comparison divider bar */}
            <div
              className="split-divider"
              style={{ left: `${sliderPos}%` }}
              onPointerDown={handleSliderPointerDown}
            >
              <div className="split-handle">
                <span>⇄</span>
              </div>
            </div>
          </div>

          {/* Floating Zoom & Pan Controls Toolbar (Top-Right) */}
          <div className="zoom-toolbar">
            <button
              className={`zoom-btn ${zoom === 1.0 ? 'active' : ''}`}
              onClick={resetZoom}
              title="Fit to Window (Reset Zoom)"
            >
              Fit
            </button>
            <button
              className={`zoom-btn ${zoom === 1.0 && pan.x === 0 ? '' : zoom === 1.0 ? 'active' : ''}`}
              onClick={() => setFixedZoom(1.0)}
              title="100% 1:1 Pixel Mapping"
            >
              100%
            </button>
            <button
              className={`zoom-btn ${zoom === 2.0 ? 'active' : ''}`}
              onClick={() => setFixedZoom(2.0)}
              title="200% Pixel Inspection"
            >
              200%
            </button>
            <button
              className={`zoom-btn ${zoom === 4.0 ? 'active' : ''}`}
              onClick={() => setFixedZoom(4.0)}
              title="400% Deep Pixel Inspection"
            >
              400%
            </button>
            <div style={{ width: '1px', height: '14px', backgroundColor: 'var(--border-color)', margin: '0 4px' }} />
            <button
              className="zoom-btn"
              onClick={() => setFixedZoom(Math.max(zoom * 0.8, 0.5))}
              title="Zoom Out"
            >
              －
            </button>
            <span className="zoom-badge">{(zoom * 100).toFixed(0)}%</span>
            <button
              className="zoom-btn"
              onClick={() => setFixedZoom(Math.min(zoom * 1.25, 8.0))}
              title="Zoom In"
            >
              ＋
            </button>
          </div>

          {/* Minimap Crop Navigator (Bottom-Right, shown when zoomed in) */}
          {zoom > 1.0 && originalSrc && (
            <div className="minimap-container">
              <img src={originalSrc} alt="Minimap Thumbnail" className="minimap-thumb" />
              <div
                className="minimap-crop-rect"
                style={{
                  width: `${Math.max(12, 100 / zoom)}%`,
                  height: `${Math.max(12, 100 / zoom)}%`,
                  left: `${Math.max(0, Math.min(100 - 100 / zoom, 50 - 50 / zoom - (pan.x / (containerRef.current?.clientWidth || 800)) * (100 / zoom)))}%`,
                  top: `${Math.max(0, Math.min(100 - 100 / zoom, 50 - 50 / zoom - (pan.y / (containerRef.current?.clientHeight || 600)) * (100 / zoom)))}%`,
                }}
              />
            </div>
          )}

          {/* Corner Tags */}
          <div className="layer-tag tag-original">{originalLabel}</div>
          <div className="layer-tag tag-upscaled">{upscaledLabel}</div>
        </>
      ) : (
        <div
          style={{
            width: '100%',
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '14px',
            color: 'var(--text-muted)',
            textAlign: 'center',
            padding: '24px',
          }}
        >
          <div
            style={{
              width: '64px',
              height: '64px',
              borderRadius: '50%',
              backgroundColor: 'var(--bg-tertiary)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: '28px',
              color: 'var(--accent-color)',
            }}
          >
            🎬
          </div>
          <div>
            <div style={{ fontSize: '16px', fontWeight: 600, color: '#f3f4f6', marginBottom: '6px' }}>
              No Frame Preview Loaded
            </div>
            <div style={{ fontSize: '13px', maxWidth: '420px', lineHeight: 1.5 }}>
              Select an input video and click <strong>"Extract Frame Preview"</strong> to inspect neural
              restoration detail, tile blending, and 100%–400% zoom before full export.
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default SplitPlayer;
