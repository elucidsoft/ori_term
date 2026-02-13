//! GPU rendering: wgpu state, atlas, pipelines, and domain-specific renderers.

pub mod atlas;
mod builtin_glyphs;
mod color_util;
mod instance_writer;
pub mod pipeline;
mod render_grid;
mod render_overlay;
mod render_settings;
mod render_tab_bar;
pub mod renderer;
pub mod state;

pub(crate) use color_util::srgb_to_linear;
pub use renderer::{FrameParams, GpuRenderer};
pub use state::GpuState;
