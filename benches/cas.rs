use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;
use std::time::Duration;
use uom::si::{
    angle::radian,
    f32::{Angle, Length, Time},
    length::foot,
    time::second,
};

use opencas::*;

/// This code is used to benchmark the openCAS
fn criterion_benchmark(c: &mut Criterion) {
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

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(1))
        .warm_up_time(Duration::from_secs(3));
    targets = criterion_benchmark
}

criterion_main!(benches);
