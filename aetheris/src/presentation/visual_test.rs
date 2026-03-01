use image::{DynamicImage, GenericImage, GenericImageView, Rgba};
use std::path::Path;

pub struct VisualRegressionEngine {
    pub tolerance: u8,
}

impl VisualRegressionEngine {
    pub fn new(tolerance: u8) -> Self {
        Self { tolerance }
    }

    pub fn compare_images(
        &self,
        actual_path: &Path,
        golden_path: &Path,
        diff_path: &Path,
    ) -> anyhow::Result<f32> {
        let actual = image::open(actual_path)?;
        let golden = image::open(golden_path)?;

        if actual.dimensions() != golden.dimensions() {
            return Err(anyhow::anyhow!(
                "Dimension mismatch: Actual {:?}, Golden {:?}",
                actual.dimensions(),
                golden.dimensions()
            ));
        }

        let (width, height) = actual.dimensions();
        let mut diff_img = DynamicImage::new_rgba8(width, height);
        let mut mismatch_count = 0;

        for y in 0..height {
            for x in 0..width {
                let p_act = actual.get_pixel(x, y);
                let p_gold = golden.get_pixel(x, y);

                if !self.pixels_match(p_act, p_gold) {
                    mismatch_count += 1;
                    diff_img.put_pixel(x, y, Rgba([255, 0, 0, 255])); // Red for difference
                } else {
                    diff_img.put_pixel(x, y, p_act);
                }
            }
        }

        let score = (mismatch_count as f32) / ((width * height) as f32);
        if mismatch_count > 0 {
            diff_img.save(diff_path)?;
        }

        Ok(score)
    }

    fn pixels_match(&self, p1: Rgba<u8>, p2: Rgba<u8>) -> bool {
        for i in 0..3 {
            // RGB only
            let diff = (p1[i] as i16 - p2[i] as i16).abs() as u8;
            if diff > self.tolerance {
                return false;
            }
        }
        true
    }
}
