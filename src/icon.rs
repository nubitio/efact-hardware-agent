use tray_icon::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconStyle {
    Color,
    Dark,
    Light,
}

impl TrayIconStyle {
    pub fn parse(style: &str) -> Self {
        match style.trim().to_ascii_lowercase().as_str() {
            "color" | "brand" | "blue" => Self::Color,
            "light" | "white" => Self::Light,
            "dark" | "black" | "mono" | "monochrome" => Self::Dark,
            _ => default_auto_style(),
        }
    }
}

fn default_auto_style() -> TrayIconStyle {
    #[cfg(target_os = "windows")]
    {
        TrayIconStyle::Color
    }

    #[cfg(not(target_os = "windows"))]
    {
        TrayIconStyle::Dark
    }
}

/// Builds a 44x44 tray icon. Color keeps the eFact tile; monochrome variants use
/// a transparent background so they fit light or dark system trays.
pub fn build_tray_icon(style: &str) -> Result<Icon, String> {
    match TrayIconStyle::parse(style) {
        TrayIconStyle::Color => build_color_tray_icon(),
        TrayIconStyle::Dark => build_monochrome_tray_icon((20, 24, 34)),
        TrayIconStyle::Light => build_monochrome_tray_icon((255, 255, 255)),
    }
}

fn build_color_tray_icon() -> Result<Icon, String> {
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
            let glyph_alpha = hardware_glyph_alpha(fx, fy);
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

fn build_monochrome_tray_icon(fg: (u8, u8, u8)) -> Result<Icon, String> {
    const SIZE: usize = 44;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = (y * SIZE + x) * 4;
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let alpha = monochrome_glyph_alpha(fx, fy);

            if alpha <= 0.0 {
                continue;
            }

            rgba[idx] = fg.0;
            rgba[idx + 1] = fg.1;
            rgba[idx + 2] = fg.2;
            rgba[idx + 3] = (alpha.min(1.0) * 255.0).round() as u8;
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

fn monochrome_glyph_alpha(x: f32, y: f32) -> f32 {
    hardware_glyph_alpha(x, y)
}

fn hardware_glyph_alpha(x: f32, y: f32) -> f32 {
    let hub = rounded_rect_alpha(x, y, 13.0, 13.0, 18.0, 18.0, 4.0);
    let inner = rounded_rect_alpha(x, y, 18.0, 18.0, 8.0, 8.0, 2.0);
    let left_pin = rounded_rect_alpha(x, y, 7.0, 18.5, 7.0, 3.0, 1.5);
    let right_pin = rounded_rect_alpha(x, y, 30.0, 18.5, 7.0, 3.0, 1.5);
    let top_pin = rounded_rect_alpha(x, y, 20.5, 7.0, 3.0, 7.0, 1.5);
    let bottom_pin = rounded_rect_alpha(x, y, 20.5, 30.0, 3.0, 7.0, 1.5);
    let left_node = circle_alpha(x, y, 8.0, 20.0, 2.5);
    let right_node = circle_alpha(x, y, 36.0, 20.0, 2.5);
    let top_node = circle_alpha(x, y, 22.0, 8.0, 2.5);
    let bottom_node = circle_alpha(x, y, 22.0, 36.0, 2.5);

    hub.max(inner)
        .max(left_pin)
        .max(right_pin)
        .max(top_pin)
        .max(bottom_pin)
        .max(left_node)
        .max(right_node)
        .max(top_node)
        .max(bottom_node)
}

fn circle_alpha(x: f32, y: f32, cx: f32, cy: f32, radius: f32) -> f32 {
    let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
    if dist <= radius {
        1.0
    } else if dist <= radius + 1.0 {
        1.0 - (dist - radius)
    } else {
        0.0
    }
}
