use crate::ca::Axis;
use crate::ca::CAContext;
use csv::WriterBuilder;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
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
            run_id: format!(
                "{}__{}__{}x{}x{}__p{:.2}__i{}__s{}",
                Self::slugify(&neighborhood),
                Self::slugify(&ruleset),
                width,
                height,
                depth,
                air_prob,
                iterations,
                seed
            ),

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
        fs::create_dir_all(file_dir)?;
        let path = file_dir.join("metadata.json");
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, self).map_err(std::io::Error::other)
    }

    fn slugify(s: &str) -> String {
        s.to_lowercase().replace([' ', ',', '[', ']'], "")
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

    pub fn set_logs(&mut self, logs: Vec<String>) {
        self.logs = logs;
    }

    pub fn logs_mut(&mut self) -> &mut Vec<String> {
        &mut self.logs
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
            writeln!(file, "{text}")?;
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
    // Identification
    pub run_id: String,
    pub seed: u64,
    pub neighborhood: String,
    pub ruleset: String,

    // Grid parameters
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub iterations: usize,
    pub air_prob: f64,

    // Performance
    pub duration_ms: u128,

    // Global density
    pub v_total: usize,
    pub porosity: f64,

    // Connectivity
    pub n_comp: usize,
    pub v_max: usize,
    pub lcr: f64,
    pub n_islands: usize,

    // Percolation
    pub percolates_x: bool,
    pub percolates_y: bool,
    pub percolates_z: bool,

    // Surface roughness
    pub surface_voxels: usize,
    pub roughness_mean: f64,
    pub roughness_std: f64,

    // Tunnel geometry (largest component only)
    pub tunnel_radius_mean: f64,
    pub tunnel_radius_std: f64,
}

impl RunResults {
    #[must_use]
    pub fn from_context(meta: &RunMetadata, ctx: &CAContext, duration_ms: u128) -> Self {
        // 1. Connected components (6-connectivity)
        let components = ctx.connected_components();
        let v_total = ctx.total_air_cells();
        let n_comp = components.len();
        let v_max = components.iter().map(std::vec::Vec::len).max().unwrap_or(0);
        let lcr = if v_total > 0 {
            v_max as f64 / v_total as f64
        } else {
            0.0
        };

        // 2. Percolation
        let percolates_x = ctx.percolates(&components, Axis::X);
        let percolates_y = ctx.percolates(&components, Axis::Y);
        let percolates_z = ctx.percolates(&components, Axis::Z);

        // 3. Roughness
        let rough = RoughnessStats::from_context(ctx);

        // 4. Distance transform (largest component only)
        let tunnel = TunnelStats::from_context(ctx, &components);

        Self {
            run_id: meta.run_id.clone(),
            seed: meta.seed,
            neighborhood: meta.neighborhood.clone(),
            ruleset: meta.ruleset.clone(),
            width: meta.width,
            height: meta.height,
            depth: meta.depth,
            iterations: meta.iterations,
            air_prob: meta.air_prob,
            duration_ms,
            v_total,
            porosity: v_total as f64 / (meta.width * meta.height * meta.depth) as f64,
            n_comp,
            v_max,
            lcr,
            n_islands: n_comp.saturating_sub(1),
            percolates_x,
            percolates_y,
            percolates_z,
            surface_voxels: rough.count,
            roughness_mean: rough.mean,
            roughness_std: rough.std,
            tunnel_radius_mean: tunnel.mean,
            tunnel_radius_std: tunnel.std,
        }
    }

    pub fn save(&self, file_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        self.append_results(file_path)?;
        Ok(())
    }

    fn append_results(&self, file_path: &std::path::Path) -> csv::Result<()> {
        fs::create_dir_all(file_path)?;
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

// Helper struct for surface roughness statistics
pub struct RoughnessStats {
    pub count: usize,
    pub mean: f64,
    pub std: f64,
}

impl RoughnessStats {
    #[must_use]
    pub fn from_context(ctx: &CAContext) -> Self {
        let mut values = Vec::new();

        let dirs: Vec<(i32, i32, i32)> = (-1..=1)
            .flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| (x, y, z))))
            .filter(|&(x, y, z)| !(x == 0 && y == 0 && z == 0))
            .collect();

        let max_n = dirs.len() as f64;

        for z in 0..ctx.depth() {
            for y in 0..ctx.height() {
                for x in 0..ctx.width() {
                    if !ctx.get(x, y, z).is_alive() {
                        continue;
                    }

                    let mut air = 0;
                    let mut solid = false;

                    for (dx, dy, dz) in &dirs {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        let nz = z as i32 + dz;

                        if nx < 0 || ny < 0 || nz < 0 {
                            solid = true;
                            continue;
                        }

                        let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);

                        if nx >= ctx.width() || ny >= ctx.height() || nz >= ctx.depth() {
                            solid = true;
                            continue;
                        }

                        if ctx.get(nx, ny, nz).is_alive() {
                            air += 1;
                        } else {
                            solid = true;
                        }
                    }

                    if solid {
                        let r = 1.0 - (air as f64 / max_n);
                        values.push(r);
                    }
                }
            }
        }

        let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
        let var =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len().max(1) as f64;

        Self {
            count: values.len(),
            mean,
            std: var.sqrt(),
        }
    }
}

// Helper struct for tunnel radius statistics
pub struct TunnelStats {
    pub mean: f64,
    pub std: f64,
}

impl TunnelStats {
    #[must_use]
    pub fn from_context(ctx: &CAContext, components: &[Vec<usize>]) -> TunnelStats {
        let Some(largest) = components.iter().max_by_key(|c| c.len()) else {
            return TunnelStats {
                mean: 0.0,
                std: 0.0,
            };
        };

        let mut dist = vec![i32::MAX; ctx.cells().len()];
        let mut queue = VecDeque::new();

        let dirs = [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ];

        // Initialize surface voxels
        for &idx in largest {
            let (x, y, z) = ctx.pos(idx);
            for (dx, dy, dz) in dirs {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                let nz = z as i32 + dz;

                if nx < 0 || ny < 0 || nz < 0 {
                    dist[idx] = 1;
                    queue.push_back(idx);
                    break;
                }

                let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);
                if nx >= ctx.width() || ny >= ctx.height() || nz >= ctx.depth() {
                    dist[idx] = 1;
                    queue.push_back(idx);
                    break;
                }

                if !ctx.get(nx, ny, nz).is_alive() {
                    dist[idx] = 1;
                    queue.push_back(idx);
                    break;
                }
            }
        }

        // BFS
        while let Some(idx) = queue.pop_front() {
            let (x, y, z) = ctx.pos(idx);
            for (dx, dy, dz) in dirs {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                let nz = z as i32 + dz;

                if nx < 0 || ny < 0 || nz < 0 {
                    continue;
                }

                let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);
                if nx >= ctx.width() || ny >= ctx.height() || nz >= ctx.depth() {
                    continue;
                }

                let nidx = ctx.idx(nx, ny, nz);
                if ctx[nidx].is_alive() && dist[nidx] > dist[idx] + 1 {
                    dist[nidx] = dist[idx] + 1;
                    queue.push_back(nidx);
                }
            }
        }

        let values: Vec<f64> = largest
            .iter()
            .map(|&i| dist[i] as f64)
            .filter(|&d| d > 0.0)
            .collect();

        let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
        let var =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len().max(1) as f64;

        TunnelStats {
            mean,
            std: var.sqrt(),
        }
    }
}
