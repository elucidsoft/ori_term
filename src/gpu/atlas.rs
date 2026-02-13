//! Multi-page glyph atlas with 2D rectangle bin packing and LRU eviction.
//!
//! Stores rasterized glyph bitmaps in a GPU texture array (`D2Array`).
//! Uses Guillotine best-short-side-fit packing within each page.
//! Supports multiple grayscale pages that grow on demand (up to `max_pages`),
//! with LRU page eviction when all pages are full.

use std::collections::HashMap;

use crate::font::FaceIdx;
use crate::icons::Icon;
use crate::render::{FontSet, FontStyle};

/// Atlas page size in pixels (width = height).
const PAGE_SIZE: u32 = 2048;

/// Maximum number of atlas pages before LRU eviction kicks in.
const MAX_PAGES: u32 = 4;

/// Cache key: character + style + size at 1/64th point precision (26.6 fixed-point).
type GlyphKey = (char, FontStyle, u32);

/// Cache key for shaped glyphs: glyph ID + face index + size (26.6 fixed-point).
type ShapedGlyphKey = (u16, FaceIdx, u32);

/// Convert a font size in points to a 26.6 fixed-point size key.
///
/// Uses `(size * 64.0).round()` for 1/64th point precision, matching
/// `FreeType`'s 26.6 convention. This eliminates rounding collisions that
/// occurred with the old `(size * 10.0).round() as u16` key at fractional
/// DPI scales.
pub fn size_key(size: f32) -> u32 {
    (size * 64.0).round() as u32
}

// Axis-aligned rectangle for the packer's free-space tracking.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// 2D rectangle bin packer using the Guillotine best-short-side-fit algorithm.
///
/// Maintains a list of free rectangles within a fixed-size page. When a glyph
/// is packed, the best-fitting free rectangle is split into two smaller ones
/// along the shorter leftover axis.
///
/// Reference: Jukka Jylanki, "A Thousand Ways to Pack the Bin" (2010).
struct RectPacker {
    width: u32,
    height: u32,
    free_rects: Vec<Rect>,
}

impl RectPacker {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            free_rects: vec![Rect {
                x: 0,
                y: 0,
                w: width,
                h: height,
            }],
        }
    }

    /// Find space for a glyph of the given dimensions.
    ///
    /// Returns the top-left position `(x, y)` within the page, or `None`
    /// if no free rectangle can fit the glyph.
    ///
    /// Uses best-short-side-fit: chooses the free rectangle where the shorter
    /// leftover side after placement is minimized, breaking ties by the longer
    /// leftover side. After placement, the chosen rectangle is split via the
    /// Guillotine method (split along the shorter leftover axis).
    fn pack(&mut self, glyph_w: u32, glyph_h: u32) -> Option<(u32, u32)> {
        let mut best_idx = None;
        let mut best_short = u32::MAX;
        let mut best_long = u32::MAX;

        for (i, r) in self.free_rects.iter().enumerate() {
            if r.w >= glyph_w && r.h >= glyph_h {
                let leftover_w = r.w - glyph_w;
                let leftover_h = r.h - glyph_h;
                let short = leftover_w.min(leftover_h);
                let long = leftover_w.max(leftover_h);
                if short < best_short || (short == best_short && long < best_long) {
                    best_idx = Some(i);
                    best_short = short;
                    best_long = long;
                }
            }
        }

        let idx = best_idx?;
        let r = self.free_rects[idx];
        let pos = (r.x, r.y);

        // Guillotine split: remove the chosen rect and add up to two children.
        self.free_rects.swap_remove(idx);
        let leftover_w = r.w - glyph_w;
        let leftover_h = r.h - glyph_h;

        // Split along the shorter leftover axis for better packing.
        if leftover_w < leftover_h {
            // Horizontal split.
            if leftover_w > 0 {
                self.free_rects.push(Rect {
                    x: r.x + glyph_w,
                    y: r.y,
                    w: leftover_w,
                    h: glyph_h,
                });
            }
            if leftover_h > 0 {
                self.free_rects.push(Rect {
                    x: r.x,
                    y: r.y + glyph_h,
                    w: r.w,
                    h: leftover_h,
                });
            }
        } else {
            // Vertical split.
            if leftover_h > 0 {
                self.free_rects.push(Rect {
                    x: r.x,
                    y: r.y + glyph_h,
                    w: glyph_w,
                    h: leftover_h,
                });
            }
            if leftover_w > 0 {
                self.free_rects.push(Rect {
                    x: r.x + glyph_w,
                    y: r.y,
                    w: leftover_w,
                    h: r.h,
                });
            }
        }

        Some(pos)
    }

    /// Reset the packer to a single free rectangle covering the full page.
    fn reset(&mut self) {
        self.free_rects.clear();
        self.free_rects.push(Rect {
            x: 0,
            y: 0,
            w: self.width,
            h: self.height,
        });
    }
}

/// A single page within the atlas texture array.
struct AtlasPage {
    packer: RectPacker,
    /// Frame counter when this page was last accessed (for LRU eviction).
    last_used_frame: u64,
    /// Number of glyphs stored in this page.
    glyph_count: u32,
}

/// UV coordinates, metrics, and page index for a glyph stored in the atlas.
pub struct AtlasEntry {
    pub uv_pos: [f32; 2],
    pub uv_size: [f32; 2],
    pub metrics: GlyphMetrics,
    /// Texture array layer index (page within the atlas).
    pub page: u32,
}

/// Subset of `fontdue::Metrics` stored per glyph.
#[derive(Clone, Copy)]
pub struct GlyphMetrics {
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub ymin: i32,
    pub advance_width: f32,
}

/// Multi-page glyph texture atlas with LRU eviction.
///
/// Uses a `wgpu::Texture` with `D2Array` view dimension. Pages are allocated
/// on demand (starting with 1) up to `MAX_PAGES`. When all pages are full,
/// the least-recently-used page is evicted (its packer is reset and all
/// entries pointing to it are removed).
pub struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pages: Vec<AtlasPage>,
    page_size: u32,
    max_pages: u32,
    /// Monotonically increasing frame counter for LRU tracking.
    frame_counter: u64,
    entries: HashMap<GlyphKey, AtlasEntry>,
    icon_entries: HashMap<(Icon, u16), AtlasEntry>,
    /// Cache for shaped glyphs (glyph ID + face index + size).
    shaped_entries: HashMap<ShapedGlyphKey, AtlasEntry>,
}

impl GlyphAtlas {
    /// Create a new atlas with one initial page.
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = Self::create_texture(device, PAGE_SIZE, MAX_PAGES);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        Self {
            texture,
            view,
            pages: vec![AtlasPage {
                packer: RectPacker::new(PAGE_SIZE, PAGE_SIZE),
                last_used_frame: 0,
                glyph_count: 0,
            }],
            page_size: PAGE_SIZE,
            max_pages: MAX_PAGES,
            frame_counter: 0,
            entries: HashMap::new(),
            icon_entries: HashMap::new(),
            shaped_entries: HashMap::new(),
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Look up a glyph in the atlas, inserting it if missing.
    ///
    /// Rasterizes the glyph via `FontSet` and uploads the bitmap to the GPU texture.
    #[allow(clippy::map_entry, reason = "Entry API unusable: upload_bitmap() borrows &mut self for packing")]
    pub fn get_or_insert(
        &mut self,
        ch: char,
        style: FontStyle,
        glyphs: &mut FontSet,
        queue: &wgpu::Queue,
    ) -> &AtlasEntry {
        let key = (ch, style, size_key(glyphs.size));

        // Cache miss: rasterize, upload, insert.
        // Uses `contains_key` (returns bool) instead of `get` (returns ref)
        // to avoid holding an immutable borrow across the miss path.
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
                self.entries.insert(key, AtlasEntry::empty());
            }
        }

        // Single lookup for both hit and miss paths: get entry + mark LRU.
        let entry = self.entries.get(&key).expect("glyph entry just inserted");
        if let Some(page) = self.pages.get_mut(entry.page as usize) {
            page.last_used_frame = self.frame_counter;
        }
        entry
    }

    /// Look up an icon in the atlas, rasterizing and inserting it if missing.
    #[allow(clippy::map_entry, reason = "Entry API unusable: upload_bitmap() borrows &mut self")]
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

    /// Look up a shaped glyph in the atlas, inserting it if missing.
    ///
    /// Uses a glyph-ID-based key (not codepoint). The `rasterize` callback is
    /// invoked on cache miss to produce the bitmap via `fontdue::rasterize_indexed`.
    #[allow(clippy::map_entry, reason = "Entry API unusable: upload_bitmap() borrows &mut self for packing")]
    pub fn get_or_insert_shaped(
        &mut self,
        glyph_id: u16,
        face_idx: FaceIdx,
        size_q6: u32,
        rasterize: impl FnOnce() -> Option<(fontdue::Metrics, Vec<u8>)>,
        queue: &wgpu::Queue,
    ) -> &AtlasEntry {
        let key = (glyph_id, face_idx, size_q6);

        if !self.shaped_entries.contains_key(&key) {
            if let Some((metrics, bitmap)) = rasterize() {
                let glyph_metrics = GlyphMetrics {
                    width: metrics.width,
                    height: metrics.height,
                    xmin: metrics.xmin,
                    ymin: metrics.ymin,
                    advance_width: metrics.advance_width,
                };
                let entry = self.upload_bitmap(&glyph_metrics, &bitmap, queue);
                self.shaped_entries.insert(key, entry);
            } else {
                self.shaped_entries.insert(key, AtlasEntry::empty());
            }
        }

        let entry = self.shaped_entries.get(&key).expect("shaped entry just inserted");
        if let Some(page) = self.pages.get_mut(entry.page as usize) {
            page.last_used_frame = self.frame_counter;
        }
        entry
    }

    /// Pre-populate the atlas with ASCII printable characters.
    pub fn precache_ascii(&mut self, glyphs: &mut FontSet, queue: &wgpu::Queue) {
        for ch in ' '..='~' {
            self.get_or_insert(ch, FontStyle::Regular, glyphs, queue);
        }
    }

    /// Advance the frame counter. Call once per frame before rendering.
    pub fn begin_frame(&mut self) {
        self.frame_counter += 1;
    }

    /// Clear all entries and reset packing state on all pages.
    ///
    /// Call this when font size changes (atlas needs to be rebuilt).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.icon_entries.clear();
        self.shaped_entries.clear();
        for page in &mut self.pages {
            page.packer.reset();
            page.glyph_count = 0;
        }
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

        // Zero-size bitmap (e.g. space) — return entry with no UV region.
        if gw == 0 || gh == 0 {
            return AtlasEntry {
                uv_pos: [0.0, 0.0],
                uv_size: [0.0, 0.0],
                metrics: *metrics,
                page: 0,
            };
        }

        let (page_idx, pos) = self.find_space(gw, gh);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: pos.0,
                    y: pos.1,
                    z: page_idx as u32,
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

        let ps = self.page_size as f32;
        self.pages[page_idx].glyph_count += 1;

        AtlasEntry {
            uv_pos: [pos.0 as f32 / ps, pos.1 as f32 / ps],
            uv_size: [gw as f32 / ps, gh as f32 / ps],
            metrics: *metrics,
            page: page_idx as u32,
        }
    }

    /// Find space for a glyph in an existing page or grow/evict as needed.
    fn find_space(&mut self, w: u32, h: u32) -> (usize, (u32, u32)) {
        // Try existing pages.
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(pos) = page.packer.pack(w, h) {
                return (i, pos);
            }
        }

        // All existing pages full. Can we add a new page?
        if (self.pages.len() as u32) < self.max_pages {
            let new_idx = self.pages.len();
            self.pages.push(AtlasPage {
                packer: RectPacker::new(self.page_size, self.page_size),
                last_used_frame: self.frame_counter,
                glyph_count: 0,
            });
            // Texture was pre-allocated with max_pages layers — no recreation needed.
            let pos = self.pages[new_idx]
                .packer
                .pack(w, h)
                .expect("fresh page must have space");
            return (new_idx, pos);
        }

        // All pages full and at max. Evict the least-recently-used page.
        let lru_idx = self
            .pages
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| p.last_used_frame)
            .map(|(i, _)| i)
            .expect("must have at least one page");

        crate::log(&format!(
            "atlas: evicting page {lru_idx} (frame {} vs current {})",
            self.pages[lru_idx].last_used_frame, self.frame_counter,
        ));

        self.pages[lru_idx].packer.reset();
        self.pages[lru_idx].glyph_count = 0;
        self.pages[lru_idx].last_used_frame = self.frame_counter;

        // Remove all entries pointing to the evicted page.
        self.entries.retain(|_, e| e.page as usize != lru_idx);
        self.icon_entries.retain(|_, e| e.page as usize != lru_idx);
        self.shaped_entries.retain(|_, e| e.page as usize != lru_idx);

        let pos = self.pages[lru_idx]
            .packer
            .pack(w, h)
            .expect("freshly cleared page must have space");
        (lru_idx, pos)
    }

    /// Create the backing texture array with pre-allocated layers.
    fn create_texture(device: &wgpu::Device, page_size: u32, max_pages: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: page_size,
                height: page_size,
                depth_or_array_layers: max_pages,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }
}

impl AtlasEntry {
    /// A zero-size entry for glyphs that have no bitmap (e.g. space, missing glyph).
    fn empty() -> Self {
        Self {
            uv_pos: [0.0, 0.0],
            uv_size: [0.0, 0.0],
            metrics: GlyphMetrics {
                width: 0,
                height: 0,
                xmin: 0,
                ymin: 0,
                advance_width: 0.0,
            },
            page: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_key_26_6_precision() {
        // 16.0pt → 1024
        assert_eq!(size_key(16.0), 1024);
        // 16.5pt → 1056
        assert_eq!(size_key(16.5), 1056);
        // Fractional DPI sizes that would collide with old 0.1pt precision.
        assert_ne!(size_key(13.95), size_key(14.05));
        // Old key: (13.95*10).round()=140, (14.05*10).round()=141 — barely distinct.
        // New key: (13.95*64).round()=893, (14.05*64).round()=899 — clearly distinct.
    }

    #[test]
    fn rect_packer_single_glyph() {
        let mut p = RectPacker::new(2048, 2048);
        let pos = p.pack(16, 20);
        assert_eq!(pos, Some((0, 0)));
    }

    #[test]
    fn rect_packer_multiple_no_overlap() {
        let mut p = RectPacker::new(256, 256);
        let mut packed = Vec::new();
        for _ in 0..50 {
            if let Some((x, y)) = p.pack(16, 20) {
                packed.push((x, y, 16u32, 20u32));
            }
        }
        // Verify no overlaps.
        for (i, a) in packed.iter().enumerate() {
            for b in &packed[i + 1..] {
                let overlap_x = a.0 < b.0 + b.2 && b.0 < a.0 + a.2;
                let overlap_y = a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
                assert!(
                    !(overlap_x && overlap_y),
                    "overlap: ({},{} {}x{}) vs ({},{} {}x{})",
                    a.0, a.1, a.2, a.3, b.0, b.1, b.2, b.3,
                );
            }
        }
    }

    #[test]
    fn rect_packer_page_full() {
        let mut p = RectPacker::new(32, 32);
        // Fill until pack returns None.
        let mut count = 0;
        while p.pack(16, 16).is_some() {
            count += 1;
            // Safety valve — 32x32 can fit at most 4 of 16x16.
            assert!(count <= 4, "packed too many");
        }
        assert_eq!(count, 4);
    }

    #[test]
    fn rect_packer_best_short_side_fit() {
        let mut p = RectPacker::new(100, 100);
        // Pack a glyph that leaves a large area, then pack another.
        let pos1 = p.pack(60, 40);
        assert!(pos1.is_some());
        // Next pack should go to the rectangle with the best short-side fit.
        let pos2 = p.pack(30, 30);
        assert!(pos2.is_some());
        assert_ne!(pos1, pos2);
    }

    #[test]
    fn rect_packer_reset() {
        let mut p = RectPacker::new(32, 32);
        while p.pack(16, 16).is_some() {}
        assert!(p.pack(16, 16).is_none());
        p.reset();
        assert!(p.pack(16, 16).is_some());
    }
}
