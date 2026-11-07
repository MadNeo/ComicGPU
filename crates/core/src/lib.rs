mod archive;
mod filters;
pub use archive::*;
pub use filters::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub deskew: bool,
    pub crop: bool,
    pub paper: f32,
    pub shadow: f32,
    pub bleed: f32,
    pub moire: f32,
    pub grayscale: bool,
    pub upscale: bool,
    pub gpu: bool,
    pub split: bool,
    pub rtl: bool,
    pub excluded: bool,
    pub approved: bool,
}
impl Options {
    pub fn validate(&self) -> anyhow::Result<()> {
        for value in [self.paper, self.shadow, self.bleed, self.moire] {
            anyhow::ensure!(
                value.is_finite() && (0.0..=1.0).contains(&value),
                "Sterkte moet tussen 0 en 1 liggen"
            );
        }
        Ok(())
    }
}
pub mod gpu;
