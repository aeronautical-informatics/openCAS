use std::{collections::BTreeMap, hash::Hash, ops::RangeInclusive};

use egui::{
    self, plot::Plot, plot::PlotImage, Align, Color32, ColorImage, DragValue, ProgressBar,
    TextureFilter, TextureHandle,
};

use serde::{Deserialize, Serialize};

mod visualize;
use visualize::VisualizerBackend;

use self::{hcas::HCasCartesianGui, vcas::VCasGui, visualize::Status};

mod hcas;
mod vcas;

type ViewerFn = Box<dyn Fn(f32, f32) -> u8 + Send + Sync>;

trait Visualizable {
    /// draw the config panel for this visualizable
    /// returns true, if a significant value changed
    fn draw_config(&mut self, ui: &mut egui::Ui) -> bool;

    /// Get the function used to color the texture by the visualizer
    fn get_fn(&self) -> ViewerFn;

    fn get_viewer_config(&self) -> ViewerConfig;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewerConfig {
    /// the output value -> Color mapping
    /// For the example of the H-CAS, these are: CoC, WL, WR, SL, SR
    pub output_variants: Vec<(String, Color32)>,

    /// From where to where to render on the x-axis
    pub x_axis_range: RangeInclusive<f32>,

    /// From where to where to render on the y-axis
    pub y_axis_range: RangeInclusive<f32>,

    /// Initial level of detail to redner
    pub min_levels: usize,

    /// Maximum level of detail to render
    pub max_levels: usize,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct TemplateApp {
    #[serde(skip)]
    viewers: BTreeMap<String, Box<dyn Visualizable>>,

    viewer_key: String,

    #[serde(skip)]
    last_viewer_config: Option<ViewerConfig>,

    #[serde(skip)]
    backend: VisualizerBackend,

    #[serde(skip)]
    texture_handle: Option<TextureHandle>,
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl Default for TemplateApp {
    fn default() -> Self {
        let viewers: BTreeMap<String, Box<dyn Visualizable>> = [
            (
                "HCAS Cartesian".into(),
                Box::new(HCasCartesianGui::default()) as _,
            ),
            ("VCAS".into(), Box::new(VCasGui::default()) as _),
        ]
        .into_iter()
        .collect();
        let viewer_key = viewers.keys().next().unwrap().into();

        Self {
            viewers,
            viewer_key,
            backend: Default::default(),
            last_viewer_config: None,
            texture_handle: None,
        }
    }
}

impl eframe::App for TemplateApp {
    /// Set the maximum size of the canvas for WebGL based renderers
    fn max_size_points(&self) -> eframe::egui::Vec2 {
        eframe::egui::Vec2 {
            x: f32::MAX,
            y: f32::MAX,
        }
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Choose Viewee", |ui| {
                    for k in self.viewers.keys() {
                        if ui.button(k).clicked() {
                            self.viewer_key = k.into();
                        }
                    }
                });
                ui.menu_button("Theme", |ui| {
                    if ui.button("Dark").clicked() {
                        ctx.set_visuals(egui::Visuals::dark());
                    }
                    if ui.button("Light").clicked() {
                        ctx.set_visuals(egui::Visuals::light());
                    }
                });
            });
        });

        let viewer_key = &self.viewer_key;
        let viewer = self.viewers.get_mut(viewer_key).unwrap();

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(format!("Settings for {viewer_key}"));
                if viewer.draw_config(ui) {
                    self.last_viewer_config = None;
                };
            });
        });

        // show progressbar
        let Status {
            quads_evaluated,
            current_level,
        } = self.backend.get_status();
        let ViewerConfig {
            min_levels,
            max_levels,
            ..
        } = viewer.get_viewer_config();
        let initial_quads = 4f32.powi(min_levels as i32);

        // repaint until all levels are done
        if current_level != max_levels {
            ctx.request_repaint();
        }

        let progress = quads_evaluated as f32 / initial_quads as f32;
        egui::TopBottomPanel::bottom("my_panel").show(ctx, |ui| {
            let text = match current_level {
                x if (0..=min_levels).contains(&x) => {
                    format!(
                        "calculating initial grid {:3.0} %",
                        quads_evaluated as f32 / initial_quads * 100.0
                    )
                }
                x if (min_levels..max_levels).contains(&x) => {
                    format!("refining the grid {current_level}/{max_levels}")
                }
                _ => {
                    format!("Done, {quads_evaluated} quads drawn in total")
                }
            };
            ui.add(ProgressBar::new(progress).text(text).animate(true));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let new_viewer_config = viewer.get_viewer_config();
            let default_color = new_viewer_config.output_variants[0].1;
            let texture_handle = self.texture_handle.get_or_insert(ctx.load_texture(
                "plot",
                ColorImage::new([1, 1], default_color),
                TextureFilter::Nearest,
            ));

            let (x_min, x_max) = new_viewer_config.x_axis_range.clone().into_inner();
            let (y_min, y_max) = new_viewer_config.y_axis_range.clone().into_inner();

            let center = [
                ((x_min + x_max) / 2.0) as f64,
                ((y_min + y_max) / 2.0) as f64,
            ];
            let size = [x_max - x_min, y_max - y_min];
            let aspect_ratio = size[0] / size[1];

            let new_viewer_config = Some(new_viewer_config);

            if self.last_viewer_config != new_viewer_config {
                // *texture_handle = ctx.load_texture("plot", ColorImage::new([1, 1], default_color));
                self.backend.start_with(
                    viewer.get_viewer_config(),
                    texture_handle.clone(),
                    viewer.get_fn(),
                );
                self.last_viewer_config = new_viewer_config;
            }

            let plot_image = PlotImage::new(texture_handle.id(), center.into(), size);

            Plot::new("my_plot")
                .data_aspect(aspect_ratio)
                .show(ui, |plot_ui| plot_ui.image(plot_image));
        });
    }
}
