use rand::{Rng, SeedableRng, rngs::SmallRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

// A cell for a cellular automation engine. Currently just boolean based, use a u8 to avoid bitpacking for performance
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct CACell(pub u8);

impl CACell {
    #[must_use]
    pub fn new(state: u8) -> Self {
        CACell(state)
    }

    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.0 != 0
    }

    pub fn set_state(&mut self, state: u8) {
        self.0 = state;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CANeighborhood {
    pub name: String,
    offsets: Vec<(i32, i32, i32)>,
}

impl CANeighborhood {
    #[must_use]
    pub fn von_neumann() -> Self {
        CANeighborhood {
            name: "von_neumann".to_string(),
            offsets: vec![
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ],
        }
    }

    #[must_use]
    pub fn moore() -> Self {
        let mut offsets = Vec::new();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if x != 0 || y != 0 || z != 0 {
                        offsets.push((x, y, z));
                    }
                }
            }
        }
        Self {
            name: "moore".to_string(),
            offsets,
        }
    }

    #[must_use]
    pub fn extended_moore(radius: i32) -> Self {
        let mut offsets = Vec::new();
        for x in -radius..=radius {
            for y in -radius..=radius {
                for z in -radius..=radius {
                    if x != 0 || y != 0 || z != 0 {
                        offsets.push((x, y, z));
                    }
                }
            }
        }
        Self {
            name: "extended_moore".to_string(),
            offsets,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CARule {
    pub name: String,
    pub birth: Vec<usize>,
    pub survival: Vec<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CAConfig {
    pub neighborhood: CANeighborhood,
    pub rule: CARule,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CAContext {
    width: usize,
    height: usize,
    depth: usize,
    cells: Vec<CACell>,
}

impl CAContext {
    #[must_use]
    pub fn new(width: usize, height: usize, depth: usize) -> Self {
        let cells = vec![CACell::new(0); width * height * depth];
        Self {
            width,
            height,
            depth,
            cells,
        }
    }

    #[must_use]
    pub fn random(
        width: usize,
        height: usize,
        depth: usize,
        seed: u64,
        air_probability: f64,
    ) -> Self {
        let n = width * height * depth;
        let mut cells = Vec::with_capacity(n);

        let mut rng = SmallRng::seed_from_u64(seed);

        for _ in 0..n {
            cells.push(CACell::new(u8::from(rng.random_bool(air_probability))));
        }

        Self {
            width,
            height,
            depth,
            cells,
        }
    }

    #[must_use]
    pub fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        x + self.width * (y + self.height * z)
    }

    #[must_use]
    pub fn pos(&self, index: usize) -> (usize, usize, usize) {
        let z = index / (self.width * self.height);
        let y = (index % (self.width * self.height)) / self.width;
        let x = index % self.width;
        (x, y, z)
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize, z: usize) -> CACell {
        self.cells[self.idx(x, y, z)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, value: CACell) {
        let i = self.idx(x, y, z);
        self.cells[i] = value;
    }

    #[must_use]
    pub fn count_air_neighbors(&self, x: usize, y: usize, z: usize, nb: &CANeighborhood) -> usize {
        let mut count = 0;

        for &(dx, dy, dz) in &nb.offsets {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            let nz = z as i32 + dz;

            if nx < 0 || ny < 0 || nz < 0 {
                continue;
            }
            let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);

            if nx >= self.width || ny >= self.height || nz >= self.depth {
                continue;
            }

            count += self.get(nx, ny, nz).0 as usize;
        }

        count
    }

    #[must_use]
    pub fn total_air_cells(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_alive()).count()
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    #[must_use]
    pub fn cells(&self) -> &[CACell] {
        &self.cells
    }

    pub fn cells_mut(&mut self) -> &mut [CACell] {
        &mut self.cells
    }
}

impl Index<usize> for CAContext {
    type Output = CACell;

    fn index(&self, index: usize) -> &Self::Output {
        &self.cells[index]
    }
}

impl IndexMut<usize> for CAContext {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.cells[index]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CAEngine {
    pub config: CAConfig,
    pub context: CAContext,
    buffer: CAContext,
}

impl CAEngine {
    #[must_use]
    pub fn new(config: CAConfig, context: CAContext) -> Self {
        let buffer = context.clone();

        Self {
            config,
            context,
            buffer,
        }
    }

    pub fn run(&mut self, iterations: usize) {
        for _ in 0..iterations {
            self.run_iteration();
        }
    }

    pub fn run_iteration(&mut self) {
        let nb = &self.config.neighborhood;
        let rule = &self.config.rule;

        // SAFELY split mutable borrows
        let (old, new) = (&self.context, &mut self.buffer);

        new.cells_mut()
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, cell)| {
                let (x, y, z) = old.pos(i);

                let alive_neighbors = old.count_air_neighbors(x, y, z, nb);
                let alive = old[i].is_alive();

                let next = if alive {
                    rule.survival.contains(&alive_neighbors)
                } else {
                    rule.birth.contains(&alive_neighbors)
                };

                cell.set_state(u8::from(next));
            });

        // Swap buffers — O(1)
        std::mem::swap(&mut self.context, &mut self.buffer);
    }
}
