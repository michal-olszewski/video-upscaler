import React from 'react';

export interface DeviceInfo {
  id: number;
  name: string;
  vendor: string | Record<string, string>;
  dedicated_vram_mb: number;
  is_integrated: boolean;
  recommended_backend: any;
  supported_backends: any[];
}

interface DeviceSelectorProps {
  devices: DeviceInfo[];
  selectedDeviceId: number;
  onSelectDevice: (id: number) => void;
  isLoading?: boolean;
}

export const DeviceSelector: React.FC<DeviceSelectorProps> = ({
  devices,
  selectedDeviceId,
  onSelectDevice,
  isLoading = false,
}) => {
  return (
    <div className="form-group">
      <label className="form-label">
        AI Compute Hardware {isLoading && ' (Probing...)'}
      </label>
      <select
        className="select-input"
        value={selectedDeviceId}
        onChange={(e) => onSelectDevice(Number(e.target.value))}
        disabled={isLoading || devices.length === 0}
      >
        {devices.map((dev) => {
          const vramStr =
            dev.dedicated_vram_mb > 0
              ? `${(dev.dedicated_vram_mb / 1024).toFixed(1)} GB VRAM`
              : dev.is_integrated
              ? 'Shared / UMA'
              : 'Host RAM';
          return (
            <option key={dev.id} value={dev.id}>
              {dev.name} ({vramStr})
            </option>
          );
        })}
      </select>
    </div>
  );
};
