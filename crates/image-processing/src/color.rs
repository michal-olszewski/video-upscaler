use ndarray::Array4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorStandard {
    Bt709,  // Standard HD / SDR
    Bt2020, // UHD / HDR
}

pub struct ColorPipeline;

impl ColorPipeline {
    /// Applies procedural film grain to counter over-smoothing / plastic skin artifacts
    pub fn apply_film_grain(image: &mut Array4<f32>, grain_amount: f32) {
        if grain_amount <= 0.0 {
            return;
        }

        let (_, channels, height, width) = image.dim();

        // Pseudo-random noise generator using simple linear congruential hash
        for y in 0..height {
            for x in 0..width {
                let seed = ((y * 1973 + x * 9277) ^ 0x5bf03635) as f32;
                let noise = ((seed.sin() * 43758.5453).fract() - 0.5) * grain_amount * 0.08;

                for c in 0..channels {
                    let val = image[[0, c, y, x]] + noise;
                    image[[0, c, y, x]] = val.clamp(0.0, 1.0);
                }
            }
        }
    }

    /// Normalizes raw byte buffer [0..255] into planar NCHW float tensor [0.0..1.0]
    pub fn rgb_bytes_to_nchw(
        rgb_data: &[u8],
        width: usize,
        height: usize,
    ) -> Array4<f32> {
        let mut tensor = Array4::<f32>::zeros((1, 3, height, width));

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                tensor[[0, 0, y, x]] = rgb_data[idx] as f32 / 255.0;
                tensor[[0, 1, y, x]] = rgb_data[idx + 1] as f32 / 255.0;
                tensor[[0, 2, y, x]] = rgb_data[idx + 2] as f32 / 255.0;
            }
        }

        tensor
    }

    /// Converts planar NCHW float tensor [0.0..1.0] back to interleaved RGB byte buffer
    pub fn nchw_to_rgb_bytes(tensor: &Array4<f32>) -> Vec<u8> {
        let (_, _, height, width) = tensor.dim();
        let mut bytes = vec![0u8; width * height * 3];

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                let r = (tensor[[0, 0, y, x]] * 255.0).clamp(0.0, 255.0) as u8;
                let g = (tensor[[0, 1, y, x]] * 255.0).clamp(0.0, 255.0) as u8;
                let b = (tensor[[0, 2, y, x]] * 255.0).clamp(0.0, 255.0) as u8;

                bytes[idx] = r;
                bytes[idx + 1] = g;
                bytes[idx + 2] = b;
            }
        }

        bytes
    }

    /// Normalizes 16-bit little-endian RGB byte buffer (RGB48LE, 6 bytes/pixel)
    /// into planar NCHW float tensor [0.0..1.0] for 10-bit/12-bit/16-bit HDR pipelines
    pub fn rgb48_bytes_to_nchw(
        rgb48_data: &[u8],
        width: usize,
        height: usize,
    ) -> Array4<f32> {
        let mut tensor = Array4::<f32>::zeros((1, 3, height, width));

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 6;
                let r = u16::from_le_bytes([rgb48_data[idx], rgb48_data[idx + 1]]) as f32 / 65535.0;
                let g = u16::from_le_bytes([rgb48_data[idx + 2], rgb48_data[idx + 3]]) as f32 / 65535.0;
                let b = u16::from_le_bytes([rgb48_data[idx + 4], rgb48_data[idx + 5]]) as f32 / 65535.0;

                tensor[[0, 0, y, x]] = r;
                tensor[[0, 1, y, x]] = g;
                tensor[[0, 2, y, x]] = b;
            }
        }

        tensor
    }

    /// Converts planar NCHW float tensor [0.0..1.0] back to 16-bit little-endian
    /// RGB byte buffer (RGB48LE, 6 bytes/pixel) preserving full 10-bit/12-bit color precision
    pub fn nchw_to_rgb48_bytes(tensor: &Array4<f32>) -> Vec<u8> {
        let (_, _, height, width) = tensor.dim();
        let mut bytes = vec![0u8; width * height * 6];

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 6;
                let r = (tensor[[0, 0, y, x]] * 65535.0).clamp(0.0, 65535.0).round() as u16;
                let g = (tensor[[0, 1, y, x]] * 65535.0).clamp(0.0, 65535.0).round() as u16;
                let b = (tensor[[0, 2, y, x]] * 65535.0).clamp(0.0, 65535.0).round() as u16;

                bytes[idx..idx + 2].copy_from_slice(&r.to_le_bytes());
                bytes[idx + 2..idx + 4].copy_from_slice(&g.to_le_bytes());
                bytes[idx + 4..idx + 6].copy_from_slice(&b.to_le_bytes());
            }
        }

        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_nchw_roundtrip() {
        let width = 16;
        let height = 16;
        let mut original_bytes = vec![0u8; width * height * 3];
        for i in 0..original_bytes.len() {
            original_bytes[i] = (i % 256) as u8;
        }

        let tensor = ColorPipeline::rgb_bytes_to_nchw(&original_bytes, width, height);
        assert_eq!(tensor.dim(), (1, 3, height, width));

        let reconstructed_bytes = ColorPipeline::nchw_to_rgb_bytes(&tensor);
        assert_eq!(original_bytes, reconstructed_bytes);
    }

    #[test]
    fn test_rgb48_nchw_roundtrip() {
        let width = 8;
        let height = 8;
        let mut original_bytes = vec![0u8; width * height * 6];
        // Populate with 16-bit test patterns
        for i in 0..(width * height * 3) {
            let val = ((i * 1023) % 65536) as u16;
            original_bytes[i * 2..i * 2 + 2].copy_from_slice(&val.to_le_bytes());
        }

        let tensor = ColorPipeline::rgb48_bytes_to_nchw(&original_bytes, width, height);
        assert_eq!(tensor.dim(), (1, 3, height, width));

        let reconstructed_bytes = ColorPipeline::nchw_to_rgb48_bytes(&tensor);
        assert_eq!(original_bytes, reconstructed_bytes);
    }

    #[test]
    fn test_film_grain_bounds() {
        let width = 32;
        let height = 32;
        let mut image = Array4::<f32>::from_elem((1, 3, height, width), 0.5);

        ColorPipeline::apply_film_grain(&mut image, 0.2);

        // Verify values remain within [0.0, 1.0]
        for val in image.iter() {
            assert!(*val >= 0.0 && *val <= 1.0, "Film grain caused value out of bounds: {}", val);
        }

        // Verify grain actually varied the values (not static 0.5)
        let has_variation = image.iter().any(|v| (*v - 0.5).abs() > 1e-4);
        assert!(has_variation, "Film grain did not introduce variation");
    }
}
