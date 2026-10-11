use ndarray::Array4;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpolationMode {
    Disabled,
    DoubleFps,     // 2x FPS (e.g. 24 -> 48 fps, 30 -> 60 fps)
    QuadrupleFps,  // 4x FPS (e.g. 24 -> 96 fps, 30 -> 120 fps)
    SlowMotion2x,  // 2x Filmic Slow-Motion (half-speed playback)
    SlowMotion4x,  // 4x Filmic Slow-Motion (quarter-speed playback)
}

impl InterpolationMode {
    /// Returns the frame multiplication factor
    pub fn multiplier(&self) -> usize {
        match self {
            Self::Disabled => 1,
            Self::DoubleFps | Self::SlowMotion2x => 2,
            Self::QuadrupleFps | Self::SlowMotion4x => 4,
        }
    }

    /// Determines whether this mode represents slow-motion playback
    pub fn is_slow_motion(&self) -> bool {
        matches!(self, Self::SlowMotion2x | Self::SlowMotion4x)
    }

    /// Calculates output container framerate
    pub fn calculate_output_fps(&self, source_fps: f64) -> f64 {
        match self {
            Self::Disabled | Self::SlowMotion2x | Self::SlowMotion4x => source_fps,
            Self::DoubleFps => source_fps * 2.0,
            Self::QuadrupleFps => source_fps * 4.0,
        }
    }

    /// Calculates expected total output frames from source frame count
    pub fn calculate_total_frames(&self, source_frames: u64) -> u64 {
        if source_frames <= 1 {
            return source_frames;
        }
        match self {
            Self::Disabled => source_frames,
            Self::DoubleFps | Self::SlowMotion2x => (source_frames * 2).saturating_sub(1),
            Self::QuadrupleFps | Self::SlowMotion4x => (source_frames * 4).saturating_sub(3),
        }
    }
}

pub struct MotionInterpolationEngine;

impl MotionInterpolationEngine {
    /// Synthesizes an intermediate frame between frame0 and frame1 at arbitrary timestep t in [0.0..1.0]
    /// using bidirectional motion-compensated optical flow synthesis.
    pub fn interpolate_pair(
        frame0: &Array4<f32>,
        frame1: &Array4<f32>,
        t: f32,
    ) -> Array4<f32> {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.001 {
            return frame0.clone();
        }
        if t >= 0.999 {
            return frame1.clone();
        }

        let (batch, channels, height, width) = frame0.dim();
        let mut out = Array4::<f32>::zeros((batch, channels, height, width));

        // Block-based optical flow search parameters
        const BLOCK_SIZE: usize = 8;
        const SEARCH_RADIUS: isize = 4;

        let blocks_y = height / BLOCK_SIZE;
        let blocks_x = width / BLOCK_SIZE;

        // Compute motion vector field (mv_y, mv_x) per block
        let mut mv_field = vec![vec![(0isize, 0isize); blocks_x]; blocks_y];

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let start_y = by * BLOCK_SIZE;
                let start_x = bx * BLOCK_SIZE;

                let mut min_sad = f32::MAX;
                let mut best_dy = 0isize;
                let mut best_dx = 0isize;

                for dy in -SEARCH_RADIUS..=SEARCH_RADIUS {
                    let cy = start_y as isize + dy;
                    if cy < 0 || (cy as usize + BLOCK_SIZE) > height {
                        continue;
                    }

                    for dx in -SEARCH_RADIUS..=SEARCH_RADIUS {
                        let cx = start_x as isize + dx;
                        if cx < 0 || (cx as usize + BLOCK_SIZE) > width {
                            continue;
                        }

                        // Sum of Absolute Differences (SAD) across luminance
                        let mut sad = 0.0f32;
                        for y in 0..BLOCK_SIZE {
                            for x in 0..BLOCK_SIZE {
                                let p0 = frame0[[0, 0, start_y + y, start_x + x]] * 0.299
                                    + frame0[[0, 1, start_y + y, start_x + x]] * 0.587
                                    + frame0[[0, 2, start_y + y, start_x + x]] * 0.114;
                                let p1 = frame1[[0, 0, (cy as usize) + y, (cx as usize) + x]] * 0.299
                                    + frame1[[0, 1, (cy as usize) + y, (cx as usize) + x]] * 0.587
                                    + frame1[[0, 2, (cy as usize) + y, (cx as usize) + x]] * 0.114;
                                sad += (p0 - p1).abs();
                            }
                        }

                        if sad < min_sad {
                            min_sad = sad;
                            best_dy = dy;
                            best_dx = dx;
                        }
                    }
                }

                mv_field[by][bx] = (best_dy, best_dx);
            }
        }

        // Bidirectional motion-compensated warping and blending
        for y in 0..height {
            let by = (y / BLOCK_SIZE).min(blocks_y.saturating_sub(1));
            for x in 0..width {
                let bx = (x / BLOCK_SIZE).min(blocks_x.saturating_sub(1));
                let (dy, dx) = mv_field[by][bx];

                // Forward warp vector from frame0: + t * mv
                let fwd_y = (y as f32 + t * dy as f32).clamp(0.0, (height - 1) as f32) as usize;
                let fwd_x = (x as f32 + t * dx as f32).clamp(0.0, (width - 1) as f32) as usize;

                // Backward warp vector from frame1: - (1 - t) * mv
                let bwd_y = (y as f32 - (1.0 - t) * dy as f32).clamp(0.0, (height - 1) as f32) as usize;
                let bwd_x = (x as f32 - (1.0 - t) * dx as f32).clamp(0.0, (width - 1) as f32) as usize;

                for c in 0..channels {
                    let p0 = frame0[[0, c, fwd_y, fwd_x]];
                    let p1 = frame1[[0, c, bwd_y, bwd_x]];

                    // Photometric consistency blend
                    let val = (1.0 - t) * p0 + t * p1;
                    out[[0, c, y, x]] = val.clamp(0.0, 1.0);
                }
            }
        }

        out
    }

    /// Synthesizes all intermediate frames between frame0 and frame1 for the requested interpolation mode
    pub fn interpolate_intermediate(
        frame0: &Array4<f32>,
        frame1: &Array4<f32>,
        mode: InterpolationMode,
    ) -> Vec<Array4<f32>> {
        match mode {
            InterpolationMode::Disabled => Vec::new(),
            InterpolationMode::DoubleFps | InterpolationMode::SlowMotion2x => {
                // Generate 1 intermediate frame at t = 0.5
                vec![Self::interpolate_pair(frame0, frame1, 0.5)]
            }
            InterpolationMode::QuadrupleFps | InterpolationMode::SlowMotion4x => {
                // Hierarchical generation at t = 0.25, 0.5, 0.75
                let mid = Self::interpolate_pair(frame0, frame1, 0.5);
                let q1 = Self::interpolate_pair(frame0, &mid, 0.5);
                let q3 = Self::interpolate_pair(&mid, frame1, 0.5);
                vec![q1, mid, q3]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_mode_calculations() {
        let mode_2x = InterpolationMode::DoubleFps;
        assert_eq!(mode_2x.multiplier(), 2);
        assert!(!mode_2x.is_slow_motion());
        assert_eq!(mode_2x.calculate_output_fps(24.0), 48.0);
        assert_eq!(mode_2x.calculate_total_frames(10), 19);

        let mode_4x = InterpolationMode::QuadrupleFps;
        assert_eq!(mode_4x.multiplier(), 4);
        assert_eq!(mode_4x.calculate_output_fps(30.0), 120.0);
        assert_eq!(mode_4x.calculate_total_frames(10), 37);

        let mode_slow2x = InterpolationMode::SlowMotion2x;
        assert!(mode_slow2x.is_slow_motion());
        assert_eq!(mode_slow2x.calculate_output_fps(24.0), 24.0);
        assert_eq!(mode_slow2x.calculate_total_frames(10), 19);
    }

    #[test]
    fn test_motion_interpolation_pair_bounds() {
        let h = 16;
        let w = 16;
        let mut f0 = Array4::<f32>::zeros((1, 3, h, w));
        let mut f1 = Array4::<f32>::zeros((1, 3, h, w));

        // Create a moving step pattern
        for y in 0..h {
            for x in 0..w {
                f0[[0, 0, y, x]] = if x < 8 { 1.0 } else { 0.0 };
                f1[[0, 0, y, x]] = if x < 12 { 1.0 } else { 0.0 };
            }
        }

        let interp = MotionInterpolationEngine::interpolate_pair(&f0, &f1, 0.5);
        assert_eq!(interp.dim(), (1, 3, h, w));

        for val in interp.iter() {
            assert!(*val >= 0.0 && *val <= 1.0, "Interpolated pixel out of bounds: {}", val);
        }
    }

    #[test]
    fn test_motion_interpolation_multi_frame() {
        let h = 8;
        let w = 8;
        let f0 = Array4::<f32>::from_elem((1, 3, h, w), 0.2);
        let f1 = Array4::<f32>::from_elem((1, 3, h, w), 0.8);

        let intermediates_2x = MotionInterpolationEngine::interpolate_intermediate(
            &f0,
            &f1,
            InterpolationMode::DoubleFps,
        );
        assert_eq!(intermediates_2x.len(), 1);
        let mid_val = intermediates_2x[0][[0, 0, 4, 4]];
        assert!((mid_val - 0.5).abs() < 0.05, "Expected ~0.5 midpoint, got {}", mid_val);

        let intermediates_4x = MotionInterpolationEngine::interpolate_intermediate(
            &f0,
            &f1,
            InterpolationMode::QuadrupleFps,
        );
        assert_eq!(intermediates_4x.len(), 3);
    }
}
