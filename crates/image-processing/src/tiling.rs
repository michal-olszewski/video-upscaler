use ndarray::{s, Array2, Array4};

#[derive(Debug, Clone, Copy)]
pub struct TileSpec {
    pub tile_size: usize,
    pub padding: usize,
    pub scale: usize,
}

impl Default for TileSpec {
    fn default() -> Self {
        Self {
            tile_size: 512,
            padding: 32,
            scale: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TileCoordinate {
    pub in_x: usize,
    pub in_y: usize,
    pub in_w: usize,
    pub in_h: usize,
    pub pad_left: usize,
    pub pad_right: usize,
    pub pad_top: usize,
    pub pad_bottom: usize,
}

pub struct TilingEngine {
    spec: TileSpec,
}

impl TilingEngine {
    pub fn new(spec: TileSpec) -> Self {
        Self { spec }
    }

    /// Computes the grid of tile coordinates for an image of dimensions (width, height)
    pub fn compute_tiles(&self, width: usize, height: usize) -> Vec<TileCoordinate> {
        let mut tiles = Vec::new();
        let step = self.spec.tile_size - (2 * self.spec.padding);

        let mut y = 0;
        while y < height {
            let h = (self.spec.tile_size).min(height - y);
            let mut x = 0;
            while x < width {
                let w = (self.spec.tile_size).min(width - x);

                let pad_left = if x == 0 { 0 } else { self.spec.padding };
                let pad_top = if y == 0 { 0 } else { self.spec.padding };
                let pad_right = if x + w >= width { 0 } else { self.spec.padding };
                let pad_bottom = if y + h >= height { 0 } else { self.spec.padding };

                tiles.push(TileCoordinate {
                    in_x: x,
                    in_y: y,
                    in_w: w,
                    in_h: h,
                    pad_left,
                    pad_right,
                    pad_top,
                    pad_bottom,
                });

                if x + w >= width {
                    break;
                }
                x += step;
            }
            if y + h >= height {
                break;
            }
            y += step;
        }

        tiles
    }

    /// Generates a 2D Hann window blending weight map for smooth overlap reconstruction
    pub fn create_hann_weight_mask(width: usize, height: usize) -> Array2<f32> {
        let mut mask = Array2::<f32>::zeros((height, width));
        use std::f32::consts::PI;

        for y in 0..height {
            let wy = 0.5 * (1.0 - (2.0 * PI * (y as f32 + 0.5) / height as f32).cos());
            for x in 0..width {
                let wx = 0.5 * (1.0 - (2.0 * PI * (x as f32 + 0.5) / width as f32).cos());
                mask[[y, x]] = (wx * wy).max(1e-4);
            }
        }

        mask
    }

    /// Stitches upscaled tiles back together with weighted blending
    pub fn stitch_tiles(
        &self,
        tiles: &[(TileCoordinate, Array4<f32>)],
        out_width: usize,
        out_height: usize,
    ) -> Array4<f32> {
        let channels = 3;
        let mut output = Array4::<f32>::zeros((1, channels, out_height, out_width));
        let mut weight_sum = Array2::<f32>::zeros((out_height, out_width));

        let scale = self.spec.scale;

        for (coord, tile_data) in tiles {
            let tile_h = tile_data.dim().2;
            let tile_w = tile_data.dim().3;

            let out_x = coord.in_x * scale;
            let out_y = coord.in_y * scale;

            let weights = Self::create_hann_weight_mask(tile_w, tile_h);

            for y in 0..tile_h {
                let dst_y = out_y + y;
                if dst_y >= out_height {
                    continue;
                }
                for x in 0..tile_w {
                    let dst_x = out_x + x;
                    if dst_x >= out_width {
                        continue;
                    }

                    let w = weights[[y, x]];
                    for c in 0..channels {
                        output[[0, c, dst_y, dst_x]] += tile_data[[0, c, y, x]] * w;
                    }
                    weight_sum[[dst_y, dst_x]] += w;
                }
            }
        }

        // Normalize accumulated weights
        for y in 0..out_height {
            for x in 0..out_width {
                let w = weight_sum[[y, x]];
                if w > 0.0 {
                    for c in 0..channels {
                        output[[0, c, y, x]] /= w;
                    }
                }
            }
        }

        output
    }
}
