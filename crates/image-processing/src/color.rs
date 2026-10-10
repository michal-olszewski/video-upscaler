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
}
