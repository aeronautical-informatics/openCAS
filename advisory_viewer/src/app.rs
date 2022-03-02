use arc_swap::ArcSwap;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Bound, RangeInclusive};
use std::sync::{Arc, RwLock};

use eframe::egui::plot::{MarkerShape, Points, Polygon};
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

use crate::app::visualize::VisualizerNode;
use visualize::Visualizable;

mod visualize;

use strum::EnumMessage;
use strum::IntoEnumIterator;

pub struct HCasCartesianGui {
    x_axis_key: HCasInput,
    y_axis_key: HCasInput,
    input_ranges: HashMap<HCasInput, RangeInclusive<f32>>,
    inputs: HashMap<HCasInput, f32>,
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

    /// own bearing
    #[strum(message = "rad", detailed_message = "ψ")]
    OwnBearing,

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
                (HCasInput::OwnBearing, angle_range.clone()),
                (HCasInput::IntruderBearing, angle_range.clone()),
            ]
            .into_iter()
            .collect(),
            inputs: [
                (HCasInput::Tau, 15.0),
                (HCasInput::X, 0.0),
                (HCasInput::Y, 0.0),
                (HCasInput::OwnBearing, 0.0),
                (HCasInput::IntruderBearing, 330.0),
            ]
            .into_iter()
            .collect(),
        };

        assert_eq!(HCasInput::iter().len(), new_self.input_ranges.len());
        assert_eq!(HCasInput::iter().len(), new_self.inputs.len());

        new_self
    }
}

impl HCasCartesianGui {
    pub fn draw_config(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("hcas_gui_settings_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("X-Axis Selector");
                egui::ComboBox::from_id_source("x_axis_combo")
                    .selected_text(self.x_axis_key.as_ref())
                    .show_ui(ui, |ui| {
                        for value in HCasInput::iter() {
                            ui.selectable_value(&mut self.x_axis_key, value, value.as_ref());
                        }
                    });
                ui.end_row();

                ui.label("Y-Axis Selector");
                egui::ComboBox::from_id_source("y_axis_combo")
                    .selected_text(self.y_axis_key.as_ref())
                    .show_ui(ui, |ui| {
                        for value in HCasInput::iter() {
                            ui.selectable_value(&mut self.y_axis_key, value, value.as_ref());
                        }
                    });
                ui.end_row();

                // add inputs for the actual input values
                for (k, v) in &mut self.inputs {
                    let unit = &format!(" {}", k.get_message().unwrap());
                    let name = k.get_detailed_message().unwrap();
                    let tooltip = k.get_documentation().unwrap();

                    let value_range = self.input_ranges.get(k).unwrap();
                    let suffix = if *k == self.x_axis_key || *k == self.y_axis_key {
                        "[inactive]"
                    } else {
                        ""
                    };
                    ui.label(format!("{name} {suffix}")).on_hover_text(tooltip);
                    ui.add(
                        egui::Slider::new(v, value_range.clone())
                            .suffix(unit)
                            .show_value(true),
                    );
                    ui.end_row();
                }

                ui.separator();
                ui.end_row();

                // add inputs for the input ranges
                for (k, v) in &mut self.input_ranges {
                    let unit = &format!(" {}", k.get_message().unwrap());
                    let name = k.get_detailed_message().unwrap();
                    let _tooltip = k.get_documentation().unwrap();

                    let (mut min, mut max) = v.clone().into_inner();
                    let drag_value_speed = 1e-2;

                    ui.label(format!("{name} range"));

                    let drag_value_size = (
                        ui.available_width() / 2.0 - ui.spacing().button_padding.x,
                        ui.spacing().interact_size.y,
                    );
                    // TODO the following is an ugly hack to get left and right aligned elements in
                    // the same row.
                    ui.horizontal_wrapped(|ui| {
                        ui.add_sized(
                            drag_value_size,
                            egui::DragValue::new(&mut min)
                                .suffix(unit)
                                .clamp_range(f32::MIN..=max)
                                .speed(drag_value_speed),
                        );
                        ui.with_layout(egui::Layout::right_to_left(), |ui| {
                            ui.add_sized(
                                drag_value_size,
                                egui::DragValue::new(&mut max)
                                    .suffix(unit)
                                    .clamp_range(min..=f32::MAX)
                                    .speed(drag_value_speed),
                            );
                        });
                    });
                    ui.end_row();
                    *v = min..=max;
                }
            });
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct AdvisoryViewerConfig {
    /// contains all input values
    pub input_values: HashMap<String, f32>,

    /// the output value -> Color mapping
    /// For the example of the H-CAS, this is fife: CoC, WL, WR, SL, SR
    pub output_variants: HashMap<u8, (String, Color32)>,

    /// The previous output
    pub previous_output: u8,

    /// Key to `input_values`, describing which input value is to be used as x axis
    pub x_axis_key: String,

    /// Key to `input_values`, describing which input value is to be used as y axis
    pub y_axis_key: String,

    /// Initial resolution of the grid
    pub min_levels: usize,

    /// Maximum levels of the Quadtree
    pub max_levels: usize,
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct AdvisoryViewer {
    pub conf: AdvisoryViewerConfig,
    // TODO Insert whatever you need to cache the quadtree
    // Remember to annotate it with
    // #[cfg_attr(feature = "persistence", serde(skip))]
    #[cfg_attr(feature = "persistence", serde(skip))]
    visualizer_tree: Arc<ArcSwap<VisualizerNode>>,
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
    viewers: HashMap<String, AdvisoryViewer>,
    viewer_key: String,
    hcas_gui: HCasCartesianGui,
}

impl Default for TemplateApp {
    fn default() -> Self {
        let hcas_av = AdvisoryViewer {
            conf: AdvisoryViewerConfig {
                previous_output: 0,
                input_values: ["τ", "forward", "left", "ψ", "θ"]
                    .into_iter()
                    .map(|e| (e.to_string(), 0.0))
                    .collect(),
                output_variants: [
                    (1, "WL", Color32::LIGHT_RED),
                    (2, "WR", Color32::LIGHT_GREEN),
                    (3, "SL", Color32::RED),
                    (4, "SR", Color32::GREEN),
                ]
                .into_iter()
                .map(|(i, k, v)| (i.try_into().unwrap(), (k.to_string(), v)))
                .collect(),
                x_axis_key: "θ".into(),
                y_axis_key: "ψ".into(),
                min_levels: 8,
                max_levels: 12,
            },
            visualizer_tree: Arc::new(ArcSwap::new(Arc::new(VisualizerNode::default()))),
            config_hash: Default::default(),
            min_level_counter: Default::default(),
            additional_quad_counter: Default::default(),
        };

        Self {
            viewers: [("HCAS".into(), hcas_av)].into_iter().collect(),
            viewer_key: "HCAS".into(),
            hcas_gui: Default::default(),
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
        let Self {
            ref mut viewers,
            ref mut viewer_key,
            ref mut hcas_gui,
        } = self;

        let av_keys: Vec<_> = viewers.keys().cloned().collect();
        let current_viewer = viewers.get_mut(viewer_key).unwrap();

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            let AdvisoryViewer { ref mut conf, .. } = current_viewer;
            ui.heading("Settings");

            /*
                egui::Grid::new("my_grid")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("AdvisoryViewer");
                        egui::ComboBox::from_id_source("viewer_combo")
                            .selected_text(viewer_key.as_str())
                            .show_ui(ui, |ui| {
                                for value in av_keys {
                                    ui.selectable_value(viewer_key, value.clone(), value);
                                }
                            });

                        ui.end_row();

                        ui.label("Minimal Detail Level");
                        ui.add(egui::Slider::new(&mut conf.min_levels, 0..=25).logarithmic(true));
                        ui.end_row();

                        ui.label("Maximum Detail Level");
                        ui.add(DragValue::new(&mut conf.max_levels).clamp_range(1..=32));
                        ui.end_row();

                        ui.label("X-Axis Selector");
                        egui::ComboBox::from_id_source("x_axis_combo")
                            .selected_text(conf.x_axis_key.as_str())
                            .show_ui(ui, |ui| {
                                for value in conf.input_values.keys() {
                                    if value == &conf.y_axis_key {
                                        continue;
                                    }

                                    ui.selectable_value(&mut conf.x_axis_key, value.to_string(), value);
                                }
                            });
                        ui.end_row();

                        ui.label("Y-Axis Selector");
                        egui::ComboBox::from_id_source("y_axis_combo")
                            .selected_text(conf.y_axis_key.as_str())
                            .show_ui(ui, |ui| {
                                for value in conf.input_values.keys() {
                                    if value == &conf.x_axis_key {
                                        continue;
                                    }
                                    ui.selectable_value(&mut conf.y_axis_key, value.to_string(), value);
                                }
                            });
                        ui.end_row();

                        for (k, v) in &mut conf.input_values {
                            if k == &conf.x_axis_key || k == &conf.y_axis_key {
                                continue;
                            }
                            ui.label(k);
                            ui.add(DragValue::new(v));
                            ui.end_row();
                        }
                        let quads = 4usize.pow(conf.min_levels as u32);
                        let current_quads = current_viewer.min_level_counter.get();

                        ui.label("Initial Quads:");
                        ui.add(
                            egui::widgets::ProgressBar::new(current_quads as f32 / quads as f32)
                                .text(format!("{}/{}", current_quads, quads)),
                        );
                        ui.end_row();

                        ui.label("Extra Quads:");
                        ui.label(current_viewer.additional_quad_counter.get().to_string());
                        ui.end_row();
                    });
            */
            hcas_gui.draw_config(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
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
        });
    }
}
