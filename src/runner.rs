use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Instant};

use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sysinfo::System;

use crate::{
    ca::{CAConfig, CAContext, CAEngine},
    data::{ConfigKey, RunInfo, RunMetadata, RunResults},
};

pub struct RunnerConfig {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub air_prob: f64,
    pub iterations: usize,
    pub seeds: Vec<u64>,
    pub neighborhoods: Vec<crate::ca::CANeighborhood>,
    pub rulesets: Vec<crate::ca::CARule>,
    pub output_dir: PathBuf,
}

pub struct Runner {
    config: RunnerConfig,
    results: Mutex<Vec<RunResults>>,
}

impl Runner {
    #[must_use]
    pub fn new(config: RunnerConfig) -> Self {
        let total_runs = config.neighborhoods.len() * config.rulesets.len() * config.seeds.len();

        Runner {
            config,
            results: Mutex::new(Vec::with_capacity(total_runs)),
        }
    }

    pub fn run(&self) {
        // Ensure directory structure exists
        std::fs::create_dir_all(self.config.output_dir.join("runs"))
            .expect("Failed to create runs directory");

        let total_runs =
            self.config.neighborhoods.len() * self.config.rulesets.len() * self.config.seeds.len();

        let pb = ProgressBar::new(total_runs as u64);
        pb.set_style(
            ProgressStyle::with_template("[Cavegen] {bar:40.cyan/blue} Cave {pos}/{len}")
                .expect("Failed to set progress bar style")
                .progress_chars("=> "),
        );

        (self.config.neighborhoods.iter())
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
        self.write_diversity_stats();
        self.write_hardware_info()
            .expect("Failed to write hardware info");
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
        let mut logs = Vec::new();

        // Time the run
        let now = Instant::now();
        engine.run(self.config.iterations, &mut logs);
        let elapsed = now.elapsed();

        let mut info = RunInfo::new(
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

        info.set_logs(logs);

        let runs_dir = self.config.output_dir.join("runs");
        info.save(&runs_dir).expect("Failed to save run info");

        let results =
            RunResults::from_context(&info.metadata, &engine.context, elapsed.as_millis());
        let mut res_lock = self.results.lock().unwrap();
        res_lock.push(results);
    }

    fn group_by_config(results: &[RunResults]) -> HashMap<ConfigKey, Vec<&RunResults>> {
        let mut map = HashMap::new();

        for r in results {
            let key = ConfigKey {
                neighborhood: r.neighborhood.clone(),
                ruleset: r.ruleset.clone(),
            };

            map.entry(key).or_insert_with(Vec::new).push(r);
        }

        map
    }

    fn write_results(&self) {
        let results = self.results.lock().unwrap();

        let path = self.config.output_dir.join("metrics.csv");
        let file = std::fs::File::create(path).unwrap();
        let mut writer = csv::Writer::from_writer(file);

        for r in results.iter() {
            writer.serialize(r).unwrap();
        }

        writer.flush().unwrap();
    }

    fn write_diversity_stats(&self) {
        let results = self.results.lock().unwrap();
        let grouped = Self::group_by_config(&results);

        let path = self.config.output_dir.join("diversity_stats.csv");
        let file = std::fs::File::create(path).unwrap();
        let mut writer = csv::Writer::from_writer(file);

        for (key, runs) in &grouped {
            let stats = crate::data::DiversityStats::from_runs(key, runs);
            writer.serialize(stats).unwrap();
        }

        writer.flush().unwrap();
    }

    fn write_hardware_info(&self) -> std::io::Result<()> {
        let sys = System::new_all();
        let path = self.config.output_dir.join("hardware.json");
        let file = std::fs::File::create(path).unwrap();
        serde_json::to_writer_pretty(file, &sys)?;
        Ok(())
    }
}
