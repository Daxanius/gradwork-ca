use crate::ca::CAContext;
use csv::WriterBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

fn metric_to_color(metric_value: f32, min: f32, max: f32) -> u8 {
    // normalize to 1..255
    let normalized = ((metric_value - min) / (max - min) * 254.0 + 1.0).clamp(1.0, 255.0);
    normalized as u8
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RunMetadata {
    pub run_id: String,
    pub seed: u64,
    pub air_prob: f64,
    pub neighborhood: String,
    pub ruleset: String,
    pub iterations: usize,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

impl RunMetadata {
    #[must_use]
    pub fn new(
        seed: u64,
        neighborhood: String,
        width: usize,
        height: usize,
        depth: usize,
        iterations: usize,
        ruleset: String,
        air_prob: f64,
    ) -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            seed,
            neighborhood,
            width,
            height,
            depth,
            iterations,
            ruleset,
            air_prob,
        }
    }

    pub fn save(&self, file_dir: &std::path::Path) -> std::io::Result<()> {
        fs::create_dir_all(&file_dir)?;
        let path = file_dir.join("metadata.json");
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, self).map_err(std::io::Error::other)
    }
}

#[derive(Serialize, Debug)]
pub struct RunInfo {
    pub metadata: RunMetadata,
    pub context: CAContext,
    pub logs: Vec<String>,
}

impl RunInfo {
    #[must_use]
    pub fn new(metadata: RunMetadata, context: CAContext) -> Self {
        Self {
            metadata,
            context,
            logs: Vec::new(),
        }
    }

    pub fn log(&mut self, log: String) {
        self.logs.push(log);
    }

    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        let run_dir = dir.join(&self.metadata.run_id);
        fs::create_dir_all(&run_dir)?;
        self.metadata.save(&run_dir)?;
        self.save_log(&run_dir)?;
        self.save_vox(&run_dir)
    }

    fn save_log(&self, run_dir: &std::path::Path) -> std::io::Result<()> {
        let path = run_dir.join("log.txt");
        let mut file = std::fs::File::create(path)?;

        for text in &self.logs {
            writeln!(file, "{}", text)?;
        }

        Ok(())
    }

    fn save_vox(&self, run_dir: &std::path::Path) -> std::io::Result<()> {
        let path = run_dir.join("grid.vox");
        let mut vox = vox_writer::VoxWriter::create_empty();

        for z in 0..self.context.depth() {
            for y in 0..self.context.height() {
                for x in 0..self.context.width() {
                    let cell = self.context.get(x, y, z);
                    if cell.is_alive() {
                        vox.add_voxel(x as i32, y as i32, z as i32, 0);
                    }
                }
            }
        }

        vox.save_to_file(path.to_string_lossy().to_string())
            .map_err(std::io::Error::other)
    }
}

#[derive(Serialize, Debug)]
pub struct RunResults {
    pub run_id: String,
    pub seed: u64,
    pub air_prob: f64,
    pub neighborhood: String,
    pub ruleset: String,
    pub iterations: usize,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub duration_ms: u128,
    pub v_total: usize,
    pub n_comp: usize,
    pub v_max: usize,
    pub lcr: f64,
}

impl RunResults {
    #[must_use]
    pub fn new(
        meta: RunMetadata,
        duration_ms: u128,
        v_total: usize,
        n_comp: usize,
        v_max: usize,
        lcr: f64,
    ) -> Self {
        Self {
            run_id: meta.run_id,
            seed: meta.seed,
            air_prob: meta.air_prob,
            neighborhood: meta.neighborhood,
            ruleset: meta.ruleset,
            iterations: meta.iterations,
            width: meta.width,
            height: meta.height,
            depth: meta.depth,
            duration_ms,
            v_total,
            n_comp,
            v_max,
            lcr,
        }
    }

    pub fn save(&self, file_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        self.append_results(file_path)?;
        Ok(())
    }

    fn append_results(&self, file_path: &std::path::Path) -> csv::Result<()> {
        fs::create_dir_all(&file_path)?;
        let file_exists = Path::new(file_path).exists();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        let mut writer = WriterBuilder::new()
            .has_headers(!file_exists)
            .from_writer(file);

        writer.serialize(self)?;
        writer.flush()?;
        Ok(())
    }
}
