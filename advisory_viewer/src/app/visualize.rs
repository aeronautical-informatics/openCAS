use arc_swap::ArcSwap;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use std::collections::HashMap;
use std::ops::{Deref, RangeInclusive, Range};
use std::sync::{Arc, RwLock};

use eframe::egui::{Color32, ColorImage, TextureHandle};
use ndarray::{Array2, Array3, ArrayBase};

use rayon::prelude::*;
use crate::app::AdvisoryViewerConfig;

use super::AdvisoryViewer;

pub trait Visualizable {
    fn start_with(&self, config: AdvisoryViewerConfig, texture: TextureHandle, f: Box<dyn Fn(f32, f32)-> u8 + Send + Sync>);
    // ((done minimal quads, target minimal quads), extra quads)
    fn get_status(&self)->((usize, usize), usize);
}

impl Visualizable for AdvisoryViewer {
    fn start_with(&self, config: AdvisoryViewerConfig, texture: TextureHandle, f: Box<dyn Fn(f32, f32)-> u8 + Send + Sync>) {
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
        let tree = Arc::new(VisualizerNode {
            value: 0,
            x_range: config.x_axis_range,
            y_range: config.y_axis_range,
            x_pixel_range: 0..side_length,
            y_pixel_range: 0..side_length,
            children: Default::default(),
        }.gen_value(&f));
        self.visualizer_tree.store(tree.clone());

        drop(prev_lock);
        drop(this_lock);

        let mut image_buffer: Array2<Color32> = Array2::default((side_length, side_length));
        image_buffer.exact_chunks_mut((2, 2)).into_iter().collect::<Vec<_>>().par_iter_mut().for_each(|_| {});

        std::thread::spawn(move || {
            tree.gen_children_rec(
                &f,
                config.min_levels,
                &this,
                &min_level_counter,
            );

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
            }
        });

        //let length = 2usize.pow(self.conf.max_levels as u32);
        //let mut buffer: Vec<_> = (0..(length*length)).flat_map(|_| Color32::RED.to_srgba_unmultiplied() ).collect();
        //for level in 0..=self.conf.max_levels {
        //    self.visualizer_tree.load().level_nodes(level)
        //        .iter()
        //        .for_each(|n| {
        //            //let image_data = n.gen_image(&self.conf.output_variants);
        //            //texture.set_partial(
        //            //    [*n.x_pixel_range.start(), *n.y_pixel_range.start()],
        //            //    image_data,
        //            //);
        //            let color = self.conf.output_variants.get(&n.value)
        //                .map(|(_, c)| c.clone()).unwrap_or_else(|| Color32::TRANSPARENT);
        //            for y in *n.y_pixel_range.start()..*n.y_pixel_range.end(){
        //                for x in *n.x_pixel_range.start()..*n.x_pixel_range.end(){
        //                    *buffer.get_mut((x+y*length)*4).unwrap() = color.to_srgba_unmultiplied()[0];
        //                    *buffer.get_mut((x+y*length)*4+1).unwrap() = color.to_srgba_unmultiplied()[1];
        //                    *buffer.get_mut((x+y*length)*4+2).unwrap() = color.to_srgba_unmultiplied()[2];
        //                    *buffer.get_mut((x+y*length)*4+3).unwrap() = color.to_srgba_unmultiplied()[3];
        //                }
        //            }
        //        })
        //}
        //texture.set(ColorImage::from_rgba_unmultiplied([length, length], buffer.as_slice()));
        //texture
    }

    fn get_status(&self) -> ((usize, usize), usize) {
        todo!()
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
    fn fill_buffer(&self, buffer: Box<dyn ArrayViewMut>, output: &AdvisoryViewerConfig) {
        if let Some(children) = self.children.load_full().deref() {
            children
                .iter()
                .flat_map(|c| c.generate_polygons(output))
                .collect()
        } else {
            if let Some((_, c)) = output.get(&self.value).cloned() {

            } else {
                vec![]
            }
        }
    }

    fn gen_image(&self, output: &HashMap<u8, (String, Color32)>) -> ColorImage {
        let color = output
            .get(&self.value)
            .map(|(_, c)| c.clone())
            .unwrap_or_else(|| Color32::TRANSPARENT);
        ColorImage::new(
            [
                self.x_pixel_range.end - self.x_pixel_range.start,
                self.y_pixel_range.end - self.y_pixel_range.start,
            ],
            color,
        )
    }

    fn level_nodes(&self, level: usize) -> Vec<VisualizerNode> {
        if level == 0 {
            vec![self.clone()]
        } else {
            if let Some(children) = self.children.load_full().deref().clone() {
                children
                    .into_par_iter()
                    .flat_map(|c| c.level_nodes(level - 1))
                    .collect()
            } else {
                vec![]
            }
        }
    }

    fn gen_value(
        mut self,
        f: &Box<dyn Fn(f32, f32)-> u8 + Send + Sync>
    ) -> Self {
        let mid_x = (self.x_range.end() + self.x_range.start()) / 2f32;
        let mid_y = (self.y_range.end() + self.y_range.start()) / 2f32;
        self.value = f(mid_x, mid_y);
        self
    }

    fn gen_children_rec(
        &self,
        f: &Box<dyn Fn(f32, f32)-> u8 + Send + Sync>,
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
                    c.gen_children_rec(
                        &f,
                        level - 1,
                        valid,
                        counter,
                    );
                    c.simplify()
                });
                self.children.store(Arc::new(Some(c)));
            }
        } else {
            counter.inc();
        }
    }

    fn gen_children(
        &self,
        f: &Box<dyn Fn(f32, f32)-> u8 + Send + Sync>
    ) {
        let mid_x = (self.x_range.end() + self.x_range.start()) / 2f32;
        let mid_y = (self.y_range.end() + self.y_range.start()) / 2f32;
        let mid_x_pixel = (self.x_pixel_range.end + self.x_pixel_range.start) / 2;
        let mid_y_pixel = (self.y_pixel_range.end + self.y_pixel_range.start) / 2;

        let bl = VisualizerNode {
            value: 0,
            x_range: *self.x_range.start()..=mid_x,
            y_range: *self.y_range.start()..=mid_y,
            x_pixel_range: self.x_pixel_range.start..mid_x_pixel,
            y_pixel_range: self.y_pixel_range.start..mid_y_pixel,
            children: Default::default(),
        };
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
        let br = VisualizerNode {
            value: 0,
            x_range: mid_x..=*self.x_range.end(),
            y_range: *self.y_range.start()..=mid_y,
            x_pixel_range: mid_x_pixel..self.x_pixel_range.end,
            y_pixel_range: self.y_pixel_range.start..mid_y_pixel,
            children: Default::default(),
        };
        self.children.store(Arc::new(Some(
            [bl, tl, tr, br].map(|c| c.gen_value(f)),
        )));
    }

    fn corners_are_identical(
        &self,
        f: &Box<dyn Fn(f32, f32)-> u8 + Send + Sync>
    ) -> bool {
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
