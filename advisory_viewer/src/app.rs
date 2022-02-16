use std::collections::HashMap;

use eframe::{
    egui::{
        self,
        plot::{Line, Plot, Value, Values},
        Color32,
    },
    epi,
};

mod visualize;

#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct AdvisoryViewerConfig {
    /// contains all input values
    pub input_values: HashMap<String, f32>,
    /// the output value -> Color mapping
    /// For the example of the H-CAS, this is fife: CoC, WL, WR, SL, SR
    pub output_variants: HashMap<String, Color32>,

    /// Key to `input_values`, describing which input value is to be used as x axis
    pub x_value_key: String,

    /// Key to `input_values`, describing which input value is to be used as y axis
    pub y_value_key: String,
}

#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct AdvisoryViewer {
    pub conf: AdvisoryViewerConfig,
    // TODO Inser whatever you need to cache the quadtree
    // Remember to annotate it with
    // #[cfg_attr(feature = "persistence", serde(skip))]
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    av: AdvisoryViewer,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            av: AdvisoryViewer {
                conf: Default::default(),
            },
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
        _ctx: &egui::CtxRef,
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
    fn update(&mut self, ctx: &egui::CtxRef, frame: &epi::Frame) {
        let AdvisoryViewer { ref mut conf, .. } = self.av;

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");
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
        });
    }
}
