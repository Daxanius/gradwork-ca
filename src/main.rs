use std::path::Path;

use gradwork_ca::ca::{CANeighborhood, CARule};
use gradwork_ca::runner::{Runner, RunnerConfig};

use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to experiment config file
    #[arg(value_name = "FILE")]
    file: Option<String>,

    /// Grid width (X dimension)
    #[arg(long, default_value_t = 128)]
    width: usize,

    /// Grid height (Y dimension)
    #[arg(long, default_value_t = 128)]
    height: usize,

    /// Grid depth (Z dimension)
    #[arg(long, default_value_t = 128)]
    depth: usize,

    /// Initial air probability (0.0–1.0)
    #[arg(long, default_value_t = 0.45)]
    air_prob: f64,

    /// Number of CA iterations
    #[arg(long, default_value_t = 6)]
    iterations: usize,

    /// Base RNG seed
    #[arg(long, default_value_t = 1234567890)]
    base_seed: u64,

    /// Number of seeds to run (pilot = 5, full = 30)
    #[arg(long, default_value_t = 30)]
    num_seeds: usize,

    /// Run a single test case instead of full experiment
    #[arg(long)]
    single: bool,
}

#[derive(Debug, Deserialize)]
struct ExperimentConfig {
    grid: GridConfig,
    generator: GeneratorConfig,
    seeds: SeedConfig,
    neighbourhoods: Vec<NeighbourhoodConfig>,
    rulesets: Vec<RuleConfig>,
}

#[derive(Debug, Deserialize)]
struct GridConfig {
    width: usize,
    height: usize,
    depth: usize,
}

#[derive(Debug, Deserialize)]
struct GeneratorConfig {
    air_prob: f64,
    iterations: usize,
}

#[derive(Debug, Deserialize)]
struct SeedConfig {
    base: u64,
    count: usize,
}

#[derive(Debug, Deserialize)]
struct NeighbourhoodConfig {
    #[serde(rename = "type")]
    kind: String,
    radius: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct RuleConfig {
    name: String,
    birth: Vec<usize>,
    survival: Vec<usize>,
}

fn neighbourhoods() -> Vec<CANeighborhood> {
    vec![
        CANeighborhood::von_neumann(),
        CANeighborhood::moore(),
        CANeighborhood::extended_moore(2),
    ]
}

fn rulesets() -> Vec<CARule> {
    vec![
        CARule {
            name: "B567_S456".into(),
            birth: vec![5, 6, 7],
            survival: vec![4, 5, 6],
        },
        CARule {
            name: "B678_S567".into(),
            birth: vec![6, 7, 8],
            survival: vec![5, 6, 7],
        },
    ]
}

fn generate_seeds(n: usize, base: u64) -> Vec<u64> {
    (0..n).map(|i| base + i as u64).collect()
}

fn load_config(path: &std::path::Path) -> ExperimentConfig {
    let text = std::fs::read_to_string(path).expect("Failed to read config file");
    toml::from_str(&text).expect("Invalid config format")
}

fn build_neighbourhood(cfg: &NeighbourhoodConfig) -> CANeighborhood {
    match cfg.kind.as_str() {
        "von_neumann" => CANeighborhood::von_neumann(),
        "moore" => CANeighborhood::moore(),
        "extended_moore" => {
            let r = cfg.radius.unwrap_or(2);
            CANeighborhood::extended_moore(r)
        }
        other => panic!("Unknown neighbourhood type: {other}"),
    }
}

fn build_ruleset(cfg: &RuleConfig) -> CARule {
    CARule {
        name: cfg.name.clone(),
        birth: cfg.birth.clone(),
        survival: cfg.survival.clone(),
    }
}

fn resolve_config(args: &Args) -> RunnerConfig {
    let mut width = args.width;
    let mut height = args.height;
    let mut depth = args.depth;
    let mut air_prob = args.air_prob;
    let mut iterations = args.iterations;
    let mut seeds = generate_seeds(args.num_seeds, args.base_seed);

    let mut neighbourhoods = neighbourhoods();
    let mut rulesets = rulesets();

    if let Some(path) = &args.file {
        let cfg = load_config(Path::new(path));

        width = cfg.grid.width;
        height = cfg.grid.height;
        depth = cfg.grid.depth;

        air_prob = cfg.generator.air_prob;
        iterations = cfg.generator.iterations;

        seeds = generate_seeds(cfg.seeds.count, cfg.seeds.base);

        neighbourhoods = cfg.neighbourhoods.iter().map(build_neighbourhood).collect();

        rulesets = cfg.rulesets.iter().map(build_ruleset).collect();
    }

    RunnerConfig {
        width,
        height,
        depth,
        air_prob,
        iterations,
        seeds,
        neighbourhoods,
        rulesets,
    }
}

fn main() {
    let args = Args::parse();
    let cfg = resolve_config(&args);

    Runner::new(cfg).run();
}
