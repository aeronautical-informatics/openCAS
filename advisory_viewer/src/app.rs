use std::{collections::BTreeMap, hash::Hash, ops::RangeInclusive};

use eframe::epi;
use egui::{
    self,
    plot::PlotImage,
    plot::{Plot, Value},
    Color32, ColorImage, DragValue, ProgressBar, TextureHandle,
};

use serde::{Deserialize, Serialize};
use uom::si::{angle::radian, f32::*, length::foot, time::second};

use strum::{EnumIter, EnumMessage, IntoEnumIterator};

mod visualize;
use visualize::VisualizerBackend;

#[derive(Deserialize, Serialize)]
pub struct HCasCartesianGui {
    x_axis_key: HCasInput,
    y_axis_key: HCasInput,
    output_colors: Vec<Color32>,
    input_ranges: BTreeMap<HCasInput, RangeInclusive<f32>>,
    inputs: BTreeMap<HCasInput, f32>,
    pra: u8,
    min_level: usize,
    max_level: usize,
}
impl HCasCartesianGui {
    const N_OUTPUTS: usize = 5;
    const DEFAULT_COLORS: [Color32; Self::N_OUTPUTS] = [
        Color32::from_rgba_premultiplied(13, 13, 13, 13),
        Color32::LIGHT_RED,
        Color32::LIGHT_GREEN,
        Color32::RED,
        Color32::GREEN,
    ];
    const OUTPUTS: [&'static str; Self::N_OUTPUTS] = ["CoC", "WL", "WR", "SL", "SR"];
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    EnumIter,
    EnumMessage,
    Deserialize,
    Serialize,
)]
pub enum HCasInput {
    /// estimated time to impact
    #[strum(message = "s", detailed_message = "τ")]
    Tau,

    /// forward looking distance to intruder plane
    #[strum(message = "ft", detailed_message = "longitidunal dist")]
    X,

    /// left looking distance to intruder plane
    #[strum(message = "ft", detailed_message = "lateral dist")]
    Y,

    /// intruder bearing
    #[strum(message = "rad", detailed_message = "θ")]
    IntruderBearing,
}

impl Default for HCasCartesianGui {
    fn default() -> Self {
        let angle_range = -std::f32::consts::PI..=std::f32::consts::PI;
        let new_self = Self {
            x_axis_key: HCasInput::X,
            y_axis_key: HCasInput::Y,
            input_ranges: [
                (HCasInput::Tau, 0.0..=60.0),
                (HCasInput::X, 0.0..=56e3),
                (HCasInput::Y, -23e3..=23e3),
                (HCasInput::IntruderBearing, angle_range),
            ]
            .into_iter()
            .collect(),
            inputs: [
                (HCasInput::Tau, 15.0),
                (HCasInput::X, 0.0),
                (HCasInput::Y, 0.0),
                (HCasInput::IntruderBearing, 0.1),
            ]
            .into_iter()
            .collect(),
            pra: 0,
            output_colors: Self::DEFAULT_COLORS.into_iter().collect(),
            min_level: 5,
            max_level: 10,
        };

        assert_eq!(HCasInput::iter().len(), new_self.input_ranges.len());
        assert_eq!(HCasInput::iter().len(), new_self.inputs.len());

        new_self
    }
}

impl Visualizable for HCasCartesianGui {
    fn draw_config(&mut self, ui: &mut egui::Ui) -> bool {
        let old_pra = self.pra;
        let old_inputs = self.inputs.clone();
        egui::Grid::new("hcas_gui_settings_grid")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("X-Axis Selector");

                // calculate the maximum width for the combo boxes
                let original_spacing = ui.spacing().clone();
                ui.spacing_mut().slider_width =
                    ui.available_width() - 2.0 * original_spacing.button_padding.x;
                egui::ComboBox::from_id_source("x_axis_combo")
                    .selected_text(self.x_axis_key.get_detailed_message().unwrap())
                    .show_ui(ui, |ui| {
                        for value in HCasInput::iter() {
                            ui.selectable_value(
                                &mut self.x_axis_key,
                                value,
                                value.get_detailed_message().unwrap(),
                            );
                        }
                    });
                ui.end_row();

                ui.label("Y-Axis Selector");
                egui::ComboBox::from_id_source("y_axis_combo")
                    .selected_text(self.y_axis_key.get_detailed_message().unwrap())
                    .show_ui(ui, |ui| {
                        for value in HCasInput::iter() {
                            ui.selectable_value(
                                &mut self.y_axis_key,
                                value,
                                value.get_detailed_message().unwrap(),
                            );
                        }
                    });
                ui.end_row();

                ui.label("Previous Advice");
                egui::ComboBox::from_id_source("previous_advice_combo")
                    .selected_text(Self::OUTPUTS[self.pra as usize])
                    .show_ui(ui, |ui| {
                        for (i, adv) in Self::OUTPUTS.iter().enumerate() {
                            ui.selectable_value(&mut self.pra, i as u8, *adv);
                        }
                    });
                ui.end_row();

                // reset width, since the combination of slider and drag value doesn't behave
                *ui.spacing_mut() = original_spacing;

                // add inputs for the actual input values
                for (k, v) in &mut self.inputs {
                    let unit = &format!("[{}]", k.get_message().unwrap());
                    let name = k.get_detailed_message().unwrap();
                    let tooltip = k.get_documentation().unwrap();

                    let value_range = self.input_ranges.get(k).unwrap();

                    ui.label(format!("{name} {unit}")).on_hover_text(tooltip);
                    ui.add_enabled(
                        *k != self.x_axis_key && *k != self.y_axis_key,
                        egui::Slider::new(v, value_range.clone()).show_value(true),
                    );
                    ui.end_row();
                }

                let mut size = (0.0, 0.0);

                // add inputs for the input ranges
                for (k, v) in &mut self.input_ranges {
                    let unit = &format!("[{}]", k.get_message().unwrap());
                    let name = k.get_detailed_message().unwrap();
                    let tooltip = k.get_documentation().unwrap();

                    let (mut min, mut max) = v.clone().into_inner();
                    let drag_value_speed = 1e-2;

                    ui.label(format!("{name} range {unit}"))
                        .on_hover_text(tooltip);

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
                        ui.with_layout(egui::Layout::right_to_left(), |ui| {
                            ui.add_sized(
                                size,
                                DragValue::new(&mut max)
                                    .clamp_range(min..=f32::MAX)
                                    .speed(drag_value_speed),
                            );
                        });
                    });
                    ui.end_row();
                    *v = min..=max;
                }

                // add inputs for the colors
                for (i, color) in self.output_colors.iter_mut().enumerate() {
                    let output_name = Self::OUTPUTS[i];
                    ui.label(format!(r#""{output_name}" Color"#));

                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().interact_size.x = size.0;
                        ui.color_edit_button_srgba(color);
                        if ui.button("reset").clicked() {
                            *color = Self::DEFAULT_COLORS[i];
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
                        .clamp_range(self.min_level..=14)
                        .speed(level_speed),
                ); // TODO change this limit
            });

        // redraw, if
        self.pra != old_pra || self.inputs != old_inputs
    }

    fn get_fn(&self) -> ViewerFn {
        use HCasInput::*;
        let last_adv = self.pra.try_into().unwrap();
        let inputs = self.inputs.clone();
        let x_axis_key = self.x_axis_key;
        let y_axis_key = self.y_axis_key;

        Box::new(move |x, y| {
            let mut cas = opencas::HCas {
                last_advisory: last_adv,
            };
            let mut inputs = inputs.clone();
            *inputs.get_mut(&x_axis_key).unwrap() = x;
            *inputs.get_mut(&y_axis_key).unwrap() = y;

            let tau = Time::new::<second>(*inputs.get(&Tau).unwrap());
            let forward = Length::new::<foot>(*inputs.get(&X).unwrap());
            let left = Length::new::<foot>(*inputs.get(&Y).unwrap());
            let psi = Angle::new::<radian>(*inputs.get(&IntruderBearing).unwrap());
            cas.process_cartesian(tau, forward, left, psi).0 as u8
        })
    }

    fn get_viewer_config(&self) -> ViewerConfig {
        let output_variants = Self::OUTPUTS
            .iter()
            .zip(self.output_colors.iter())
            .map(|(n, c)| (n.to_string(), *c))
            .collect();
        let x_axis_range = self.input_ranges.get(&self.x_axis_key).unwrap().clone();
        let y_axis_range = self.input_ranges.get(&self.y_axis_key).unwrap().clone();
        ViewerConfig {
            output_variants,
            x_axis_range,
            y_axis_range,
            min_levels: self.min_level, // TODO make name consistent
            max_levels: self.max_level, // TODO make name consistent
        }
    }
}

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
    /// For the example of the H-CAS, this is fife: CoC, WL, WR, SL, SR
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

impl Default for TemplateApp {
    fn default() -> Self {
        let viewers: BTreeMap<String, Box<dyn Visualizable>> = [(
            "HCAS Cartesian".into(),
            Box::new(HCasCartesianGui::default()) as _,
        )]
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

impl epi::App for TemplateApp {
    fn name(&self) -> &str {
        "Advisory Viewer"
    }

    /// Called once before the first frame.
    fn setup(
        &mut self,
        _ctx: &egui::Context,
        _frame: &epi::Frame,
        _storage: Option<&dyn epi::Storage>,
    ) {
        // Load previous app state (if any).
        if let Some(storage) = _storage {
            *self = epi::get_value(storage, epi::APP_KEY).unwrap_or_default()
        }
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn epi::Storage) {
        epi::set_value(storage, epi::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
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
            ui.heading(format!("Settings for {viewer_key}"));
            if viewer.draw_config(ui) {
                self.last_viewer_config = None;
            };
        });

        // show progressbar
        let status = self.backend.get_status();
        let ViewerConfig {
            min_levels,
            max_levels,
            ..
        } = viewer.get_viewer_config();
        let initial_quads = 4f32.powi(min_levels as i32);
        let current_level = status.current_level;
        let quads_evaluated = status.quads_evaluated;

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
            let texture_handle = self
                .texture_handle
                .get_or_insert(ctx.load_texture("plot", ColorImage::new([1, 1], default_color)));

            let (x_min, x_max) = new_viewer_config.x_axis_range.clone().into_inner();
            let (y_min, y_max) = new_viewer_config.y_axis_range.clone().into_inner();

            let center = Value::new((x_min + x_max) / 2.0, (y_min + y_max) / 2.0);
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

            let plot_image = PlotImage::new(texture_handle.id(), center, size);

            Plot::new("my_plot")
                .data_aspect(aspect_ratio)
                .show(ui, |plot_ui| plot_ui.image(plot_image));
        });
    }
}
