use ndarray::Array4;

/// Edge-preserving spatial bilateral denoise filter for NCHW image tensors
pub struct SpatialDenoiseFilter;

impl SpatialDenoiseFilter {
    /// Applies edge-preserving bilateral smoothing in-place on an NCHW tensor (values in [0.0, 1.0]).
    /// `strength` is clamped to [0.0, 1.0]. A strength of 0.0 is a no-op.
    pub fn apply(tensor: &mut Array4<f32>, strength: f32) {
        if strength <= 0.001 {
            return;
        }

        let strength = strength.clamp(0.0, 1.0);
        let (_, channels, height, width) = tensor.dim();

        if height <= 2 || width <= 2 {
            return;
        }

        // Spatial and photometric variances based on strength
        let sigma_s = 1.0 + strength * 1.5;
        let sigma_r = 0.04 + strength * 0.12;
        let two_sigma_s_sq = 2.0 * sigma_s * sigma_s;
        let two_sigma_r_sq = 2.0 * sigma_r * sigma_r;

        let radius: isize = if strength > 0.5 { 2 } else { 1 };

        let copy = tensor.clone();

        for c in 0..channels {
            for y in 0..height {
                for x in 0..width {
                    let center_val = copy[[0, c, y, x]];
                    let mut weight_sum = 0.0f32;
                    let mut val_sum = 0.0f32;

                    for dy in -radius..=radius {
                        let ny = y as isize + dy;
                        if ny < 0 || ny >= height as isize {
                            continue;
                        }

                        for dx in -radius..=radius {
                            let nx = x as isize + dx;
                            if nx < 0 || nx >= width as isize {
                                continue;
                            }

                            let spatial_dist_sq = (dx * dx + dy * dy) as f32;
                            let spatial_weight = (-spatial_dist_sq / two_sigma_s_sq).exp();

                            let sample_val = copy[[0, c, ny as usize, nx as usize]];
                            let range_diff = sample_val - center_val;
                            let range_weight = (-(range_diff * range_diff) / two_sigma_r_sq).exp();

                            let weight = spatial_weight * range_weight;
                            weight_sum += weight;
                            val_sum += weight * sample_val;
                        }
                    }

                    if weight_sum > 0.0 {
                        // Blend between original and filtered by strength
                        let filtered = val_sum / weight_sum;
                        tensor[[0, c, y, x]] = center_val * (1.0 - strength) + filtered * strength;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_denoise_preserves_constant_image() {
        let mut tensor = Array4::<f32>::from_elem((1, 3, 16, 16), 0.5);
        let orig = tensor.clone();
        SpatialDenoiseFilter::apply(&mut tensor, 0.8);

        for (a, b) in tensor.iter().zip(orig.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn test_denoise_smooths_noise_while_preserving_edges() {
        let mut tensor = Array4::<f32>::zeros((1, 3, 20, 20));
        // High contrast vertical edge: left half is 0.0, right half is 1.0
        for x in 10..20 {
            for y in 0..20 {
                for c in 0..3 {
                    tensor[[0, c, y, x]] = 1.0;
                }
            }
        }

        // Add a noise spike in the middle of smooth region (e.g. analog tape noise)
        tensor[[0, 0, 5, 5]] = 0.2; // noise in left region

        SpatialDenoiseFilter::apply(&mut tensor, 0.8);

        // The noise spike should be suppressed significantly from 0.2
        assert!(tensor[[0, 0, 5, 5]] < 0.15);

        // The edge boundary contrast should be preserved
        assert!(tensor[[0, 0, 10, 8]] < 0.05); // still near 0
        assert!(tensor[[0, 0, 10, 12]] > 0.95); // still near 1
    }

    #[test]
    fn test_denoise_zero_strength_is_noop() {
        let mut tensor = Array4::<f32>::from_elem((1, 3, 8, 8), 0.75);
        let orig = tensor.clone();
        SpatialDenoiseFilter::apply(&mut tensor, 0.0);
        assert_eq!(tensor, orig);
    }
}
