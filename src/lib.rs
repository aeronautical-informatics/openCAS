//! Cas_rs has been created by DLR FT-SSY-AES. It is an updated rust version of previous work done by Stanford Intelligent Systems Laboratory (SISL).
//! SISL has created neural network projects called `VerticalCAS` and `HorizontalCAS`. These systems represent the behaviour of the
//! Airborne Collision Avoidance System (ACAS) which is currently in development to replace TCAS. For more details on SISL work please go
//! [here](https://github.com/sisl/HorizontalCAS).
//! This project currently focuses to use the done work on embedded systems in safety critical environments. Therefore, the orginal code made in
//! Python and Julia is not sufficient.
//! Currently the project only implements the code of HorizontalCAS. The implementation of VerticalCAS is coming soon.

use inference::Vector;
use uom::si::f32::*;
//use uom:: si::u8::*;
use uom::si::angle::radian;
use uom::si::length::foot;
use uom::si::time::second;
//use uom::si::velocity::{foot_per_minute, foot_per_second};

/// This module contains autogenerated instances of all nnet files found in the `nnets` directory.
/// Every nnet file is the written representation of a trained neural network.
/// The nnet file contains all weight matrices and bias vectors (plus some more useful data) that define the network.
/// Therefore, this module 'digitalizes' all files and makes them awailable at runtime without any further file parsing.
#[allow(dead_code)]
#[allow(non_upper_case_globals)]
#[allow(clippy::approx_constant)]
mod nnets {
    use crate::inference::{Layer, NNet};
    use nalgebra::{matrix, vector};

    include!(concat!(env!("OUT_DIR"), "/nnets.rs"));
}

/// This module is inferencing the input data specific to the network with the network itself.
/// In doing so, the input data will be passed through all network layers and an evaluation will be given as the network output.
mod inference;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

/// This will store the last given advisory in order to locate the correct network in the evaluation. 
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

impl HCas {
    /// As you can see by looking into the `nnets` folder, there have been 40 individual networks trained.
    /// The reason behind this is that SISL has had runtime problems with bigger networks.
    /// A way to circumvent this problem is to cut a potentionally big network into many smaller ones.
    /// The smaller networks are in return specifically trained just for a certain set of possible scenerios.
    /// The purpose of this method is to pick the best specialized network to the current situation.
    /// This is done by correlating the remaining time until inpact `tau` [sec] and the last given advisory in `HCas`
    /// to the given networks. once this is done, the current inputs `range` [ft],
    ///  `theta`('bearing' angle from home ship to intruder) [rad] and
    /// `psi` (bearing angle of intruder relative to flight direction of the homeship) [rad] are fed through to
    /// the correct network.   
    pub fn process(
        &mut self,
        tau: Time,
        range: Length,
        theta: Angle,
        psi: Angle,
    ) -> (HAdvisory, f32) {
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

        let nnet = &nnets::HCAS_NNETS[self.last_advisory as usize][index];
        let inputs: Vector<3> = nalgebra::vector![
            range.get::<foot>(),
            theta.get::<radian>(),
            psi.get::<radian>()
        ];

        let evaluated = nnet.eval(inputs);
        let priority = evaluated.max();

        self.last_advisory = match evaluated.imax() {
            0 => HAdvisory::ClearOfConflict,
            1 => HAdvisory::WeakLeft,
            2 => HAdvisory::WeakRight,
            3 => HAdvisory::StrongLeft,
            4 => HAdvisory::StrongRight,
            _ => todo!(),
        };

        (self.last_advisory, priority)
    }
}
