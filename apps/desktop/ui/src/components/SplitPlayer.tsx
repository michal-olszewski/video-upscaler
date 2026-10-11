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
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const handlePointerDown = (e: React.PointerEvent) => {
    setIsDragging(true);
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    setIsDragging(false);
    try {
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {
      // Ignore if not captured
    }
  };

  const handlePointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (!isDragging || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
      const percent = (x / rect.width) * 100;
      setSliderPos(percent);
    },
    [isDragging]
  );

  const hasImages = Boolean(originalSrc && upscaledSrc);

  return (
    <div
      ref={containerRef}
      className="split-viewer"
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
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
          {/* Background layer: Original input */}
          <div
            className="split-layer layer-original"
            style={{
              backgroundColor: '#0b0c10',
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

          {/* Foreground layer: Upscaled output (clipped by slider position) */}
          <div
            className="split-layer layer-upscaled"
            style={{
              clipPath: `inset(0 0 0 ${sliderPos}%)`,
              backgroundColor: 'transparent',
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

          {/* Vertical comparison divider bar */}
          <div
            className="split-divider"
            style={{ left: `${sliderPos}%` }}
            onPointerDown={handlePointerDown}
          >
            <div className="split-handle">
              <span>⇄</span>
            </div>
          </div>

          {/* Corner tags */}
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
              restoration detail and tile blending before launching full export.
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
