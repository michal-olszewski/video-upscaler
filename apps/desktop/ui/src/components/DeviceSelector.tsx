import React from 'react';

export interface DeviceOption {
  id: string;
  name: string;
  backend: string;
  vram: string;
}

interface DeviceSelectorProps {
  selectedDeviceId: string;
  onSelectDevice: (id: string) => void;
}

export const DeviceSelector: React.FC<DeviceSelectorProps> = ({
  selectedDeviceId,
  onSelectDevice,
}) => {
  const devices: DeviceOption[] = [
    {
      id: 'nvidia-rtx-4060',
      name: 'NVIDIA GeForce RTX 4060 Laptop GPU',
      backend: 'TensorRT / CUDA (Tensor Cores FP16)',
      vram: '8 GB GDDR6',
    },
    {
      id: 'amd-890m',
      name: 'AMD Radeon 890M Graphics',
      backend: 'Microsoft DirectML (DirectX 12 Compute)',
      vram: 'Shared System VRAM',
    },
    {
      id: 'apple-silicon-ane',
      name: 'Apple Silicon M-Series Neural Engine',
      backend: 'CoreML / Metal Performance Shaders',
      vram: 'Unified Memory (UMA)',
    },
    {
      id: 'cpu-fallback',
      name: 'Host Multi-Core CPU',
      backend: 'ONNX Runtime CPU / AVX-512',
      vram: 'Host RAM',
    },
  ];

  return (
    <div className="form-group">
      <label className="form-label">AI Compute Hardware</label>
      <select
        className="select-input"
        value={selectedDeviceId}
        onChange={(e) => onSelectDevice(e.target.value)}
      >
        {devices.map((dev) => (
          <option key={dev.id} value={dev.id}>
            {dev.name} — {dev.backend}
          </option>
        ))}
      </select>
    </div>
  );
};
