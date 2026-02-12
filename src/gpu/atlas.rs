use std::collections::HashMap;

use crate::icons::Icon;
use crate::render::{FontSet, FontStyle};

/// Cache key includes font size (tenths of a point) so the same atlas
/// can hold glyphs rasterized at different sizes (grid vs UI font).
type GlyphKey = (char, FontStyle, u16);

fn size_key(size: f32) -> u16 {
    (size * 10.0).round() as u16
}

/// UV coordinates and metrics for a glyph stored in the atlas texture.
pub struct AtlasEntry {
    pub uv_pos: [f32; 2],
    pub uv_size: [f32; 2],
    pub metrics: GlyphMetrics,
}

/// Subset of `fontdue::Metrics` that we store per glyph.
#[derive(Clone, Copy)]
pub struct GlyphMetrics {
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub ymin: i32,
    pub advance_width: f32,
}

/// Row-packed glyph texture atlas.
///
/// Stores rasterized glyph bitmaps in a GPU texture (`R8Unorm`).
/// Uses row-based packing: glyphs are placed left-to-right in rows,
/// advancing to the next row when the current one is full.
pub struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    row_x: u32,
    row_y: u32,
    row_tallest: u32,
    entries: HashMap<GlyphKey, AtlasEntry>,
    icon_entries: HashMap<(Icon, u16), AtlasEntry>,
}

impl GlyphAtlas {
    pub fn new(device: &wgpu::Device) -> Self {
        let width = 1024;
        let height = 1024;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width,
            height,
            row_x: 0,
            row_y: 0,
            row_tallest: 0,
            entries: HashMap::new(),
            icon_entries: HashMap::new(),
        }
    }

    /// Look up a glyph in the atlas, inserting it if missing.
    ///
    /// Rasterizes the glyph via `FontSet` and uploads the bitmap to the GPU texture.
    #[allow(clippy::map_entry)] // self.upload_bitmap() borrows &mut self for row packing
    pub fn get_or_insert(
        &mut self,
        ch: char,
        style: FontStyle,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) -> &AtlasEntry {
        let key = (ch, style, size_key(glyphs.size));
        if !self.entries.contains_key(&key) {
            glyphs.ensure(ch, style);
            if let Some((metrics, bitmap)) = glyphs.get(ch, style) {
                let glyph_metrics = GlyphMetrics {
                    width: metrics.width,
                    height: metrics.height,
                    xmin: metrics.xmin,
                    ymin: metrics.ymin,
                    advance_width: metrics.advance_width,
                };
                let entry = self.upload_bitmap(&glyph_metrics, bitmap, queue);
                self.entries.insert(key, entry);
            } else {
                // No glyph available — insert a zero-size entry
                self.entries.insert(
                    key,
                    AtlasEntry {
                        uv_pos: [0.0, 0.0],
                        uv_size: [0.0, 0.0],
                        metrics: GlyphMetrics {
                            width: 0,
                            height: 0,
                            xmin: 0,
                            ymin: 0,
                            advance_width: 0.0,
                        },
                    },
                );
            }
        }
        self.entries.get(&key).expect("glyph entry just inserted")
    }

    /// Look up an icon in the atlas, rasterizing and inserting it if missing.
    #[allow(clippy::map_entry)]
    pub fn get_or_insert_icon(
        &mut self,
        icon: Icon,
        size_px: u16,
        queue: &wgpu::Queue,
    ) -> &AtlasEntry {
        let key = (icon, size_px);
        if !self.icon_entries.contains_key(&key) {
            let bmp = icon.rasterize(u32::from(size_px));
            let metrics = GlyphMetrics {
                width: bmp.width as usize,
                height: bmp.height as usize,
                xmin: 0,
                ymin: 0,
                advance_width: bmp.width as f32,
            };
            let entry = self.upload_bitmap(&metrics, &bmp.data, queue);
            self.icon_entries.insert(key, entry);
        }
        self.icon_entries
            .get(&key)
            .expect("icon entry just inserted")
    }

    /// Get a glyph entry if it already exists in the atlas.
    pub fn get(&self, ch: char, style: FontStyle, font_size: f32) -> Option<&AtlasEntry> {
        self.entries.get(&(ch, style, size_key(font_size)))
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Pre-populate the atlas with ASCII printable characters.
    pub fn precache_ascii(&mut self, glyphs: &mut FontSet, queue: &wgpu::Queue) {
        for ch in ' '..='~' {
            self.get_or_insert(ch, FontStyle::Regular, glyphs, queue);
        }
    }

    /// Clear all entries and reset packing state.
    /// Call this when font size changes (atlas needs to be rebuilt).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.icon_entries.clear();
        self.row_x = 0;
        self.row_y = 0;
        self.row_tallest = 0;
    }

    /// Allocate atlas space, upload bitmap, and return the entry.
    fn upload_bitmap(
        &mut self,
        metrics: &GlyphMetrics,
        bitmap: &[u8],
        queue: &wgpu::Queue,
    ) -> AtlasEntry {
        let gw = metrics.width as u32;
        let gh = metrics.height as u32;

        // Zero-size bitmap (e.g. space) — return entry with no UV region
        if gw == 0 || gh == 0 {
            return AtlasEntry {
                uv_pos: [0.0, 0.0],
                uv_size: [0.0, 0.0],
                metrics: *metrics,
            };
        }

        // Row packing: check if glyph fits in current row
        if self.row_x + gw > self.width {
            // Advance to next row
            self.row_y += self.row_tallest + 1;
            self.row_x = 0;
            self.row_tallest = 0;
        }

        // Check if atlas is full
        if self.row_y + gh > self.height {
            crate::log("glyph atlas full — entry not inserted");
            return AtlasEntry {
                uv_pos: [0.0, 0.0],
                uv_size: [0.0, 0.0],
                metrics: *metrics,
            };
        }

        // Upload bitmap to the atlas texture
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.row_x,
                    y: self.row_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(gw),
                rows_per_image: Some(gh),
            },
            wgpu::Extent3d {
                width: gw,
                height: gh,
                depth_or_array_layers: 1,
            },
        );

        let uv_pos = [
            self.row_x as f32 / self.width as f32,
            self.row_y as f32 / self.height as f32,
        ];
        let uv_size = [
            gw as f32 / self.width as f32,
            gh as f32 / self.height as f32,
        ];

        self.row_x += gw + 1; // 1px gap between entries
        self.row_tallest = self.row_tallest.max(gh);

        AtlasEntry {
            uv_pos,
            uv_size,
            metrics: *metrics,
        }
    }
}
