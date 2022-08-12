use std::{collections::HashMap, ops::RangeInclusive};

use egui::{
    self, plot::Plot, plot::PlotImage, Align, Color32, ColorImage, DragValue, ProgressBar,
    TextureFilter, TextureHandle,
};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use uom::si::{angle::radian, f32::*, length::foot, time::second, velocity::foot_per_second};
mod visualize;
use visualize::VisualizerBackend;

use self::{visualizables::VisualizableKey, visualize::Status};

mod visualizables;

type ViewerFn = Box<dyn Fn(f32, f32) -> ViewerOutput + Send + Sync>;
type ViewerOutput = usize;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Input {
    name: &'static str,
    description: &'static str,
    unit: &'static str,
    range: RangeInclusive<f32>,
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}]", self.name, self.unit)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    name: &'static str,
    description: &'static str,
    color: Color32,
}

#[derive(Deserialize, Serialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct Visualizable {
    pub viewee: VisualizableKey,
    pub x_axis_index: usize,
    pub y_axis_index: usize,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub input_values: Vec<f32>,
    pub pra: usize,
    pub min_level: usize,
    pub max_level: usize,
}

impl Visualizable {
    pub fn x_axis(&self) -> &Input {
        &self.inputs[self.x_axis_index]
    }

    pub fn y_axis(&self) -> &Input {
        &self.inputs[self.y_axis_index]
    }

    pub fn output_to_index(&self, output: &Output) -> usize {
        self.outputs
            .iter()
            .enumerate()
            .find(|(_, x)| *x == output)
            .unwrap()
            .0
    }

    pub fn get_fn(&self) -> ViewerFn {
        let last_adv = self.pra;
        let (x_axis_index, y_axis_index) = (self.x_axis_index, self.y_axis_index);
        let inputs = self.input_values.clone();

        match self.viewee {
            VisualizableKey::HCasCartesian => Box::new(move |x, y| {
                let mut cas = opencas::HCas {
                    last_advisory: (last_adv as u8).try_into().unwrap(),
                };

                let get_value = |index: usize| {
                    if index == x_axis_index {
                        x
                    } else if index == y_axis_index {
                        y
                    } else {
                        inputs[index]
                    }
                };

                let tau = Time::new::<second>(get_value(0));
                let forward = Length::new::<foot>(get_value(1));
                let left = Length::new::<foot>(get_value(2));
                let psi = Angle::new::<radian>(get_value(3));
                cas.process_cartesian(tau, forward, left, psi).0 as usize
            }),
            VisualizableKey::VCas => Box::new(move |x, y| {
                let mut cas = opencas::VCas {
                    last_advisory: (last_adv as u8).try_into().unwrap(),
                };

                let get_value = |index: usize| {
                    if index == x_axis_index {
                        x
                    } else if index == y_axis_index {
                        y
                    } else {
                        inputs[index]
                    }
                };

                let tau = Time::new::<second>(get_value(0));
                let delta_altitude = Length::new::<foot>(get_value(1));
                let own_roc = Velocity::new::<foot_per_second>(get_value(2));
                let intruder_roc = Velocity::new::<foot_per_second>(get_value(3));

                cas.process(delta_altitude, own_roc, intruder_roc, tau).0 as usize
            }),
        }
    }

    pub fn get_viewer_config(&self) -> ViewerConfig {
        let output_variants = self.outputs.iter().map(|o| (o.name, o.color)).collect();
        let x_axis_range = self.inputs[self.x_axis_index].range.clone();
        let y_axis_range = self.inputs[self.y_axis_index].range.clone();
        let (min_levels, max_levels) = (self.min_level, self.max_level);
        ViewerConfig {
            output_variants,
            x_axis_range,
            y_axis_range,
            min_levels,
            max_levels,
        }
    }

    /// draw the config panel for this visualizable
    /// returns true, if a significant value changed
    fn draw_config(&mut self, ui: &mut egui::Ui) -> bool {
        let previous_input_values = self.input_values.clone();
        //let old_pra = self.pra;
        //let old_inputs = self.inputs.clone();
        egui::Grid::new("gui_settings_grid")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("X-Axis Selector");

                // calculate the maximum width for the combo boxes
                let original_spacing = ui.spacing().clone();
                ui.spacing_mut().slider_width =
                    ui.available_width() - 2.0 * original_spacing.button_padding.x;
                egui::ComboBox::from_id_source("x_axis_combo")
                    .selected_text(self.x_axis().name)
                    .show_ui(ui, |ui| {
                        for (i, value) in self.inputs.iter().enumerate() {
                            ui.selectable_value(&mut self.x_axis_index, i, value.name);
                        }
                    });
                ui.end_row();

                ui.label("Y-Axis Selector");
                egui::ComboBox::from_id_source("y_axis_combo")
                    .selected_text(self.y_axis().name)
                    .show_ui(ui, |ui| {
                        for (i, value) in self.inputs.iter().enumerate() {
                            ui.selectable_value(&mut self.y_axis_index, i, value.name);
                        }
                    });
                ui.end_row();

                ui.label("Previous Advice");
                egui::ComboBox::from_id_source("previous_advice_combo")
                    .selected_text(self.outputs[self.pra].name)
                    .show_ui(ui, |ui| {
                        for (i, adv) in self.outputs.iter().enumerate() {
                            ui.selectable_value(&mut self.pra, i, adv.name);
                        }
                    });
                ui.end_row();

                // reset width, since the combination of slider and drag value doesn't behave
                *ui.spacing_mut() = original_spacing;

                let drag_value_speed = 1e-2;
                let mut size = (0.0, 0.0);
                // add inputs for the actual input values
                for (idx, input) in &mut self
                    .inputs
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| *idx != self.x_axis_index && *idx != self.y_axis_index)
                {
                    ui.label(input.to_string()).on_hover_text(input.description);

                    size = (ui.available_width(), ui.spacing().interact_size.y);
                    ui.add_sized(
                        size,
                        DragValue::new(&mut self.input_values[idx])
                            .clamp_range(input.range.clone())
                            .speed(drag_value_speed),
                    );
                    //});
                    ui.end_row();
                }

                // add inputs for the input ranges
                for i in &mut self.inputs {
                    let (mut min, mut max) = i.range.clone().into_inner();

                    ui.label(format!("{} range [{}]", i.name, i.unit))
                        .on_hover_text(i.description);

                    size = (
                        ui.available_width() / 2.0 - ui.spacing().button_padding.x,
                        ui.spacing().interact_size.y,
                    );
                    // TODO the following is an ugly hack to get left and right aligned elements in
                    // the same row.
                    ui.horizontal_wrapped(|ui| {
                        ui.add_sized(
                            size,
                            DragValue::new(&mut min)
                                .clamp_range(f32::MIN..=max)
                                .speed(drag_value_speed),
                        );
                        ui.with_layout(egui::Layout::right_to_left(Align::LEFT), |ui| {
                            ui.add_sized(
                                size,
                                DragValue::new(&mut max)
                                    .clamp_range(min..=f32::MAX)
                                    .speed(drag_value_speed),
                            );
                        });
                        i.range = min..=max;
                    });
                    ui.end_row();
                    // TODO fix range setting
                }

                // add inputs for the output colors
                for o in &mut self.outputs {
                    ui.label(format!(r#""{}" Color"#, o.name))
                        .on_hover_text(o.description);

                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().interact_size.x = size.0;
                        // TODO add color buttons
                        ui.color_edit_button_srgba(&mut o.color);
                        if ui.button("reset").clicked() {
                            let default_viewee: Visualizable = self.viewee.into();
                            o.color =
                                default_viewee.outputs[default_viewee.output_to_index(o)].color;
                        }
                    });
                    ui.end_row();
                }

                ui.spacing_mut().interact_size.x = size.0;

                // min and max level
                let level_speed = 1e-2;
                ui.label("Min level")
                    .on_hover_text("The start resolution of the plot.");
                ui.add(
                    DragValue::new(&mut self.min_level)
                        .clamp_range(1..=self.max_level)
                        .speed(level_speed),
                );
                ui.end_row();
                ui.label("Max level")
                    .on_hover_text("The maximum resolution of the plot");
                ui.add(
                    DragValue::new(&mut self.max_level)
                        .clamp_range(self.min_level..=15) // 2 ** (15 * 2) is equivalent to more than 1 giga pixel in resolution, that should suffice
                        .speed(level_speed),
                );
            });
        self.input_values != previous_input_values
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewerConfig {
    /// the output value -> Color mapping
    /// For the example of the H-CAS, these are: CoC, WL, WR, SL, SR
    pub output_variants: Vec<(&'static str, Color32)>,

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
    viewers: HashMap<VisualizableKey, Visualizable>,

    viewer_key: VisualizableKey,

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
        let viewer_key = VisualizableKey::HCasCartesian;
        let viewers: HashMap<_, _> = VisualizableKey::iter().map(|vk| (vk, vk.into())).collect();

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
                        if ui.button(k.as_ref()).clicked() {
                            self.viewer_key = *k;
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
                ui.heading(format!("Settings for {}", viewer_key.as_ref()));
                if viewer.draw_config(ui) {
                    self.last_viewer_config = None;
                }
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
                        "calculating initial grid {:3.0} %, level {x}/{min_levels}",
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

            // ensures that different plots remmeber their position and zoombox seperately
            let plot_id = format!(
                "{viewer_key}:{}:{}",
                viewer.x_axis_index, viewer.y_axis_index
            );

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

            Plot::new(&plot_id)
                .data_aspect(aspect_ratio)
                .show(ui, |plot_ui| plot_ui.image(plot_image));
        });
    }
}
