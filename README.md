# Video Upscaler Studio

A high-performance, cross-platform video upscaling and enhancement application designed to rival **Topaz Video AI** and **DaVinci Resolve Studio's Super Scale**.

Built with a modular **Rust core engine**, an **ONNX Runtime multi-backend inference layer**, and a modern **interactive desktop GUI**.

---

## 🚀 Hardware Acceleration Architecture

The application is engineered to achieve maximum throughput across heterogeneous hardware without vendor lock-in:

| Platform | Target Hardware | Inference Execution Provider | Hardware Codec |
| :--- | :--- | :--- | :--- |
| **Windows** | **NVIDIA GPUs** (RTX 4060, 3080, etc.) | `TensorRT` / `CUDA` (FP16 Tensor Cores) | `h264_nvenc` / `hevc_nvenc` / `av1_nvenc` |
| **Windows** | **AMD Radeon GPUs** (Radeon 890M, 7900 XTX) | `DirectML` (DirectX 12 Compute) | `h264_amf` / `hevc_amf` / `av1_amf` |
| **macOS** | **Apple Silicon** (M1/M2/M3/M4 Max/Pro/Ultra) | `CoreML` (Apple Neural Engine + Metal) | `h264_videotoolbox` / `hevc_videotoolbox` |
| **Universal** | Multi-Core CPU (x86_64 / ARM64) | ONNX Runtime CPU (AVX-512 / NEON) | `libx264` / `libx265` / `libsvtav1` / `prores` |

---

## 📦 Project Structure

```text
video-upscaler/
├── Cargo.toml                          # Workspace root
├── crates/
│   ├── inference-runtime/              # Multi-EP abstraction (CUDA, TensorRT, DirectML, CoreML)
│   ├── image-processing/               # Overlap tiling, Hann window blending, film grain generator
│   ├── video-io/                       # Hardware decode/encode probe (NVENC, AMF, VideoToolbox)
│   ├── models/                         # Model registry (Real-ESRGAN, BasicVSR++, Compact SPAN)
│   └── engine-core/                    # Producer-Consumer ring buffer pipeline & job scheduler
├── apps/
│   └── desktop/                        # Native desktop application
│       ├── src/                        # Native backend runner
│       └── ui/                         # React + TypeScript frontend
│           ├── src/components/SplitPlayer.tsx     # Topaz/DaVinci style split slider comparison
│           ├── src/components/DeviceSelector.tsx  # Dynamic GPU switcher
│           └── src/components/ModelSettings.tsx   # Tiling, grain, and codec controls
├── models/
│   └── cache/                          # Local ONNX model weights
└── scripts/
    ├── setup_dev_env.ps1               # Automated toolchain installer for Windows
    └── download_test_model.ps1         # ONNX model downloader
```

---

## 🛠️ Key Technical Features

1. **VRAM-Safe Tiling & Apodization**:
   - Deep neural networks processing 4K/8K video frames easily exceed consumer VRAM limits.
   - The engine partitions frames into overlapping tiles (e.g. 512×512 with 32px padding) and reconstructs them using a **2D Hann window weighting function**, eliminating tile boundary seams.

2. **Apple Silicon Unified Memory Optimization**:
   - Detects Unified Memory Architecture (UMA) on macOS to eliminate PCIe bus latency and process full frames when sufficient memory is present.

3. **Natural Film Grain Synthesizer**:
   - AI super-resolution models often produce a waxy or "plastic" texture on human skin and flat surfaces. The post-processing pipeline includes an adjustable procedural grain generator to restore cinematic texture.

4. **Interactive Dual-Layer Comparison Viewport**:
   - Real-time side-by-side / split curtain comparison player allows scrubbing and comparing upscaled output against original footage.

---

## 💻 Getting Started

### Prerequisites
* Windows 10/11 or macOS 12+
* [Rust](https://rustup.rs/) (1.75+)
* [Node.js](https://nodejs.org/) (v18+)
* [FFmpeg](https://ffmpeg.org/)

### Quick Setup on Windows
Run the automated environment setup script from PowerShell:
```powershell
.\scripts\setup_dev_env.ps1
```

### Download Test Models
```powershell
.\scripts\download_test_model.ps1
```

### Running the Project
1. **Build the Rust workspace**:
   ```bash
   cargo build --workspace
   ```
2. **Start the Desktop UI**:
   ```bash
   cd apps/desktop/ui
   npm install
   npm run dev
   ```
