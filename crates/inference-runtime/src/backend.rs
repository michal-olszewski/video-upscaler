use serde::{Deserialize, Serialize};

/// Supported inference execution backends
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionBackend {
    /// NVIDIA CUDA execution provider
    Cuda { device_id: u32 },
    /// NVIDIA TensorRT execution provider (FP16/INT8 optimized engine)
    TensorRt { device_id: u32, fp16: bool },
    /// Microsoft DirectML (Windows - AMD Radeon, Intel Arc, NVIDIA)
    DirectMl { device_id: u32 },
    /// Apple CoreML (macOS Apple Silicon Neural Engine / Metal)
    CoreMl { use_ane: bool },
    /// High-performance CPU fallback
    Cpu { thread_count: usize },
}

impl Default for ExecutionBackend {
    fn default() -> Self {
        Self::Cpu {
            thread_count: num_cpus::get().max(1),
        }
    }
}

impl std::fmt::Display for ExecutionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cuda { device_id } => write!(f, "NVIDIA CUDA (Device {})", device_id),
            Self::TensorRt { device_id, fp16 } => {
                write!(f, "NVIDIA TensorRT (Device {}, FP16: {})", device_id, fp16)
            }
            Self::DirectMl { device_id } => {
                write!(f, "DirectML / DirectX 12 (Adapter {})", device_id)
            }
            Self::CoreMl { use_ane } => {
                write!(
                    f,
                    "Apple CoreML ({})",
                    if *use_ane { "Neural Engine + GPU" } else { "Metal GPU" }
                )
            }
            Self::Cpu { thread_count } => write!(f, "CPU Fallback ({} threads)", thread_count),
        }
    }
}
mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}
