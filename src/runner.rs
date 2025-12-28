use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use uuid::Uuid;

use crate::{
    ca::{CAConfig, CAContext, CAEngine},
    data::{MetricsRow, RunMetadata, save_run},
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
}

impl Runner {
    #[must_use]
    pub fn new(config: RunnerConfig) -> Self {
        Runner { config }
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

        pb.finish_with_message("Cavegen complete");
    }

    fn run_single(
        &self,
        neighborhood: &crate::ca::CANeighborhood,
        rule: &crate::ca::CARule,
        seed: u64,
    ) {
        let run_id = Uuid::new_v4().to_string();

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

        let meta = RunMetadata {
            run_id: run_id.clone(),
            seed,
            neighborhood: neighborhood.name.clone(),
            grid_size: (self.config.width, self.config.height, self.config.depth),
            iterations: self.config.iterations,
        };

        let metrics = MetricsRow {
            run_id: run_id.clone(),
            iterations: self.config.iterations,
            v_total: 0,
            n_comp: 0,
            v_max: 0,
            lcr: 0.0,
        };

        save_run(run_id, &engine.context, &meta, &metrics).unwrap();
    }
}
