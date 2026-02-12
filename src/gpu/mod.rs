//! GPU rendering: wgpu state, atlas, pipelines, and domain-specific renderers.

pub mod atlas;
mod color_util;
pub mod pipeline;
mod render_grid;
mod render_overlay;
mod render_settings;
mod render_tab_bar;
pub mod renderer;
pub mod state;

pub(crate) use color_util::srgb_to_linear;
pub use state::GpuState;
