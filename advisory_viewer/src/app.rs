use arc_swap::ArcSwap;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Bound, RangeInclusive};
use std::sync::{Arc, RwLock};

use eframe::egui::plot::{MarkerShape, Points, Polygon, Text};
use eframe::{
    egui::{
        self,
        plot::{Line, Plot, Value, Values},
        Color32, DragValue,
    },
    epi,
};

use uom::si::angle::radian;
use uom::si::f32::*;
use uom::si::length::foot;
use uom::si::time::second;

use strum::EnumMessage;
use strum::IntoEnumIterator;

pub struct HCasCartesianGui {
    x_axis_key: HCasInput,
    y_axis_key: HCasInput,
    output_colors: Vec<Color32>,
    input_ranges: HashMap<HCasInput, RangeInclusive<f32>>,
    inputs: HashMap<HCasInput, f32>,
    pra: u8,
}
impl HCasCartesianGui {
    const N_OUTPUTS: usize = 5;
    const DEFAULT_COLORS: [Color32; Self::N_OUTPUTS] = [
        Color32::TRANSPARENT,
        Color32::LIGHT_RED,
        Color32::LIGHT_GREEN,
        Color32::RED,
        Color32::GREEN,
    ];
    const OUTPUTS: [&'static str; Self::N_OUTPUTS] = ["CoC", "WL", "WR", "SL", "SR"];
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    strum::EnumString,
    strum::EnumIter,
    strum::Display,
    strum::AsRefStr,
    strum::EnumMessage,
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
                (HCasInput::Y, -56e3..=56e3),
                (HCasInput::IntruderBearing, angle_range.clone()),
            ]
            .into_iter()
            .collect(),
            inputs: [
                (HCasInput::Tau, 15.0),
                (HCasInput::X, 0.0),
                (HCasInput::Y, 0.0),
                (HCasInput::IntruderBearing, 330.0),
            ]
            .into_iter()
            .collect(),
            pra: 0,
            output_colors: Self::DEFAULT_COLORS.into_iter().collect(),
        };

        assert_eq!(HCasInput::iter().len(), new_self.input_ranges.len());
        assert_eq!(HCasInput::iter().len(), new_self.inputs.len());

        new_self
    }
}

impl Visualizable for HCasCartesianGui {
    fn draw_config(&mut self, ui: &mut egui::Ui) {
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
                    let unit = &format!(" {}", k.get_message().unwrap());
                    let name = k.get_detailed_message().unwrap();
                    let tooltip = k.get_documentation().unwrap();

                    let value_range = self.input_ranges.get(k).unwrap();

                    ui.label(format!("{name} [{unit}]")).on_hover_text(tooltip);
                    ui.add_enabled(
                        *k != self.x_axis_key && *k != self.y_axis_key,
                        egui::Slider::new(v, value_range.clone()).show_value(true),
                    );
                    ui.end_row();
                }

                // add inputs for the input ranges
                for (k, v) in &mut self.input_ranges {
                    let unit = &format!(" {}", k.get_message().unwrap());
                    let name = k.get_detailed_message().unwrap();
                    let tooltip = k.get_documentation().unwrap();

                    let (mut min, mut max) = v.clone().into_inner();
                    let drag_value_speed = 1e-2;

                    ui.label(format!("{name} range [{unit}]"))
                        .on_hover_text(tooltip);

                    let size = (
                        ui.available_width() / 2.0 - ui.spacing().button_padding.x,
                        ui.spacing().interact_size.y,
                    );
                    // TODO the following is an ugly hack to get left and right aligned elements in
                    // the same row.
                    ui.horizontal_wrapped(|ui| {
                        ui.add_sized(
                            size,
                            egui::DragValue::new(&mut min)
                                .clamp_range(f32::MIN..=max)
                                .speed(drag_value_speed),
                        );
                        ui.with_layout(egui::Layout::right_to_left(), |ui| {
                            ui.add_sized(
                                size,
                                egui::DragValue::new(&mut max)
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
                    ui.label(format!("Output {i} Color"));

                    ui.horizontal_wrapped(|ui| {
                        let size = (
                            ui.available_width() / 2.0 - ui.spacing().button_padding.x,
                            ui.spacing().interact_size.y,
                        );

                        ui.spacing_mut().interact_size.x = size.0;

                        ui.color_edit_button_srgba(color);
                        if ui.button("reset").clicked() {
                            *color = Self::DEFAULT_COLORS[i];
                        }
                    });
                    ui.end_row();
                }
            });
    }

    fn get_fn(&self) -> ViewerFn {
        use HCasInput::*;
        let last_adv = self.pra.try_into().unwrap();
        let mut inputs = self.inputs.clone();
        let x_axis_key = self.x_axis_key;
        let y_axis_key = self.y_axis_key;

        Box::new(move |x, y| {
            let mut cas = opencas::HCas {
                last_advisory: last_adv,
            };
            *inputs.get_mut(&x_axis_key).unwrap() = x;
            *inputs.get_mut(&y_axis_key).unwrap() = y;

            let tau = Time::new::<second>(*inputs.get(&Tau).unwrap());
            let forward = Length::new::<foot>(*inputs.get(&X).unwrap());
            let left = Length::new::<foot>(*inputs.get(&Y).unwrap());
            let psi = Angle::new::<radian>(*inputs.get(&IntruderBearing).unwrap());
            cas.process_cartesian(tau, forward, left, psi).0 as u8
        })
    }

    fn draw_plot(&mut self, ui: &mut egui::Ui) {
        todo!();
    }

    fn get_viewer_config(&self) -> ViewerConfig {
        let output_variants = Self::OUTPUTS
            .iter()
            .zip(self.output_colors.iter())
            .enumerate()
            .map(|(i, (n, c))| (i as u8, (n.to_string(), *c)))
            .collect();
        let x_axis_range = self.input_ranges.get(&self.x_axis_key).unwrap().clone();
        let y_axis_range = self.input_ranges.get(&self.y_axis_key).unwrap().clone();
        ViewerConfig {
            output_variants,
            x_axis_range,
            y_axis_range,
            min_levels: 0,  // TODO add ui for this
            max_levels: 14, // TODO add ui for this
        }
    }
}

type ViewerFn = Box<dyn FnMut(f32, f32) -> u8 + Send + Sync>;

trait Visualizable {
    /// draw the config panel for this visualizable
    fn draw_config(&mut self, ui: &mut egui::Ui);

    /// draw the actual thing
    fn draw_plot(&mut self, ui: &mut egui::Ui);

    /// Get the function used to color the texture by the visualizer
    fn get_fn(&self) -> ViewerFn;

    fn get_viewer_config(&self) -> ViewerConfig;
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct ViewerConfig {
    /// the output value -> Color mapping
    /// For the example of the H-CAS, this is fife: CoC, WL, WR, SL, SR
    pub output_variants: HashMap<u8, (String, Color32)>,

    /// From where to where to render on the x-axis
    pub x_axis_range: RangeInclusive<f32>,

    /// From where to where to render on the y-axis
    pub y_axis_range: RangeInclusive<f32>,

    /// Initial level of detail to redner
    pub min_levels: usize,

    /// Maximum level of detail to render
    pub max_levels: usize,
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct AdvisoryViewer {
    pub conf: ViewerConfig,
    // TODO Insert whatever you need to cache the quadtree
    // Remember to annotate it with
    // #[cfg_attr(feature = "persistence", serde(skip))]
    //#[cfg_attr(feature = "persistence", serde(skip))]
    //visualizer_tree: Arc<ArcSwap<VisualizerNode>>,
    #[cfg_attr(feature = "persistence", serde(skip))]
    config_hash: Arc<RwLock<u64>>,
    #[cfg_attr(feature = "persistence", serde(skip))]
    min_level_counter: Arc<RelaxedCounter>,
    #[cfg_attr(feature = "persistence", serde(skip))]
    additional_quad_counter: Arc<RelaxedCounter>,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    viewers: HashMap<String, Box<dyn Visualizable>>,
    viewer_key: String,
}

impl Default for TemplateApp {
    fn default() -> Self {
        let viewers: HashMap<String, Box<dyn Visualizable>> = [(
            "HCAS Cartesian".into(),
            Box::new(HCasCartesianGui::default()) as _,
        )]
        .into_iter()
        .collect();
        let viewer_key = viewers.keys().next().unwrap().into();

        Self {
            viewers,
            viewer_key,
        }
    }
}

impl epi::App for TemplateApp {
    fn name(&self) -> &str {
        "eframe template"
    }

    /// Called once before the first frame.
    fn setup(
        &mut self,
        _ctx: &egui::Context,
        _frame: &epi::Frame,
        _storage: Option<&dyn epi::Storage>,
    ) {
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        #[cfg(feature = "persistence")]
        if let Some(storage) = _storage {
            *self = epi::get_value(storage, epi::APP_KEY).unwrap_or_default()
        }
    }

    /// Called by the frame work to save state before shutdown.
    /// Note that you must enable the `persistence` feature for this to work.
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn epi::Storage) {
        epi::set_value(storage, epi::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, frame: &epi::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Choose Viewee", |ui| {
                    for k in self.viewers.keys() {
                        if ui.button(k).clicked() {
                            self.viewer_key = k.into();
                        }
                    }
                });
            });
        });

        let viewer_key = &self.viewer_key;
        let viewer = self.viewers.get_mut(viewer_key).unwrap();

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading(format!("Settings for {viewer_key}"));
            viewer.draw_config(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let sin = (0..1000).map(|i| {
                let x = i as f64 * 0.01;
                Value::new(x, x.sin())
            });
            let line = Line::new(Values::from_values_iter(sin));
            Plot::new("my_plot")
                .data_aspect(1.0)
                .show(ui, |plot_ui| plot_ui.line(line));

            /*
            let polygons = match viewer_key.as_str() {
                "HCAS" => current_viewer.get_points(
                    |x, y, c| {
                        let mut config = c.clone();
                        let mut cas = opencas::HCas {
                            last_advisory: config.previous_output.try_into().unwrap(),
                        };

                        *config.input_values.get_mut(&config.x_axis_key).unwrap() = x;
                        *config.input_values.get_mut(&config.y_axis_key).unwrap() = y;

                        let tau = Time::new::<second>(*config.input_values.get("τ").unwrap());
                        let forward =
                            Length::new::<foot>(*config.input_values.get("forward").unwrap());
                        let left = Length::new::<foot>(*config.input_values.get("left").unwrap());
                        let psi = Angle::new::<radian>(*config.input_values.get("ψ").unwrap());
                        cas.process_cartesian(tau, forward, left, psi).0 as u8
                    },
                    0.0..=56e3,
                    -23e3..=23e3,
                ),
                _ => {
                    vec![]
                }
            };
            Plot::new("my_plot").data_aspect(1.0).show(ui, |plot_ui| {
                for p in polygons.into_iter() {
                    plot_ui.polygon(p);
                }
            });
            */
        });
    }
}
