use tray_icon::Icon;

/// Builds a 44×44 tray icon (macOS menu-bar retina size) with an eFact-branded
/// hardware glyph: rounded blue tile + white scale pan.
pub fn build_tray_icon() -> Result<Icon, String> {
    const SIZE: usize = 44;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    let bg = (29, 78, 216u8); // eFact blue #1D4ED8
    let fg = (255u8, 255, 255);
    let radius = 10.0f32;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = (y * SIZE + x) * 4;
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            let inside_tile = rounded_rect_alpha(
                fx,
                fy,
                4.0,
                4.0,
                SIZE as f32 - 8.0,
                SIZE as f32 - 8.0,
                radius,
            );
            let pan_alpha = scale_pan_alpha(fx, fy);
            let stem_alpha = scale_stem_alpha(fx, fy);

            let glyph_alpha = pan_alpha.max(stem_alpha);
            let alpha = (inside_tile * 255.0).round() as u8;
            if alpha == 0 {
                rgba[idx + 3] = 0;
                continue;
            }

            let blend = glyph_alpha;
            let (r, g, b) = if blend > 0.05 {
                let t = blend.min(1.0);
                (
                    lerp(bg.0, fg.0, t) as u8,
                    lerp(bg.1, fg.1, t) as u8,
                    lerp(bg.2, fg.2, t) as u8,
                )
            } else {
                bg
            };

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = alpha;
        }
    }

    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32).map_err(|err| err.to_string())
}

fn lerp(a: u8, b: u8, t: f32) -> f32 {
    a as f32 + (b as f32 - a as f32) * t
}

fn rounded_rect_alpha(
    x: f32,
    y: f32,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    radius: f32,
) -> f32 {
    let right = left + width;
    let bottom = top + height;

    if x < left || y < top || x > right || y > bottom {
        return 0.0;
    }

    let cx = x.clamp(left + radius, right - radius);
    let cy = y.clamp(top + radius, bottom - radius);
    let dx = x - cx;
    let dy = y - cy;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist <= radius {
        1.0
    } else if dist <= radius + 1.0 {
        1.0 - (dist - radius)
    } else {
        0.0
    }
}

fn scale_pan_alpha(x: f32, y: f32) -> f32 {
    // Bowl: ellipse centered at (22, 27)
    let cx = 22.0;
    let cy = 27.0;
    let rx = 11.5;
    let ry = 5.5;
    let nx = (x - cx) / rx;
    let ny = (y - cy) / ry;
    let d = nx * nx + ny * ny;

    if d <= 1.0 {
        if y >= cy - 1.5 {
            1.0
        } else {
            0.0
        }
    } else if d <= 1.15 && y >= cy - 1.5 {
        1.0 - ((d - 1.0) / 0.15)
    } else {
        0.0
    }
}

fn scale_stem_alpha(x: f32, y: f32) -> f32 {
    // Vertical stem + top hook
    let in_stem = (20.5..=23.5).contains(&x) && (12.0..=24.0).contains(&y);
    let in_hook = (10.0..=13.5).contains(&y)
        && (18.0..=26.0).contains(&x)
        && (y - 11.5).abs() + (x - 22.0).abs() * 0.55 < 4.0;
    if in_stem || in_hook {
        1.0
    } else {
        0.0
    }
}
