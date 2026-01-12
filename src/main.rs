use std::path::{Path, PathBuf};

use gradwork_ca::ca::{CANeighborhood, CARule};
use gradwork_ca::runner::{Runner, RunnerConfig};

use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to experiment config file
    #[arg(value_name = "FILE")]
    file: String,
}

#[derive(Debug, Deserialize)]
struct ExperimentConfig {
    folder: Option<PathBuf>,
    grid: GridConfig,
    generator: GeneratorConfig,
    seeds: SeedConfig,
    neighborhoods: Vec<NeighborhoodConfig>,
    rulesets: Vec<CARule>,
}

#[derive(Debug, Deserialize)]
struct GridConfig {
    width: usize,
    height: usize,
    depth: usize,
}

#[derive(Debug, Deserialize)]
struct GeneratorConfig {
    air_percentage: f64,
    iterations: usize,
}

#[derive(Debug, Deserialize)]
struct SeedConfig {
    base: u64,
    count: usize,
}

#[derive(Debug, Deserialize)]
struct NeighborhoodConfig {
    #[serde(rename = "type")]
    kind: String,
    radius: Option<i32>,
}

fn generate_seeds(n: usize, base: u64) -> Vec<u64> {
    (0..n).map(|i| base + i as u64).collect()
}

fn load_config(path: &std::path::Path) -> ExperimentConfig {
    let text = std::fs::read_to_string(path).expect("Failed to read config file");
    serde_json::from_str(&text).expect("Invalid config format")
}

fn build_neighborhood(cfg: &NeighborhoodConfig) -> CANeighborhood {
    match cfg.kind.as_str() {
        "von_neumann" => CANeighborhood::von_neumann(),
        "moore" => CANeighborhood::moore(),
        "extended_moore" => {
            let r = cfg.radius.unwrap_or(2);
            CANeighborhood::extended_moore(r)
        }
        other => panic!("Unknown neighborhood type: {other}"),
    }
}

fn resolve_config(args: &Args) -> RunnerConfig {
    let cfg = load_config(Path::new(&args.file));
    let width = cfg.grid.width;
    let height = cfg.grid.height;
    let depth = cfg.grid.depth;

    let air_percentage = cfg.generator.air_percentage;
    let iterations = cfg.generator.iterations;
    let seeds = generate_seeds(cfg.seeds.count, cfg.seeds.base);

    let neighborhoods = cfg.neighborhoods.iter().map(build_neighborhood).collect();

    let rulesets = cfg.rulesets;
    let mut output_dir = PathBuf::from("data");
    if let Some(folder) = cfg.folder {
        output_dir = folder;
    }

    RunnerConfig {
        width,
        height,
        depth,
        air_percentage,
        iterations,
        seeds,
        neighborhoods,
        rulesets,
        output_dir,
    }
}

fn main() {
    let args = Args::parse();
    let cfg = resolve_config(&args);

    Runner::new(cfg).run();
}
