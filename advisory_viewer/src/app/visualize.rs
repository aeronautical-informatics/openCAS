use std::ops::RangeBounds;

use eframe::egui::plot::Points;

use super::{AdvisoryViewer, AdvisoryViewerConfig};

pub trait Visualizable {
    /// Returns a `Vec` of `Points`.
    ///
    /// There must be zero or one instance of `Points` for every combination of Level in the
    /// quadtree and output_variant. When there are 3 levels in the quadtree and 5 different output
    /// values, than the return value should be a Vec of up to 15 elements. It is possible however,
    /// that less than 15 elements are present, if one output value never occurs in that level of
    /// the quadtree.
    ///
    /// # Arguments
    /// + `f`: The actual function which maps the input_values to one of the output_variants
    /// + `initial_grid_strid`: The distance between two points on the regular grid for the first
    ///   level of the quadtree
    /// + `x_range`: Range of x-values to be calculated
    /// + `y_range`: Range of y-values to be calculated
    fn get_points<
        F: FnMut(f32, f32, &AdvisoryViewerConfig) -> u8,
        X: RangeBounds<f32>,
        Y: RangeBounds<f32>,
    >(
        &mut self,
        f: F,
        x_range: X,
        y_range: Y,
    ) -> Vec<Points>;
}

impl Visualizable for AdvisoryViewer {
    fn get_points<
        F: FnMut(f32, f32, &AdvisoryViewerConfig) -> u8,
        X: RangeBounds<f32>,
        Y: RangeBounds<f32>,
    >(
        &mut self,
        f: F,
        x_range: X,
        y_range: Y,
    ) -> Vec<Points> {
        todo!()
    }
}
