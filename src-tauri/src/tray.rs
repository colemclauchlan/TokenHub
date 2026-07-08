//! Dynamic tray icon: a circle that fills bottom-up with white "liquid" showing the
//! 5-hour window utilization — fully white at 100%. `render_rgba` is pure → testable.

/// Icon size in px (Windows scales this for the tray).
pub const SIZE: u32 = 32;

fn set_px(buf: &mut [u8], w: u32, x: u32, y: u32, rgba: [u8; 4]) {
    if x >= w {
        return;
    }
    let idx = ((y * w + x) * 4) as usize;
    if idx + 4 <= buf.len() {
        buf[idx..idx + 4].copy_from_slice(&rgba);
    }
}

/// Color of the filled portion. `mono` = white; `multi` = green/amber/red by load.
fn fill_color(pct: u32, color_mode: &str) -> [u8; 4] {
    if color_mode == "mono" {
        [245, 245, 245, 255]
    } else if pct >= 90 {
        [216, 92, 74, 255] // red
    } else if pct >= 75 {
        [217, 164, 65, 255] // amber
    } else {
        [87, 178, 106, 255] // green
    }
}

/// Render the tray icon in the chosen style + color mode (driven by 5h load).
pub fn render_icon(style: &str, color_mode: &str, five_pct: u32, _seven_pct: u32) -> (Vec<u8>, u32, u32) {
    match style {
        "ring" => render_ring(five_pct, color_mode),
        "bar" => render_bar(five_pct, color_mode),
        _ => render_fill(five_pct, color_mode), // "fill" (default)
    }
}

/// Backwards-compatible white liquid-fill circle (used at startup + in tests).
pub fn render_rgba(five_pct: u32, _seven_pct: u32) -> (Vec<u8>, u32, u32) {
    render_fill(five_pct, "mono")
}

/// Circle fills bottom-up with the fill color in proportion to `pct`.
fn render_fill(pct: u32, color_mode: &str) -> (Vec<u8>, u32, u32) {
    let (w, h) = (SIZE, SIZE);
    let mut buf = vec![0u8; (w * h * 4) as usize]; // transparent
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let r = w as f32 / 2.0 - 1.0;
    let frac = (pct.min(100) as f32) / 100.0;
    let fill_line = (cy + r) - frac * (2.0 * r); // pixels with y >= fill_line are filled
    let liquid = fill_color(pct, color_mode);
    let track = [34, 39, 47, 255];
    let ring = [72, 80, 92, 255];
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= r {
                let color = if dist >= r - 1.5 {
                    ring
                } else if (y as f32) >= fill_line {
                    liquid
                } else {
                    track
                };
                set_px(&mut buf, w, x, y, color);
            }
        }
    }
    (buf, w, h)
}

/// Progress ring (donut) that fills clockwise from the top.
fn render_ring(pct: u32, color_mode: &str) -> (Vec<u8>, u32, u32) {
    let (w, h) = (SIZE, SIZE);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let outer = w as f32 / 2.0 - 1.0;
    let inner = outer * 0.58;
    let frac = (pct.min(100) as f32) / 100.0;
    let fill = fill_color(pct, color_mode);
    let track = [40, 46, 55, 255];
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= outer && dist >= inner {
                let mut a = dx.atan2(-dy) / (2.0 * std::f32::consts::PI); // 0 at top, clockwise
                if a < 0.0 {
                    a += 1.0;
                }
                let color = if a <= frac { fill } else { track };
                set_px(&mut buf, w, x, y, color);
            }
        }
    }
    (buf, w, h)
}

/// Horizontal bar that fills left→right.
fn render_bar(pct: u32, color_mode: &str) -> (Vec<u8>, u32, u32) {
    let (w, h) = (SIZE, SIZE);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let frac = (pct.min(100) as f32) / 100.0;
    let fill = fill_color(pct, color_mode);
    let track = [40, 46, 55, 255];
    let border = [72, 80, 92, 255];
    let top = (h as f32 * 0.34) as u32;
    let bot = (h as f32 * 0.66) as u32;
    let left = 2u32;
    let right = w - 2;
    let fill_x = left + (((right - left) as f32) * frac) as u32;
    for y in top..bot {
        for x in left..right {
            let edge = y == top || y == bot - 1 || x == left || x == right - 1;
            let color = if edge {
                border
            } else if x < fill_x {
                fill
            } else {
                track
            };
            set_px(&mut buf, w, x, y, color);
        }
    }
    (buf, w, h)
}

/// Tooltip text for the tray icon.
pub fn tooltip(five_pct: u32, five_reset: &str, seven_pct: u32, seven_reset: &str) -> String {
    format!("TokenHub\n5h: {five_pct}% · {five_reset}\n7d: {seven_pct}% · {seven_reset}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn dimensions() {
        let (buf, w, h) = render_rgba(50, 0);
        assert_eq!(w, SIZE);
        assert_eq!(h, SIZE);
        assert_eq!(buf.len() as u32, SIZE * SIZE * 4);
    }

    #[test]
    fn half_fill_bottom_white_top_dark() {
        let (buf, w, _h) = render_rgba(50, 0);
        let bottom = px(&buf, w, 16, 26);
        assert_eq!(bottom[3], 255);
        assert!(bottom[0] > 200, "bottom should be white liquid");
        let top = px(&buf, w, 16, 6);
        assert!(top[0] < 100, "top should be dark track");
    }

    #[test]
    fn full_is_white() {
        let (buf, w, _h) = render_rgba(100, 0);
        let c = px(&buf, w, 16, 16);
        assert!(c[0] > 200 && c[1] > 200 && c[2] > 200);
    }

    #[test]
    fn empty_is_dark() {
        let (buf, w, _h) = render_rgba(0, 0);
        let c = px(&buf, w, 16, 16);
        assert!(c[0] < 100);
    }

    #[test]
    fn tooltip_formats() {
        let t = tooltip(36, "3h", 23, "3d");
        assert!(t.contains("5h: 36% · 3h"));
        assert!(t.contains("7d: 23% · 3d"));
    }
}
