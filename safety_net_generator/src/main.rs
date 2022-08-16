#[macro_use]
extern crate log;

use std::{
    collections::BTreeSet,
    fs::{self, File},
    path::PathBuf,
    thread::{self, JoinHandle},
    time::Instant,
};

use anyhow::Result;
use clap::Parser;

use uom::si::f32::*;

use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/*


/// Result of an evaluation
/// `T`: Type of evaluation result, e.g. f32
/// `P`: Type of the problem
enum EvaluationResult<P, T> {
    Expected(P),
    Unexpected {
        problem: P,
        expectation: T,
        surprise: T,
    },
}

/// Implement this trait to get a generator
/// `P`: Problem Type
/// `PI`: ProblemIndex
/// `C`: Configuration
/// `S`: Solution
trait SafetyNetGenerator<'a, C, P, PI, S>
where
    C: Deserialize<'a> + Serialize,
    P: Send + Sync,
    PI: Default + Iterator<Item = P> + Deserialize<'a> + Serialize,
    S: Send + Eq,
{
    fn expected(p: &P) -> S;
    fn optimized(p: &P) -> S;
    fn look_for_trouble(&self, mut pi: PI) {
        let capacity = 1024;
        let n_threads = 32;

        let (problem_sender, problem_receiver) = bounded(capacity);
        let (solution_sender, solution_receiver) = bounded(capacity);

        let mut pool = Vec::with_capacity(n_threads);
        for i in 0..n_threads {
            let problem_receiver = problem_receiver.clone();
            let solution_sender = solution_sender.clone();
            pool.push(thread::spawn(move || loop {
                let problem = problem_receiver.recv().expect("unable to receive");

                let expectation = Self::expected(&problem);
                let optimized = Self::optimized(&problem);
                let result = if expectation == optimized {
                    EvaluationResult::Expected(problem)
                } else {
                    EvaluationResult::Unexpected {
                        problem,
                        expectation,
                        surprise: optimized,
                    }
                };

                solution_sender.send(result);
            }));
        }

        let mut problems_submitted = BTreeSet::new();
        while let Some(next_problem) = pi.next() {
            problems_submitted.insert(next_problem.clone());
            problem_sender.send(next_problem);
        }
    }
}

*/

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    batch_size: usize,
    result_dir: PathBuf,
    #[serde(default)]
    threads: usize,
    hcas_config: HCas,
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    config_file: PathBuf,
}

fn main() -> Result<()> {
    std::env::set_var(
        "RUST_LOG",
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
    );
    pretty_env_logger::init();
    let args = Args::parse();

    info!("reading config file {}", args.config_file.display());
    let mut file = File::open(args.config_file)?;
    let mut config: Config = serde_yaml::from_reader(&mut file)?;
    trace!("config parsed:\n{:?}", config);

    if config.threads == 0 {
        config.threads = num_cpus::get();
        debug!("autodected {} threads", config.threads);
    }

    if config.result_dir.is_dir() {
        info!("{} exists and is a directory", config.result_dir.display());
    } else {
        info!("{} doesn't exist, creating it", config.result_dir.display());
        fs::create_dir(&config.result_dir)?;
    }

    info!("launching {} threads", config.threads);
    let mut thread_pool: Vec<JoinHandle<Result<()>>> = Vec::with_capacity(config.threads);
    for thread_id in 0..config.threads {
        let config = config.clone();
        thread_pool.push(thread::spawn(move || {
            debug!("thead {thread_id} launched");

            let mut next_batch_to_do = thread_id;

            while config
                .result_dir
                .join(format!("batch_{next_batch_to_do}.yaml"))
                .is_file()
            {
                next_batch_to_do += config.threads;
            }
            info!("thread {thread_id} continues with batch_{next_batch_to_do}");

            /*
            let re = Regex::new(r"^batch_\d+$").unwrap();
            for entry in fs::read_dir(&pb)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let file_name_string = file_name.to_string_lossy();
                if re.is_match(&file_name_string) {
                    let caps = re.captures(&file_name_string).unwrap();

                    let batch_id: usize = caps[1].parse().unwrap();
                }
            }
            */

            Ok(())
        }));
    }

    for (i, thread) in thread_pool.into_iter().enumerate() {
        thread.join().unwrap()?;
        debug!("joined thread {i}");
    }

    info!("all done, exiting");

    Ok(())
}

//
// Implementation for HCas
//
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HCas {
    x: [Length; 3],
    y: [Length; 3],
    tau: [Time; 3],
    rel_bearing: [Angle; 3],
    batch_size: usize,
}
impl BatchCompute for HCas {
    type Result = u8;
    type Batch = HCasBatch;

    fn get_batch(&self, batch_id: BatchId) -> Self::Batch {
        let global_id = batch_id as u128 * self.batch_size as u128;

        // TODO return the actual problem
        todo!("return an instance of HCasBatch")
    }
}

pub struct HCasBatch {
    hcas: HCas,
    batch_id: usize,
    current_idx: usize,
}

impl Iterator for HCasBatch {
    type Item = Option<Anomaly<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = None;
        todo!("calculate the result of the current hcas inputs");

        self.current_idx += 1;

        result
    }
}

//
// Maschinenraum
//

/// Numerical type identifying a batch
pub type BatchId = usize;

pub trait BatchCompute:
    Clone + core::fmt::Debug + PartialEq + Serialize + DeserializeOwned
{
    /// Type of a result
    type Result: Clone + core::fmt::Debug + PartialEq;
    type Batch: Iterator<Item = Option<Anomaly<Self::Result>>>;

    fn get_batch(&self, batch_id: BatchId) -> Self::Batch;
    fn calibrate_batch_size(&mut self) {
        let mut batch = self.get_batch(0);
        let now = Instant::now();

        for solution in batch {
            match solution {
                Some(Anomaly) => {
                    panic!("oh no");
                }
                None => {
                    info!("kalm");
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anomaly<R>
where
    R: Clone + core::fmt::Debug + PartialEq,
{
    expected: R,
    got: R,
}
/*
pub trait FindAnomaly:
    Sized + Clone + core::fmt::Debug + PartialEq + Serialize + DeserializeOwned
{
    type Input: Clone + core::fmt::Debug + PartialEq + Serialize + DeserializeOwned;
    /// Looks for anomalies based on some input I
    fn find_anomaly(input: Self::Input) -> Option<Anomaly<Self>>;
}
*/
