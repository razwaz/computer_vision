use cv::{
    bitarray::{BitArray, Hamming},
    feature::akaze::Akaze,
    image::{
        image::{self, DynamicImage, GenericImageView, Rgba, RgbaImage},
        imageproc::drawing,
    },
    knn::{Knn, LinearKnn},
};
use imageproc::pixelops;
use itertools::Itertools;
use palette::{FromColor, Hsv, RgbHue, Srgb};

use rayon::prelude::*;

use std::thread;

fn main() {
    let src_image = image::open("./Data/image11.png")
        .expect("Failed to open image file");
    println!("First file opened");
    let dst_image = image::open("./Data/image22.png")
        .expect("Failed to open image file");
    println!("Second file opened");

    let akaze = Akaze::default();

    let (key_points_src, descriptors_src) = akaze.extract(&src_image);
    println!("First file keypoint and descriptors");
    let (key_points_dst, descriptors_dst) = akaze.extract(&dst_image);
    println!("Second file keypoint and descriptors");
    let matches = symmetric_matching(&descriptors_src, &descriptors_dst);
    println!("Images matched");

    let canvas_width = src_image.dimensions().0 + dst_image.dimensions().0;
    let canvas_height = std::cmp::max(src_image.dimensions().1, dst_image.dimensions().1);

    println!("Canvas created");
    let rgba_image_src = src_image.to_rgba8();
    println!("First image to rgba8");
    let rgba_image_dst = dst_image.to_rgba8();
    println!("Second image to rgba8");
    let mut canvas = RgbaImage::from_pixel(canvas_width, canvas_height, Rgba([0, 0, 0, 255]));

    let mut render_image_onto_canvas_x_offset = |image: &RgbaImage, x_offset: u32| {
        let (width, height) = image.dimensions();
        for (x, y) in (0..width).cartesian_product(0..height) {
            canvas.put_pixel(x + x_offset, y, *image.get_pixel(x, y));
        }
    };

    println!("Images rendered to canvas");

    render_image_onto_canvas_x_offset(&rgba_image_src, 0);
    render_image_onto_canvas_x_offset(&rgba_image_dst, rgba_image_src.dimensions().0);

    for (ix, &[kpa, kpb]) in matches.iter().enumerate() {
        // Compute a color by rotating through a color wheel on only the most saturated colors.
        let ix = ix as f64;
        let hsv = Hsv::new(RgbHue::from_radians(ix * 0.1), 1.0, 1.0);
        let rgb = Srgb::from_color(hsv);

        // Draw the line between the keypoints in the two images.
        let point_to_i32_tup =
            |point: (f32, f32), off: u32| (point.0 as i32 + off as i32, point.1 as i32);
        drawing::draw_antialiased_line_segment_mut(
            &mut canvas,
            point_to_i32_tup(key_points_src[kpa].point, 0),
            point_to_i32_tup(key_points_dst[kpb].point, rgba_image_src.dimensions().0),
            Rgba([
                (rgb.red * 255.0) as u8,
                (rgb.green * 255.0) as u8,
                (rgb.blue * 255.0) as u8,
                255,
            ]),
            pixelops::interpolate,
        );
    }

    println!("Lines drawn on image");

    let out_image = DynamicImage::ImageRgba8(canvas);

    let image_file_path = tempfile::Builder::new()
        .suffix(".png")
        .tempfile()
        .unwrap()
        .into_temp_path();
    out_image.save(&image_file_path).unwrap();

    open::that(&image_file_path).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(600));
}

/// This function performs non-symmetric matching from a to b.
fn matching(a_descriptors: &[BitArray<64>], b_descriptors: &[BitArray<64>]) -> Vec<Option<usize>> {
    let knn_b = LinearKnn {
        metric: Hamming,
        iter: b_descriptors.iter(),
    };
    (0..a_descriptors.len())
        .into_par_iter()
        .map(|a_feature| {
            let knn = knn_b.knn(&a_descriptors[a_feature], 2);
            if knn[0].distance + 24 < knn[1].distance {
                Some(knn[0].index)
            } else {
                None
            }
        })
        .collect()
}

/// This function performs symmetric matching between `a` and `b`.
///
/// Symmetric matching requires a feature in `b` to be the best match for a feature in `a`
/// and for the same feature in `a` to be the best match for the same feature in `b`.
/// The feature that a feature matches to in one direction might not be reciprocated.
/// Consider a 1d line. Three features are in a line `X`, `Y`, and `Z` like `X---Y-Z`.
/// `Y` is closer to `Z` than to `X`. The closest match to `X` is `Y`, but the closest
/// match to `Y` is `Z`. Therefore `X` and `Y` do not match symmetrically. However,
/// `Y` and `Z` do form a symmetric match, because the closest point to `Y` is `Z`
/// and the closest point to `Z` is `Y`.
///
/// Symmetric matching is very important for our purposes and gives stronger matches.
fn symmetric_matching(a: &[BitArray<64>], b: &[BitArray<64>]) -> Vec<[usize; 2]> {
    // The best match for each feature in frame a to frame b's features.
    let forward_matches = matching(a, b);
    println!("Forward match done");
    // The best match for each feature in frame b to frame a's features.
    let reverse_matches = matching(b, a);
    println!("Reverse match done");
    forward_matches
        .into_par_iter()
        .enumerate()
        .filter_map(move |(aix, bix)| {
            // First we only proceed if there was a sufficient bix match.
            // Filter out matches which are not symmetric.
            // Symmetric is defined as the best and sufficient match of a being b,
            // and likewise the best and sufficient match of b being a.
            bix.map(|bix| [aix, bix])
                .filter(|&[aix, bix]| reverse_matches[bix] == Some(aix))
        })
        .collect()
}

