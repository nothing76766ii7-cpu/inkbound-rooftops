//! Original vector pictogram & geometric UI primitives rendered in pure Rust (PRD v2.0 section 4.5).
//!
//! Draws smooth alpha-masked circular buttons, stroked rings, thick vector strokes,
//! crisp 7-segment HUD digits, and recognizable action icons (sword, rift crescent,
//! jump chevron, slide glyph, double-chevron dash, paint brush, pause bars, heart).

use engine_core::Palette;
use platform_input::TouchButton;

/// Single 2D UI vertex in normalized `[0, 1]` screen coordinates (top-left origin).
#[derive(Debug, Clone, Copy)]
pub struct UiVert {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

/// Pushes a filled triangle.
#[inline]
pub fn push_tri(out: &mut Vec<UiVert>, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: [f32; 4]) {
    out.push(UiVert { pos: a, color });
    out.push(UiVert { pos: b, color });
    out.push(UiVert { pos: c, color });
}

/// Pushes a filled axis-aligned rectangle.
pub fn push_rect(out: &mut Vec<UiVert>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    let a = [x, y];
    let b = [x, y + h];
    let c = [x + w, y + h];
    let d = [x + w, y];
    push_tri(out, a, b, c, color);
    push_tri(out, a, c, d, color);
}

/// Pushes a thick vector line segment (aspect-corrected so circles/icons never stretch).
pub fn push_line(
    out: &mut Vec<UiVert>,
    p0: [f32; 2],
    p1: [f32; 2],
    thickness: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let dx = (p1[0] - p0[0]) * aspect;
    let dy = p1[1] - p0[1];
    let len = (dx * dx + dy * dy).sqrt().max(1e-5);
    let nx = (-dy / len) * (thickness * 0.5) / aspect;
    let ny = (dx / len) * (thickness * 0.5);

    let a = [p0[0] + nx, p0[1] + ny];
    let b = [p0[0] - nx, p0[1] - ny];
    let c = [p1[0] - nx, p1[1] - ny];
    let d = [p1[0] + nx, p1[1] + ny];
    push_tri(out, a, b, c, color);
    push_tri(out, a, c, d, color);
}

/// Pushes a smooth aspect-corrected filled circle (true round mobile touch buttons).
pub fn push_circle(
    out: &mut Vec<UiVert>,
    cx: f32,
    cy: f32,
    radius: f32,
    aspect: f32,
    segments: usize,
    color: [f32; 4],
) {
    let rx = radius / aspect;
    let ry = radius;
    let center = [cx, cy];
    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        let p0 = [cx + a0.cos() * rx, cy + a0.sin() * ry];
        let p1 = [cx + a1.cos() * rx, cy + a1.sin() * ry];
        push_tri(out, center, p0, p1, color);
    }
}

/// Pushes a smooth aspect-corrected hollow ring (joystick boundary & button ink strokes).
pub fn push_ring(
    out: &mut Vec<UiVert>,
    cx: f32,
    cy: f32,
    outer_r: f32,
    inner_r: f32,
    aspect: f32,
    segments: usize,
    color: [f32; 4],
) {
    let ox = outer_r / aspect;
    let oy = outer_r;
    let ix = inner_r / aspect;
    let iy = inner_r;
    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        let (c0, s0) = (a0.cos(), a0.sin());
        let (c1, s1) = (a1.cos(), a1.sin());
        let o0 = [cx + c0 * ox, cy + s0 * oy];
        let o1 = [cx + c1 * ox, cy + s1 * oy];
        let i0 = [cx + c0 * ix, cy + s0 * iy];
        let i1 = [cx + c1 * ix, cy + s1 * iy];
        push_tri(out, o0, i0, i1, color);
        push_tri(out, o0, i1, o1, color);
    }
}

/// Draws a crisp original vector pictogram inside a circular button at `(cx, cy)`.
pub fn push_button_icon(
    out: &mut Vec<UiVert>,
    button: TouchButton,
    cx: f32,
    cy: f32,
    r: f32,
    aspect: f32,
    ink: [f32; 4],
) {
    let pt = |u: f32, v: f32| [cx + (u * r) / aspect, cy + v * r];
    let stroke = (r * 0.13).max(0.004);

    match button {
        TouchButton::Slash => {
            // Diagonal sword blade + crossguard + pommel
            push_line(out, pt(-0.42, 0.42), pt(0.46, -0.46), stroke * 1.35, aspect, ink);
            // Crossguard
            push_line(out, pt(-0.36, 0.08), pt(-0.08, 0.36), stroke * 1.15, aspect, ink);
            // Subtle orange slash arc accent
            push_line(out, pt(-0.35, -0.35), pt(0.15, -0.45), stroke * 0.7, aspect, Palette::ROOF_ORANGE);
        }
        TouchButton::DimensionalSlash => {
            // Split dimensional crescent + diagonal ink rift
            push_line(out, pt(-0.48, 0.38), pt(0.48, -0.38), stroke * 1.2, aspect, Palette::ROOF_ORANGE);
            push_line(out, pt(-0.32, -0.35), pt(0.05, -0.42), stroke * 1.1, aspect, ink);
            push_line(out, pt(0.05, -0.42), pt(0.38, -0.10), stroke * 1.1, aspect, ink);
            push_line(out, pt(-0.38, 0.12), pt(-0.05, 0.42), stroke * 1.1, aspect, ink);
            push_line(out, pt(-0.05, 0.42), pt(0.32, 0.35), stroke * 1.1, aspect, ink);
        }
        TouchButton::Jump => {
            // Upward leaping chevron + vertical stem
            push_line(out, pt(0.0, 0.42), pt(0.0, -0.36), stroke * 1.25, aspect, ink);
            push_line(out, pt(-0.34, -0.04), pt(0.0, -0.40), stroke * 1.25, aspect, ink);
            push_line(out, pt(0.34, -0.04), pt(0.0, -0.40), stroke * 1.25, aspect, ink);
        }
        TouchButton::Dash => {
            // Double forward chevrons (>>)
            push_line(out, pt(-0.36, -0.32), pt(-0.04, 0.0), stroke * 1.2, aspect, ink);
            push_line(out, pt(-0.04, 0.0), pt(-0.36, 0.32), stroke * 1.2, aspect, ink);
            push_line(out, pt(0.04, -0.32), pt(0.36, 0.0), stroke * 1.2, aspect, ink);
            push_line(out, pt(0.36, 0.0), pt(0.04, 0.32), stroke * 1.2, aspect, ink);
        }
        TouchButton::Slide => {
            // Low ground bar + crouched diagonal slide arrow
            push_line(out, pt(-0.42, 0.32), pt(0.42, 0.32), stroke * 1.1, aspect, ink);
            push_line(out, pt(-0.32, -0.22), pt(0.28, 0.16), stroke * 1.2, aspect, ink);
            push_line(out, pt(0.06, 0.22), pt(0.32, 0.18), stroke * 1.0, aspect, ink);
        }
        TouchButton::Paint => {
            // Calligraphy brush handle + ink drop tip
            push_line(out, pt(-0.38, 0.38), pt(0.18, -0.18), stroke * 1.25, aspect, ink);
            push_circle(out, cx + (0.28 * r) / aspect, cy - 0.28 * r, r * 0.22, aspect, 12, Palette::PAINT_TEAL);
        }
        TouchButton::Pause => {
            // Crisp two vertical pause bars (||)
            push_line(out, pt(-0.20, -0.36), pt(-0.20, 0.36), stroke * 1.5, aspect, ink);
            push_line(out, pt(0.20, -0.36), pt(0.20, 0.36), stroke * 1.5, aspect, ink);
        }
        TouchButton::Interact => {
            // Diamond interaction glyph
            push_line(out, pt(0.0, -0.38), pt(0.38, 0.0), stroke * 1.1, aspect, ink);
            push_line(out, pt(0.38, 0.0), pt(0.0, 0.38), stroke * 1.1, aspect, ink);
            push_line(out, pt(0.0, 0.38), pt(-0.38, 0.0), stroke * 1.1, aspect, ink);
            push_line(out, pt(-0.38, 0.0), pt(0.0, -0.38), stroke * 1.1, aspect, ink);
        }
    }
}

/// Draws a small crisp vector heart icon for the top-left health bar.
pub fn push_heart_icon(out: &mut Vec<UiVert>, cx: f32, cy: f32, size: f32, aspect: f32, color: [f32; 4]) {
    let r = size * 0.32;
    push_circle(out, cx - r * 0.85 / aspect, cy - r * 0.25, r, aspect, 10, color);
    push_circle(out, cx + r * 0.85 / aspect, cy - r * 0.25, r, aspect, 10, color);
    push_tri(
        out,
        [cx - size * 0.56 / aspect, cy - r * 0.05],
        [cx + size * 0.56 / aspect, cy - r * 0.05],
        [cx, cy + size * 0.62],
        color,
    );
}

/// Draws crisp vector numeric strings (`0-9` and `/`) for HUD counters (`100/100`, `0/3`).
pub fn push_hud_number_pair(
    out: &mut Vec<UiVert>,
    x: f32,
    y: f32,
    height: f32,
    aspect: f32,
    left: u32,
    right: u32,
    color: [f32; 4],
) {
    let text = format!("{left}/{right}");
    let char_w = (height * 0.55) / aspect;
    let gap = (height * 0.22) / aspect;
    let mut cursor_x = x;

    for ch in text.chars() {
        draw_glyph(out, ch, cursor_x, y, char_w, height, aspect, color);
        cursor_x += char_w + gap;
    }
}

fn draw_glyph(
    out: &mut Vec<UiVert>,
    ch: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    aspect: f32,
    color: [f32; 4],
) {
    let t = (h * 0.14).max(0.0025);
    if ch == '/' {
        push_line(out, [x, y + h], [x + w, y], t, aspect, color);
        return;
    }
    // 7-segment bitmask: top, top-left, top-right, mid, bot-left, bot-right, bot
    let mask: u8 = match ch {
        '0' => 0b111_0111,
        '1' => 0b001_0010,
        '2' => 0b101_1101,
        '3' => 0b101_1011,
        '4' => 0b011_1010,
        '5' => 0b110_1011,
        '6' => 0b110_1111,
        '7' => 0b101_0010,
        '8' => 0b111_1111,
        '9' => 0b111_1011,
        _ => 0,
    };
    let mid_y = y + h * 0.5;
    if mask & 0b100_0000 != 0 {
        push_line(out, [x, y], [x + w, y], t, aspect, color);
    }
    if mask & 0b010_0000 != 0 {
        push_line(out, [x, y], [x, mid_y], t, aspect, color);
    }
    if mask & 0b001_0000 != 0 {
        push_line(out, [x + w, y], [x + w, mid_y], t, aspect, color);
    }
    if mask & 0b000_1000 != 0 {
        push_line(out, [x, mid_y], [x + w, mid_y], t, aspect, color);
    }
    if mask & 0b000_0100 != 0 {
        push_line(out, [x, mid_y], [x, y + h], t, aspect, color);
    }
    if mask & 0b000_0010 != 0 {
        push_line(out, [x + w, mid_y], [x + w, y + h], t, aspect, color);
    }
    if mask & 0b000_0001 != 0 {
        push_line(out, [x, y + h], [x + w, y + h], t, aspect, color);
    }
}
