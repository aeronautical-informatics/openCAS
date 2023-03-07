//! openCAS has been created by DLR FT-SSY-AES. It is an updated rust version of previous work done by Stanford Intelligent Systems Laboratory (SISL).
//! SISL has created neural network projects called `VerticalCAS` and `HorizontalCAS`. These systems represent the behaviour of the
//! Airborne Collision Avoidance System (ACAS) which is currently in development to replace TCAS. For more details on SISL work please go
//! [here](https://github.com/sisl/HorizontalCAS).
//! This project currently focuses to use the done work on embedded systems in safety critical environments. Therefore, the orginal code made in
//! Python and Julia is not sufficient.
#![cfg_attr(not(test), no_std)]

use core::convert::TryFrom;

use inference::Vector;

#[allow(unused_imports)]
use num::Float;

use uom::si::angle::radian;
use uom::si::f32::*;
use uom::si::length::foot;
use uom::si::time::second;
use uom::si::velocity::foot_per_minute;

/// This module contains autogenerated instances of all nnet files found in the `nnets` directory.
/// Every nnet file is the written representation of a trained neural network. The nnet file
/// contains all weight matrices and bias vectors (plus some more useful data) that define the
/// network. Therefore, this module 'digitalizes' all files and makes them available at runtime
/// without any further file parsing.
#[allow(dead_code)]
#[allow(non_upper_case_globals)]
#[allow(clippy::approx_constant)]
mod nnets {
    use crate::inference::{Layer, NNet};
    use nalgebra::{matrix, vector};

    include!(concat!(env!("OUT_DIR"), "/nnets.rs"));
}

/// This module is inferencing the input data specific to the network with the network itself. In
/// doing so, the input data will be passed through all network layers and an evaluation will be
/// given as the network output.
mod inference;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

/// This will store the last given advisory in order to locate the correct network in the
/// evaluation.
pub struct HCas {
    pub last_advisory: HAdvisory,
}

/// HAdvisory stores all possible output evaluations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAdvisory {
    ClearOfConflict = 0,
    WeakLeft = 1,
    WeakRight = 2,
    StrongLeft = 3,
    StrongRight = 4,
}

impl TryFrom<u8> for HAdvisory {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => Self::ClearOfConflict,
            1 => Self::WeakLeft,
            2 => Self::WeakRight,
            3 => Self::StrongLeft,
            4 => Self::StrongRight,
            _ => return Err(()),
        })
    }
}

impl HCas {
    /// HorizontalCAS consists of 40 different neural networks (smaller network = les runtime). The
    /// splitting parameters are:
    ///
    /// + time until impact: tau [sec]
    /// + previous given advisory: pra [-]
    ///
    /// Based on those parameters the right network is chosen.
    ///
    /// HorizontalCAS, as we use it, needs three inputs:
    /// + `range` [ft]: absolute distance between homeship and intruder
    /// + `theta`[rad]: angle from homeships heading to intruder in the mathematical sense
    ///   (counterclockwise)
    /// + `psi` [rad]:  angle of intruder relative to flight direction of the homeship
    ///   (mathematical sense => counterclockwise)
    ///
    /// Example:
    ///
    /// `theta` = 5° => intruder is slidely on the left of the homeships heading.
    /// `psi` = 90° / pi/2 => intruder is flying to the left perpendicular to the homeships
    /// heading. See [figure 3](https://arxiv.org/pdf/1912.07084.pdf).
    pub fn process_polar(
        &mut self,
        tau: Time,
        range: Length,
        theta: Angle,
        psi: Angle,
    ) -> (HAdvisory, f32) {
        self.process_cartesian(
            tau,
            range * (theta.get::<radian>().cos()),
            range * (theta.get::<radian>().sin()),
            psi,
        )
    }

    pub fn process_cartesian(
        &mut self,
        tau: Time,
        forward_range: Length,
        left_range: Length,
        psi: Angle,
    ) -> (HAdvisory, f32) {
        // match the value of tau to the corresponding tau trained networks
        let index = match tau.get::<second>() {
            t if (0.0..5.0).contains(&t) => 0,
            t if (5.0..10.0).contains(&t) => 1,
            t if (10.0..15.0).contains(&t) => 2,
            t if (15.5..20.0).contains(&t) => 3,
            t if (20.0..30.0).contains(&t) => 4,
            t if (30.0..40.0).contains(&t) => 5,
            t if (40.0..60.0).contains(&t) => 6,
            _ => 7,
        };

        // find the correct network by using the last given advisory and the tau index
        let nnet = &nnets::HCAS_NNETS[self.last_advisory as usize][index];

        // generate the network inputs as a vector [x,y,psi]
        let inputs: Vector<3> = nalgebra::vector![
            forward_range.get::<foot>(),
            left_range.get::<foot>(),
            psi.get::<radian>()
        ];

        // do the actual evalutaion (see inference.rs)
        let evaluated = nnet.eval(inputs);

        // find the highest value in the returning vector
        let priority = evaluated.max();

        // find the index of said highest value and map the index to the possible advisories
        // the unwrap will never actually panic
        self.last_advisory = (evaluated.imax() as u8).try_into().unwrap();
        (self.last_advisory, priority)
    }
}

//***** Here begins the verticalCAS *****//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

/// This will store the last given advisory in order to locate the correct network in the
/// evaluation.
pub struct VCas {
    pub last_advisory: VAdvisory,
}

/// VAdvisory stores all possible output evaluations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAdvisory {
    ClearOfConflict = 0,
    DoNotClimb = 1,
    DoNotDescend = 2,
    Descend1500 = 3,
    Climb1500 = 4,
    StrengthenDescend1500 = 5,
    StrengthenClimb1500 = 6,
    StrengthenDescend2500 = 7,
    StrengthenClimb2500 = 8,
}

impl TryFrom<u8> for VAdvisory {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => Self::ClearOfConflict,
            1 => Self::DoNotClimb,
            2 => Self::DoNotDescend,
            3 => Self::Descend1500,
            4 => Self::Climb1500,
            5 => Self::StrengthenDescend1500,
            6 => Self::StrengthenClimb1500,
            7 => Self::StrengthenDescend2500,
            8 => Self::StrengthenClimb2500,
            _ => return Err(()),
        })
    }
}

impl VCas {
    /// The VerticalCAS contains 9 different networks.
    ///
    /// There are 4 specific inputs:
    /// + Relative intruder altitude [ft]: vertical distance between intruder and homeship
    /// + Vertical speed of homeship [ft/min]
    /// + Vertical speed of intruder [ft/min]
    /// + time until horizontal seperation loss: tau [sec]
    pub fn process(
        &mut self,
        height: Length,
        vertical_speed_homeship: Velocity,
        vertical_speed_intruder: Velocity,
        tau: Time,
    ) -> (VAdvisory, f32) {
        // find the correct network by selecting the last given advisory
        let nnet = &nnets::VCAS_NNETS[self.last_advisory as usize];

        // generate input vector for network
        let inputs: Vector<4> = nalgebra::vector![
            height.get::<foot>(),
            vertical_speed_homeship.get::<foot_per_minute>(),
            vertical_speed_intruder.get::<foot_per_minute>(),
            tau.get::<second>()
        ];

        // evaluate the network
        let evaluated = nnet.eval(inputs);

        // find highest value within the the return vector
        let priority = evaluated.max();

        // find index of said highest value and map to the possible advisories
        // the unwrap will never actually panic
        self.last_advisory = (evaluated.imax() as u8).try_into().unwrap();
        (self.last_advisory, priority)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::mem::size_of_val;

    #[test]
    pub fn check_hcas_size() {
        let size = size_of_val(&nnets::HCAS_NNETS);
        assert!(
            size <= 1 << 19,
            "The size of the HCAS_NNETS is bigger than 512 KiB"
        );
    }

    #[test]
    pub fn check_vcas_size() {
        let size = size_of_val(&nnets::VCAS_NNETS);
        assert!(
            size <= 1 << 19,
            "The size of the VCAS_NNETS is bigger than 512 KiB"
        );
    }

    #[test]
    pub fn test_index() {
        let mut vcas = VCas {
            last_advisory: VAdvisory::StrengthenDescend2500,
        };
        let (adv, value) = vcas.process(
            Length::new::<foot>(0.0),
            Velocity::new::<foot_per_minute>(0.0),
            Velocity::new::<foot_per_minute>(0.0),
            Time::new::<second>(15.0),
        );

        println!("adv: {:#?} and value: {:#?}", adv, value);
    }
}
