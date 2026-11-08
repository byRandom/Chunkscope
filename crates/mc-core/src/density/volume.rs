//! `DensityVolume`: a box of block positions sampled together.

/// `DensityVolume(sizeX, sizeY, sizeZ, minBlockX, minBlockY, minBlockZ,
/// stepBlockX, stepBlockY, stepBlockZ)`. Values are stored Y-fastest:
/// `index = (z * size_x + x) * size_y + y`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DensityVolume {
    pub size_x: i32,
    pub size_y: i32,
    pub size_z: i32,
    pub min_block_x: i32,
    pub min_block_y: i32,
    pub min_block_z: i32,
    pub step_block_x: i32,
    pub step_block_y: i32,
    pub step_block_z: i32,
}

impl DensityVolume {
    /// A volume with block step 1 on every axis.
    pub const fn new(
        size_x: i32,
        size_y: i32,
        size_z: i32,
        min_x: i32,
        min_y: i32,
        min_z: i32,
    ) -> Self {
        Self::with_steps(size_x, size_y, size_z, min_x, min_y, min_z, 1, 1, 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn with_steps(
        size_x: i32,
        size_y: i32,
        size_z: i32,
        min_block_x: i32,
        min_block_y: i32,
        min_block_z: i32,
        step_block_x: i32,
        step_block_y: i32,
        step_block_z: i32,
    ) -> Self {
        assert!(
            size_x > 0 && size_y > 0 && size_z > 0,
            "volume sizes must be positive"
        );
        Self {
            size_x,
            size_y,
            size_z,
            min_block_x,
            min_block_y,
            min_block_z,
            step_block_x,
            step_block_y,
            step_block_z,
        }
    }

    /// One block column: what `getBaseHeight` samples.
    pub const fn column(x: i32, min_y: i32, height: i32, z: i32) -> Self {
        Self::new(1, height, 1, x, min_y, z)
    }

    pub const fn size(&self) -> usize {
        (self.size_x * self.size_y * self.size_z) as usize
    }

    pub const fn index(&self, x: i32, y: i32, z: i32) -> usize {
        ((z * self.size_x + x) * self.size_y + y) as usize
    }

    pub const fn block_x(&self, i: i32) -> i32 {
        self.min_block_x
            .wrapping_add(i.wrapping_mul(self.step_block_x))
    }

    pub const fn block_y(&self, i: i32) -> i32 {
        self.min_block_y
            .wrapping_add(i.wrapping_mul(self.step_block_y))
    }

    pub const fn block_z(&self, i: i32) -> i32 {
        self.min_block_z
            .wrapping_add(i.wrapping_mul(self.step_block_z))
    }
}

impl DensityVolume {
    /// `maxBlockX()`: `min + size * step - 1` (the last block of the last
    /// step, not the last sampled block).
    pub const fn max_block_x(&self) -> i32 {
        self.min_block_x
            .wrapping_add(self.size_x.wrapping_mul(self.step_block_x))
            .wrapping_sub(1)
    }

    pub const fn max_block_y(&self) -> i32 {
        self.min_block_y
            .wrapping_add(self.size_y.wrapping_mul(self.step_block_y))
            .wrapping_sub(1)
    }

    pub const fn max_block_z(&self) -> i32 {
        self.min_block_z
            .wrapping_add(self.size_z.wrapping_mul(self.step_block_z))
            .wrapping_sub(1)
    }
}
