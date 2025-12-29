use std::{path::Path, sync::Mutex};

use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::{
    ca::{CAConfig, CAContext, CAEngine},
    data::{RunInfo, RunMetadata, RunResults},
};

pub struct RunnerConfig {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub air_prob: f64,
    pub iterations: usize,
    pub seeds: Vec<u64>,
    pub neighbourhoods: Vec<crate::ca::CANeighborhood>,
    pub rulesets: Vec<crate::ca::CARule>,
}

pub struct Runner {
    config: RunnerConfig,
    results: Mutex<Vec<RunResults>>,
}

impl Runner {
    #[must_use]
    pub fn new(config: RunnerConfig) -> Self {
        let total_runs = config.neighbourhoods.len() * config.rulesets.len() * config.seeds.len();

        Runner {
            config,
            results: Mutex::new(Vec::with_capacity(total_runs)),
        }
    }

    pub fn run(&self) {
        let total_runs =
            self.config.neighbourhoods.len() * self.config.rulesets.len() * self.config.seeds.len();

        let pb = ProgressBar::new(total_runs as u64);
        pb.set_style(
            ProgressStyle::with_template("[Cavegen] {bar:40.cyan/blue} Cave {pos}/{len}")
                .expect("Failed to set progress bar style")
                .progress_chars("=> "),
        );

        (self.config.neighbourhoods.iter())
            .flat_map(|n| {
                self.config
                    .rulesets
                    .iter()
                    .flat_map(move |r| self.config.seeds.iter().map(move |&s| (n, r, s)))
            })
            .par_bridge()
            .for_each(|(n, r, s)| {
                self.run_single(n, r, s);
                pb.inc(1);
            });

        self.write_results();
        pb.finish_with_message("Cavegen complete");
    }

    fn run_single(
        &self,
        neighborhood: &crate::ca::CANeighborhood,
        rule: &crate::ca::CARule,
        seed: u64,
    ) {
        let context = CAContext::random(
            self.config.width,
            self.config.height,
            self.config.depth,
            seed,
            self.config.air_prob,
        );

        let config = CAConfig {
            neighborhood: neighborhood.clone(),
            rule: rule.clone(),
        };

        let mut engine = CAEngine::new(config, context);
        engine.run(self.config.iterations);

        let info = RunInfo::new(
            RunMetadata::new(
                seed,
                neighborhood.name.clone(),
                self.config.width,
                self.config.height,
                self.config.depth,
                self.config.iterations,
                rule.name.clone(),
                self.config.air_prob,
            ),
            engine.context.clone(),
        );
        info.save(Path::new("data/runs/"))
            .expect("Something went wrong");

        let results = RunResults::new(info.metadata, 0, 0, 0, 0, 0.0);
        let mut res_lock = self.results.lock().unwrap();
        res_lock.push(results);
    }

    fn write_results(&self) {
        let results = self.results.lock().unwrap();

        let file = std::fs::File::create("data/metrics.csv").unwrap();
        let mut writer = csv::Writer::from_writer(file);

        for r in results.iter() {
            writer.serialize(r).unwrap();
        }

        writer.flush().unwrap();
    }
}
