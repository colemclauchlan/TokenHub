//! Dynamic tray icon: rasterize the 5h / 7d mini-bars into RGBA so the tray glyph
//! reflects live usage (like the green sysicon). `render_rgba` is pure → testable.

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

fn fill_rect(buf: &mut [u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32, rgba: [u8; 4]) {
    for y in y0..y1 {
        for x in x0..x1 {
            set_px(buf, w, x, y, rgba);
        }
    }
}

fn bar_color(pct: u32) -> [u8; 4] {
    if pct >= 90 {
        [216, 92, 74, 255] // red
    } else if pct >= 75 {
        [217, 164, 65, 255] // amber
    } else {
        [208, 119, 74, 255] // orange
    }
}

/// Draw one horizontal bar with `pct` fill across [x0,x1).
fn draw_bar(buf: &mut [u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32, pct: u32) {
    let track = [42, 47, 55, 255];
    fill_rect(buf, w, x0, y0, x1, y1, track);
    let span = x1.saturating_sub(x0);
    let fill = x0 + (span * pct.min(100)) / 100;
    fill_rect(buf, w, x0, y0, fill, y1, bar_color(pct));
}

/// Produce a `SIZE`×`SIZE` RGBA buffer with two bars (5h top, 7d bottom).
pub fn render_rgba(five_pct: u32, seven_pct: u32) -> (Vec<u8>, u32, u32) {
    let w = SIZE;
    let h = SIZE;
    let mut buf = vec![0u8; (w * h * 4) as usize]; // transparent
    // top bar (5h): y 6..14 ; bottom bar (7d): y 18..26 ; x 3..29
    draw_bar(&mut buf, w, 3, 6, 29, 14, five_pct);
    draw_bar(&mut buf, w, 3, 18, 29, 26, seven_pct);
    (buf, w, h)
}

/// Tooltip text for the tray icon.
pub fn tooltip(five_pct: u32, five_reset: &str, seven_pct: u32, seven_reset: &str) -> String {
    format!(
        "AI Usage Bar\n5h: {five_pct}% · {five_reset}\n7d: {seven_pct}% · {seven_reset}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_dimensions_and_fill() {
        let (buf, w, h) = render_rgba(50, 100);
        assert_eq!(w, SIZE);
        assert_eq!(h, SIZE);
        assert_eq!(buf.len() as u32, SIZE * SIZE * 4);
        // a pixel near the far right of the 7d bar (100%) should be filled (alpha=255)
        let x = 28;
        let y = 22;
        let idx = ((y * w + x) * 4) as usize;
        assert_eq!(buf[idx + 3], 255);
    }

    #[test]
    fn tooltip_formats() {
        let t = tooltip(36, "3h", 23, "3d");
        assert!(t.contains("5h: 36% · 3h"));
        assert!(t.contains("7d: 23% · 3d"));
    }
}
