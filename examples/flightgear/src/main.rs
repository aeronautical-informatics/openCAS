//! This demo illustrates how the openCAS system can be integra&ted with Flightgear

use std::f32::consts::PI;
use std::f32::INFINITY;
use std::{env, error::Error, time::Instant};

use futures::prelude::*;

use async_tungstenite::tungstenite::Message;
use opencas::*;
use serde::__private::de;
use serde::{Deserialize, Serialize};
use uom::si::angle::{degree, radian}; // self,
use uom::si::f32::Time;
use uom::si::f32::*;
use uom::si::length::{foot, meter};
use uom::si::time::second; //Time
use uom::si::velocity::{foot_per_second, knot}; //foot_per_minute //, time

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

// 1.: do lat/lon conversion to distance with haversine formula
// formula is:
// a =sin(delta_lat/2)^2 + cos(lat1)*cos(lat2)*sin(del_lng/2)^2
// c = atan2(sqrt(a), sqrt(1-a))
// distance = R * c
//
// 2.: calculate the bearing angle and make it relative to the heading angle of the ownship
// => get a relative (mathematical correct) angle from heading of the ownship to position of intruder
// heading = atan2(sin(del_lng)*cos(lat2), (cos(lat1)*sin(lat2)-sin(lat1)*cos(lat2)*cos(del_lng)))
// see https://www.movable-type.co.uk/scripts/latlong.html
fn haversine(ownship: &AircraftState, intruder: &AircraftState) -> (Length, Angle) {
    //basics for calculation
    const RADIUS_EARTH: f32 = 6271e3; // in meter
    let radius = RADIUS_EARTH + ownship.altitude.get::<meter>();
    let del_lat = (intruder.lat - ownship.lat).get::<radian>();
    let del_lng = (intruder.lng - ownship.lng).get::<radian>();

    // distance calc
    let a = (del_lat / 2.0).sin().powi(2)
        + ownship.lat.get::<radian>().cos()
            * intruder.lat.get::<radian>().cos()
            * (del_lng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    let range = Length::new::<meter>(radius * c);

    //heading calc: convert atan2() (+/-180 deg) into true bearings => easier to calculate
    let bearing = (del_lng.sin() * intruder.lat.get::<radian>().cos()).atan2(
        ownship.lat.get::<radian>().cos() * intruder.lat.get::<radian>().sin()
            - ownship.lat.get::<radian>().sin()
                * intruder.lat.get::<radian>().cos()
                * del_lng.cos(),
    );
    // check if initial bearing is correct (checked with online tool - see reference above)
    //println!("bearing: {:?}", bearing * 180.0 / PI);
    
    // this step takes into account that signed addition happens (theta > 180deg => -(360 -x); vise versa)
    let theta = Angle::new::<radian>(match ownship.heading.get::<radian>() - bearing {
        b if b < PI => b,
        _ => ownship.heading.get::<radian>() - bearing - 2.0 * PI,
    });
    (range, theta)
    //ranging error ~1% which is surprisingly high => maybe f32 not precise enough?
}

// calculate relative heading angles between intruder and ownship
/* If I am correct, the aviation industry measures the heading angle clockwise
while the nerual network expects values counter-clockwise (mathmetical sense)
therefore, you need an angle conversion from clockwise to counter-clockwise while
still representing the relative angle between ownship and intruder correctly */
fn heading_angles(ownship: &AircraftState, intruder: &AircraftState) -> Angle {
    //(mathmetical correct angle intruder) - (mathmetical correct angle ownship)
    //(360deg - intr.heading(clockwise)) - (360deg - own.heading(clockwise))
    // 360deg -intr.heading(cw)- 360deg + own.headin(cw)
    match (ownship.heading - intruder.heading).get::<degree>() {
        // use the "inverse" plane (0..180)
        psi if (-360.0..-180.0).contains(&psi) => Angle::new::<degree>(360.0 + psi),
        psi if (180.0..360.0).contains(&psi) => Angle::new::<degree>(psi - 360.0),
        _ => ownship.heading - intruder.heading,
    }
    // be careful here!
    // Paper says it is in degrees, trained networks say it is in radian  -> whats the training data?
}

// calculate relative altitudes between intruder and ownship
// If value positive => Intruder above ownship - else below
fn relative_altitudes(ownship: &AircraftState, intruder: &AircraftState) -> Length {
    intruder.altitude - ownship.altitude
}

// calculate tau until horizontal collision THE HARDEST CALCULATIO OF THEM ALL...
// After a lot of confusion and consideration, the final decision comes from the paper
// "Julian/Sharma/Jeannin/Kochenderfer: Verifying Aircraft Collision Avoidance Neural Networks Through Linear Approximations of Safe Regions"
// which states that tau is equal to tau = (r - r_p)/v_rel
// r==horizontal separation aka range; r_p==safety range (minimal distance for NMAC -> in paper == 500ft); v_rel== relative velocity
fn calc_tau_horizontal(ownship: &AircraftState, intruder: &AircraftState) -> Time {
    // get range
    let (range, theta) = haversine(ownship, intruder);
    let r_p = Length::new::<foot>(500.0);

    // get relative velocity relative to north => should make it a bit easier than doing the calculations in a transformed system
    let v_fwrd_ownship = ownship.groundspeed * ownship.heading.cos();
    let v_sidewrd_ownship = ownship.groundspeed * ownship.heading.sin();
    let v_fwrd_intruder = intruder.groundspeed * intruder.heading.cos();
    let v_sidewrd_intruder = intruder.groundspeed * intruder.heading.sin();

    // Speed relative to ownship V_io
    let v_fwrd = v_fwrd_intruder - v_fwrd_ownship;
    let v_sdwrd = v_sidewrd_intruder - v_sidewrd_ownship;

    // do I need to do the mathematical conversion here or can I just trust the uom lib to do it correctly afterwards?
    let v_rel = Velocity::new::<foot_per_second>(
        (v_fwrd.get::<foot_per_second>().powi(2) + v_sdwrd.get::<foot_per_second>().powi(2)).sqrt(),
    );

    // get x/y direction for vector math
    let x_direction = (range - r_p) * (-theta).sin();
    let y_direction = (range - r_p) * (-theta).cos();

    // math from "Collision Avoidance Law Using Information Amount" Seiya Ueno and Takehiro Higuchi
    let tau =
        -(x_direction * v_sdwrd + y_direction * v_fwrd) / (v_sdwrd * v_sdwrd + v_fwrd * v_fwrd);
    
        tau
    }
    /*
    let alpha_rel = v_fwrd.atan2(v_sdwrd);
    println!("v_rel über alter Ansatz: {:?}", v_rel.get::<knot>());
    println!("v_fwrd über alter Ansatz: {:?}", v_fwrd.get::<knot>());
    println!("v_sdwrd über alter Ansatz: {:?}", v_sdwrd.get::<knot>());
    println!("alpha_rel über alter Ansatz: {:?}", alpha_rel.get::<degree>());
    */
    
// calculate tau until vertical collision (it's not pretty.. but it works)
fn calc_tau_vertical(ownship: &AircraftState, intruder: &AircraftState) -> Time {
    // first, get relative altitudes
    let h_p = Length::new::<foot>(100.0); // safety margin above and below ownship
    let altitude = match relative_altitudes(ownship, intruder) {
        alt if alt.is_sign_positive() => alt - h_p,
        _ => relative_altitudes(ownship, intruder) + h_p,
    };
    //println!("Altitude: {:?}", altitude.get::<foot>());

    let delta_speed = intruder.vertical_speed - ownship.vertical_speed;
    //println!("delta speed: {:?}", delta_speed.get::<foot_per_second>());
    -(altitude) / delta_speed
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
            let mut now = Instant::now();
            let cur_ts = Time::new::<second>(leaf.ts);

            // Next frame begins
            if cur_ts > ts {
                //calculations for HCAS
                let psi = heading_angles(&user, &ai);
                let (range, theta) = haversine(&user, &ai);
                let tau_vertical = calc_tau_vertical(&user, &ai);

                // do inference for hcas
                let (adv_h, _) = hcas.process_polar(tau_vertical, range, theta, psi);
                hcas.last_advisory = adv_h;

                println!("Processed frame, time consumed: {:?}", now.elapsed(),);
                println!("{:?}", adv_h);

                //new time
                now = Instant::now();

                // calculations for VCAS
                let tau_horizontal = calc_tau_horizontal(&user, &ai);
                let altitude_difference = relative_altitudes(&user, &ai);
                let vertical_speed_user = user.vertical_speed; // needs to be foot per minute at inference
                let vertical_speed_ai = ai.vertical_speed; // needs to be foot per minute

                // do inference for vcas
                
                let (adv_v, _) = vcas.process(
                    altitude_difference,
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

#[cfg(test)]
mod test {
    use super::*;
    use uom::si::length::kilometer;
    //use uom::si::velocity::{foot_per_second, knot}; //foot_per_minute //, time

    #[test]
    pub fn test_hcas_processing() {
        //Instantiate HCAS
        let mut hcas = HCas {
            last_advisory: HAdvisory::ClearOfConflict,
        };

        // Init Aircraft Structs
        let user = AircraftState {
            groundspeed: Velocity::new::<knot>(8.0),
            vertical_speed: Velocity::new::<foot_per_second>(300.0),
            lat: Angle::new::<degree>(50.06638889),
            lng: Angle::new::<degree>(-5.08138889),
            altitude: Length::new::<foot>(2100.0),
            heading: Angle::new::<degree>(345.0),
        };

        let ai = AircraftState {
            groundspeed: Velocity::new::<knot>(14.0),
            vertical_speed: Velocity::new::<foot_per_second>(500.0),
            lat: Angle::new::<degree>(50.14388889),
            lng: Angle::new::<degree>(-5.03666667),
            altitude: Length::new::<foot>(1000.0),
            heading: Angle::new::<degree>(250.0),
        };

        // for sake of testing
        /*
        println!("Print Primary Parameters: User");
        println!("Altitude {:?}", user.altitude.get::<foot>());
        println!("Vertical Speed {:?}",user.vertical_speed.get::<foot_per_second>());
        println!("Groundspeed {:?}", user.groundspeed.get::<foot_per_second>());
        println!("Lattitude {:?}", user.lat.get::<degree>());
        println!("Longitude {:?}", user.lng.get::<degree>());
        println!("Heading {:?}", user.heading.get::<degree>());
        
        println!("Print Primary Parameters: AI");
        println!("Altitude {:?}", ai.altitude.get::<foot>());
        println!("Vertical Speed {:?}",ai.vertical_speed.get::<foot_per_second>());
        println!("Groundspeed {:?}", ai.groundspeed.get::<foot_per_second>());
        println!("Lattitude {:?}", ai.lat.get::<degree>());
        println!("Longitude {:?}", ai.lng.get::<degree>());
        println!("Heading {:?}", ai.heading.get::<degree>());
        */
        let now = Instant::now();

        // Calculate params for networks
        let psi = heading_angles(&user, &ai);
        let (range, theta) = haversine(&user, &ai);
        let tau_vertical = calc_tau_vertical(&user, &ai);
        
        //println!("tau vertical: {:?}", tau_vertical);
        //println!("range: {:?}", range.get::<kilometer>());
        //println!("theta: {:?}", theta.get::<degree>());
        //println!("psi: {:?}", psi.get::<degree>());

        // do inference for hcas
        let (adv_h, _) = hcas.process_polar(tau_vertical, range, theta, psi);
        hcas.last_advisory = adv_h;

        println!("Processed frame, time consumed: {:?}", now.elapsed());
        println!("{:?}", adv_h);
    }

    #[test]
    pub fn test_vcas_processing() {
        //Instantiate VCAS
        let mut vcas = VCas {
            last_advisory: VAdvisory::ClearOfConflict,
        };

        // Init Aircraft Structs
        let user = AircraftState {
            groundspeed: Velocity::new::<foot_per_second>(600.0),
            vertical_speed: Velocity::new::<foot_per_second>(300.0),
            lat: Angle::new::<degree>(50.06638889),
            lng: Angle::new::<degree>(-5.08138889),
            altitude: Length::new::<foot>(1200.0),
            heading: Angle::new::<degree>(45.0),
        };

        let ai = AircraftState {
            groundspeed: Velocity::new::<foot_per_second>(600.0),
            vertical_speed: Velocity::new::<foot_per_second>(500.0),
            lat: Angle::new::<degree>(50.14388889),
            lng: Angle::new::<degree>(-5.03666667),
            altitude: Length::new::<foot>(1000.0),
            heading: Angle::new::<degree>(90.0),
        };

        /*
        // for sake of testing
        println!("Print Primary Parameters: User");
        println!("Groundspeed {:?}", user.groundspeed);
        println!("Vertical Speed {:?}", user.vertical_speed);
        println!("Lattitude {:?}", user.lat);
        println!("Longitude {:?}", user.lng);
        println!("Altitude {:?}", user.altitude);
        println!("Heading {:?}", user.heading);

        println!("Print Primary Parameters: AI");
        println!("Groundspeed {:?}", ai.groundspeed);
        println!("Vertical Speed {:?}", ai.vertical_speed);
        println!("Lattitude {:?}", ai.lat);
        println!("Longitude {:?}", ai.lng);
        println!("Altitude {:?}", ai.altitude);
        println!("Heading {:?}", ai.heading);
        */

        let now = Instant::now();

        // Calculate params for networks
        let tau_horizontal = calc_tau_horizontal(&user, &ai);
        let altitude_difference = relative_altitudes(&user, &ai);
        let vertical_speed_user = user.vertical_speed;
        let vertical_speed_ai = ai.vertical_speed;

        // do inference for vcas
        let (adv_v, _) = vcas.process(
            altitude_difference,
            vertical_speed_user,
            vertical_speed_ai,
            tau_horizontal,
        );

        vcas.last_advisory = adv_v;

        println!("Processed frame, time consumed: {:?}", now.elapsed(),);
        println!("{:?}", adv_v);
    }
}
