use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;
use std::time::Duration;
use uom::si::{
    angle::radian,
    f32::{Angle, Length, Time, Velocity},
    length::foot,
    time::second, velocity::foot_per_minute,
};

use opencas::*;

/// This code is used to benchmark HorizontalCAS
fn criterion_benchmark_horizontal(c: &mut Criterion) {
    let mut group = c.benchmark_group("hcas");

    let mut rng = rand::thread_rng();

    // genrate random inputs for evaluation
    let range = Length::new::<foot>(rng.gen_range(0.0f32..56e3));
    let theta = Angle::new::<radian>(rng.gen_range(-10.0f32..10.0));
    let psi = Angle::new::<radian>(rng.gen_range(-10.0f32..10.0));

    // iterate through all networks
    for pra in [
        HAdvisory::ClearOfConflict,
        HAdvisory::WeakLeft,
        HAdvisory::WeakRight,
        HAdvisory::StrongLeft,
        HAdvisory::StrongRight,
    ] {
        for tau in [0, 5, 10, 15, 20, 30, 40, 60].iter() {
            let bench_name = format!("pra={pra:?} tau={tau:02}");
            let tau = Time::new::<second>(*tau as f32);
            let mut cas = HCas { last_advisory: pra };

            group.bench_function(&bench_name, |b| {
                b.iter(|| cas.process(tau, range, theta, psi))
            });
        }
    }
}


/// This code is used to benchmark VerticalCAS
fn criterion_benchmark_vertical(c: &mut Criterion) {
    let mut group = c.benchmark_group("vcas");

    let mut rng = rand::thread_rng();

    // genrate random inputs for evaluation
    let height = Length::new::<foot>(rng.gen_range(-8e30f32..8e3));
    let vert_speed_homeship = Velocity::new::<foot_per_minute>(rng.gen_range(-6e3f32..6e3));
    let vert_speed_intruder = Velocity::new::<foot_per_minute>(rng.gen_range(-6e3f32..6e3));
    let tau = Time::new::<second>(rng.gen_range(0f32..40.0));
    
    // iterate through all networks
    for pra in [
        VAdvisory::ClearOfConflict,
        VAdvisory::DoNotClimb,
        VAdvisory::DoNotDescend,
        VAdvisory::Climb1500,
        VAdvisory::Descend1500,
        VAdvisory::StrengthenClimb1500,
        VAdvisory::StrengthenDescend1500,
        VAdvisory::StrengthenClimb2500,
        VAdvisory::StrengthenDescend2500
    ] {
            let bench_name = format!("pra={pra:?}");
            let mut cas = VCas { last_advisory: pra };

            group.bench_function(&bench_name, |b| {
                b.iter(|| cas.process(height, vert_speed_homeship, vert_speed_intruder, tau))
            });
        
    }
}

/// run the benchmark on both CAS
criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_secs(3));
    targets = criterion_benchmark_horizontal, criterion_benchmark_vertical
}

criterion_main!(benches);
