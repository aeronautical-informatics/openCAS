use core::f32::consts::PI;
use uom::si::angle::{degree, radian}; // self,
use uom::si::f32::Time;
use uom::si::f32::*;
use uom::si::length::{foot, meter};
use uom::si::ratio::ratio;
use uom::typenum::P2;
// use uom::si::time::second; //Time
// use uom::si::velocity::{foot_per_second, knot}; //foot_per_minute //, time

/// AircraftState:
/// state parameters of any given aircraft needed for OpenCAS
#[derive(Clone, Default)]
pub struct AircraftState {
    pub groundspeed: Velocity,
    pub vertical_speed: Velocity,
    pub lng: Angle,
    pub lat: Angle,
    pub altitude: Length,
    pub heading: Angle,
}

/// Haversine
///
/// Output:
///
/// + Range ρ - distance between ownship and intruder (absolute distance)
/// + Theta θ - angle from heading of ownship to intruder (positive - from north counterclockwise)
///
/// example:
/// + intruder is in front of ownship -> theta = 0 degrees
/// + intruder is left of ownship -> theta = 90 degrees
/// + intruder is right of ownship -> theta = 270 degrees

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
pub fn haversine(ownship: &AircraftState, intruder: &AircraftState) -> (Length, Angle) {
    //basics for calculation
    const RADIUS_EARTH: f32 = 6271e3; // in meter
    let radius = Length::new::<meter>(RADIUS_EARTH) + ownship.altitude;
    let del_lat = intruder.lat - ownship.lat;
    let del_lng = intruder.lng - ownship.lng;

    // distance calc
    let a = (del_lat / 2.0).sin().powi(P2::new())
        + ownship.lat.cos() * intruder.lat.cos() * (del_lng / 2.0).sin().powi(P2::new());
    let c = 2.0
        * a.sqrt()
            .atan2((Ratio::new::<ratio>(1.0 - a.get::<ratio>())).sqrt());
    let range = radius * c;

    //heading calc: convert atan2() (+/-180 deg) into true bearings => easier to calculate
    let bearing = (del_lng.sin() * intruder.lat.cos()).atan2(
        ownship.lat.cos() * intruder.lat.sin()
            - ownship.lat.sin() * intruder.lat.cos() * del_lng.cos(),
    );
    // check if initial bearing is correct (checked with online tool - see reference above)
    //println!("bearing: {:?}", bearing * 180.0 / PI);

    // this step takes into account that signed addition happens (theta > 180deg => -(360 -x); vise versa)
    let theta = match ownship.heading - bearing {
        b if b < Angle::new::<radian>(PI) => b,
        _ => ownship.heading - bearing - Angle::new::<radian>(2.0 * PI),
    };
    (range, theta)
    //ranging error ~1% which is surprisingly high => maybe f32 not precise enough?
}

/// Relative heading between intruder and ownship
///
/// Output:
///
/// Psi ψ - angle between heading of ownship and heading of intruder (possitive - from north counterclockwise)
///
/// example:
/// + both have the same heading -> psi = 0 degrees
/// + intruder flies perpendicular to the left of ownship -> psi = 90 degrees

/* The aviation industry measures the heading angle clockwise
while the nerual network expects values counter-clockwise (mathmetical sense)
therefore, you need an angle conversion from clockwise to counter-clockwise while
still representing the relative angle between ownship and intruder correctly */
pub fn heading_angles(ownship: &AircraftState, intruder: &AircraftState) -> Angle {
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

/// Relative Altitude
///
/// Output:
///
/// Height difference h - height between ownship and intruder (positive - ownship above intruder)

// calculate relative altitudes between intruder and ownship
// If value positive => Intruder above ownship - else below
pub fn relative_altitudes(ownship: &AircraftState, intruder: &AircraftState) -> Length {
    intruder.altitude - ownship.altitude
}

/// Time until horizontal separation is lost
///
/// Output:
///
/// Tau_horizontal τ - calculation based on CPA (closest point of aproach)

// calculate tau until horizontal collision THE HARDEST CALCULATION OF THEM ALL...
// After a lot of confusion and consideration, the final decision comes from the paper
// "Julian/Sharma/Jeannin/Kochenderfer: Verifying Aircraft Collision Avoidance Neural Networks Through Linear Approximations of Safe Regions"
// which states that tau is equal to tau = (r - r_p)/v_rel
// r==horizontal separation aka range; r_p==safety range (minimal distance for NMAC -> in paper == 500ft); v_rel== relative velocity
pub fn calc_tau_horizontal(ownship: &AircraftState, intruder: &AircraftState) -> Time {
    // get range
    let (range, theta) = haversine(ownship, intruder);
    let r_p = Length::new::<foot>(500.0);

    // get relative velocity relative to north => should make it a bit easier than doing the calculations in a transformed system
    let v_fwrd_ownship = ownship.groundspeed * ownship.heading.cos();
    let v_sidewrd_ownship = ownship.groundspeed * ownship.heading.sin();
    let v_fwrd_intruder = intruder.groundspeed * intruder.heading.cos();
    let v_sidewrd_intruder = intruder.groundspeed * intruder.heading.sin();

    // Speed relative to ownship V_io
    let v_fwrd = v_fwrd_ownship - v_fwrd_intruder;
    let v_sdwrd = v_sidewrd_ownship - v_sidewrd_intruder;

    // do I need to do the mathematical conversion here or can I just trust the uom lib to do it correctly afterwards?
    // let v_rel = Velocity::new::<foot_per_second>(
    //     (v_fwrd.get::<foot_per_second>().powi(2) + v_sdwrd.get::<foot_per_second>().powi(2)).sqrt(),
    // );

    // get x/y direction for vector math
    let x_direction = (range - r_p) * (theta).sin();
    let y_direction = (range - r_p) * (theta).cos();

    // math from "Collision Avoidance Law Using Information Amount" Seiya Ueno and Takehiro Higuchi
    let tau =
        -(x_direction * v_sdwrd - y_direction * v_fwrd) / (v_sdwrd * v_sdwrd + v_fwrd * v_fwrd);

    tau
}

/// Time until vertical separation is lost
///
/// Output:
///
/// Tau_vertical τ

// calculate tau until vertical collision
pub fn calc_tau_vertical(ownship: &AircraftState, intruder: &AircraftState) -> Time {
    // first, get relative altitudes
    let h_p = Length::new::<foot>(100.0); // safety margin above and below ownship
    let altitude = match relative_altitudes(ownship, intruder) {
        alt if alt.is_sign_positive() => alt - h_p,
        _ => relative_altitudes(ownship, intruder) + h_p,
    };

    let delta_speed = intruder.vertical_speed - ownship.vertical_speed;
    -(altitude) / delta_speed
}
