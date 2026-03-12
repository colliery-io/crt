//! Golden file comparison framework for visual regression testing.
//!
//! Compares rendered frames against reference ("golden") images with
//! configurable perceptual tolerance. Supports per-platform golden files
//! and generates diff images on failure.
//!
//! # Environment variables
//!
//! - `UPDATE_GOLDEN=1`: Write actual output as the new golden file instead of comparing.
//! - `VISUAL_TEST_TOLERANCE`: Override the default tolerance (0.5%) for all tests.
//!
//! # Directory layout
//!
//! ```text
//! tests/visual/
//!   golden/           # Reference images (checked into git)
//!     test_name.macos.png
//!     test_name.linux.png
//!   failures/         # Generated on failure (gitignored)
//!     test_name.actual.png
//!     test_name.diff.png
//! ```

use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

/// Default tolerance: allow up to 0.5% of pixels to differ.
pub const DEFAULT_TOLERANCE: f64 = 0.5;

/// Result of comparing a rendered frame against a golden file.
#[derive(Debug)]
pub struct ComparisonResult {
    /// Whether the images match within the given tolerance.
    pub matched: bool,
    /// Percentage of pixels that differ (0.0 – 100.0).
    pub diff_percentage: f64,
    /// Number of pixels that differ.
    pub diff_pixels: usize,
    /// Total number of pixels compared.
    pub total_pixels: usize,
    /// PNG-encoded diff image (only generated when images differ).
    /// Changed pixels are highlighted in red; unchanged pixels are dimmed.
    pub diff_image: Option<Vec<u8>>,
}

/// Per-pixel color distance threshold.
/// Two pixels are "different" if the max channel delta exceeds this value.
/// This accounts for minor anti-aliasing and rounding differences.
const PIXEL_THRESHOLD: u8 = 2;

/// Compare actual rendered pixels (raw RGBA) against a golden file.
///
/// `actual_png` is PNG-encoded bytes. `golden_path` should point to the
/// platform-specific golden file (use [`golden_path`] to resolve it).
/// `tolerance` is a percentage (0.0 – 100.0) of pixels allowed to differ.
pub fn compare_with_golden(
    actual_png: &[u8],
    golden_path: &Path,
    tolerance: f64,
) -> ComparisonResult {
    let actual = image::load_from_memory(actual_png)
        .expect("failed to decode actual PNG")
        .to_rgba8();

    if !golden_path.exists() {
        // No golden file yet — always fail (unless UPDATE_GOLDEN is set,
        // which the caller handles before calling this).
        return ComparisonResult {
            matched: false,
            diff_percentage: 100.0,
            diff_pixels: (actual.width() * actual.height()) as usize,
            total_pixels: (actual.width() * actual.height()) as usize,
            diff_image: None,
        };
    }

    let golden = image::open(golden_path)
        .unwrap_or_else(|e| panic!("failed to open golden file {}: {e}", golden_path.display()))
        .to_rgba8();

    compare_images(&actual, &golden, tolerance)
}

/// Compare two RGBA images with the given tolerance percentage.
pub fn compare_images(actual: &RgbaImage, golden: &RgbaImage, tolerance: f64) -> ComparisonResult {
    let (aw, ah) = actual.dimensions();
    let (gw, gh) = golden.dimensions();

    if aw != gw || ah != gh {
        // Dimension mismatch is always a failure.
        return ComparisonResult {
            matched: false,
            diff_percentage: 100.0,
            diff_pixels: (aw * ah).max(gw * gh) as usize,
            total_pixels: (aw * ah).max(gw * gh) as usize,
            diff_image: None,
        };
    }

    let total_pixels = (aw * ah) as usize;
    let mut diff_pixels = 0usize;
    let mut diff_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(aw, ah);

    for y in 0..ah {
        for x in 0..aw {
            let ap = actual.get_pixel(x, y);
            let gp = golden.get_pixel(x, y);

            let dr = ap[0].abs_diff(gp[0]);
            let dg = ap[1].abs_diff(gp[1]);
            let db = ap[2].abs_diff(gp[2]);
            let da = ap[3].abs_diff(gp[3]);

            if dr > PIXEL_THRESHOLD || dg > PIXEL_THRESHOLD || db > PIXEL_THRESHOLD || da > PIXEL_THRESHOLD {
                diff_pixels += 1;
                // Highlight differing pixel in red.
                diff_img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            } else {
                // Dim the matching pixel for context.
                diff_img.put_pixel(x, y, Rgba([ap[0] / 4, ap[1] / 4, ap[2] / 4, 255]));
            }
        }
    }

    let diff_percentage = if total_pixels == 0 {
        0.0
    } else {
        (diff_pixels as f64 / total_pixels as f64) * 100.0
    };

    let matched = diff_percentage <= tolerance;

    // Only encode diff image if there are actual differences.
    let diff_image = if diff_pixels > 0 {
        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        diff_img
            .write_to(&mut cursor, image::ImageFormat::Png)
            .ok();
        Some(png_bytes)
    } else {
        None
    };

    ComparisonResult {
        matched,
        diff_percentage,
        diff_pixels,
        total_pixels,
        diff_image,
    }
}

/// Resolve the platform-specific golden file path for a test name.
///
/// Returns `{golden_dir}/{test_name}.{platform}.png` where platform
/// is `macos` or `linux`.
pub fn golden_path(golden_dir: &Path, test_name: &str) -> PathBuf {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    golden_dir.join(format!("{test_name}.{platform}.png"))
}

/// Write the actual image as the new golden file (for `UPDATE_GOLDEN=1`).
pub fn update_golden(actual_png: &[u8], path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, actual_png)
        .unwrap_or_else(|e| panic!("failed to write golden file {}: {e}", path.display()));
}

/// Save failure artifacts (actual + diff) to the failures directory.
pub fn save_failure_artifacts(
    failures_dir: &Path,
    test_name: &str,
    actual_png: &[u8],
    result: &ComparisonResult,
) {
    std::fs::create_dir_all(failures_dir).ok();
    let actual_path = failures_dir.join(format!("{test_name}.actual.png"));
    std::fs::write(&actual_path, actual_png).ok();

    if let Some(diff_png) = &result.diff_image {
        let diff_path = failures_dir.join(format!("{test_name}.diff.png"));
        std::fs::write(&diff_path, diff_png).ok();
    }
}

/// Check if golden files should be updated (via `UPDATE_GOLDEN=1`).
pub fn should_update_golden() -> bool {
    std::env::var("UPDATE_GOLDEN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Read the tolerance override from `VISUAL_TEST_TOLERANCE`, if set.
pub fn env_tolerance() -> Option<f64> {
    std::env::var("VISUAL_TEST_TOLERANCE")
        .ok()
        .and_then(|v| v.parse().ok())
}

/// Convenience function: run a full visual comparison for a test.
///
/// Handles UPDATE_GOLDEN, platform-aware path resolution, artifact saving,
/// and returns the comparison result with a descriptive error message.
pub fn assert_visual_match(
    actual_png: &[u8],
    test_name: &str,
    project_root: &Path,
    tolerance: Option<f64>,
) -> ComparisonResult {
    let golden_dir = project_root.join("tests/visual/golden");
    let failures_dir = project_root.join("tests/visual/failures");
    let path = golden_path(&golden_dir, test_name);
    let tol = tolerance
        .or_else(env_tolerance)
        .unwrap_or(DEFAULT_TOLERANCE);

    if should_update_golden() {
        update_golden(actual_png, &path);
        return ComparisonResult {
            matched: true,
            diff_percentage: 0.0,
            diff_pixels: 0,
            total_pixels: 0,
            diff_image: None,
        };
    }

    let result = compare_with_golden(actual_png, &path, tol);

    if !result.matched {
        save_failure_artifacts(&failures_dir, test_name, actual_png, &result);
        panic!(
            "Visual regression: {test_name}\n\
             Golden: {}\n\
             Diff: {:.2}% ({} / {} pixels differ, tolerance: {:.2}%)\n\
             Failure artifacts saved to: {}",
            path.display(),
            result.diff_percentage,
            result.diff_pixels,
            result.total_pixels,
            tol,
            failures_dir.display(),
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let img: RgbaImage = ImageBuffer::from_fn(width, height, |_, _| Rgba(color));
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        buf
    }

    #[test]
    fn test_identical_images_match() {
        let red_png = solid_png(16, 16, [255, 0, 0, 255]);
        let golden_img = image::load_from_memory(&red_png).unwrap().to_rgba8();
        let actual_img = image::load_from_memory(&red_png).unwrap().to_rgba8();
        let result = compare_images(&actual_img, &golden_img, 0.0);
        assert!(result.matched);
        assert_eq!(result.diff_pixels, 0);
        assert_eq!(result.diff_percentage, 0.0);
        assert!(result.diff_image.is_none());
    }

    #[test]
    fn test_slightly_different_within_tolerance() {
        // Create two 10x10 images that differ by 1 pixel
        let mut img_a = RgbaImage::from_fn(10, 10, |_, _| Rgba([100, 100, 100, 255]));
        let img_b = RgbaImage::from_fn(10, 10, |_, _| Rgba([100, 100, 100, 255]));

        // Change 1 pixel (1% of 100 total)
        img_a.put_pixel(5, 5, Rgba([200, 0, 0, 255]));

        let result = compare_images(&img_a, &img_b, 2.0); // 2% tolerance
        assert!(result.matched, "1% diff should be within 2% tolerance");
        assert_eq!(result.diff_pixels, 1);
    }

    #[test]
    fn test_different_images_fail() {
        let red_png = solid_png(8, 8, [255, 0, 0, 255]);
        let blue_png = solid_png(8, 8, [0, 0, 255, 255]);
        let red_img = image::load_from_memory(&red_png).unwrap().to_rgba8();
        let blue_img = image::load_from_memory(&blue_png).unwrap().to_rgba8();

        let result = compare_images(&red_img, &blue_img, 0.5);
        assert!(!result.matched, "completely different images should not match");
        assert_eq!(result.diff_pixels, 64); // 8x8 = 64
        assert_eq!(result.diff_percentage, 100.0);
    }

    #[test]
    fn test_diff_image_generated_on_mismatch() {
        let mut img_a = RgbaImage::from_fn(4, 4, |_, _| Rgba([100, 100, 100, 255]));
        let img_b = RgbaImage::from_fn(4, 4, |_, _| Rgba([100, 100, 100, 255]));
        img_a.put_pixel(2, 2, Rgba([255, 0, 0, 255]));

        let result = compare_images(&img_a, &img_b, 0.0);
        assert!(!result.matched);
        assert!(result.diff_image.is_some(), "diff image should be generated");

        // Decode the diff image and verify the differing pixel is red
        let diff = image::load_from_memory(result.diff_image.as_ref().unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(diff.dimensions(), (4, 4));
        let changed_pixel = diff.get_pixel(2, 2);
        assert_eq!(changed_pixel, &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_dimension_mismatch_fails() {
        let img_a = RgbaImage::from_fn(10, 10, |_, _| Rgba([0, 0, 0, 255]));
        let img_b = RgbaImage::from_fn(10, 20, |_, _| Rgba([0, 0, 0, 255]));

        let result = compare_images(&img_a, &img_b, 100.0);
        assert!(!result.matched, "dimension mismatch should always fail");
    }

    #[test]
    fn test_pixel_threshold_ignores_minor_differences() {
        // Difference of 1 per channel should be within PIXEL_THRESHOLD
        let img_a = RgbaImage::from_fn(4, 4, |_, _| Rgba([100, 100, 100, 255]));
        let img_b = RgbaImage::from_fn(4, 4, |_, _| Rgba([101, 101, 101, 255]));

        let result = compare_images(&img_a, &img_b, 0.0);
        assert!(result.matched, "sub-threshold differences should be ignored");
        assert_eq!(result.diff_pixels, 0);
    }

    #[test]
    fn test_golden_path_platform_suffix() {
        let dir = Path::new("/tmp/golden");
        let path = golden_path(dir, "test_cursor");
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with(".macos.png") || path_str.ends_with(".linux.png"),
            "path should have platform suffix: {path_str}"
        );
    }
}
