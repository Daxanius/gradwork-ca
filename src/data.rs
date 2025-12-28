use crate::ca::CAContext;
use csv::WriterBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

static METRICS_LOCK: Mutex<()> = Mutex::new(());

pub fn prepare_run_dir(run_id: &str) -> std::io::Result<()> {
    let run_dir = format!("data/runs/{run_id}");
    fs::create_dir_all(&run_dir)?;
    Ok(())
}

pub fn save_vox(context: &CAContext, run_id: &str) -> std::io::Result<()> {
    let path = format!("data/runs/{run_id}/grid.vox");

    let mut vox = vox_writer::VoxWriter::create_empty();

    for z in 0..context.depth() {
        for y in 0..context.height() {
            for x in 0..context.width() {
                let cell = context.get(x, y, z);
                if cell.is_alive() {
                    vox.add_voxel(x as i32, y as i32, z as i32, 0);
                }
            }
        }
    }

    vox.save_to_file(path).map_err(std::io::Error::other)
}

fn metric_to_color(metric_value: f32, min: f32, max: f32) -> u8 {
    // normalize to 1..255
    let normalized = ((metric_value - min) / (max - min) * 254.0 + 1.0).clamp(1.0, 255.0);
    normalized as u8
}

pub fn save_metadata(meta: &RunMetadata) -> std::io::Result<()> {
    let path = format!("data/runs/{}/metadata.json", meta.run_id);
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, meta).map_err(std::io::Error::other)
}

pub fn append_metrics(row: &MetricsRow) -> csv::Result<()> {
    let _guard = METRICS_LOCK.lock().unwrap();

    let file_path = "data/metrics.csv";
    let file_exists = Path::new(file_path).exists();

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)?;

    let mut writer = WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(file);

    writer.serialize(row)?;
    writer.flush()?;
    Ok(())
}

pub fn save_log(run_id: &str, text: &str) -> std::io::Result<()> {
    let path = format!("data/runs/{}/log.txt", run_id);
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "{}", text)?;
    Ok(())
}

pub fn save_run(
    run_id: String,
    context: &CAContext,
    meta: &RunMetadata,
    metrics: &MetricsRow,
) -> Result<(), Box<dyn std::error::Error>> {
    prepare_run_dir(&run_id)?;

    save_vox(context, &run_id)?;
    save_metadata(meta)?;
    append_metrics(metrics)?;
    save_log(&run_id, "Run completed successfully")?;

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RunMetadata {
    pub run_id: String,
    pub seed: u64,
    pub neighborhood: String,
    pub grid_size: (usize, usize, usize),
    pub iterations: usize,
}

#[derive(Serialize, Debug)]
pub struct MetricsRow {
    pub run_id: String,
    pub iterations: usize,
    pub v_total: usize,
    pub n_comp: usize,
    pub v_max: usize,
    pub lcr: f64,
}
