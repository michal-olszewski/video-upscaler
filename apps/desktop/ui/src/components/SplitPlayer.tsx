import React, { useState, useRef, useCallback } from 'react';

interface SplitPlayerProps {
  originalSrc?: string;
  upscaledSrc?: string;
  originalLabel?: string;
  upscaledLabel?: string;
}

export const SplitPlayer: React.FC<SplitPlayerProps> = ({
  originalLabel = 'Original (1080p)',
  upscaledLabel = 'AI Super Scale (4K UHD)',
}) => {
  const [sliderPos, setSliderPos] = useState<number>(50); // percentage 0..100
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const handlePointerDown = () => {
    setIsDragging(true);
  };

  const handlePointerUp = () => {
    setIsDragging(false);
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

  return (
    <div
      ref={containerRef}
      className="split-viewer"
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerLeave={handlePointerUp}
    >
      {/* Background layer: Original input */}
      <div
        className="split-layer layer-original"
        style={{
          background: 'linear-gradient(135deg, #1e293b 0%, #0f172a 100%)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          filter: 'blur(2px)', // Visual representation of lower resolution
        }}
      >
        <span style={{ color: '#64748b', fontSize: '20px', fontWeight: 600 }}>
          Input Source Frame (1080p SDR)
        </span>
      </div>

      {/* Foreground layer: Upscaled output (clipped by slider position) */}
      <div
        className="split-layer layer-upscaled"
        style={{
          clipPath: `inset(0 0 0 ${sliderPos}%)`,
          background: 'linear-gradient(135deg, #0284c7 0%, #0369a1 100%)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <span style={{ color: '#ffffff', fontSize: '20px', fontWeight: 600 }}>
          Upscaled Output (4K Crisp Neural Detail)
        </span>
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
    </div>
  );
};
