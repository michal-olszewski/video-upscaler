use ndarray::{Array4, Array5};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Temporal consistency and flicker suppression mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalMode {
    /// Single-frame super-resolution (T=1, no temporal context)
    Disabled,
    /// Fast 3-frame temporal window (T=3: [t-1, t, t+1])
    Fast3Frame,
    /// High-quality 5-frame temporal window (T=5: [t-2, t-1, t, t+1, t+2])
    Quality5Frame,
}

impl Default for TemporalMode {
    fn default() -> Self {
        Self::Disabled
    }
}

impl TemporalMode {
    pub fn window_size(&self) -> usize {
        match self {
            Self::Disabled => 1,
            Self::Fast3Frame => 3,
            Self::Quality5Frame => 5,
        }
    }

    pub fn radius(&self) -> usize {
        match self {
            Self::Disabled => 0,
            Self::Fast3Frame => 1,
            Self::Quality5Frame => 2,
        }
    }
}

/// A structured temporal window centered on a target frame
#[derive(Debug, Clone)]
pub struct TemporalWindow {
    /// Index of the central target frame being reconstructed
    pub target_index: u64,
    /// Ordered frames in window: [t - R, ..., t, ..., t + R]
    pub frames: Vec<(u64, Vec<u8>)>,
}

/// Streaming sliding window maintaining bounded temporal context with boundary clamping
pub struct TemporalSlidingWindow {
    mode: TemporalMode,
    radius: usize,
    buffer: VecDeque<(u64, Vec<u8>)>,
    next_emit_index: u64,
    total_received: u64,
}

impl TemporalSlidingWindow {
    pub fn new(mode: TemporalMode) -> Self {
        let radius = mode.radius();
        Self {
            mode,
            radius,
            buffer: VecDeque::with_capacity(mode.window_size() + 2),
            next_emit_index: 0,
            total_received: 0,
        }
    }

    /// Pushes a new decoded frame into the sliding window.
    /// Returns any temporal windows that are now ready to be processed.
    pub fn push_frame(&mut self, frame_index: u64, frame_bytes: Vec<u8>) -> Vec<TemporalWindow> {
        if self.radius == 0 {
            // Disabled: emit frame immediately as 1-element window
            return vec![TemporalWindow {
                target_index: frame_index,
                frames: vec![(frame_index, frame_bytes)],
            }];
        }

        self.buffer.push_back((frame_index, frame_bytes));
        self.total_received += 1;

        let mut ready_windows = Vec::new();

        // While we have enough lookahead in the buffer to emit the next target frame:
        // Target frame index `next_emit_index` requires frames up to `next_emit_index + radius`.
        while let Some(window) = self.try_emit_window() {
            ready_windows.push(window);
        }

        ready_windows
    }

    fn try_emit_window(&mut self) -> Option<TemporalWindow> {
        if self.buffer.is_empty() {
            return None;
        }

        let target_idx = self.next_emit_index;
        let required_lookahead = target_idx + self.radius as u64;

        let latest_available = self.buffer.back()?.0;
        if latest_available < required_lookahead {
            // Not enough forward frames in buffer yet
            return None;
        }

        let window = self.build_window_for_target(target_idx);
        self.next_emit_index += 1;

        // Clean up obsolete frames from the front of the buffer:
        // A frame at index `k` is no longer needed once target index has advanced past `k + radius`
        while let Some(front) = self.buffer.front() {
            if (front.0 + self.radius as u64) < self.next_emit_index {
                self.buffer.pop_front();
            } else {
                break;
            }
        }

        Some(window)
    }

    /// Flushes any remaining target frames at the end of the stream with right-clamped boundaries
    pub fn flush(&mut self) -> Vec<TemporalWindow> {
        if self.radius == 0 || self.buffer.is_empty() {
            return Vec::new();
        }

        let mut remaining = Vec::new();
        while self.next_emit_index < self.total_received {
            let window = self.build_window_for_target(self.next_emit_index);
            remaining.push(window);
            self.next_emit_index += 1;
        }

        self.buffer.clear();
        remaining
    }

    fn build_window_for_target(&self, target_idx: u64) -> TemporalWindow {
        let mut window_frames = Vec::with_capacity(self.mode.window_size());
        let r = self.radius as i64;
        let t = target_idx as i64;

        for offset in -r..=r {
            let wanted_idx = (t + offset).clamp(0, self.total_received.saturating_sub(1) as i64) as u64;

            // Locate frame in buffer or fallback to nearest available
            let frame_data = self
                .buffer
                .iter()
                .find(|(idx, _)| *idx == wanted_idx)
                .or_else(|| {
                    if wanted_idx <= self.buffer.front().map(|f| f.0).unwrap_or(0) {
                        self.buffer.front()
                    } else {
                        self.buffer.back()
                    }
                });

            if let Some((idx, bytes)) = frame_data {
                window_frames.push((*idx, bytes.clone()));
            }
        }

        TemporalWindow {
            target_index: target_idx,
            frames: window_frames,
        }
    }
}

/// Engine for temporal feature fusion and flicker suppression
pub struct TemporalFusionEngine;

impl TemporalFusionEngine {
    /// Builds a 5D batch tensor `[1, T, 3, H, W]` normalized to [0.0, 1.0] from a temporal window
    pub fn build_5d_tensor(
        window: &TemporalWindow,
        width: usize,
        height: usize,
    ) -> Array5<f32> {
        let t = window.frames.len();
        let mut tensor = Array5::<f32>::zeros((1, t, 3, height, width));

        for (t_idx, (_frame_num, raw_bytes)) in window.frames.iter().enumerate() {
            let _frame_size = width * height;
            for y in 0..height {
                for x in 0..width {
                    let pixel_idx = (y * width + x) * 3;
                    if pixel_idx + 2 < raw_bytes.len() {
                        tensor[[0, t_idx, 0, y, x]] = raw_bytes[pixel_idx] as f32 / 255.0;
                        tensor[[0, t_idx, 1, y, x]] = raw_bytes[pixel_idx + 1] as f32 / 255.0;
                        tensor[[0, t_idx, 2, y, x]] = raw_bytes[pixel_idx + 2] as f32 / 255.0;
                    }
                }
            }
        }

        tensor
    }

    /// Performs edge-aware bilateral temporal fusion on the target frame using neighbor frames
    /// to eliminate high-frequency flickering while preserving sharp motion boundaries.
    pub fn suppress_flicker_bilateral(
        center_upscaled: &mut Array4<f32>,
        neighbor_upscaled: &[Array4<f32>],
        temporal_strength: f32, // typically 0.25..0.5
    ) {
        if neighbor_upscaled.is_empty() || temporal_strength <= 0.0 {
            return;
        }

        let (_batch, _channels, height, width) = center_upscaled.dim();

        let sigma_color = 0.08f32; // Color distance sensitivity
        let two_sigma_sq = 2.0 * sigma_color * sigma_color;

        for y in 0..height {
            for x in 0..width {
                let mut sum_weights = 1.0f32;
                let mut accum_c = [
                    center_upscaled[[0, 0, y, x]],
                    center_upscaled[[0, 1, y, x]],
                    center_upscaled[[0, 2, y, x]],
                ];

                for neighbor in neighbor_upscaled {
                    let n0 = neighbor[[0, 0, y, x]];
                    let n1 = neighbor[[0, 1, y, x]];
                    let n2 = neighbor[[0, 2, y, x]];

                    // Photometric color difference
                    let diff_sq = (accum_c[0] - n0).powi(2)
                        + (accum_c[1] - n1).powi(2)
                        + (accum_c[2] - n2).powi(2);

                    // Bilateral weight falls off exponentially if motion occurs
                    let weight = (-diff_sq / two_sigma_sq).exp() * temporal_strength;

                    accum_c[0] += n0 * weight;
                    accum_c[1] += n1 * weight;
                    accum_c[2] += n2 * weight;
                    sum_weights += weight;
                }

                center_upscaled[[0, 0, y, x]] = accum_c[0] / sum_weights;
                center_upscaled[[0, 1, y, x]] = accum_c[1] / sum_weights;
                center_upscaled[[0, 2, y, x]] = accum_c[2] / sum_weights;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_mode_properties() {
        assert_eq!(TemporalMode::Disabled.window_size(), 1);
        assert_eq!(TemporalMode::Disabled.radius(), 0);

        assert_eq!(TemporalMode::Fast3Frame.window_size(), 3);
        assert_eq!(TemporalMode::Fast3Frame.radius(), 1);

        assert_eq!(TemporalMode::Quality5Frame.window_size(), 5);
        assert_eq!(TemporalMode::Quality5Frame.radius(), 2);
    }

    #[test]
    fn test_sliding_window_fast_3frame_preserves_count() {
        let total_frames = 10u64;
        let mut window = TemporalSlidingWindow::new(TemporalMode::Fast3Frame);
        let mut emitted = Vec::new();

        for i in 0..total_frames {
            let dummy_bytes = vec![i as u8; 12];
            let ready = window.push_frame(i, dummy_bytes);
            emitted.extend(ready);
        }

        let flushed = window.flush();
        emitted.extend(flushed);

        // Crucial guarantee: exact 1:1 count preservation
        assert_eq!(emitted.len(), total_frames as usize);

        // Verify index ordering
        for (idx, w) in emitted.iter().enumerate() {
            assert_eq!(w.target_index, idx as u64);
            assert_eq!(w.frames.len(), 3);
        }

        // Verify boundary clamping at start (frame 0 should have clamped left neighbor 0)
        assert_eq!(emitted[0].frames[0].0, 0); // left clamped
        assert_eq!(emitted[0].frames[1].0, 0); // center
        assert_eq!(emitted[0].frames[2].0, 1); // right

        // Verify middle frame (frame 5 should have 4, 5, 6)
        assert_eq!(emitted[5].frames[0].0, 4);
        assert_eq!(emitted[5].frames[1].0, 5);
        assert_eq!(emitted[5].frames[2].0, 6);

        // Verify boundary clamping at end (frame 9 should have right clamped to 9)
        assert_eq!(emitted[9].frames[0].0, 8);
        assert_eq!(emitted[9].frames[1].0, 9);
        assert_eq!(emitted[9].frames[2].0, 9);
    }

    #[test]
    fn test_sliding_window_quality_5frame_preserves_count() {
        let total_frames = 12u64;
        let mut window = TemporalSlidingWindow::new(TemporalMode::Quality5Frame);
        let mut emitted = Vec::new();

        for i in 0..total_frames {
            let dummy_bytes = vec![i as u8; 12];
            let ready = window.push_frame(i, dummy_bytes);
            emitted.extend(ready);
        }

        let flushed = window.flush();
        emitted.extend(flushed);

        assert_eq!(emitted.len(), total_frames as usize);

        for (idx, w) in emitted.iter().enumerate() {
            assert_eq!(w.target_index, idx as u64);
            assert_eq!(w.frames.len(), 5);
        }

        // Frame 0: clamped [0, 0, 0, 1, 2]
        assert_eq!(emitted[0].frames[0].0, 0);
        assert_eq!(emitted[0].frames[1].0, 0);
        assert_eq!(emitted[0].frames[2].0, 0);
        assert_eq!(emitted[0].frames[3].0, 1);
        assert_eq!(emitted[0].frames[4].0, 2);

        // Frame 6: [4, 5, 6, 7, 8]
        assert_eq!(emitted[6].frames[0].0, 4);
        assert_eq!(emitted[6].frames[1].0, 5);
        assert_eq!(emitted[6].frames[2].0, 6);
        assert_eq!(emitted[6].frames[3].0, 7);
        assert_eq!(emitted[6].frames[4].0, 8);

        // Frame 11 (last): [9, 10, 11, 11, 11]
        assert_eq!(emitted[11].frames[0].0, 9);
        assert_eq!(emitted[11].frames[1].0, 10);
        assert_eq!(emitted[11].frames[2].0, 11);
        assert_eq!(emitted[11].frames[3].0, 11);
        assert_eq!(emitted[11].frames[4].0, 11);
    }

    #[test]
    fn test_bilateral_temporal_fusion_filters_noise() {
        let mut center = Array4::<f32>::from_elem((1, 3, 4, 4), 0.5);
        // Add random high-frequency noise spike to one pixel in center
        center[[0, 0, 2, 2]] = 0.55;

        // Neighbor frames have stable 0.5 signal
        let n1 = Array4::<f32>::from_elem((1, 3, 4, 4), 0.5);
        let n2 = Array4::<f32>::from_elem((1, 3, 4, 4), 0.5);

        TemporalFusionEngine::suppress_flicker_bilateral(&mut center, &[n1, n2], 0.5);

        // High frequency noise should be smoothed towards stable neighbor values
        let val = center[[0, 0, 2, 2]];
        assert!(val < 0.55 && val >= 0.50, "Flicker noise should be dampened: got {}", val);
    }
}
