# ⚡ Video Upscaler Studio

[![Build & Release Desktop Studio](https://github.com/michal-olszewski/video-upscaler/actions/workflows/build_and_release.yml/badge.svg)](https://github.com/michal-olszewski/video-upscaler/actions/workflows/build_and_release.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-informational.svg)](https://github.com/michal-olszewski/video-upscaler)
[![Engine](https://img.shields.io/badge/Engine-DirectML%20%7C%20CoreML%20%7C%20CUDA-orange.svg)](https://github.com/michal-olszewski/video-upscaler)

A high-performance, cross-platform video super-resolution and neural restoration studio engineered to match the quality and feature parity of **Topaz Video AI** and **DaVinci Resolve Studio Super Scale**.

Powered by a decoupled **Rust engine core**, native hardware **DirectML (DirectX 12)** and **Apple CoreML (Neural Engine)** inference providers, an asynchronous streaming FFmpeg I/O pipeline, and a modern **interactive desktop GUI** (`tao` + `wry`).

---

## 🎬 Key Highlights

* **Standalone Desktop Application**: Runs as a self-contained native desktop app (`Video Upscaler.exe` on Windows, `Video Upscaler.app` on macOS) without requiring an external browser window or command prompt.
* **Dual-Mode Temporal Flicker Suppression**: Eliminates high-frequency inter-frame texture shimmering using edge-aware bilateral temporal fusion ($T=3$ Fast vs. $T=5$ Quality) with strict 1:1 frame count invariance.
* **Bounded VRAM Ring-Buffer Architecture**: Zero-copy 3-stage pipeline isolating Decode (Producer), Neural Inference (Worker), and Hardware Encode (Consumer) via `tokio::sync::mpsc::channel(8)` backpressure. Safely upscale 4K/8K footage on GPUs with 4–8 GB VRAM.
* **2D Hann-Window Apodization & Tiling**: Overlapped sub-tile spatial partitioning with smooth raised-cosine weight blending completely eliminates tile seams and grid artifacts.
* **10-Bit Color & HDR10 Passthrough (DaVinci Resolve Parity)**: Complete high bit depth video pipeline (`yuv420p10le`, `p010le`, `RGB48LE`) preserving BT.2020 color primaries and SMPTE 2084 PQ High Dynamic Range metadata without color clipping or banding.
* **Next-Gen AV1 & ProRes Mastering**: Hardware AV1 encoding on AMD AMF (`av1_amf`) and NVIDIA NVENC (`av1_nvenc`), SVT-AV1 CPU encoding, and Apple ProRes 422 HQ 10-bit master output.
* **Interactive Viewport Canvas Zoom & Pan (Topaz Parity)**: Cursor-centered mouse-wheel zoom ($50\% - 800\%$), click-and-drag pan, quick zoom presets (`Fit`, `100%`, `200%`, `400%`), and a mini-map crop navigator in the before/after comparison curtain.
* **Multi-File Batch Queue Manager**: Queue entire directories of videos, configure per-job overrides, start/pause processing, monitor live per-job FPS and ETA counters, and clear completed jobs.
* **In-App Model Zoo Downloader**: Automatic atomic downloading of production ONNX weights directly from HuggingFace with live byte/percentage progress tracking.
* **Studio Workflow Presets**: One-click professional profiles tailored for Web/Social, Cinematic 4K Master, Anime/Animation, and Archival Restoration.

---

## 🚀 Hardware Acceleration Architecture

Video Upscaler Studio achieves native silicon acceleration without vendor lock-in:

| Platform | Target Hardware | Neural Inference Provider | Hardware Video Codecs |
| :--- | :--- | :--- | :--- |
| **Windows** | **AMD Radeon GPUs** (Radeon 890M, 7900 XTX) | **DirectML** (DirectX 12 Compute) | AMD AMF (`hevc_amf`, `av1_amf`, `h264_amf`) |
| **Windows** | **NVIDIA GPUs** (RTX 4060, 4090, 3080) | **DirectML** / **CUDA** (FP16 Tensor Cores) | NVIDIA NVENC (`hevc_nvenc`, `av1_nvenc`, `h264_nvenc`) |
| **macOS** | **Apple Silicon** (M1/M2/M3/M4 Max/Pro/Base) | **Apple CoreML** (Neural Engine + Metal) | VideoToolbox (`hevc_videotoolbox`, ProRes 422 HQ) |
| **Cross-Platform** | Multi-Core CPU (x86_64 & ARM64) | ONNX Runtime CPU (Multi-threaded AVX/NEON) | Software CPU (`libsvtav1`, `libx265`, `libx264`, `prores_ks`) |

---

## 🧠 Supported Neural Models

All models download automatically inside the application with atomic `.part` validation:

| Model | Scale | Target Use Case | Temporal Window | File Size |
| :--- | :---: | :--- | :---: | :---: |
| **Compact SPAN (Fast)** | $2\times$ | Ultra-fast real-time upscaling for web & standard video | $T=3$ Fast / $T=1$ | **1.65 MB** |
| **Real-ESRGAN x4plus** | $4\times$ | Universal high-detail super-resolution for live-action & film | $T=5$ Quality / $T=1$ | **67.1 MB** |
| **Real-ESRGAN Anime 6B** | $4\times$ | Hand-drawn anime, illustrations & cel-shaded animation | $T=3$ Fast | **67.0 MB** |
| **ClearReality / BasicVSR** | $4\times$ | Film-grade restoration with maximum temporal coherence | $T=5$ Quality | **1.70 MB** |

---

## 🎛️ Workflow Presets

| Preset | Model | Scale | Encoder | Quality (CRF) | Temporal Mode | Film Grain |
| :--- | :--- | :---: | :--- | :---: | :--- | :---: |
| **⚡ Web & Social** | Compact SPAN | $2\times$ | AMD AMF / NVENC | 20 | Fast 3-Frame | 8% |
| **🎬 Cinematic 4K Master** | Real-ESRGAN x4plus | $4\times$ | NVENC / ProRes | 16 | Quality 5-Frame | 15% |
| **✨ Anime & Animation** | Real-ESRGAN Anime | $4\times$ | NVENC / AMF | 18 | Fast 3-Frame | 2% |
| **📼 Archival Restoration** | Compact SPAN | $2\times$ | Apple ProRes / x265 | 16 | Quality 5-Frame | 20% |

---

## 📦 Workspace Architecture

The workspace is organized into modular Rust crates and a modern frontend:

```text
video-upscaler/
├── Cargo.toml                                # Workspace manifest
├── .github/workflows/
│   └── build_and_release.yml                 # Automated Windows & macOS CI/CD packaging
├── crates/
│   ├── inference-runtime/                    # DirectML, CoreML, and CUDA execution providers
│   │   ├── src/session.rs                    # ONNX Runtime session & dynamic scale invariance
│   │   ├── src/hardware.rs                   # System GPU/NPU probe
│   │   └── src/backend.rs                    # Execution backend definitions
│   ├── image-processing/                     # Computer vision & mathematics
│   │   ├── src/tiling.rs                     # 2D Hann window overlapping grid decomposition
│   │   ├── src/color.rs                      # Fast RGB24 <-> NCHW tensor conversions
│   │   └── src/grain.rs                      # Film grain synthesis
│   ├── video-io/                             # Hardware video streaming
│   │   ├── src/demuxer.rs                    # FFmpeg streaming raw RGB24 demuxer
│   │   ├── src/muxer.rs                      # FFmpeg hardware muxer (AMF, NVENC, VideoToolbox)
│   │   ├── src/metadata.rs                   # FFprobe video and stream inspection
│   │   └── src/ffmpeg_bin.rs                 # Auto-discovery across Winget, PATH, and Homebrew
│   ├── models/                               # Neural weights management
│   │   ├── src/registry.rs                   # Catalog of supported ONNX super-resolution models
│   │   ├── src/downloader.rs                 # Atomic chunked HTTP streaming downloader
│   │   └── src/loader.rs                     # Local model cache resolver
│   └── engine-core/                          # Pipeline orchestration
│       ├── src/pipeline.rs                   # 3-stage ring buffer pipeline with backpressure
│       ├── src/temporal.rs                   # Recurrent temporal sliding window & bilateral fusion
│       ├── src/preview.rs                    # Sub-second frame preview generator
│       └── src/queue.rs                      # Multi-job batch queue scheduler
└── apps/
    └── desktop/                              # Standalone desktop application
        ├── Cargo.toml                        # Desktop app manifest & packaging metadata
        ├── icons/                            # Multi-resolution icons (.ico, .png, .icns)
        ├── src/main.rs                       # Tao native window, Wry webview, & Axum API
        └── ui/                               # React 18 + TypeScript + Vite studio interface
            ├── src/components/SplitPlayer.tsx     # Before/After comparison curtain slider
            ├── src/components/BatchQueue.tsx      # Multi-file batch queue manager view
            ├── src/components/DeviceSelector.tsx  # Compute device switcher
            └── src/components/ModelSettings.tsx   # Model selection, presets & live downloader
```

---

## 🛠️ Building & Running

### Prerequisites
* **Rust**: `1.80+` ([rustup.rs](https://rustup.rs/))
* **Node.js**: `v20+` ([nodejs.org](https://nodejs.org/))
* **FFmpeg**: Installed and available in PATH (e.g. via `winget install Gyan.FFmpeg` on Windows or `brew install ffmpeg` on macOS)

### 1. Build the Frontend UI
```bash
cd apps/desktop/ui
npm install
npm run build
cd ../../..
```

### 2. Run Workspace Test Suite (19 Tests)
```bash
cargo test --workspace
```

### 3. Launch the Desktop Application
```bash
# Debug run
cargo run -p desktop

# Optimized Release Build
cargo run -p desktop --release
```

### 4. Headless Server Mode (Docker / Render Farm)
To run Video Upscaler Studio as a headless background rendering daemon without creating an OS window:
```bash
target/release/video-upscaler.exe --headless
# Or on macOS:
./target/release/video-upscaler --headless
```
The REST API will be available at `http://127.0.0.1:3001`.

---

## 🌐 REST API Reference

When the desktop shell or headless server is active, it exposes the following endpoints:

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/health` | Healthcheck and engine version status |
| `GET` | `/api/devices` | Discovers available compute hardware (DirectML GPUs, CoreML NPU, CPU) |
| `GET` | `/api/models` | Lists model catalog and reports dynamic `is_cached` status |
| `POST` | `/api/models/download` | Initiates asynchronous background weight download |
| `GET` | `/api/models/downloads` | Polls active download percentage and downloaded bytes |
| `POST` | `/api/dialog/open-file` | Spawns native Win32/macOS single video file dialog |
| `POST` | `/api/dialog/open-files` | Spawns native multi-file dialog for batch import |
| `POST` | `/api/dialog/save-file` | Spawns native save file dialog with format filters |
| `POST` | `/api/probe` | Inspects video dimensions, framerate, duration, and codecs via FFprobe |
| `POST` | `/api/preview` | Generates Base64 comparison preview for timestamp in seconds |
| `POST` | `/api/upscale` | Starts a single video upscale render job |
| `GET` | `/api/status` | Queries live progress (percentage, current frame, FPS, ETA) |
| `POST` | `/api/cancel` | Instantly aborts active rendering pipeline |
| `GET` | `/api/queue` | Queries batch queue state, job list, and active job ID |
| `POST` | `/api/queue/add` | Adds an upscale job to the batch queue |
| `POST` | `/api/queue/remove` | Removes a job from the batch queue |
| `POST` | `/api/queue/clear` | Clears completed or cancelled batch jobs |
| `POST` | `/api/queue/start` | Starts sequential batch queue execution |
| `POST` | `/api/queue/pause` | Pauses queue execution after the current job |

---

## 📜 License

Licensed under either of:
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
