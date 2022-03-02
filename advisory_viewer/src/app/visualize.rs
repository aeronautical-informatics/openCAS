use arc_swap::ArcSwap;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use eframe::egui::epaint::RectShape;
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::ops::{Bound, Deref, DerefMut, Range, RangeBounds, RangeInclusive};
use std::sync::{Arc, RwLock};

use eframe::egui::plot::{MarkerShape, Points, Polygon, Value, Values};
use eframe::egui::util::hash;
use eframe::egui::{Color32, Pos2, Rect};

use rayon::prelude::*;
use rayon::ScopeFifo;

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
    /// + `initial_grid_stride`: The distance between two points on the regular grid for the first
    ///   level of the quadtree
    /// + `x_range`: Range of x-values to be calculated
    /// + `y_range`: Range of y-values to be calculated
    fn get_points<F: 'static + FnMut(f32, f32, &AdvisoryViewerConfig) -> u8 + Send + Sync + Clone>(
        &mut self,
        f: F,
        x_range: RangeInclusive<f32>,
        y_range: RangeInclusive<f32>,
    ) -> Vec<Polygon>;
}

impl Visualizable for AdvisoryViewer {
    fn get_points<
        F: 'static + FnMut(f32, f32, &AdvisoryViewerConfig) -> u8 + Send + Sync + Clone,
    >(
        &mut self,
        f: F,
        x_range: RangeInclusive<f32>,
        y_range: RangeInclusive<f32>,
    ) -> Vec<Polygon> {
        let hash = hash(
            self.conf
                .input_values
                .values()
                .map(|v| v.to_string())
                .chain(vec![
                    self.conf.min_levels.to_string(),
                    self.conf.max_levels.to_string(),
                    self.conf.x_axis_key.clone(),
                    self.conf.y_axis_key.clone(),
                ])
                .fold(String::from(""), |p, v| p + &v),
        );
        let mut hash_lock = self.config_hash.write().unwrap();
        let old_hash = std::mem::replace(hash_lock.deref_mut(), hash);

        if hash != old_hash {
            let min_counter = self.min_level_counter.clone();
            min_counter.reset();
            let additional_counter = self.additional_quad_counter.clone();
            additional_counter.reset();
            let config = self.conf.clone();
            let tree_swap = self.visualizer_tree.clone();
            let current_hash = self.config_hash.clone();
            let this_hash = hash.clone();

            std::thread::spawn(move || {
                let tree = VisualizerNode {
                    value: 0,
                    x_range: x_range.clone(),
                    y_range: y_range.clone(),
                    children: Default::default(),
                }.gen_value(f.clone(), &config);
                {
                    let current_hash = current_hash.read().unwrap();
                    if *current_hash != this_hash {
                        return;
                    }
                    tree_swap.store(Arc::new(tree.clone()))
                }

                tree.gen_children_rec(
                    f.clone(),
                    config.min_levels,
                    current_hash.clone(),
                    this_hash,
                    &min_counter,
                    &config,
                );

                for level in 0..config.max_levels{
                    tree.level_nodes(level).par_iter()
                        .filter(|_| *current_hash.read().unwrap() == this_hash)
                        .filter(|n| n.children.load().is_none())
                        .filter(|n| !n.corners_are_identical(f.clone(), &config))
                        .for_each(|n| {
                            n.gen_children(f.clone(), &config);
                            additional_counter.add(4);
                        });
                }
            });
        }

        drop(hash_lock);

        self.visualizer_tree
            .load()
            .generate_polygons(&self.conf.output_variants)
    }
}

struct Item {
    point: [f32; 2],
    value: u8,
}

#[derive(Clone)]
pub struct VisualizerNode {
    value: u8,
    x_range: RangeInclusive<f32>,
    y_range: RangeInclusive<f32>,
    children: Arc<ArcSwap<Option<[VisualizerNode; 4]>>>,
}

impl VisualizerNode {
    fn generate_polygons(&self, output: &HashMap<u8, (String, Color32)>) -> Vec<Polygon> {
        if let Some(children) = self.children.load_full().deref() {
            children
                .iter()
                .flat_map(|c| c.generate_polygons(output))
                .collect()
        } else {
            if let Some((_, c)) = output.get(&self.value).cloned() {
                vec![Polygon::new(Values::from_values(vec![
                    Value::new(*self.x_range.start(), *self.y_range.start()),
                    Value::new(*self.x_range.start(), *self.y_range.end()),
                    Value::new(*self.x_range.end(), *self.y_range.end()),
                    Value::new(*self.x_range.end(), *self.y_range.start()),
                ]))
                .color(c)
                .fill_alpha(1.0)]
            } else {
                vec![]
            }
        }
    }

    fn level_nodes(&self, level: usize) -> Vec<VisualizerNode>{
        if level == 0 {
            vec![self.clone()]
        }else {
            if let Some(children) = self.children.load_full().deref().clone() {
                children.into_par_iter().flat_map(|c| c.level_nodes(level - 1)).collect()
            } else {
                vec![]
            }
        }
    }

    fn gen_value<F: FnMut(f32, f32, &AdvisoryViewerConfig) -> u8>(
        mut self,
        mut f: F,
        config: &AdvisoryViewerConfig,
    ) -> Self {
        let mid_x = (self.x_range.end() + self.x_range.start()) / 2f32;
        let mid_y = (self.y_range.end() + self.y_range.start()) / 2f32;
        self.value = f(mid_x, mid_y, config);
        self
    }

    fn gen_children_rec<F: FnMut(f32, f32, &AdvisoryViewerConfig) -> u8 + Send + Sync + Clone>(
        &self,
        f: F,
        level: usize,
        current_hash: Arc<RwLock<u64>>,
        this_hash: u64,
        counter: &RelaxedCounter,
        config: &AdvisoryViewerConfig,
    ) {
        if level > 0 {
            self.gen_children(f.clone(), config);
            if let Some(mut c) = self.children.load_full().deref().clone() {
                c.par_iter_mut().for_each(|c| {
                    if *current_hash.read().unwrap() != this_hash {
                        return;
                    }
                    c.gen_children_rec(
                        f.clone(),
                        level - 1,
                        current_hash.clone(),
                        this_hash,
                        counter,
                        config,
                    );
                    c.simplify()
                });
                self.children.store(Arc::new(Some(c)));
            }
        } else {
            counter.inc();
        }
    }

    fn gen_children<F: FnMut(f32, f32, &AdvisoryViewerConfig) -> u8 + Clone>(
        &self,
        f: F,
        config: &AdvisoryViewerConfig,
    ) {
        let mid_x = (self.x_range.end() + self.x_range.start()) / 2f32;
        let mid_y = (self.y_range.end() + self.y_range.start()) / 2f32;

        let bl = VisualizerNode {
            value: 0,
            x_range: *self.x_range.start()..=mid_x,
            y_range: *self.y_range.start()..=mid_y,
            children: Default::default(),
        };
        let tl = VisualizerNode {
            value: 0,
            x_range: *self.x_range.start()..=mid_x,
            y_range: mid_y..=*self.y_range.end(),
            children: Default::default(),
        };
        let tr = VisualizerNode {
            value: 0,
            x_range: mid_x..=*self.x_range.end(),
            y_range: mid_y..=*self.y_range.end(),
            children: Default::default(),
        };
        let br = VisualizerNode {
            value: 0,
            x_range: mid_x..=*self.x_range.end(),
            y_range: *self.y_range.start()..=mid_y,
            children: Default::default(),
        };
        self.children.store(Arc::new(Some(
            [bl, tl, tr, br].map(|c| c.gen_value(f.clone(), config)),
        )));
    }

    fn corners_are_identical<F: FnMut(f32, f32, &AdvisoryViewerConfig) -> u8 + Clone>(
        &self,
        f: F,
        config: &AdvisoryViewerConfig,
    ) -> bool {
        let bl = f.clone()(*self.x_range.start(), *self.y_range.start(), config);
        let tl = f.clone()(*self.x_range.start(), *self.y_range.end(), config);
        let tr = f.clone()(*self.x_range.end(), *self.y_range.end(), config);
        let br = f.clone()(*self.x_range.end(), *self.y_range.start(), config);
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
            children: Default::default(),
        }
    }
}
