//! This demo illustrates how the openCAS system can be integra&ted with Flightgear

use std::f32::INFINITY;
use std::{env, error::Error, time::Instant};

use futures::prelude::*;

use async_tungstenite::tungstenite::Message;
use opencas::*;
use serde::{Deserialize, Serialize};
use uom::si::angle::{self, degree, radian};
use uom::si::length::{foot, meter};
use uom::si::time::{second, Time};
use uom::si::velocity::{foot_per_minute, foot_per_second, knot};
use uom::si::{f32::*, time};

#[derive(Serialize)]
struct FlightgearCommand {
    command: String,
    node: String,
}

/// Yields AircraftStates from a Flightgear http/json connection
///
/// # Arguments
/// `base_uri` - The base URI of the Flightgear http interface. Something like `localhost:5400`.
async fn new_flightgear_stream(
    base_uri: &str,
) -> Result<impl Stream<Item = Result<PropertyTreeLeaf, Box<dyn Error>>>, Box<dyn Error>> {
    let url = format!("ws://{}/PropertyListener", base_uri);
    let (mut stream, _) = async_tungstenite::async_std::connect_async(url).await?;

    for node in KEYS {
        let sub = FlightgearCommand {
            command: "addListener".to_string(),
            node: node.to_string(),
        };
        stream
            .send(Message::Binary(serde_json::to_vec(&sub)?))
            .await?;
    }

    Ok(stream
        .map(|msg| -> Result<_, Box<dyn Error>> { Ok(serde_json::from_slice(&msg?.into_data())?) }))
}

#[derive(Deserialize)]
struct PropertyTreeLeaf {
    pub path: String,
    pub ts: f32,
    pub value: f32,
}

#[derive(Clone, Default)]
struct AircraftState {
    pub groundspeed: Velocity,
    pub vertical_speed: Velocity,
    pub lng: Angle,
    pub lat: Angle,
    pub altitude: Length,
    pub heading: Angle,
}

const KEYS: &[&str] = &[
    "/velocities/groundspeed-kt",
    "/velocities/vertical-speed-fps",
    "/position/altitude-ft",
    "/position/longitude-deg",
    "/position/latitude-deg",
    "/orientation/heading-deg",
    "/ai/models/aircraft[0]/velocities/groundspeed-kt",
    "/ai/models/aircraft[0]/position/longitude-deg",
    "/ai/models/aircraft[0]/position/latitude-deg",
    "/ai/models/aircraft[0]/orientation/heading-deg",
    "/ai/models/aircraft[0]/velocities/vertical-speed-fps",
    "/ai/models/aircraft[0]/position/altitude-ft",
];

const USAGE: &str = "usage: <Flightgear base url>";

// http://localhost:5400/json/velocities?i=y&t=y&d=3

// do lat/lon conversion to distance
// formula is:
// a =sin(delta_lat/2)^2 + cos(lat1)*cos(lat2)*sin(del_lng/2)^2
// c = atan2(sqrt(a), sqrt(1-a))
// distance = R * c
fn haversine(ownship: &AircraftState, intruder: &AircraftState) -> Length {
    const radius_earth: f32 = 6271e3;
    let Radius = radius_earth + ownship.altitude.get::<meter>();
    let del_lat = intruder.lat.get::<radian>() - ownship.lat.get::<radian>();
    let del_lng = intruder.lng.get::<radian>() - ownship.lng.get::<radian>();
    let a = (del_lat/2.0).sin().powi(2) + ownship.lat.get::<radian>().cos() * intruder.lat.get::<radian>().cos() * (del_lng/2.0).sin().powi(2);
    let c = a.sqrt().atan2((1-a).sqrt());
    let range = Length::new::<meter>(Radius*c);
}

// calculate relative heading angles between intruder and ownship
fn heading_angles(ownship: &AircraftState, intruder: &AircraftState) -> Angle {
    ownship.heading - intruder.heading //convert from deg to rad
                                       // be careful here!
                                       // Paper says it is in degrees, trained networks say it is in radian  -> whats the training data?
}

// calculate relative altitudes between intruder and ownship
// If value positive => Intruder above ownship - else below
fn relative_altitudes(ownship: &AircraftState, intruder: &AircraftState) -> Length {
    intruder.altitude - ownship.altitude
}

/*
// calculate relative horizontal speeds between intruder and ownship
fn relative_horizontal_speed(ownship: &AircraftState, intruder: &AircraftState){
    todo!();
}*/

// calculate tau until horizontal collision
fn calc_tau_horizontal(ownship: &AircraftState, intruder: &AircraftState) {
    todo!();
}

// calculate tau until vertical collision
fn calc_tau_vertical(ownship: &AircraftState, intruder: &AircraftState) -> Time {
    // first, get relative altitudes
    let altitude = relative_altitudes(ownship, intruder);

    // if the intruder is above the ownship
    if altitude.is_sign_positive() == true {
        // if the ownship vertical speed is greater than intruder
        if ownship.vertical_speed > intruder.vertical_speed {

            //both climbing but ownship climbs faster and catches up
            if (ownship.vertical_speed.is_sign_positive()
                && intruder.vertical_speed.is_sign_positive())
                == true
            {
                let rel_speed = ownship.vertical_speed - intruder.vertical_speed;
                return altitude / rel_speed;

            // intruder is descending but ownship is climbing
            } else if (ownship.vertical_speed.is_sign_positive()
                && intruder.vertical_speed.is_sign_negative())
                == true
            {
                let rel_speed = ownship.vertical_speed - intruder.vertical_speed;
                return altitude / rel_speed;

            // if both are descending but ownship is slower than intruder so intruder catches up ((-5 ft/min) > (-15 ft/min) but |-5| < |-15|)
            } else if (ownship.vertical_speed.is_sign_negative()
                && intruder.vertical_speed.is_sign_negative())
                == true
            {
                let rel_speed = intruder.vertical_speed.abs() - ownship.vertical_speed.abs();
                return altitude / rel_speed;

            // if any of has no vertical speed
            /***** CHECK: .is_sign_positive/negative FOR COVERAGE OF 0.0!! *****/
            } else {
                return Time::new::<second>(INFINITY);
            }
        // if intruder climbing rate is higher than ownship
        // 1) intruder climbs faster than ownship -> not critical
        // 2) intruder climbs while ownship falls -> not critical
        // 3) both descend but intruder with a slower descending rate ((-5 ft/min) > (-15 ft/min) but |-5| < |-15|)
        } else if ownship.vertical_speed < intruder.vertical_speed {
            return Time::new::<second>(INFINITY);
        }

    // if ownship is above intruder
    } else if altitude.is_sign_negative() == true {

        // if the intruder vertical speed is greater than ownship
        if intruder.vertical_speed > ownship.vertical_speed {

            //both climbing but intruder climbs faster and catches up
            if (ownship.vertical_speed.is_sign_positive()
                && intruder.vertical_speed.is_sign_positive())
                == true
            {
                let rel_speed = intruder.vertical_speed - ownship.vertical_speed;
                return altitude.abs() / rel_speed;
                
            // ownship is descending but intruder is climbing
            } else if (intruder.vertical_speed.is_sign_positive()
                && ownship.vertical_speed.is_sign_negative())
                == true
            {
                let rel_speed = intruder.vertical_speed - ownship.vertical_speed;
                return altitude.abs() / rel_speed;

                // if both are descending but intruder is slower than ownship so it cathces up ((-5 ft/min) > (-15 ft/min) but |-5| < |-15|)
            } else if (ownship.vertical_speed.is_sign_negative()
                && intruder.vertical_speed.is_sign_negative())
                == true
            {
                let rel_speed = ownship.vertical_speed.abs() - intruder.vertical_speed.abs();
                return altitude.abs() / rel_speed;

            // if any of has no vertical speed
            /***** CHECK: .is_sign_positive/negative FOR COVERAGE OF 0.0!! *****/
            } else {
                return Time::new::<second>(INFINITY);
            }
        // if ownship climbing rate is higher than intruder
        // 1) ownship climbs faster than intruder -> not critical
        // 2) ownship climbs while intruder falls -> not critical
        // 3) both descend but ownship with a slower descending rate ((-5 ft/min) > (-15 ft/min) but |-5| < |-15|)
        } else if intruder.vertical_speed < ownship.vertical_speed {
            return Time::new::<second>(INFINITY);
        }

    // if altitude difference is 0
    } else {
        return Time::new::<second>(0.0);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    smol::block_on(async {
        let args: Vec<String> = env::args().collect();
        let base_uri = args.get(1).expect(USAGE);

        let mut fg_stream = new_flightgear_stream(base_uri.as_str()).await?;

        //Instantiate HCAS
        let mut hcas = HCas {
            last_advisory: HAdvisory::ClearOfConflict,
        };

        //Instantiate VCAS
        let mut vcas = VCas {
            last_advisory: VAdvisory::ClearOfConflict,
        };

        let mut ai = AircraftState::default();
        let mut user = AircraftState::default();

        let mut ts = Time::new::<second>(0.0);

        loop {
            let leaf = fg_stream.next().await.unwrap()?;
            let now = Instant::now();
            let cur_ts = Time::new::<second>(leaf.ts);

            // Next frame begins
            if cur_ts > ts {
                let psi = heading_angles(&user, &ai); 
                let range = haversine(&user, &ai); 

                // find formula for angle lat/lng user <--> ai
                let theta = todo!();

                let tau_vertical = calc_tau_vertical(&user,&ai);
                let tau_horizontal = todo!();
                let height_difference = relative_altitudes(&user, &ai); 
                let vertical_speed_user = user.vertical_speed; // needs to be foot per minute at inference
                let vertical_speed_ai = ai.vertical_speed; // needs to be foot per minute

                // do inference for hcas
                let (adv_h, _) = hcas.process_polar(tau_vertical, range, theta, psi);
                hcas.last_advisory = adv_h;

                println!("Processed frame, time consumed: {:?}", now.elapsed(),);
                println!("{:?}", adv_h);

                // do inference for vcas
                let (adv_v, _) = vcas.process(
                    height_difference,
                    vertical_speed_user,
                    vertical_speed_ai,
                    tau_horizontal,
                );
                vcas.last_advisory = adv_v;

                println!("Processed frame, time consumed: {:?}", now.elapsed(),);
                println!("{:?}", adv_v);
                ts = cur_ts;
            }

            match leaf.path.as_str() {
                "/velocities/groundspeed-kt" => {
                    user.groundspeed = Velocity::new::<knot>(leaf.value)
                }
                "/position/longitude-deg" => user.lng = Angle::new::<degree>(leaf.value),
                "/position/latitude-deg" => user.lat = Angle::new::<degree>(leaf.value),
                "/orientation/heading-deg" => user.heading = Angle::new::<degree>(leaf.value),
                "/velocities/vertical-speed-fps" => {
                    user.vertical_speed = Velocity::new::<foot_per_second>(leaf.value)
                }
                "/position/altitude-ft" => user.altitude = Length::new::<foot>(leaf.value),
                "/ai/models/aircraft[0]/velocities/groundspeed-kt" => {
                    ai.groundspeed = Velocity::new::<knot>(leaf.value)
                }
                "/ai/models/aircraft[0]/position/longitude-deg" => {
                    ai.lng = Angle::new::<degree>(leaf.value)
                }
                "/ai/models/aircraft[0]/position/latitude-deg" => {
                    ai.lat = Angle::new::<degree>(leaf.value)
                }
                "/ai/models/aircraft[0]/orientation/heading-deg" => {
                    ai.heading = Angle::new::<degree>(leaf.value)
                }
                "/ai/models/aircraft[0]/velocities/vertical-speed-fps" => {
                    ai.vertical_speed = Velocity::new::<foot_per_second>(leaf.value)
                }
                "/ai/models/aircraft[0]/position/altitude-ft" => {
                    ai.altitude = Length::new::<foot>(leaf.value)
                }
                _ => {}
            }
        }
    })
}
