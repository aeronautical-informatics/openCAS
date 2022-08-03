use arc_swap::ArcSwap;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use std::ops::{Deref, DerefMut, Range, RangeInclusive};
use std::sync::{Arc, RwLock};

use egui::{Color32, ColorImage, TextureFilter, TextureHandle};
use uuid::Uuid;

use super::ViewerConfig;

pub struct Status {
    pub current_level: usize,
    pub quads_evaluated: usize,
}

pub struct VisualizerBackend {
    pub conf: ArcSwap<Option<ViewerConfig>>,
    #[cfg_attr(feature = "persistence", serde(skip))]
    data: Arc<RwLock<(VisualizerNode, Uuid)>>,
    #[cfg_attr(feature = "persistence", serde(skip))]
    quad_counter: Arc<RelaxedCounter>,
    level_done: Arc<ArcSwap<usize>>,
}

impl Default for VisualizerBackend {
    fn default() -> Self {
        Self {
            conf: Default::default(),
            data: Arc::new(RwLock::new((VisualizerNode::default(), Uuid::new_v4()))),
            quad_counter: Default::default(),
            level_done: Default::default(),
        }
    }
}

impl VisualizerBackend {
    pub fn start_with(
        &self,
        config: ViewerConfig,
        mut texture: TextureHandle,
        f: Box<dyn Fn(f32, f32) -> u8 + Send + Sync>,
    ) {
        let uuid = Uuid::new_v4();
        let data = self.data.clone();
        let quad_counter = self.quad_counter.clone();
        let level_done = self.level_done.clone();
        self.conf.store(Arc::new(Some(config.clone())));

        let thread = async move {
            let mut lock = data.write().unwrap();
            (*lock).1 = uuid;

            quad_counter.reset();
            let side_length = 2usize.pow(config.max_levels as u32);
            drop(lock);

            let check_data = data.clone();
            let check_uuid = uuid;
            let valid = move || check_data.read().unwrap().deref().1 == check_uuid;

            let tree = VisualizerNode {
                value: 0,
                x_range: config.x_axis_range.clone(),
                y_range: config.y_axis_range.clone(),
                x_pixel_range: 0..side_length,
                y_pixel_range: 0..side_length,
                children: Default::default(),
            }
            .gen_value(&f);

            {
                let mut lock = data.write().unwrap();
                if lock.deref().1 != uuid {
                    return;
                }
                lock.deref_mut().0 = tree.clone();
                texture.set(
                    ColorImage::new([side_length, side_length], Color32::TRANSPARENT),
                    TextureFilter::Nearest,
                );
                level_done.store(Arc::new(1));
            }

            tree.gen_children_rec(&f, config.min_levels, &valid, &quad_counter);

            {
                let lock = data.read().unwrap();
                if lock.deref().1 != uuid {
                    return;
                }
                tree.set_partially(&texture, &config);
            }

            for level in 0..config.max_levels {
                if !valid() {
                    return;
                }
                tree.level_nodes(level)
                    .iter()
                    .filter(|n| n.children.load().is_none())
                    .filter(|n| !n.corners_are_identical(&f))
                    .for_each(|n| {
                        n.gen_children(&f);
                        quad_counter.add(4);
                    });
                {
                    let lock = data.read().unwrap();
                    if lock.deref().1 != uuid {
                        return;
                    }
                    tree.set_partially(&texture, &config);
                    level_done.store(Arc::new(level + 1));
                }
            }
        };
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(thread);
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(|| futures::executor::block_on(thread));
    }

    pub fn get_status(&self) -> Status {
        Status {
            current_level: *self.level_done.load_full(),
            quads_evaluated: self.quad_counter.get(),
        }
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
    fn set_partially(&self, texture: &TextureHandle, config: &ViewerConfig) {
        if let Some(children) = self.children.load_full().deref() {
            children
                .iter()
                .for_each(|n| n.set_partially(texture, config));
        } else {
            let color = config
                .output_variants
                .get(self.value as usize)
                .map(|(_, c)| *c)
                .unwrap_or_else(|| Color32::TRANSPARENT);
            let size = texture.size();
            texture.clone().set_partial(
                [self.x_pixel_range.start, size[1] - self.y_pixel_range.end],
                ColorImage::new([self.x_pixel_range.len(), self.y_pixel_range.len()], color),
                egui::TextureFilter::Nearest,
            );
        }
    }

    fn level_nodes(&self, level: usize) -> Vec<VisualizerNode> {
        if level == 0 {
            vec![self.clone()]
        } else if let Some(children) = self.children.load_full().deref().clone() {
            children
                .into_iter()
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
        valid: &(dyn Fn() -> bool + Send + Sync),
        counter: &RelaxedCounter,
    ) {
        if level > 0 {
            self.gen_children(f);
            if let Some(mut c) = self.children.load_full().deref().clone() {
                if !valid() {
                    return;
                }
                c.iter_mut().for_each(|c| {
                    c.gen_children_rec(f, level - 1, valid, counter);
                    c.simplify()
                });
                self.children.store(Arc::new(Some(c)));
            }
        } else if valid() {
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
            return c.iter().all(|c| c.self_and_descendants_are(v));
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
