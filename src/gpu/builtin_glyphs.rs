//! Built-in pixel-perfect rendering for box drawing, block elements, braille, and Powerline glyphs.
//!
//! These bypass the font pipeline entirely, producing geometrically precise
//! results regardless of which font is loaded. Configurable via `builtin_glyphs`
//! config option (default: on).

use super::instance_writer::InstanceWriter;

/// Returns `true` if the character should be rendered as a built-in glyph
/// (bypassing the font pipeline).
#[cfg(test)]
fn is_builtin_glyph(c: char) -> bool {
    matches!(c,
        '\u{2500}'..='\u{257F}' |  // Box Drawing
        '\u{2580}'..='\u{259F}' |  // Block Elements
        '\u{2800}'..='\u{28FF}' |  // Braille Patterns
        '\u{E0A0}'..='\u{E0A3}' |  // Powerline
        '\u{E0B0}'..='\u{E0D4}'    // Powerline Extra
    )
}

/// Render a built-in glyph as geometric primitives into the instance buffer.
///
/// Returns `true` if the character was handled.
#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
pub fn draw_builtin_glyph(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    match c {
        '\u{2500}'..='\u{257F}' => draw_box_drawing(c, x, y, w, h, fg, bg),
        '\u{2580}'..='\u{259F}' => draw_block_element(c, x, y, w, h, fg, bg),
        '\u{2800}'..='\u{28FF}' => draw_braille(c, x, y, w, h, fg, bg),
        '\u{E0A0}'..='\u{E0A3}' | '\u{E0B0}'..='\u{E0D4}' => {
            draw_powerline(c, x, y, w, h, fg, bg)
        }
        _ => false,
    }
}

// Box Drawing (U+2500-U+257F)
//
// Each character is decomposed into segments from/to the cell center.
// Segment directions: left, right, up, down. Each can be thin (1px) or thick (3px).
//
// Encoding: 4 bytes per char — [left, right, up, down]
// 0 = absent, 1 = thin (light), 2 = thick (heavy), 3 = double

/// Line weight for box drawing segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Weight {
    None = 0,
    Light = 1,
    Heavy = 2,
    Double = 3,
}

impl Weight {
    fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::Light,
            2 => Self::Heavy,
            3 => Self::Double,
            _ => Self::None,
        }
    }

    fn is_some(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Segments for a box drawing character: [left, right, up, down].
fn box_segments(c: char) -> [Weight; 4] {
    let idx = (c as u32 - 0x2500) as usize;
    if idx >= BOX_DRAWING_TABLE.len() {
        return [Weight::None; 4];
    }
    let row = BOX_DRAWING_TABLE[idx];
    [
        Weight::from_byte(row[0]),
        Weight::from_byte(row[1]),
        Weight::from_byte(row[2]),
        Weight::from_byte(row[3]),
    ]
}

// Table: [left, right, up, down] for U+2500..U+257F (128 entries).
// 0=none, 1=light, 2=heavy, 3=double.
#[rustfmt::skip]
const BOX_DRAWING_TABLE: [[u8; 4]; 128] = [
    // U+2500-U+250F: horizontal/vertical lines
    [1,1,0,0], // 2500 ─  light horizontal
    [2,2,0,0], // 2501 ━  heavy horizontal
    [0,0,1,1], // 2502 │  light vertical
    [0,0,2,2], // 2503 ┃  heavy vertical
    [1,1,0,0], // 2504 ┄  light triple dash horizontal (render as light)
    [2,2,0,0], // 2505 ┅  heavy triple dash horizontal
    [0,0,1,1], // 2506 ┆  light triple dash vertical
    [0,0,2,2], // 2507 ┇  heavy triple dash vertical
    [1,1,0,0], // 2508 ┈  light quadruple dash horizontal
    [2,2,0,0], // 2509 ┉  heavy quadruple dash horizontal
    [0,0,1,1], // 250A ┊  light quadruple dash vertical
    [0,0,2,2], // 250B ┋  heavy quadruple dash vertical
    [0,1,0,1], // 250C ┌  light down and right
    [0,2,0,1], // 250D ┍  down light and right heavy
    [0,1,0,2], // 250E ┎  down heavy and right light
    [0,2,0,2], // 250F ┏  heavy down and right
    // U+2510-U+251F
    [1,0,0,1], // 2510 ┐  light down and left
    [2,0,0,1], // 2511 ┑  down light and left heavy
    [1,0,0,2], // 2512 ┒  down heavy and left light
    [2,0,0,2], // 2513 ┓  heavy down and left
    [0,1,1,0], // 2514 └  light up and right
    [0,2,1,0], // 2515 ┕  up light and right heavy
    [0,1,2,0], // 2516 ┖  up heavy and right light
    [0,2,2,0], // 2517 ┗  heavy up and right
    [1,0,1,0], // 2518 ┘  light up and left
    [2,0,1,0], // 2519 ┙  up light and left heavy
    [1,0,2,0], // 251A ┚  up heavy and left light
    [2,0,2,0], // 251B ┛  heavy up and left
    [0,1,1,1], // 251C ├  light vertical and right
    [0,2,1,1], // 251D ┝  vertical light and right heavy
    [0,1,2,1], // 251E ┞  up heavy and right down light
    [0,1,1,2], // 251F ┟  down heavy and right up light
    // U+2520-U+252F
    [0,1,2,2], // 2520 ┠  vertical heavy and right light
    [0,2,2,1], // 2521 ┡  down light and right up heavy
    [0,2,1,2], // 2522 ┢  up light and right down heavy
    [0,2,2,2], // 2523 ┣  heavy vertical and right
    [1,0,1,1], // 2524 ┤  light vertical and left
    [2,0,1,1], // 2525 ┥  vertical light and left heavy
    [1,0,2,1], // 2526 ┦  up heavy and left down light
    [1,0,1,2], // 2527 ┧  down heavy and left up light
    [1,0,2,2], // 2528 ┨  vertical heavy and left light
    [2,0,2,1], // 2529 ┩  down light and left up heavy
    [2,0,1,2], // 252A ┪  up light and left down heavy
    [2,0,2,2], // 252B ┫  heavy vertical and left
    [1,1,0,1], // 252C ┬  light down and horizontal
    [2,1,0,1], // 252D ┭  left heavy and right down light
    [1,2,0,1], // 252E ┮  right heavy and left down light
    [2,2,0,1], // 252F ┯  down light and horizontal heavy
    // U+2530-U+253F
    [1,1,0,2], // 2530 ┰  down heavy and horizontal light
    [2,1,0,2], // 2531 ┱  right light and left down heavy
    [1,2,0,2], // 2532 ┲  left light and right down heavy
    [2,2,0,2], // 2533 ┳  heavy down and horizontal
    [1,1,1,0], // 2534 ┴  light up and horizontal
    [2,1,1,0], // 2535 ┵  left heavy and right up light
    [1,2,1,0], // 2536 ┶  right heavy and left up light
    [2,2,1,0], // 2537 ┷  up light and horizontal heavy
    [1,1,2,0], // 2538 ┸  up heavy and horizontal light
    [2,1,2,0], // 2539 ┹  right light and left up heavy
    [1,2,2,0], // 253A ┺  left light and right up heavy
    [2,2,2,0], // 253B ┻  heavy up and horizontal
    [1,1,1,1], // 253C ┼  light vertical and horizontal
    [2,1,1,1], // 253D ┽  left heavy and right vertical light
    [1,2,1,1], // 253E ┾  right heavy and left vertical light
    [2,2,1,1], // 253F ┿  vertical light and horizontal heavy
    // U+2540-U+254F
    [1,1,2,1], // 2540 ╀  up heavy and down horizontal light
    [1,1,1,2], // 2541 ╁  down heavy and up horizontal light
    [1,1,2,2], // 2542 ╂  vertical heavy and horizontal light
    [2,1,2,1], // 2543 ╃  left up heavy and right down light
    [1,2,2,1], // 2544 ╄  right up heavy and left down light
    [2,1,1,2], // 2545 ╅  left down heavy and right up light
    [1,2,1,2], // 2546 ╆  right down heavy and left up light
    [2,2,2,1], // 2547 ╇  down light and up horizontal heavy
    [2,2,1,2], // 2548 ╈  up light and down horizontal heavy
    [2,1,2,2], // 2549 ╉  right light and left vertical heavy
    [1,2,2,2], // 254A ╊  left light and right vertical heavy
    [2,2,2,2], // 254B ╋  heavy vertical and horizontal
    [1,1,0,0], // 254C ╌  light double dash horizontal (render as light)
    [2,2,0,0], // 254D ╍  heavy double dash horizontal
    [0,0,1,1], // 254E ╎  light double dash vertical
    [0,0,2,2], // 254F ╏  heavy double dash vertical
    // U+2550-U+255F: double lines
    [3,3,0,0], // 2550 ═  double horizontal
    [0,0,3,3], // 2551 ║  double vertical
    [0,1,0,3], // 2552 ╒  down single and right double (approx)
    [0,3,0,1], // 2553 ╓  down double and right single
    [0,3,0,3], // 2554 ╔  double down and right
    [1,0,0,3], // 2555 ╕  down single and left double
    [3,0,0,1], // 2556 ╖  down double and left single
    [3,0,0,3], // 2557 ╗  double down and left
    [0,1,3,0], // 2558 ╘  up single and right double
    [0,3,1,0], // 2559 ╙  up double and right single
    [0,3,3,0], // 255A ╚  double up and right
    [1,0,3,0], // 255B ╛  up single and left double
    [3,0,1,0], // 255C ╜  up double and left single
    [3,0,3,0], // 255D ╝  double up and left
    [0,1,3,3], // 255E ╞  vertical single and right double
    [0,3,1,1], // 255F ╟  vertical double and right single
    // U+2560-U+256F
    [0,3,3,3], // 2560 ╠  double vertical and right
    [1,0,3,3], // 2561 ╡  vertical single and left double
    [3,0,1,1], // 2562 ╢  vertical double and left single
    [3,0,3,3], // 2563 ╣  double vertical and left
    [1,1,0,3], // 2564 ╤  down single and horizontal double
    [3,3,0,1], // 2565 ╥  down double and horizontal single
    [3,3,0,3], // 2566 ╦  double down and horizontal
    [1,1,3,0], // 2567 ╧  up single and horizontal double
    [3,3,1,0], // 2568 ╨  up double and horizontal single
    [3,3,3,0], // 2569 ╩  double up and horizontal
    [1,1,3,3], // 256A ╪  vertical single and horizontal double
    [3,3,1,1], // 256B ╫  vertical double and horizontal single
    [3,3,3,3], // 256C ╬  double vertical and horizontal
    [0,0,0,0], // 256D ╭  arc down and right (fallback: corner)
    [0,0,0,0], // 256E ╮  arc down and left
    [0,0,0,0], // 256F ╯  arc up and left
    // U+2570-U+257F
    [0,0,0,0], // 2570 ╰  arc up and right
    [0,0,0,0], // 2571 ╱  light diagonal upper right to lower left
    [0,0,0,0], // 2572 ╲  light diagonal upper left to lower right
    [0,0,0,0], // 2573 ╳  light diagonal cross
    [1,0,0,0], // 2574 ╴  light left
    [0,0,1,0], // 2575 ╵  light up
    [0,1,0,0], // 2576 ╶  light right
    [0,0,0,1], // 2577 ╷  light down
    [2,0,0,0], // 2578 ╸  heavy left
    [0,0,2,0], // 2579 ╹  heavy up
    [0,2,0,0], // 257A ╺  heavy right
    [0,0,0,2], // 257B ╻  heavy down
    [1,2,0,0], // 257C ╼  light left and heavy right
    [0,0,1,2], // 257D ╽  light up and heavy down
    [2,1,0,0], // 257E ╾  heavy left and light right
    [0,0,2,1], // 257F ╿  heavy up and light down
];

#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
fn draw_box_drawing(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    let idx = c as u32 - 0x2500;

    // Rounded corners (U+256D-U+2570): render as simple right-angle corners.
    // True arc rendering would need the shader; this is a reasonable fallback.
    if (0x6D..=0x70).contains(&idx) {
        return draw_rounded_corner(c, x, y, w, h, fg, bg);
    }

    // Diagonals (U+2571-U+2573): render as thin angled lines via small rects.
    if (0x71..=0x73).contains(&idx) {
        return draw_diagonal(c, x, y, w, h, fg, bg);
    }

    let [left, right, up, down] = box_segments(c);
    if !left.is_some() && !right.is_some() && !up.is_some() && !down.is_some() {
        return false;
    }

    let cx = x + (w / 2.0).floor();
    let cy = y + (h / 2.0).floor();
    let thin = 1.0f32.max((w / 8.0).round());
    let thick = (thin * 3.0).min(w / 2.0);

    // Draw each segment.
    draw_h_segment(left, cx, x, cy, thin, thick, fg, bg);
    draw_h_segment(right, x + w, cx, cy, thin, thick, fg, bg);
    draw_v_segment(up, cy, y, cx, thin, thick, w, fg, bg);
    draw_v_segment(down, y + h, cy, cx, thin, thick, w, fg, bg);

    true
}

/// Draw a horizontal segment from `from_x` to `to_x` at vertical center `cy`.
fn draw_h_segment(
    weight: Weight,
    to_x: f32,
    from_x: f32,
    cy: f32,
    thin: f32,
    thick: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) {
    let lx = from_x.min(to_x);
    let rx = from_x.max(to_x);
    let seg_w = rx - lx;
    if seg_w <= 0.0 {
        return;
    }
    match weight {
        Weight::None => {}
        Weight::Light => {
            let t = thin;
            bg.push_rect(lx, cy - (t / 2.0).floor(), seg_w, t, fg);
        }
        Weight::Heavy => {
            let t = thick;
            bg.push_rect(lx, cy - (t / 2.0).floor(), seg_w, t, fg);
        }
        Weight::Double => {
            let gap = (thin * 2.0).max(2.0);
            let t = thin;
            bg.push_rect(lx, cy - (gap / 2.0).floor() - t, seg_w, t, fg);
            bg.push_rect(lx, cy + (gap / 2.0).ceil(), seg_w, t, fg);
        }
    }
}

/// Draw a vertical segment from `from_y` to `to_y` at horizontal center `cx`.
fn draw_v_segment(
    weight: Weight,
    to_y: f32,
    from_y: f32,
    cx: f32,
    thin: f32,
    thick: f32,
    _cell_w: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) {
    let ty = from_y.min(to_y);
    let by = from_y.max(to_y);
    let seg_h = by - ty;
    if seg_h <= 0.0 {
        return;
    }
    match weight {
        Weight::None => {}
        Weight::Light => {
            let t = thin;
            bg.push_rect(cx - (t / 2.0).floor(), ty, t, seg_h, fg);
        }
        Weight::Heavy => {
            let t = thick;
            bg.push_rect(cx - (t / 2.0).floor(), ty, t, seg_h, fg);
        }
        Weight::Double => {
            let gap = (thin * 2.0).max(2.0);
            let t = thin;
            bg.push_rect(cx - (gap / 2.0).floor() - t, ty, t, seg_h, fg);
            bg.push_rect(cx + (gap / 2.0).ceil(), ty, t, seg_h, fg);
        }
    }
}

/// Draw rounded corners (U+256D-U+2570) as right-angle segments.
#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
fn draw_rounded_corner(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    let thin = 1.0f32.max((w / 8.0).round());
    let thick = thin * 3.0;
    let cx = x + (w / 2.0).floor();
    let cy = y + (h / 2.0).floor();
    // Render as simple corners (matching the straight-line versions).
    match c {
        '\u{256D}' => {
            // Arc down and right → ┌
            draw_h_segment(Weight::Light, x + w, cx, cy, thin, thick, fg, bg);
            draw_v_segment(Weight::Light, y + h, cy, cx, thin, thick, w, fg, bg);
        }
        '\u{256E}' => {
            // Arc down and left → ┐
            draw_h_segment(Weight::Light, cx, x, cy, thin, thick, fg, bg);
            draw_v_segment(Weight::Light, y + h, cy, cx, thin, thick, w, fg, bg);
        }
        '\u{256F}' => {
            // Arc up and left → ┘
            draw_h_segment(Weight::Light, cx, x, cy, thin, thick, fg, bg);
            draw_v_segment(Weight::Light, cy, y, cx, thin, thick, w, fg, bg);
        }
        '\u{2570}' => {
            // Arc up and right → └
            draw_h_segment(Weight::Light, x + w, cx, cy, thin, thick, fg, bg);
            draw_v_segment(Weight::Light, cy, y, cx, thin, thick, w, fg, bg);
        }
        _ => return false,
    }
    true
}

/// Draw diagonal lines (U+2571-U+2573) as small rectangular steps.
#[expect(clippy::many_single_char_names, reason = "Geometric drawing")]
fn draw_diagonal(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    let steps = h as usize;
    let thin = 1.0f32.max((w / 8.0).round());

    match c {
        '\u{2571}' => {
            // ╱ upper right to lower left
            for i in 0..steps {
                let frac = i as f32 / h;
                let px = x + w - w * frac - thin;
                let py = y + i as f32;
                bg.push_rect(px, py, thin, 1.0, fg);
            }
        }
        '\u{2572}' => {
            // ╲ upper left to lower right
            for i in 0..steps {
                let frac = i as f32 / h;
                let px = x + w * frac;
                let py = y + i as f32;
                bg.push_rect(px, py, thin, 1.0, fg);
            }
        }
        '\u{2573}' => {
            // ╳ diagonal cross (both diagonals)
            for i in 0..steps {
                let frac = i as f32 / h;
                let py = y + i as f32;
                bg.push_rect(x + w - w * frac - thin, py, thin, 1.0, fg);
                bg.push_rect(x + w * frac, py, thin, 1.0, fg);
            }
        }
        _ => return false,
    }
    true
}

// Block Elements (U+2580-U+259F)

#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
fn draw_block_element(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    match c {
        // Upper half block
        '\u{2580}' => bg.push_rect(x, y, w, (h / 2.0).round(), fg),
        // Lower N/8 blocks (U+2581-U+2587)
        '\u{2581}'..='\u{2587}' => {
            let eighths = (c as u32 - 0x2580) as f32;
            let bh = (h * eighths / 8.0).round();
            bg.push_rect(x, y + h - bh, w, bh, fg);
        }
        // Full block
        '\u{2588}' => bg.push_rect(x, y, w, h, fg),
        // Left N/8 blocks (U+2589-U+258F): 7/8 down to 1/8
        '\u{2589}'..='\u{258F}' => {
            let eighths = (0x2590 - c as u32) as f32;
            bg.push_rect(x, y, (w * eighths / 8.0).round(), h, fg);
        }
        // Right half
        '\u{2590}' => {
            let hw = (w / 2.0).round();
            bg.push_rect(x + w - hw, y, hw, h, fg);
        }
        // Shade blocks (25%, 50%, 75%)
        '\u{2591}'..='\u{2593}' => {
            let alpha = (c as u32 - 0x2590) as f32 * 0.25;
            bg.push_rect(x, y, w, h, [fg[0], fg[1], fg[2], fg[3] * alpha]);
        }
        // Upper 1/8
        '\u{2594}' => bg.push_rect(x, y, w, (h / 8.0).round(), fg),
        // Right 1/8
        '\u{2595}' => {
            let bw = (w / 8.0).round();
            bg.push_rect(x + w - bw, y, bw, h, fg);
        }
        // Quadrant block elements (U+2596-U+259F)
        '\u{2596}'..='\u{259F}' => draw_quadrant(c, x, y, w, h, fg, bg),
        _ => return false,
    }
    true
}

/// Draw a quadrant block element (U+2596-U+259F) from a bitmask.
///
/// Each quadrant char maps to a 4-bit mask: TL, TR, BL, BR.
#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
fn draw_quadrant(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) {
    // Bitmask per quadrant: bit 3=TL, bit 2=TR, bit 1=BL, bit 0=BR
    // Index 0 = U+2596, index 9 = U+259F
    const QUADRANT_MASKS: [u8; 10] = [
        0b0010, // U+2596: lower left
        0b0001, // U+2597: lower right
        0b1000, // U+2598: upper left
        0b1011, // U+2599: upper left + lower left + lower right
        0b1001, // U+259A: upper left + lower right
        0b1110, // U+259B: upper left + upper right + lower left
        0b1101, // U+259C: upper left + upper right + lower right
        0b0100, // U+259D: upper right
        0b0110, // U+259E: upper right + lower left
        0b0111, // U+259F: upper right + lower left + lower right
    ];

    let idx = (c as u32 - 0x2596) as usize;
    let mask = QUADRANT_MASKS[idx];
    let hw = (w / 2.0).round();
    let hh = (h / 2.0).round();

    if mask & 0b1000 != 0 { bg.push_rect(x,      y,      hw,      hh,      fg); } // TL
    if mask & 0b0100 != 0 { bg.push_rect(x + hw,  y,      w - hw,  hh,      fg); } // TR
    if mask & 0b0010 != 0 { bg.push_rect(x,       y + hh, hw,      h - hh,  fg); } // BL
    if mask & 0b0001 != 0 { bg.push_rect(x + hw,  y + hh, w - hw,  h - hh,  fg); } // BR
}

// Braille Patterns (U+2800-U+28FF)

#[expect(clippy::many_single_char_names, reason = "Geometric drawing with standard x/y/w/h/c names")]
fn draw_braille(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    // Braille bit layout (standard):
    // bit 0 = (col=0, row=0), bit 1 = (col=0, row=1), bit 2 = (col=0, row=2),
    // bit 3 = (col=1, row=0), bit 4 = (col=1, row=1), bit 5 = (col=1, row=2),
    // bit 6 = (col=0, row=3), bit 7 = (col=1, row=3)
    const POSITIONS: [(usize, usize, u32); 8] = [
        (0, 0, 0), (0, 1, 1), (0, 2, 2),
        (1, 0, 3), (1, 1, 4), (1, 2, 5),
        (0, 3, 6), (1, 3, 7),
    ];

    let bits = c as u32 - 0x2800;
    if bits == 0 {
        return true; // Empty braille — no dots.
    }

    let dot_w = (w / 5.0).round().max(2.0);
    let dot_h = (h / 10.0).round().max(2.0);

    for (col, row, bit) in POSITIONS {
        if bits & (1 << bit) != 0 {
            let dx = x + w * (0.25 + col as f32 * 0.5) - dot_w / 2.0;
            let dy = y + h * ((row as f32 + 0.5) / 4.0) - dot_h / 2.0;
            bg.push_rect(dx, dy, dot_w, dot_h, fg);
        }
    }

    true
}

// Powerline Glyphs (U+E0A0-U+E0A3, U+E0B0-U+E0D4)

#[expect(clippy::many_single_char_names, reason = "Geometric drawing")]
fn draw_powerline(
    c: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) -> bool {
    match c {
        // U+E0B0, U+E0B4: right-pointing solid triangle / rounded separator
        '\u{E0B0}' | '\u{E0B4}' => draw_triangle_right(x, y, w, h, fg, bg),
        // U+E0B1: right-pointing triangle (outline)
        '\u{E0B1}' => draw_triangle_right_thin(x, y, w, h, fg, bg),
        // U+E0B2, U+E0B6: left-pointing solid triangle / rounded separator
        '\u{E0B2}' | '\u{E0B6}' => draw_triangle_left(x, y, w, h, fg, bg),
        // U+E0B3: left-pointing triangle (outline)
        '\u{E0B3}' => draw_triangle_left_thin(x, y, w, h, fg, bg),
        // Everything else (icons, unhandled extra glyphs) — use font path.
        _ => return false,
    }
    true
}

/// Solid right-pointing triangle filling the entire cell.
fn draw_triangle_right(x: f32, y: f32, w: f32, h: f32, fg: [f32; 4], bg: &mut InstanceWriter) {
    let steps = h as usize;
    let mid = h / 2.0;
    for i in 0..steps {
        let frac = (i as f32 - mid).abs() / mid;
        let line_w = w * (1.0 - frac);
        if line_w > 0.0 {
            bg.push_rect(x, y + i as f32, line_w, 1.0, fg);
        }
    }
}

/// Thin right-pointing triangle (outline only).
fn draw_triangle_right_thin(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) {
    let steps = h as usize;
    let mid = h / 2.0;
    let thin = 1.0f32.max((w / 8.0).round());
    for i in 0..steps {
        let frac = (i as f32 - mid).abs() / mid;
        let edge_x = w * (1.0 - frac);
        if edge_x > 0.0 {
            bg.push_rect(x + edge_x - thin, y + i as f32, thin, 1.0, fg);
        }
    }
}

/// Solid left-pointing triangle filling the entire cell.
fn draw_triangle_left(x: f32, y: f32, w: f32, h: f32, fg: [f32; 4], bg: &mut InstanceWriter) {
    let steps = h as usize;
    let mid = h / 2.0;
    for i in 0..steps {
        let frac = (i as f32 - mid).abs() / mid;
        let line_w = w * (1.0 - frac);
        if line_w > 0.0 {
            bg.push_rect(x + w - line_w, y + i as f32, line_w, 1.0, fg);
        }
    }
}

/// Thin left-pointing triangle (outline only).
fn draw_triangle_left_thin(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: [f32; 4],
    bg: &mut InstanceWriter,
) {
    let steps = h as usize;
    let mid = h / 2.0;
    let thin = 1.0f32.max((w / 8.0).round());
    for i in 0..steps {
        let frac = (i as f32 - mid).abs() / mid;
        let edge_x = w * (1.0 - frac);
        if edge_x > 0.0 {
            bg.push_rect(x + w - edge_x, y + i as f32, thin, 1.0, fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_builtin_box_drawing_range() {
        for c in '\u{2500}'..='\u{257F}' {
            assert!(is_builtin_glyph(c), "U+{:04X} should be builtin", c as u32);
        }
    }

    #[test]
    fn is_builtin_block_elements_range() {
        for c in '\u{2580}'..='\u{259F}' {
            assert!(is_builtin_glyph(c), "U+{:04X} should be builtin", c as u32);
        }
    }

    #[test]
    fn is_builtin_braille_range() {
        for c in '\u{2800}'..='\u{28FF}' {
            assert!(is_builtin_glyph(c), "U+{:04X} should be builtin", c as u32);
        }
    }

    #[test]
    fn is_builtin_powerline_range() {
        for c in '\u{E0A0}'..='\u{E0A3}' {
            assert!(is_builtin_glyph(c), "U+{:04X} should be builtin", c as u32);
        }
        for c in '\u{E0B0}'..='\u{E0D4}' {
            assert!(is_builtin_glyph(c), "U+{:04X} should be builtin", c as u32);
        }
    }

    #[test]
    fn is_builtin_excludes_normal_chars() {
        assert!(!is_builtin_glyph('A'));
        assert!(!is_builtin_glyph(' '));
        assert!(!is_builtin_glyph('0'));
        // CJK
        assert!(!is_builtin_glyph('\u{4E00}'));
        // Emoji
        assert!(!is_builtin_glyph('\u{1F600}'));
    }

    #[test]
    fn box_drawing_table_length() {
        assert_eq!(BOX_DRAWING_TABLE.len(), 128);
    }

    #[test]
    fn box_segments_horizontal_line() {
        let [l, r, u, d] = box_segments('\u{2500}');
        assert!(l.is_some());
        assert!(r.is_some());
        assert!(!u.is_some());
        assert!(!d.is_some());
    }

    #[test]
    fn box_segments_vertical_line() {
        let [l, r, u, d] = box_segments('\u{2502}');
        assert!(!l.is_some());
        assert!(!r.is_some());
        assert!(u.is_some());
        assert!(d.is_some());
    }

    #[test]
    fn box_segments_cross() {
        // U+253C ┼ — all four directions
        let segs = box_segments('\u{253C}');
        assert!(segs.iter().all(|w| w.is_some()));
    }
}
