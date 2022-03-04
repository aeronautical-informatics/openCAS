use arc_swap::ArcSwap;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use std::ops::{Deref, Range, RangeInclusive};
use std::sync::{Arc, RwLock};

use eframe::egui::{Color32, ColorImage, TextureHandle};
use ndarray::{Array2, ArrayViewMut2, Axis};

use crate::app::AdvisoryViewerConfig;
use rayon::prelude::*;

use super::AdvisoryViewer;

pub trait Visualizable {
    fn start_with(
        &self,
        config: AdvisoryViewerConfig,
        texture: TextureHandle,
        f: Box<dyn Fn(f32, f32) -> u8 + Send + Sync>,
    );
    // ((done minimal quads, target minimal quads), extra quads)
    fn get_status(&self) -> ((usize, usize), usize);
}

impl Visualizable for AdvisoryViewer {
    fn start_with(
        &self,
        config: AdvisoryViewerConfig,
        mut texture: TextureHandle,
        f: Box<dyn Fn(f32, f32) -> u8 + Send + Sync>,
    ) {
        let prev = self.valid.load_full();
        let mut prev_lock = prev.write().unwrap();
        let this = Arc::new(RwLock::new(true));
        let this_lock = this.write().unwrap();
        self.valid.store(this.clone());
        *prev_lock = false;

        let min_level_counter = self.min_level_counter.clone();
        min_level_counter.reset();
        let additional_quad_counter = self.additional_quad_counter.clone();
        additional_quad_counter.reset();
        let side_length = 2usize.pow(config.max_levels as u32);
        let tree = Arc::new(
            VisualizerNode {
                value: 0,
                x_range: config.x_axis_range.clone(),
                y_range: config.y_axis_range.clone(),
                x_pixel_range: 0..side_length,
                y_pixel_range: 0..side_length,
                children: Default::default(),
            }
            .gen_value(&f),
        );
        self.visualizer_tree.store(tree.clone());

        drop(prev_lock);
        drop(this_lock);

        let mut image_buffer: Array2<Color32> = Array2::default((side_length, side_length));

        std::thread::spawn(move || {
            tree.gen_children_rec(&f, config.min_levels, &this, &min_level_counter);
            tree.fill_buffer(image_buffer.view_mut(), &config);
            texture.set(ColorImage::from_rgba_unmultiplied(
                [side_length, side_length],
                &image_buffer
                    .par_iter()
                    .flat_map(|c| c.to_srgba_unmultiplied())
                    .collect::<Vec<_>>(),
            ));

            for level in 0..config.max_levels {
                tree.level_nodes(level)
                    .par_iter()
                    .filter(|n| n.children.load().is_none())
                    .filter(|n| !n.corners_are_identical(&f))
                    .filter(|_| *this.read().unwrap())
                    .for_each(|n| {
                        n.gen_children(&f);
                        additional_quad_counter.add(4);
                    });
                tree.fill_buffer(image_buffer.view_mut(), &config);
            }
        });
    }

    fn get_status(&self) -> ((usize, usize), usize) {
        let min_quads_target = 2usize.pow(self.conf.min_levels as u32).pow(2);
        ((self.min_level_counter.get(), min_quads_target), self.additional_quad_counter.get())
    }
}

#[derive(Clone)]
pub struct VisualizerNode {
    value: u8,
    x_range: RangeInclusive<f32>,
    y_range: RangeInclusive<f32>,
    x_pixel_range: Range<usize>,
    y_pixel_range: Range<usize>,
    children: Arc<ArcSwap<Option<[VisualizerNode; 4]>>>,
}

impl VisualizerNode {
    fn fill_buffer(&self, mut buffer: ArrayViewMut2<'_, Color32>, config: &AdvisoryViewerConfig) {
        if let Some(children) = self.children.load_full().deref() {
            let len = buffer.len_of(Axis(0)) / 2;
            let buffers = buffer
                .exact_chunks_mut((len, len))
                .into_iter()
                .collect::<Vec<_>>();
            children
                .into_par_iter()
                .zip(buffers)
                .for_each(|(n, b)| n.fill_buffer(b, config));
        } else {
            buffer.fill(
                config
                    .output_variants
                    .get(&self.value)
                    .map(|(_, c)| *c)
                    .unwrap_or_else(|| Color32::TRANSPARENT),
            );
        }
    }

    fn level_nodes(&self, level: usize) -> Vec<VisualizerNode> {
        if level == 0 {
            vec![self.clone()]
        } else if let Some(children) = self.children.load_full().deref().clone() {
            children
                .into_par_iter()
                .flat_map(|c| c.level_nodes(level - 1))
                .collect()
        } else {
            vec![]
        }
    }

    fn gen_value(mut self, f: &(dyn Fn(f32, f32) -> u8 + Send + Sync)) -> Self {
        let mid_x = (self.x_range.end() + self.x_range.start()) / 2f32;
        let mid_y = (self.y_range.end() + self.y_range.start()) / 2f32;
        self.value = f(mid_x, mid_y);
        self
    }

    fn gen_children_rec(
        &self,
        f: &(dyn Fn(f32, f32) -> u8 + Send + Sync),
        level: usize,
        valid: &RwLock<bool>,
        counter: &RelaxedCounter,
    ) {
        if level > 0 {
            self.gen_children(f);
            if let Some(mut c) = self.children.load_full().deref().clone() {
                c.par_iter_mut().for_each(|c| {
                    if !*valid.read().unwrap() {
                        return;
                    }
                    c.gen_children_rec(f, level - 1, valid, counter);
                    c.simplify()
                });
                self.children.store(Arc::new(Some(c)));
            }
        } else {
            counter.inc();
        }
    }

    fn gen_children(&self, f: &(dyn Fn(f32, f32) -> u8 + Send + Sync)) {
        let mid_x = (self.x_range.end() + self.x_range.start()) / 2f32;
        let mid_y = (self.y_range.end() + self.y_range.start()) / 2f32;
        let mid_x_pixel = (self.x_pixel_range.end + self.x_pixel_range.start) / 2;
        let mid_y_pixel = (self.y_pixel_range.end + self.y_pixel_range.start) / 2;

        let tl = VisualizerNode {
            value: 0,
            x_range: *self.x_range.start()..=mid_x,
            y_range: mid_y..=*self.y_range.end(),
            x_pixel_range: self.x_pixel_range.start..mid_x_pixel,
            y_pixel_range: mid_y_pixel..self.y_pixel_range.end,
            children: Default::default(),
        };
        let tr = VisualizerNode {
            value: 0,
            x_range: mid_x..=*self.x_range.end(),
            y_range: mid_y..=*self.y_range.end(),
            x_pixel_range: mid_x_pixel..self.x_pixel_range.end,
            y_pixel_range: mid_y_pixel..self.y_pixel_range.end,
            children: Default::default(),
        };
        let bl = VisualizerNode {
            value: 0,
            x_range: *self.x_range.start()..=mid_x,
            y_range: *self.y_range.start()..=mid_y,
            x_pixel_range: self.x_pixel_range.start..mid_x_pixel,
            y_pixel_range: self.y_pixel_range.start..mid_y_pixel,
            children: Default::default(),
        };
        let br = VisualizerNode {
            value: 0,
            x_range: mid_x..=*self.x_range.end(),
            y_range: *self.y_range.start()..=mid_y,
            x_pixel_range: mid_x_pixel..self.x_pixel_range.end,
            y_pixel_range: self.y_pixel_range.start..mid_y_pixel,
            children: Default::default(),
        };
        self.children
            .store(Arc::new(Some([tl, tr, bl, br].map(|c| c.gen_value(f)))));
    }

    fn corners_are_identical(&self, f: &(dyn Fn(f32, f32) -> u8 + Send + Sync)) -> bool {
        let bl = f(*self.x_range.start(), *self.y_range.start());
        let tl = f(*self.x_range.start(), *self.y_range.end());
        let tr = f(*self.x_range.end(), *self.y_range.end());
        let br = f(*self.x_range.end(), *self.y_range.start());
        [bl, tl, tr, br].iter().all(|a| a.eq(&self.value))
    }

    fn simplify(&mut self) {
        if self.self_and_descendants_are(self.value) {
            self.children.store(Default::default());
        } /* else {
              if let Some(c) = self.children.as_deref_mut(){
                  c.par_iter_mut().for_each(|c| c.simplify());
              }
          } */
    }

    fn self_and_descendants_are(&self, v: u8) -> bool {
        if self.value != v {
            return false;
        }
        if let Some(c) = self.children.load_full().deref() {
            return c.into_par_iter().all(|c| c.self_and_descendants_are(v));
        }
        true
    }
}

impl Default for VisualizerNode {
    fn default() -> Self {
        VisualizerNode {
            value: 255,
            x_range: 0.0..=0.0,
            y_range: 0.0..=0.0,
            x_pixel_range: 0..0,
            y_pixel_range: 0..0,
            children: Default::default(),
        }
    }
}
