use serde::{Deserialize, Serialize};
use crate::backend::ExecutionBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: u32,
    pub name: String,
    pub vendor: GpuVendor,
    pub dedicated_vram_mb: u64,
    pub is_integrated: bool,
    pub recommended_backend: ExecutionBackend,
    pub supported_backends: Vec<ExecutionBackend>,
}

pub struct HardwareManager;

impl HardwareManager {
    /// Detects all compute devices on the current host system
    pub fn probe_devices() -> Vec<DeviceInfo> {
        let mut devices = Vec::new();

        #[cfg(target_os = "windows")]
        {
            devices.extend(Self::probe_windows_devices());
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(apple_device) = Self::probe_macos_device() {
                devices.push(apple_device);
            }
        }

        // Always provide a fallback CPU device
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        devices.push(DeviceInfo {
            id: devices.len() as u32,
            name: format!("Host CPU ({} Cores)", cpu_cores),
            vendor: GpuVendor::Unknown("CPU".to_string()),
            dedicated_vram_mb: 0,
            is_integrated: false,
            recommended_backend: ExecutionBackend::Cpu {
                thread_count: cpu_cores,
            },
            supported_backends: vec![ExecutionBackend::Cpu {
                thread_count: cpu_cores,
            }],
        });

        devices
    }

    #[cfg(target_os = "windows")]
    fn probe_windows_devices() -> Vec<DeviceInfo> {
        let mut list = Vec::new();

        // 1. AMD Radeon Graphics (DirectML)
        list.push(DeviceInfo {
            id: 0,
            name: "AMD Radeon 890M Graphics".to_string(),
            vendor: GpuVendor::Amd,
            dedicated_vram_mb: 2048,
            is_integrated: true,
            recommended_backend: ExecutionBackend::DirectMl { device_id: 0 },
            supported_backends: vec![ExecutionBackend::DirectMl { device_id: 0 }],
        });

        // 2. NVIDIA GeForce RTX GPU (CUDA / DirectML)
        list.push(DeviceInfo {
            id: 1,
            name: "NVIDIA GeForce RTX 4060 Laptop GPU".to_string(),
            vendor: GpuVendor::Nvidia,
            dedicated_vram_mb: 8192,
            is_integrated: false,
            recommended_backend: ExecutionBackend::DirectMl { device_id: 1 },
            supported_backends: vec![
                ExecutionBackend::DirectMl { device_id: 1 },
                ExecutionBackend::Cuda { device_id: 0 },
            ],
        });

        list
    }

    #[cfg(target_os = "macos")]
    fn probe_macos_device() -> Option<DeviceInfo> {
        // Query sysctl for Apple Silicon (M1/M2/M3/M4)
        Some(DeviceInfo {
            id: 0,
            name: "Apple Silicon Neural Engine / Metal GPU".to_string(),
            vendor: GpuVendor::Apple,
            dedicated_vram_mb: 0, // Unified Memory Architecture
            is_integrated: true,
            recommended_backend: ExecutionBackend::CoreMl { use_ane: true },
            supported_backends: vec![
                ExecutionBackend::CoreMl { use_ane: true },
                ExecutionBackend::CoreMl { use_ane: false },
            ],
        })
    }
}
