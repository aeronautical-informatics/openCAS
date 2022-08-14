use super::*;

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    strum::AsRefStr,
    strum::Display,
    strum::EnumIter,
)]
pub enum VisualizableKey {
    #[strum(serialize = "Horizontal CAS Cartesian")]
    HCasCartesian,
    #[strum(serialize = "Vertical CAS")]
    VCas,
}

impl From<VisualizableKey> for Visualizable {
    fn from(v: VisualizableKey) -> Self {
        match v {
            VisualizableKey::HCasCartesian => Visualizable {
                viewee: v,
                x_axis_index: 1,
                y_axis_index: 2,
                inputs: vec![
                    Input {
                        name: "τ",
                        description: "estimated time to impact",
                        unit: "s",
                        range: 0.0..=60.0,
                    },
                    Input {
                        name: "x",
                        description: "forward looking distance to intruder plane",
                        unit: "ft",
                        range: 0.0..=56e3,
                    },
                    Input {
                        name: "y",
                        description: "left lookin distance to intruder plane",
                        unit: "ft",
                        range: -(56e3 / 2.0)..=(56e3 / 2.0),
                    },
                    Input {
                        name: "ψ",
                        description: "relative intruder bearing, positive value points left",
                        unit: "rad",
                        range: -std::f32::consts::PI..=std::f32::consts::PI,
                    },
                ],
                outputs: vec![
                    Output {
                        name: "COC",
                        description: "clear of conflict",
                        color: Color32::from_rgba_premultiplied(13, 13, 13, 13),
                    },
                    Output {
                        name: "WL",
                        description: "weak left",
                        color: Color32::LIGHT_RED,
                    },
                    Output {
                        name: "WR",
                        description: "weak right",
                        color: Color32::LIGHT_GREEN,
                    },
                    Output {
                        name: "SL",
                        description: "strong left",
                        color: Color32::RED,
                    },
                    Output {
                        name: "SR",
                        description: "strong right",
                        color: Color32::GREEN,
                    },
                ],
                input_values: vec![10.0, 0.0, 00.0, 0.0],
                pra: 0,
                min_level: 5,
                #[cfg(target_arch = "wasm32")]
                max_level: 8,
                #[cfg(not(target_arch = "wasm32"))]
                max_level: 10,
            },
            VisualizableKey::VCas => {
                let speed_range = -1e2..=1e2;
                Visualizable {
                viewee: v,
                x_axis_index: 0,
                y_axis_index: 1,
                inputs: vec![
                    Input {
                        name: "τ",
                        description: "estimated time to impact",
                        unit: "s",
                        range: 0.0..=40.0,
                    },
                    Input {
                        name: "Δ altitude",
                        description: "altitude difference between ownship and intruder, positive value means intruder is above ownship",
                        unit: "ft",
                        range: -8e3..=8e3,
                    },
                    Input {
                        name: "own roc",
                        description: "ownship rate of climb, positive value means ownship increases altitude",
                        unit: "fps",
                        range: speed_range.clone(),
                    },
                    Input {
                        name: "intruder roc",
                        description: "intruder rate of climb, positive value means the intruder increases altitude",
                        unit: "fps",
                        range: speed_range,
                    },
                ],
                outputs: vec![
                    Output {
                        name: "COC",
                        description: "clear of conflict",
                        color: Color32::from_rgba_premultiplied(13, 13, 13, 13),
                    },
                    Output {
                        name: "DNC",
                        description: "do not climb",
                        color: Color32::KHAKI,
                    },
                    Output {
                        name: "DND",
                        description: "do not descent",
                        color: Color32::LIGHT_GRAY,
                    },
                    Output {
                        name: "DES1500",
                        description: "descent 1500 foot per minute",
                        color: Color32::LIGHT_YELLOW,
                    },
                    Output {
                        name: "CL1500",
                        description: "climb 1500 foot per minute",
                        color: Color32::LIGHT_BLUE,
                    },
                    Output {
                        name: "SDES1500",
                        description: "strengthen descent 1500 foot per minute",
                        color: Color32::YELLOW,
                    },
                    Output {
                        name: "SCL1500",
                        description: "strengthen climb 1500 foot per minute",
                        color: Color32::BLUE,
                    },
                    Output {
                        name: "SDES2500",
                        description: "strengthen descent 2500 foot per minute",
                        color: Color32::BROWN,
                    },
                    Output {
                        name: "SCL2500",
                        description: "strengthen climb 2500 foot per minute",
                        color: Color32::DARK_BLUE,
                    },
                ],
                input_values: vec![10.0, 0.0, 00.0, 0.0],
                pra: 0,
                min_level: 5,
                #[cfg(target_arch = "wasm32")]
                max_level: 8,
                #[cfg(not(target_arch = "wasm32"))]
                max_level: 10,
            }
            }
        }
    }
}
