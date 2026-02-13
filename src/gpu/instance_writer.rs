//! GPU instance byte buffer writer — shared infrastructure for all render submodules.

use super::pipeline::INSTANCE_STRIDE;

/// Reuse an existing GPU buffer if it has enough capacity, otherwise create a new one.
///
/// When `existing` is `Some` and its size >= the data length, writes data
/// into the existing buffer via `queue.write_buffer()` (no allocation).
/// Otherwise creates a fresh buffer.
pub(super) fn reuse_or_create_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    existing: Option<wgpu::Buffer>,
    data: &[u8],
    label: &str,
) -> wgpu::Buffer {
    let needed = (data.len() as u64).max(INSTANCE_STRIDE);
    if let Some(buf) = existing {
        if buf.size() >= needed {
            if !data.is_empty() {
                queue.write_buffer(&buf, 0, data);
            }
            return buf;
        }
    }
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: needed,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    if !data.is_empty() {
        queue.write_buffer(&buf, 0, data);
    }
    buf
}

/// Writes cell instance data to a byte buffer without unsafe code.
pub(super) struct InstanceWriter {
    data: Vec<u8>,
    pub(super) opacity: f32,
}

impl InstanceWriter {
    pub(super) fn new() -> Self {
        Self {
            data: Vec::with_capacity(4096),
            opacity: 1.0,
        }
    }

    /// Reuse an existing byte buffer, clearing its contents but keeping its allocation.
    pub(super) fn from_buffer(mut buf: Vec<u8>) -> Self {
        buf.clear();
        Self {
            data: buf,
            opacity: 1.0,
        }
    }

    /// Consume the writer, returning the underlying byte buffer for reuse.
    pub(super) fn into_buffer(self) -> Vec<u8> {
        self.data
    }

    /// Premultiply a color by the current opacity.
    fn premultiply(&self, color: [f32; 4]) -> [f32; 4] {
        if self.opacity < 1.0 {
            [
                color[0] * self.opacity,
                color[1] * self.opacity,
                color[2] * self.opacity,
                color[3] * self.opacity,
            ]
        } else {
            color
        }
    }

    /// Push a colored background rectangle (no texture, sharp corners).
    /// When opacity < 1.0, the color is premultiplied by opacity.
    pub(super) fn push_rect(&mut self, x: f32, y: f32, w: f32, h: f32, bg_color: [f32; 4]) {
        self.push_colored_rect(x, y, w, h, bg_color, 0.0);
    }

    /// Push a colored background rectangle with rounded top corners.
    pub(super) fn push_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bg_color: [f32; 4],
        radius: f32,
    ) {
        self.push_colored_rect(x, y, w, h, bg_color, radius);
    }

    /// Push a colored rectangle with all four corners rounded.
    /// Negative radius signals the shader to round all 4 corners.
    pub(super) fn push_all_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bg_color: [f32; 4],
        radius: f32,
    ) {
        self.push_colored_rect(x, y, w, h, bg_color, -radius);
    }

    /// Common path for all background rectangle variants.
    fn push_colored_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bg_color: [f32; 4],
        corner_radius: f32,
    ) {
        let color = self.premultiply(bg_color);
        self.push_raw(
            [x, y],
            [w, h],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            color,
            0,
            corner_radius,
        );
    }

    /// Push a textured glyph quad (alpha-blended).
    /// `bg_color` is passed through to the shader for contrast/correction.
    pub(super) fn push_glyph(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv_pos: [f32; 2],
        uv_size: [f32; 2],
        fg_color: [f32; 4],
        bg_color: [f32; 4],
    ) {
        self.push_raw([x, y], [w, h], uv_pos, uv_size, fg_color, bg_color, 1, 0.0);
    }

    /// Write a full 80-byte instance record.
    #[expect(clippy::too_many_arguments, reason = "Maps 1:1 to the GPU instance struct layout")]
    fn push_raw(
        &mut self,
        pos: [f32; 2],
        size: [f32; 2],
        uv_pos: [f32; 2],
        uv_size: [f32; 2],
        fg_color: [f32; 4],
        bg_color: [f32; 4],
        flags: u32,
        corner_radius: f32,
    ) {
        for &v in &pos {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &size {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &uv_pos {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &uv_size {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &fg_color {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        for &v in &bg_color {
            self.data.extend_from_slice(&v.to_ne_bytes());
        }
        self.data.extend_from_slice(&flags.to_ne_bytes());
        self.data.extend_from_slice(&corner_radius.to_ne_bytes());
        // 8 bytes padding to reach 80-byte stride
        self.data.extend_from_slice(&[0u8; 8]);
    }

    pub(super) fn count(&self) -> u32 {
        (self.data.len() / INSTANCE_STRIDE as usize) as u32
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}
