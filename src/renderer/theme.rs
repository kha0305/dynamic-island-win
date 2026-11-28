use super::*;

pub(super) struct BrushPalette {
    pub(super) bg: ID2D1SolidColorBrush,
    pub(super) border: ID2D1SolidColorBrush,
    pub(super) glass_fill: ID2D1LinearGradientBrush,
    pub(super) glass_sheen: ID2D1LinearGradientBrush,
    pub(super) glass_border: ID2D1LinearGradientBrush,
    pub(super) glass_inner_rim: ID2D1LinearGradientBrush,
    pub(super) glass_edge_shadow: ID2D1SolidColorBrush,
    pub(super) surface_card: ID2D1SolidColorBrush,
    pub(super) surface_hover: ID2D1SolidColorBrush,
    pub(super) card_border: ID2D1SolidColorBrush,
    pub(super) text_primary: ID2D1SolidColorBrush,
    pub(super) text_secondary: ID2D1SolidColorBrush,
    pub(super) text_shadow: ID2D1SolidColorBrush,
    pub(super) accent: ID2D1SolidColorBrush,
    pub(super) accent_soft: ID2D1SolidColorBrush,
    pub(super) sys_accent: ID2D1SolidColorBrush,
    pub(super) sys_accent_soft: ID2D1SolidColorBrush,
    pub(super) anti_accent: ID2D1SolidColorBrush,
    pub(super) switch_off: ID2D1SolidColorBrush,
    pub(super) knob: ID2D1SolidColorBrush,
    pub(super) danger_bg: ID2D1SolidColorBrush,
    pub(super) danger_hover: ID2D1SolidColorBrush,
    pub(super) danger_border: ID2D1SolidColorBrush,
    pub(super) power_ok: ID2D1SolidColorBrush,
    pub(super) power_warn: ID2D1SolidColorBrush,
    pub(super) art_bg: ID2D1SolidColorBrush,
}

pub(super) struct IconGeometries {
    pub(super) round_stroke: ID2D1StrokeStyle,
    pub(super) play_triangle: ID2D1PathGeometry,
    pub(super) prev_triangle: ID2D1PathGeometry,
    pub(super) next_triangle: ID2D1PathGeometry,
    pub(super) speaker_cone: ID2D1PathGeometry,
}

impl IslandRenderer {
    pub(super) fn create_clamped_bitmap_brush(
        &self,
        bitmap: &ID2D1Bitmap1,
    ) -> windows::core::Result<ID2D1BitmapBrush> {
        let bmp_base: ID2D1Bitmap = bitmap.cast()?;
        let bmp_props = D2D1_BITMAP_BRUSH_PROPERTIES {
            extendModeX: D2D1_EXTEND_MODE_CLAMP,
            extendModeY: D2D1_EXTEND_MODE_CLAMP,
            interpolationMode: D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
        };
        let brush_props = D2D1_BRUSH_PROPERTIES {
            opacity: 1.0,
            transform: Matrix3x2::identity(),
        };
        unsafe {
            ID2D1RenderTarget::CreateBitmapBrush(
                &self.d2d_context,
                &bmp_base,
                Some(&bmp_props),
                Some(&brush_props),
            )
        }
    }

    pub(super) fn apply_media_theme_colors(&mut self, accent: [f32; 3], glow_alpha: f32) {
        if let Ok(glow) = create_ambient_glow_brush(&self.d2d_context, accent, glow_alpha) {
            self.ambient_glow_brush = glow;
        }
        unsafe {
            self.brushes.accent.SetColor(&D2D1_COLOR_F {
                r: accent[0],
                g: accent[1],
                b: accent[2],
                a: 1.0,
            });
            self.brushes.accent_soft.SetColor(&D2D1_COLOR_F {
                r: accent[0] * 0.56,
                g: accent[1] * 0.56,
                b: accent[2] * 0.56,
                a: 0.52,
            });
        }
    }

    pub(super) fn upload_artwork_bitmap(&self, art: &DecodedArtwork) -> windows::core::Result<ID2D1Bitmap1> {
        self.upload_bgra_bitmap(art.width, art.height, &art.bgra)
    }

    pub(super) fn upload_bgra_bitmap(
        &self,
        width: u32,
        height: u32,
        bgra: &[u8],
    ) -> windows::core::Result<ID2D1Bitmap1> {
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            ..Default::default()
        };
        let pitch = width.saturating_mul(4);
        unsafe {
            self.d2d_context.CreateBitmap(
                D2D_SIZE_U { width, height },
                Some(bgra.as_ptr() as *const core::ffi::c_void),
                pitch,
                &props,
            )
        }
    }
}

pub(super) fn create_icon_geometries(factory: &ID2D1Factory1) -> windows::core::Result<IconGeometries> {
    let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
        startCap: D2D1_CAP_STYLE_ROUND,
        endCap: D2D1_CAP_STYLE_ROUND,
        dashCap: D2D1_CAP_STYLE_ROUND,
        lineJoin: D2D1_LINE_JOIN_ROUND,
        miterLimit: 10.0,
        ..Default::default()
    };
    let round_stroke = unsafe { ID2D1Factory::CreateStrokeStyle(factory, &stroke_props, None) }?;

    let play_triangle = make_triangle_geometry(
        factory,
        Vector2 { X: -4.0, Y: -5.4 },
        Vector2 { X: 5.2, Y: 0.0 },
        Vector2 { X: -4.0, Y: 5.4 },
    )?;
    let prev_triangle = make_triangle_geometry(
        factory,
        Vector2 { X: 3.8, Y: -4.4 },
        Vector2 { X: -3.8, Y: 0.0 },
        Vector2 { X: 3.8, Y: 4.4 },
    )?;
    let next_triangle = make_triangle_geometry(
        factory,
        Vector2 { X: -3.8, Y: -4.4 },
        Vector2 { X: 3.8, Y: 0.0 },
        Vector2 { X: -3.8, Y: 4.4 },
    )?;
    let speaker_cone = make_quad_geometry(
        factory,
        Vector2 { X: -2.0, Y: -2.4 },
        Vector2 { X: 2.2, Y: -5.0 },
        Vector2 { X: 2.2, Y: 5.0 },
        Vector2 { X: -2.0, Y: 2.4 },
    )?;

    Ok(IconGeometries {
        round_stroke,
        play_triangle,
        prev_triangle,
        next_triangle,
        speaker_cone,
    })
}

fn make_quad_geometry(
    factory: &ID2D1Factory1,
    p0: Vector2,
    p1: Vector2,
    p2: Vector2,
    p3: Vector2,
) -> windows::core::Result<ID2D1PathGeometry> {
    let path = unsafe { factory.CreatePathGeometry() }?;
    let sink = unsafe { path.Open() }?;
    unsafe {
        sink.BeginFigure(p0, D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(p1);
        sink.AddLine(p2);
        sink.AddLine(p3);
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(path.into())
}

fn make_triangle_geometry(
    factory: &ID2D1Factory1,
    p0: Vector2,
    p1: Vector2,
    p2: Vector2,
) -> windows::core::Result<ID2D1PathGeometry> {
    let path = unsafe { factory.CreatePathGeometry() }?;
    let sink = unsafe { path.Open() }?;
    unsafe {
        sink.BeginFigure(p0, D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(p1);
        sink.AddLine(p2);
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
    }
    Ok(path.into())
}

pub(super) fn boost_accent_for_readability(rgb: [f32; 3]) -> [f32; 3] {
    let max_c = rgb[0].max(rgb[1]).max(rgb[2]);
    let min_c = rgb[0].min(rgb[1]).min(rgb[2]);
    let sat = if max_c > 0.01 {
        (max_c - min_c) / max_c
    } else {
        0.0
    };
    if sat < 0.14 {
        return [0.56, 0.86, 1.0];
    }
    let scale = (0.98 / max_c.max(0.08)).clamp(1.0, 2.6);
    let vivid = |c: f32| {
        let scaled = (c * scale).clamp(0.0, 1.0);
        let avg = (rgb[0] + rgb[1] + rgb[2]) * scale / 3.0;
        (avg + (scaled - avg) * 1.25).clamp(0.08, 1.0)
    };
    // Pha nhẹ ánh sáng trắng (~34%) để thanh timeline và nhãn nguồn phát nổi bật trên nền màu đậm.
    let vr = vivid(rgb[0]);
    let vg = vivid(rgb[1]);
    let vb = vivid(rgb[2]);
    [
        vr + (1.0 - vr) * 0.34,
        vg + (1.0 - vg) * 0.34,
        vb + (1.0 - vb) * 0.34,
    ]
}

pub(super) fn build_ambient_aura_bgra(art: &DecodedArtwork) -> ([u8; 16 * 10 * 4], u32, u32) {
    const GW: usize = 16;
    const GH: usize = 10;
    let mut grid = [[0.0_f32; 3]; GW * GH];
    let w = art.width.max(1) as usize;
    let h = art.height.max(1) as usize;

    for gy in 0..GH {
        let y0 = gy * h / GH;
        let y1 = ((gy + 1) * h / GH).max(y0 + 1).min(h);
        for gx in 0..GW {
            let x0 = gx * w / GW;
            let x1 = ((gx + 1) * w / GW).max(x0 + 1).min(w);
            let mut sr = 0.0_f32;
            let mut sg = 0.0_f32;
            let mut sb = 0.0_f32;
            let mut count = 0.0_f32;
            for py in y0..y1 {
                let row = py * w;
                for px in x0..x1 {
                    let idx = (row + px) * 4;
                    if idx + 3 >= art.bgra.len() {
                        continue;
                    }
                    let a = art.bgra[idx + 3] as f32 / 255.0;
                    if a < 0.1 {
                        continue;
                    }
                    sb += art.bgra[idx] as f32 / 255.0;
                    sg += art.bgra[idx + 1] as f32 / 255.0;
                    sr += art.bgra[idx + 2] as f32 / 255.0;
                    count += 1.0;
                }
            }
            if count > 0.0 {
                let inv = 1.0 / count;
                grid[gy * GW + gx] = [sr * inv, sg * inv, sb * inv];
            }
        }
    }

    for _ in 0..2 {
        let src = grid;
        for gy in 0..GH {
            for gx in 0..GW {
                let mut sum = [0.0_f32; 3];
                let mut cnt = 0.0_f32;
                for dy in -1_isize..=1 {
                    let ny = (gy as isize + dy).clamp(0, GH as isize - 1) as usize;
                    for dx in -1_isize..=1 {
                        let nx = (gx as isize + dx).clamp(0, GW as isize - 1) as usize;
                        let p = src[ny * GW + nx];
                        sum[0] += p[0];
                        sum[1] += p[1];
                        sum[2] += p[2];
                        cnt += 1.0;
                    }
                }
                grid[gy * GW + gx] = [sum[0] / cnt, sum[1] / cnt, sum[2] / cnt];
            }
        }
    }

    let mut out = [0_u8; GW * GH * 4];
    for (i, rgb) in grid.iter().enumerate() {
        let mean = (rgb[0] + rgb[1] + rgb[2]) / 3.0;
        let vib = |c: f32| (mean + (c - mean) * 1.68).clamp(0.0, 1.0);
        let mut r = vib(rgb[0]);
        let mut g = vib(rgb[1]);
        let mut b = vib(rgb[2]);
        let peak = r.max(g).max(b);
        if peak > 0.64 {
            let scale = 0.64 / peak;
            r *= scale;
            g *= scale;
            b *= scale;
        }
        let base = i * 4;
        out[base] = (b * 255.0).round() as u8;
        out[base + 1] = (g * 255.0).round() as u8;
        out[base + 2] = (r * 255.0).round() as u8;
        out[base + 3] = 255;
    }
    (out, GW as u32, GH as u32)
}

pub(super) fn extract_vibrant_accent_rgb(bgra: &[u8]) -> [f32; 3] {
    let mut acc_r = 0.0_f32;
    let mut acc_g = 0.0_f32;
    let mut acc_b = 0.0_f32;
    let mut total_w = 0.0_f32;

    for px in bgra.as_chunks::<4>().0.iter().step_by(7) {
        let a = px[3] as f32 / 255.0;
        if a < 0.4 {
            continue;
        }
        let b = px[0] as f32 / 255.0;
        let g = px[1] as f32 / 255.0;
        let r = px[2] as f32 / 255.0;
        let max_c = r.max(g).max(b);
        let min_c = r.min(g).min(b);
        let sat = if max_c > 0.01 {
            (max_c - min_c) / max_c
        } else {
            0.0
        };
        let lum = 0.299 * r + 0.587 * g + 0.114 * b;
        let lum_bias = 1.0 - (lum - 0.52).abs() * 1.4;
        let w = (sat * sat * 2.5 + 0.12) * lum_bias.max(0.15);
        acc_r += r * w;
        acc_g += g * w;
        acc_b += b * w;
        total_w += w;
    }

    if total_w <= 0.01 {
        return [0.24, 0.72, 0.98];
    }
    let inv = 1.0 / total_w;
    [
        (acc_r * inv).clamp(0.12, 0.98),
        (acc_g * inv).clamp(0.12, 0.98),
        (acc_b * inv).clamp(0.16, 0.98),
    ]
}

pub(super) fn create_ambient_glow_brush(
    ctx: &ID2D1DeviceContext,
    rgb: [f32; 3],
    peak_alpha: f32,
) -> windows::core::Result<ID2D1RadialGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: D2D1_COLOR_F {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
                a: peak_alpha,
            },
        },
        D2D1_GRADIENT_STOP {
            position: 0.55,
            color: D2D1_COLOR_F {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
                a: peak_alpha * 0.30,
            },
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: D2D1_COLOR_F {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
                a: 0.0,
            },
        },
    ];
    let collection = unsafe {
        ID2D1RenderTarget::CreateGradientStopCollection(
            ctx,
            &stops,
            D2D1_GAMMA_2_2,
            D2D1_EXTEND_MODE_CLAMP,
        )
    }?;
    let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
        center: Vector2 { X: 0.0, Y: 0.0 },
        gradientOriginOffset: Vector2 { X: 0.0, Y: 0.0 },
        radiusX: 100.0,
        radiusY: 60.0,
    };
    unsafe { ctx.CreateRadialGradientBrush(&props, None, &collection) }
}

fn create_linear_gradient(
    ctx: &ID2D1DeviceContext,
    stops: &[D2D1_GRADIENT_STOP],
) -> windows::core::Result<ID2D1LinearGradientBrush> {
    let collection = unsafe {
        ID2D1RenderTarget::CreateGradientStopCollection(
            ctx,
            stops,
            D2D1_GAMMA_2_2,
            D2D1_EXTEND_MODE_CLAMP,
        )
    }?;
    let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
        startPoint: Vector2 { X: 0.0, Y: 0.0 },
        endPoint: Vector2 { X: 0.0, Y: 1.0 },
    };
    unsafe { ctx.CreateLinearGradientBrush(&props, None, &collection) }
}

pub(super) fn create_brushes(ctx: &ID2D1DeviceContext) -> windows::core::Result<BrushPalette> {
    let make = |r, g, b, a| -> windows::core::Result<ID2D1SolidColorBrush> {
        unsafe { ctx.CreateSolidColorBrush(&D2D1_COLOR_F { r, g, b, a }, None) }
    };

    let glass_fill = create_linear_gradient(
        ctx,
        &[
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: D2D1_COLOR_F {
                    r: 0.030,
                    g: 0.038,
                    b: 0.060,
                    a: 0.62,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.45,
                color: D2D1_COLOR_F {
                    r: 0.020,
                    g: 0.026,
                    b: 0.044,
                    a: 0.67,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    r: 0.012,
                    g: 0.016,
                    b: 0.030,
                    a: 0.72,
                },
            },
        ],
    )?;

    let glass_sheen = create_linear_gradient(
        ctx,
        &[
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: D2D1_COLOR_F {
                    r: 0.92,
                    g: 0.96,
                    b: 1.0,
                    a: 0.11,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.16,
                color: D2D1_COLOR_F {
                    r: 0.90,
                    g: 0.95,
                    b: 1.0,
                    a: 0.02,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.50,
                color: D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.0,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.86,
                color: D2D1_COLOR_F {
                    r: 0.88,
                    g: 0.94,
                    b: 1.0,
                    a: 0.01,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    r: 0.92,
                    g: 0.96,
                    b: 1.0,
                    a: 0.05,
                },
            },
        ],
    )?;

    let glass_border = create_linear_gradient(
        ctx,
        &[
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: D2D1_COLOR_F {
                    r: 0.96,
                    g: 0.98,
                    b: 1.0,
                    a: 0.54,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.34,
                color: D2D1_COLOR_F {
                    r: 0.90,
                    g: 0.96,
                    b: 1.0,
                    a: 0.20,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.68,
                color: D2D1_COLOR_F {
                    r: 0.85,
                    g: 0.93,
                    b: 1.0,
                    a: 0.16,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    r: 0.94,
                    g: 0.97,
                    b: 1.0,
                    a: 0.38,
                },
            },
        ],
    )?;

    let glass_inner_rim = create_linear_gradient(
        ctx,
        &[
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: D2D1_COLOR_F {
                    r: 0.95,
                    g: 0.98,
                    b: 1.0,
                    a: 0.14,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.45,
                color: D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.0,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    r: 0.84,
                    g: 0.94,
                    b: 1.0,
                    a: 0.09,
                },
            },
        ],
    )?;

    Ok(BrushPalette {
        bg: make(0.048, 0.054, 0.072, 0.88)?,
        border: make(1.0, 1.0, 1.0, 0.28)?,
        glass_fill,
        glass_sheen,
        glass_border,
        glass_inner_rim,
        glass_edge_shadow: make(0.02, 0.03, 0.06, 0.34)?,
        surface_card: make(0.045, 0.058, 0.095, 0.50)?,
        surface_hover: make(0.10, 0.135, 0.21, 0.64)?,
        card_border: make(1.0, 1.0, 1.0, 0.09)?,
        text_primary: make(0.99, 0.99, 1.0, 1.0)?,
        text_secondary: make(0.92, 0.95, 1.0, 0.96)?,
        text_shadow: make(0.01, 0.015, 0.03, 0.56)?,
        accent: make(0.38, 0.85, 1.0, 1.0)?,
        accent_soft: make(0.14, 0.42, 0.62, 0.54)?,
        sys_accent: make(0.38, 0.85, 1.0, 1.0)?,
        sys_accent_soft: make(0.14, 0.42, 0.62, 0.54)?,
        anti_accent: make(0.38, 0.85, 1.0, 1.0)?,
        switch_off: make(0.19, 0.23, 0.31, 0.54)?,
        knob: make(0.99, 0.99, 1.0, 1.0)?,
        danger_bg: make(0.58, 0.10, 0.13, 0.64)?,
        danger_hover: make(0.74, 0.15, 0.18, 0.78)?,
        danger_border: make(0.96, 0.44, 0.46, 0.56)?,
        power_ok: make(0.36, 0.90, 0.56, 1.0)?,
        power_warn: make(0.98, 0.52, 0.28, 1.0)?,
        art_bg: make(0.10, 0.12, 0.18, 0.75)?,
    })
}

pub(super) fn bitmap_rect_transform(
    src_left: f32,
    src_top: f32,
    src_right: f32,
    src_bottom: f32,
    dst: D2D_RECT_F,
) -> Matrix3x2 {
    let sw = (src_right - src_left).max(1.0);
    let sh = (src_bottom - src_top).max(1.0);
    let dw = (dst.right - dst.left).max(1.0);
    let dh = (dst.bottom - dst.top).max(1.0);
    let sx = dw / sw;
    let sy = dh / sh;
    Matrix3x2 {
        M11: sx,
        M12: 0.0,
        M21: 0.0,
        M22: sy,
        M31: dst.left - src_left * sx,
        M32: dst.top - src_top * sy,
    }
}

pub(super) fn compute_center_crop_rect((w, h): (u32, u32)) -> Option<D2D_RECT_F> {
    if w == 0 || h == 0 {
        return None;
    }
    let wf = w as f32;
    let hf = h as f32;
    let side = wf.min(hf);
    let left = (wf - side) * 0.5;
    let top = (hf - side) * 0.5;
    Some(D2D_RECT_F {
        left,
        top,
        right: left + side,
        bottom: top + side,
    })
}
