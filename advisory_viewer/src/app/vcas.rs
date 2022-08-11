use strum::{EnumIter, EnumMessage, IntoEnumIterator};
use uom::si::{f32::*, length::foot, time::second, velocity::foot_per_second};

use super::*;

#[derive(Deserialize, Serialize)]
pub struct VCasGui {
    x_axis_key: VCasInput,
    y_axis_key: VCasInput,
    output_colors: Vec<Color32>,
    input_ranges: BTreeMap<VCasInput, RangeInclusive<f32>>,
    inputs: BTreeMap<VCasInput, f32>,
    pra: u8,
    min_level: usize,
    max_level: usize,
}

impl VCasGui {
    const OUTPUTS: &'static [(&'static str, Color32)] = &[
        ("COC", Color32::from_rgba_premultiplied(13, 13, 13, 13)), // Clear of Conflict
        ("DNC", Color32::KHAKI),                                   // do not climb
        ("DND", Color32::LIGHT_GRAY),                              // do not descent
        ("DES1500", Color32::LIGHT_YELLOW),                        // descent 1500 foot per minute
        ("CL1500", Color32::LIGHT_BLUE),                           // climb 1500 foot per minute
        ("SDES1500", Color32::YELLOW), // strengthen descent 1500 foot per minute
        ("SCL1500", Color32::BLUE),    // strengthen climb 1500 foot per minute
        ("SDES2500", Color32::BROWN),  // strengthen descent 2500 foot per minute
        ("SCL2500", Color32::DARK_BLUE), // strengthen climb 2500 foot per minute
    ];
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
pub enum VCasInput {
    /// estimated time to impact
    #[strum(message = "s", detailed_message = "τ")]
    Tau,

    /// altitude difference between ownship and intruder, positive value means intruder is above ownship
    #[strum(message = "ft", detailed_message = "vertical distance")]
    DeltaHeight,

    /// ownship rate of climb, positive value means ownship increases altitude
    #[strum(message = "fps", detailed_message = "rate of climb")]
    VerticalSpeedOwn,

    /// intruder rate of climb, positive value means the intruder increases altitude
    #[strum(message = "fps", detailed_message = "intruder rate of climb")]
    VerticalSpeedIntruder,
}

impl Default for VCasGui {
    fn default() -> Self {
        let speed_range = -1e2..=1e2;
        let new_self = Self {
            x_axis_key: VCasInput::Tau,
            y_axis_key: VCasInput::DeltaHeight,
            input_ranges: [
                (VCasInput::Tau, 0.0..=40.0),
                (VCasInput::DeltaHeight, -8e3..=8e3),
                (VCasInput::VerticalSpeedOwn, speed_range.clone()),
                (VCasInput::VerticalSpeedIntruder, speed_range),
            ]
            .into_iter()
            .collect(),
            inputs: [
                (VCasInput::Tau, 15.0),
                (VCasInput::DeltaHeight, 0.0),
                (VCasInput::VerticalSpeedOwn, 0.0),
                (VCasInput::VerticalSpeedIntruder, 0.1),
            ]
            .into_iter()
            .collect(),
            pra: 0,
            output_colors: Self::OUTPUTS.iter().map(|x| x.1).collect(),
            min_level: 5,
            #[cfg(target_arch = "wasm32")]
            max_level: 8,
            #[cfg(not(target_arch = "wasm32"))]
            max_level: 10,
        };

        assert_eq!(VCasInput::iter().len(), new_self.input_ranges.len());
        assert_eq!(VCasInput::iter().len(), new_self.inputs.len());

        new_self
    }
}

impl Visualizable for VCasGui {
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
                        for value in VCasInput::iter() {
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
                        for value in VCasInput::iter() {
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
                    .selected_text(Self::OUTPUTS[self.pra as usize].0)
                    .show_ui(ui, |ui| {
                        for (i, adv) in Self::OUTPUTS.iter().enumerate() {
                            ui.selectable_value(&mut self.pra, i as u8, adv.0);
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
                        ui.with_layout(egui::Layout::right_to_left(Align::LEFT), |ui| {
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
                    let output_name = Self::OUTPUTS[i].0;
                    ui.label(format!(r#""{output_name}" Color"#));

                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().interact_size.x = size.0;
                        ui.color_edit_button_srgba(color);
                        if ui.button("reset").clicked() {
                            *color = Self::OUTPUTS[i].1;
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
        use VCasInput::*;
        let last_adv = self.pra.try_into().unwrap();
        let inputs = self.inputs.clone();
        let x_axis_key = self.x_axis_key;
        let y_axis_key = self.y_axis_key;

        Box::new(move |x, y| {
            let mut cas = opencas::VCas {
                last_advisory: last_adv,
            };
            let mut inputs = inputs.clone();
            *inputs.get_mut(&x_axis_key).unwrap() = x;
            *inputs.get_mut(&y_axis_key).unwrap() = y;

            let tau = Time::new::<second>(*inputs.get(&Tau).unwrap());
            let delta_height = Length::new::<foot>(*inputs.get(&DeltaHeight).unwrap());
            let vertical_speed_own =
                Velocity::new::<foot_per_second>(*inputs.get(&VerticalSpeedOwn).unwrap());
            let vertical_speed_intruder =
                Velocity::new::<foot_per_second>(*inputs.get(&VerticalSpeedIntruder).unwrap());
            cas.process(
                delta_height,
                vertical_speed_own,
                vertical_speed_intruder,
                tau,
            )
            .0 as u8
        })
    }

    fn get_viewer_config(&self) -> ViewerConfig {
        let output_variants = Self::OUTPUTS
            .iter()
            .zip(self.output_colors.iter())
            .map(|(n, c)| (n.0.to_string(), *c))
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
