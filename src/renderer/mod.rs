mod anti_view;
mod compositor;
mod expanded_view;
mod hud_views;
mod media_views;
mod settings_views;
mod shelf_view;
#[cfg(test)]
mod tests;
mod theme;

use compositor::{
    CompositionHost, attach_swapchain_backbuffer, create_low_power_d3d11_device,
};
use theme::{
    BrushPalette, IconGeometries, bitmap_rect_transform, boost_accent_for_readability,
    build_ambient_aura_bgra, compute_center_crop_rect, create_ambient_glow_brush, create_brushes,
    create_icon_geometries, extract_vibrant_accent_rgb,
};

use std::mem::ManuallyDrop;

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_BRUSH_PROPERTIES,
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES,
    D2D1_CAP_STYLE_ROUND, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2,
    D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1, D2D1_LINE_JOIN_ROUND,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, D2D1CreateFactory, ID2D1Bitmap, ID2D1Bitmap1,
    ID2D1BitmapBrush, ID2D1Device, ID2D1DeviceContext, ID2D1Factory, ID2D1Factory1, ID2D1Geometry,
    ID2D1Image, ID2D1Layer, ID2D1LinearGradientBrush, ID2D1PathGeometry, ID2D1RadialGradientBrush,
    ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS,
    DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory,
    IDWriteInlineObject, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, IDXGIDevice, IDXGIFactory6,
};
use windows::core::{Interface, PCWSTR};
use windows_numerics::{Matrix3x2, Vector2};

use crate::hud::{AudioOutputDevice, VolumeSnapshot};
use crate::media::{ArtworkUpdate, DecodedArtwork};
use crate::state::{
    AppConfig, HudContent, IslandMode, IslandNoticeView, MAX_AUDIO_DEVICES_VISIBLE,
    MAX_SHELF_VISIBLE_ROWS, MediaDisplayState, NoticeKind, PillRectDip, UiAction, UiHitbox,
    UiRectDip, audio_devices_height_for_count, compute_surface_size_px,
    current_windows_filetime_ticks, fill_audio_devices_hitboxes, fill_interactive_hitboxes,
    fill_notice_hitboxes, fill_volume_hud_hitboxes, format_duration_ticks, settings_section_tops,
    shelf_pill_rect, target_dimensions_for_mode, target_dimensions_for_state,
};
use crate::tray::to_wide_null;
use crate::updater::APP_VERSION;

#[derive(Clone, Copy, Default)]
pub struct ShelfRenderInput<'a> {
    pub visible: bool,
    pub open: bool,
    pub drag_hover: bool,
    pub items: &'a [crate::shelf::ShelfItem],
    pub scroll: usize,
}

pub struct RenderFrameInput<'a> {
    pub pill: PillRectDip,
    pub mode: IslandMode,
    pub pill_opacity: f32,
    pub content_alpha: f32,
    pub config: AppConfig,
    pub media: &'a MediaDisplayState,
    pub active_hud: Option<HudContent>,
    pub volume_scrub_ratio: Option<f32>,
    pub hovered_action: Option<UiAction>,
    pub timeline_scrub_ratio: Option<f32>,
    pub monitor_label: &'a str,
    pub visualizer_phase: f32,
    pub audio_devices: &'a [AudioOutputDevice],
    pub audio_device_scroll: usize,
    pub volume_snapshot: VolumeSnapshot,
    pub current_audio_device_name: Option<&'a str>,
    pub notice: Option<IslandNoticeView<'a>>,
    pub available_update_tag: Option<&'a str>,
    pub anti: Option<&'a crate::anti::AntiProgressSnapshot>,
    pub anti_hooks_installed: bool,
    pub anti_spinner_phase: f32,
    pub shelf: ShelfRenderInput<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextFormatSlot {
    Title,
    Subtitle,
    BodyLeft,
    Compact,
    CompactSub,
    Button,
    ButtonCenter,
    Small,
    SmallLeft,
    SmallRight,
}

struct TextLayoutCacheEntry {
    slot: TextFormatSlot,
    text: String,
    w_tenth: i32,
    h_tenth: i32,
    layout: IDWriteTextLayout,
}

struct TextFormats {
    title: IDWriteTextFormat,
    subtitle: IDWriteTextFormat,
    body_left: IDWriteTextFormat,
    compact: IDWriteTextFormat,
    compact_sub: IDWriteTextFormat,
    button: IDWriteTextFormat,
    button_center: IDWriteTextFormat,
    small: IDWriteTextFormat,
    small_left: IDWriteTextFormat,
    small_right: IDWriteTextFormat,
}

impl TextFormats {
    fn by_slot(&self, slot: TextFormatSlot) -> &IDWriteTextFormat {
        match slot {
            TextFormatSlot::Title => &self.title,
            TextFormatSlot::Subtitle => &self.subtitle,
            TextFormatSlot::BodyLeft => &self.body_left,
            TextFormatSlot::Compact => &self.compact,
            TextFormatSlot::CompactSub => &self.compact_sub,
            TextFormatSlot::Button => &self.button,
            TextFormatSlot::ButtonCenter => &self.button_center,
            TextFormatSlot::Small => &self.small,
            TextFormatSlot::SmallLeft => &self.small_left,
            TextFormatSlot::SmallRight => &self.small_right,
        }
    }
}

#[derive(Clone, Copy)]
struct DrawCanvas<'a> {
    ctx: &'a ID2D1DeviceContext,
    scale: f32,
    offset_x_px: f32,
    offset_y_px: f32,
}

impl<'a> DrawCanvas<'a> {
    fn base_transform(&self) -> Matrix3x2 {
        Matrix3x2 {
            M11: self.scale,
            M12: 0.0,
            M21: 0.0,
            M22: self.scale,
            M31: self.offset_x_px,
            M32: self.offset_y_px,
        }
    }

    fn set_local_translation(&self, tx: f32, ty: f32) {
        let mat = Matrix3x2 {
            M11: self.scale,
            M12: 0.0,
            M21: 0.0,
            M22: self.scale,
            M31: self.offset_x_px + tx * self.scale,
            M32: self.offset_y_px + ty * self.scale,
        };
        unsafe {
            self.ctx.SetTransform(&mat);
        }
    }

    fn reset_transform(&self) {
        let mat = self.base_transform();
        unsafe {
            self.ctx.SetTransform(&mat);
        }
    }
}

pub struct IslandRenderer {
    _d3d_device: ID3D11Device,
    d2d_factory: ID2D1Factory1,
    _d2d_device: ID2D1Device,
    d2d_context: ID2D1DeviceContext,
    dwrite_factory: IDWriteFactory,
    comp_host: CompositionHost,
    artwork_bitmap: Option<ID2D1Bitmap1>,
    artwork_brush: Option<ID2D1BitmapBrush>,
    artwork_aura_bitmap: Option<ID2D1Bitmap1>,
    artwork_aura_brush: Option<ID2D1BitmapBrush>,
    artwork_size_px: (u32, u32),
    ambient_glow_brush: ID2D1RadialGradientBrush,
    clip_layer: ID2D1Layer,
    brushes: BrushPalette,
    fonts: TextFormats,
    icons: IconGeometries,
    utf16_scratch: Vec<u16>,
    text_cache: Vec<TextLayoutCacheEntry>,
    hitbox_scratch: Vec<UiHitbox>,
    adapter_name: String,
    dpi_scale: f32,
}

impl IslandRenderer {
    pub fn new(hwnd: HWND, dpi_scale: f32) -> windows::core::Result<Self> {
        let factory6: IDXGIFactory6 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }?;
        let (d3d_device, adapter_name) = create_low_power_d3d11_device(&factory6)?;
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;

        let d2d_factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;
        let d2d_device = unsafe { d2d_factory.CreateDevice(&dxgi_device) }?;
        let d2d_context =
            unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE) }?;

        let clamped_dpi = dpi_scale.clamp(0.5, 4.0);
        let (surface_w, surface_h) = compute_surface_size_px(clamped_dpi);
        let mut comp_host = CompositionHost::new(
            hwnd,
            &d3d_device,
            &dxgi_device,
            &d2d_device,
            &factory6,
            surface_w as f32,
            surface_h as f32,
        )?;

        if let CompositionHost::DirectCompFallback {
            swap_chain,
            target_bitmap,
            ..
        } = &mut comp_host
        {
            *target_bitmap = Some(attach_swapchain_backbuffer(&d2d_context, swap_chain)?);
        }

        let clip_layer = unsafe { d2d_context.CreateLayer(None) }?;
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;

        let brushes = create_brushes(&d2d_context)?;
        let ambient_glow_brush = create_ambient_glow_brush(&d2d_context, [0.32, 0.80, 1.0], 0.14)?;
        let fonts = create_text_formats(&dwrite_factory)?;
        let icons = create_icon_geometries(&d2d_factory)?;

        let mut renderer = Self {
            _d3d_device: d3d_device,
            d2d_factory,
            _d2d_device: d2d_device,
            d2d_context,
            dwrite_factory,
            comp_host,
            artwork_bitmap: None,
            artwork_brush: None,
            artwork_aura_bitmap: None,
            artwork_aura_brush: None,
            artwork_size_px: (0, 0),
            ambient_glow_brush,
            clip_layer,
            brushes,
            fonts,
            icons,
            utf16_scratch: Vec::with_capacity(128),
            text_cache: Vec::with_capacity(32),
            hitbox_scratch: Vec::with_capacity(8),
            adapter_name,
            dpi_scale: clamped_dpi,
        };
        renderer.warm_up_text_formats();
        Ok(renderer)
    }

    pub fn hide_backdrop(&mut self) {
        self.comp_host.hide_backdrop();
    }

    pub fn has_glass_backdrop(&self) -> bool {
        self.comp_host.has_live_backdrop()
    }

    pub fn has_blur_effect(&self) -> bool {
        matches!(
            &self.comp_host,
            CompositionHost::WinRtGlass {
                backdrop: Some(layer),
                ..
            } if layer.blur_effect_active
        )
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    pub fn update_artwork(&mut self, update: ArtworkUpdate) {
        match update {
            ArtworkUpdate::Unchanged => {}
            ArtworkUpdate::Clear => {
                self.artwork_brush = None;
                self.artwork_bitmap = None;
                self.artwork_aura_brush = None;
                self.artwork_aura_bitmap = None;
                self.artwork_size_px = (0, 0);
                self.apply_media_theme_colors([0.38, 0.85, 1.0], 0.14);
            }
            ArtworkUpdate::Set(art) => {
                let sz = (art.width, art.height);
                let accent = boost_accent_for_readability(extract_vibrant_accent_rgb(&art.bgra));
                self.apply_media_theme_colors(accent, 0.28);

                let (aura_bgra, aura_w, aura_h) = build_ambient_aura_bgra(&art);
                self.artwork_aura_bitmap = self
                    .upload_bgra_bitmap(aura_w, aura_h, &aura_bgra)
                    .ok();
                self.artwork_aura_brush = self
                    .artwork_aura_bitmap
                    .as_ref()
                    .and_then(|bmp| self.create_clamped_bitmap_brush(bmp).ok());

                match self.upload_artwork_bitmap(&art) {
                    Ok(bmp) => {
                        self.artwork_brush = self.create_clamped_bitmap_brush(&bmp).ok();
                        self.artwork_bitmap = Some(bmp);
                        self.artwork_size_px = sz;
                    }
                    Err(_) => {
                        self.artwork_brush = None;
                        self.artwork_bitmap = None;
                        self.artwork_size_px = (0, 0);
                    }
                }
            }
        }
    }

    fn warm_up_text_formats(&mut self) {
        let samples: &[(TextFormatSlot, &str)] = &[
            (TextFormatSlot::Title, "Win Dynamic Island"),
            (TextFormatSlot::Title, "Thiết bị phát âm thanh"),
            (TextFormatSlot::Title, "Có bản cập nhật mới"),
            (TextFormatSlot::Subtitle, "Win Dynamic Island"),
            (TextFormatSlot::BodyLeft, "Bạn đang sử dụng phiên bản mới nhất."),
            (TextFormatSlot::Compact, "Đang cắm sạc"),
            (TextFormatSlot::Compact, "Đang dùng pin"),
            (TextFormatSlot::Compact, "Pin yếu"),
            (TextFormatSlot::Compact, "Win Dynamic Island"),
            (TextFormatSlot::CompactSub, "Bấm để mở rộng hoặc cuộn chỉnh loa"),
            (TextFormatSlot::Button, "Bảng điều khiển nhanh"),
            (TextFormatSlot::ButtonCenter, "Cập nhật ngay"),
            (TextFormatSlot::ButtonCenter, "Thoát ứng dụng"),
            (TextFormatSlot::Small, "Cài đặt"),
            (TextFormatSlot::SmallLeft, "Âm thanh"),
            (TextFormatSlot::SmallRight, "Đang dùng"),
            (TextFormatSlot::SmallRight, "100%"),
            (TextFormatSlot::CompactSub, "HUD & Âm thanh"),
            (TextFormatSlot::Button, "Kiểm tra cập nhật"),
            (TextFormatSlot::Button, "Thiết bị phát"),
        ];
        for &(slot, text) in samples {
            let wide = to_wide_null(text);
            let fmt = self.fonts.by_slot(slot);
            if let Ok(layout) = unsafe { self.dwrite_factory.CreateTextLayout(&wide, fmt, 180.0, 28.0) } {
                let mut metrics = Default::default();
                let _ = unsafe { layout.GetMetrics(&mut metrics) };
            }
        }
    }

    pub fn resize_for_dpi(&mut self, dpi_scale: f32) -> windows::core::Result<()> {
        let clamped = dpi_scale.clamp(0.5, 4.0);
        if (self.dpi_scale - clamped).abs() < 0.001 {
            return Ok(());
        }
        let (surface_w, surface_h) = compute_surface_size_px(clamped);
        self.comp_host
            .resize_content(&self.d2d_context, surface_w as f32, surface_h as f32)?;
        self.dpi_scale = clamped;
        self.text_cache.clear();
        Ok(())
    }

    pub fn render(&mut self, input: &RenderFrameInput<'_>) -> windows::core::Result<()> {
        let scale = self.dpi_scale.clamp(0.5, 4.0);
        let shelf_rect = if input.shelf.visible && input.pill_opacity > 0.02 {
            shelf_pill_rect(
                input.pill,
                input.mode,
                input.shelf.open,
                input.shelf.items.len(),
            )
        } else {
            None
        };
        self.comp_host
            .update_backdrops(input.pill, shelf_rect, input.pill_opacity, scale);

        let clear = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        let has_backdrop = self.comp_host.has_live_backdrop();

        match &self.comp_host {
            CompositionHost::WinRtGlass {
                surface_interop, ..
            } => {
                let surface_interop = surface_interop.clone();
                let mut offset = POINT::default();
                let draw_ctx: ID2D1DeviceContext =
                    unsafe { surface_interop.BeginDraw(None, &mut offset) }?;
                // Giữ baseline 96 DPI và phóng đại tường minh qua ma trận world để không bao giờ bị rơi về 1x trên màn hình HiDPI.
                let canvas = DrawCanvas {
                    ctx: &draw_ctx,
                    scale,
                    offset_x_px: offset.x as f32,
                    offset_y_px: offset.y as f32,
                };
                let base_transform = canvas.base_transform();
                unsafe {
                    draw_ctx.SetDpi(96.0, 96.0);
                    draw_ctx.SetTransform(&base_transform);
                    draw_ctx.Clear(Some(&clear));
                }
                self.draw_scene(canvas, input, shelf_rect, has_backdrop);
                unsafe {
                    draw_ctx.SetTransform(&Matrix3x2::identity());
                }
                drop(draw_ctx);
                unsafe {
                    surface_interop.EndDraw()?;
                }
                Ok(())
            }
            CompositionHost::DirectCompFallback {
                swap_chain,
                target_bitmap,
                ..
            } => {
                let Some(bitmap) = target_bitmap.clone() else {
                    return Ok(());
                };
                let swap_chain = swap_chain.clone();
                let d2d_ctx = self.d2d_context.clone();
                let target_image: ID2D1Image = bitmap.cast()?;
                let canvas = DrawCanvas {
                    ctx: &d2d_ctx,
                    scale,
                    offset_x_px: 0.0,
                    offset_y_px: 0.0,
                };
                let base_transform = canvas.base_transform();
                unsafe {
                    d2d_ctx.SetTarget(&target_image);
                    d2d_ctx.BeginDraw();
                    d2d_ctx.SetDpi(96.0, 96.0);
                    d2d_ctx.SetTransform(&base_transform);
                    d2d_ctx.Clear(Some(&clear));
                }
                self.draw_scene(canvas, input, shelf_rect, false);
                unsafe {
                    d2d_ctx.SetTransform(&Matrix3x2::identity());
                    d2d_ctx.EndDraw(None, None)?;
                    swap_chain.Present(0, DXGI_PRESENT(0)).ok()?;
                }
                Ok(())
            }
        }
    }

    fn draw_scene(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
        shelf_rect: Option<PillRectDip>,
        has_backdrop: bool,
    ) {
        if input.pill_opacity > 0.02 && input.pill.height >= 2.0 {
            let show_media_aura = !matches!(
                input.mode,
                IslandMode::Slim
                    | IslandMode::Settings
                    | IslandMode::AudioDevices
                    | IslandMode::Notice
            ) && input.active_hud.is_none()
                && input.anti.is_none()
                && input.media.has_session;
            unsafe {
                self.draw_pill_shell(
                    canvas,
                    input.pill,
                    input.pill_opacity,
                    has_backdrop,
                    show_media_aura,
                );
            }
            let (_, target_h, _, _) =
                target_dimensions_for_state(input.mode, input.audio_devices.len());
            let effective_alpha = input.content_alpha.clamp(0.0, 1.0)
                * mode_content_reveal_alpha(input.pill, input.mode, target_h);
            if effective_alpha > 0.03 && input.mode != IslandMode::Hidden {
                unsafe {
                    self.draw_clipped_contents(canvas, input, effective_alpha);
                }
            }
        }
        if let Some(s_rect) = shelf_rect {
            unsafe {
                self.draw_shelf_surface(
                    canvas,
                    s_rect,
                    input.shelf,
                    input.pill_opacity,
                    has_backdrop,
                    input.hovered_action,
                );
            }
        }
    }

    unsafe fn draw_pill_shell(
        &self,
        canvas: DrawCanvas<'_>,
        pill: PillRectDip,
        opacity: f32,
        has_backdrop: bool,
        show_media_aura: bool,
    ) {
        let top = pill.top + 0.5;
        let bottom = pill.top + pill.height - 0.5;
        let center_x = pill.left + pill.width * 0.5;
        let rounded = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: pill.left + 0.5,
                top,
                right: pill.left + pill.width - 0.5,
                bottom,
            },
            radiusX: pill.corner_radius,
            radiusY: pill.corner_radius,
        };
        let inner_rim_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: pill.left + 1.6,
                top: pill.top + 1.6,
                right: pill.left + pill.width - 1.6,
                bottom: pill.top + pill.height - 1.6,
            },
            radiusX: (pill.corner_radius - 1.1).max(1.0),
            radiusY: (pill.corner_radius - 1.1).max(1.0),
        };

        let start = Vector2 { X: center_x, Y: top };
        let end = Vector2 {
            X: center_x,
            Y: bottom,
        };
        let glow_center = Vector2 {
            X: pill.left + (pill.width * 0.24).min(96.0),
            Y: pill.top + pill.height * 0.45,
        };
        let opacity = opacity.clamp(0.0, 1.0);
        let has_video_aura = show_media_aura && self.artwork_aura_brush.is_some();

        unsafe {
            if !has_backdrop {
                self.brushes.bg.SetOpacity(opacity);
                canvas.ctx.FillRoundedRectangle(&rounded, &self.brushes.bg);
            }

            if has_video_aura && let Some(aura_brush) = &self.artwork_aura_brush {
                // Giữ lớp nền khói tối đủ đầm (~0.48-0.56) dưới aura để nền trắng không tẩy nhạt màu video.
                self.brushes.glass_fill.SetStartPoint(start);
                self.brushes.glass_fill.SetEndPoint(end);
                self.brushes.glass_fill.SetOpacity(opacity * 0.78);
                canvas
                    .ctx
                    .FillRoundedRectangle(&rounded, &self.brushes.glass_fill);

                let dst = rounded.rect;
                let mat = bitmap_rect_transform(0.0, 0.0, 16.0, 10.0, dst);
                aura_brush.SetTransform(&mat);
                aura_brush.SetOpacity(opacity * 0.54);
                canvas.ctx.FillRoundedRectangle(&rounded, aura_brush);
            } else {
                // Giữ độ đầm kính khói tối đầy đủ khi không có aura (Settings/Slim/chờ) để tránh lóa trên nền trắng.
                self.brushes.glass_fill.SetStartPoint(start);
                self.brushes.glass_fill.SetEndPoint(end);
                self.brushes.glass_fill.SetOpacity(opacity);
                canvas
                    .ctx
                    .FillRoundedRectangle(&rounded, &self.brushes.glass_fill);
            }

            let glow_scale = if has_video_aura { 1.0 } else { 0.24 };
            self.ambient_glow_brush.SetCenter(glow_center);
            self.ambient_glow_brush
                .SetRadiusX((pill.width * 0.62).max(40.0));
            self.ambient_glow_brush
                .SetRadiusY((pill.height * 0.96).max(24.0));
            self.ambient_glow_brush.SetOpacity(opacity * glow_scale);
            canvas
                .ctx
                .FillRoundedRectangle(&rounded, &self.ambient_glow_brush);

            self.brushes.glass_sheen.SetStartPoint(start);
            self.brushes.glass_sheen.SetEndPoint(end);
            self.brushes.glass_sheen.SetOpacity(opacity);
            canvas
                .ctx
                .FillRoundedRectangle(&rounded, &self.brushes.glass_sheen);

            self.brushes.glass_inner_rim.SetStartPoint(start);
            self.brushes.glass_inner_rim.SetEndPoint(end);
            self.brushes.glass_inner_rim.SetOpacity(opacity);
            canvas.ctx.DrawRoundedRectangle(
                &inner_rim_rect,
                &self.brushes.glass_inner_rim,
                0.9,
                None,
            );

            self.brushes.glass_edge_shadow.SetOpacity(opacity);
            canvas.ctx.DrawRoundedRectangle(
                &rounded,
                &self.brushes.glass_edge_shadow,
                2.0,
                None,
            );

            self.brushes.glass_border.SetStartPoint(start);
            self.brushes.glass_border.SetEndPoint(end);
            self.brushes.glass_border.SetOpacity(opacity);
            canvas
                .ctx
                .DrawRoundedRectangle(&rounded, &self.brushes.glass_border, 1.15, None);
        }
    }

    unsafe fn draw_clipped_contents(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
        effective_alpha: f32,
    ) {
        let (target_w, target_h, target_r, _) =
            target_dimensions_for_state(input.mode, input.audio_devices.len());
        let need_clip = effective_alpha < 0.995
            || (input.pill.width - target_w).abs() > 0.5
            || (input.pill.height - target_h).abs() > 0.5
            || (input.pill.corner_radius - target_r).abs() > 0.25;

        if need_clip {
            let inner_rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: input.pill.left + 1.0,
                    top: input.pill.top + 1.0,
                    right: input.pill.left + input.pill.width - 1.0,
                    bottom: input.pill.top + input.pill.height - 1.0,
                },
                radiusX: (input.pill.corner_radius - 1.0).max(1.0),
                radiusY: (input.pill.corner_radius - 1.0).max(1.0),
            };

            let Ok(geom) = (unsafe {
                self.d2d_factory
                    .CreateRoundedRectangleGeometry(&inner_rounded)
            }) else {
                return;
            };
            let Ok(geom_base): windows::core::Result<ID2D1Geometry> = geom.cast() else {
                return;
            };

            let mut layer_params = D2D1_LAYER_PARAMETERS1 {
                contentBounds: inner_rounded.rect,
                geometricMask: ManuallyDrop::new(Some(geom_base)),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: Matrix3x2::identity(),
                opacity: effective_alpha.clamp(0.0, 1.0),
                opacityBrush: ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS1_NONE,
            };

            unsafe {
                canvas.ctx.PushLayer(&layer_params, &self.clip_layer);
                // Giải phóng refcount ManuallyDrop ngay sau PushLayer để không rò rỉ đối tượng COM.
                ManuallyDrop::drop(&mut layer_params.geometricMask);
                ManuallyDrop::drop(&mut layer_params.opacityBrush);
            }
        }

        match input.mode {
            IslandMode::Slim => unsafe { self.draw_slim(canvas, input) },
            IslandMode::Compact => unsafe { self.draw_compact(canvas, input) },
            IslandMode::Expanded => unsafe { self.draw_expanded(canvas, input) },
            IslandMode::Settings => unsafe { self.draw_settings(canvas, input) },
            IslandMode::AudioDevices => unsafe { self.draw_audio_devices(canvas, input) },
            IslandMode::Notice => unsafe { self.draw_notice(canvas, input) },
            IslandMode::Hidden => {}
        }

        if need_clip {
            unsafe {
                canvas.ctx.PopLayer();
            }
        }
    }

    unsafe fn get_or_create_text_layout(
        &mut self,
        text: &str,
        slot: TextFormatSlot,
        max_w: f32,
        max_h: f32,
    ) -> Option<IDWriteTextLayout> {
        let w_tenth = (max_w * 10.0).round() as i32;
        let h_tenth = (max_h * 10.0).round() as i32;

        if let Some(idx) = self.text_cache.iter().position(|e| {
            e.slot == slot && e.w_tenth == w_tenth && e.h_tenth == h_tenth && e.text == text
        }) {
            return Some(self.text_cache[idx].layout.clone());
        }

        self.utf16_scratch.clear();
        self.utf16_scratch.extend(text.encode_utf16());
        let format = self.fonts.by_slot(slot);
        let created = unsafe {
            self.dwrite_factory
                .CreateTextLayout(&self.utf16_scratch, format, max_w, max_h)
        }
        .ok()?;
        if self.text_cache.len() >= 32 {
            self.text_cache.remove(0);
        }
        self.text_cache.push(TextLayoutCacheEntry {
            slot,
            text: text.to_owned(),
            w_tenth,
            h_tenth,
            layout: created.clone(),
        });
        Some(created)
    }

    unsafe fn measure_text_width(
        &mut self,
        text: &str,
        slot: TextFormatSlot,
        max_w: f32,
        max_h: f32,
    ) -> Option<f32> {
        let layout = unsafe { self.get_or_create_text_layout(text, slot, max_w, max_h) }?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe { layout.GetMetrics(&mut metrics) }.ok()?;
        Some(metrics.width)
    }

    unsafe fn draw_text(
        &mut self,
        canvas: DrawCanvas<'_>,
        text: &str,
        slot: TextFormatSlot,
        brush: &ID2D1SolidColorBrush,
        rect: UiRectDip,
    ) {
        if rect.width() <= 1.0 || rect.height() <= 1.0 {
            return;
        }
        let Some(layout) =
            (unsafe { self.get_or_create_text_layout(text, slot, rect.width(), rect.height()) })
        else {
            return;
        };

        unsafe {
            canvas.ctx.DrawTextLayout(
                Vector2 {
                    X: rect.left,
                    Y: rect.top + 0.6,
                },
                &layout,
                &self.brushes.text_shadow,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
            canvas.ctx.DrawTextLayout(
                Vector2 {
                    X: rect.left,
                    Y: rect.top,
                },
                &layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
    }
}

fn create_text_formats(factory: &IDWriteFactory) -> windows::core::Result<TextFormats> {
    Ok(TextFormats {
        title: make_format(
            factory,
            14.2,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        subtitle: make_format(
            factory,
            12.0,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        body_left: make_format(
            factory,
            12.0,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
            DWRITE_WORD_WRAPPING_WRAP,
        )?,
        compact: make_format(
            factory,
            12.2,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        compact_sub: make_format(
            factory,
            10.8,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        button: make_format(
            factory,
            12.2,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        button_center: make_format(
            factory,
            12.2,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        small: make_format(
            factory,
            11.2,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        small_left: make_format(
            factory,
            11.2,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        small_right: make_format(
            factory,
            11.2,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
    })
}

fn make_format(
    factory: &IDWriteFactory,
    size: f32,
    weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
    align: DWRITE_TEXT_ALIGNMENT,
    paragraph_align: DWRITE_PARAGRAPH_ALIGNMENT,
    wrapping: DWRITE_WORD_WRAPPING,
) -> windows::core::Result<IDWriteTextFormat> {
    let family = to_wide_null("Segoe UI Variable Text");
    let locale = to_wide_null("vi-VN");
    let fmt = unsafe {
        factory.CreateTextFormat(
            PCWSTR(family.as_ptr()),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            PCWSTR(locale.as_ptr()),
        )
    }?;

    unsafe {
        fmt.SetTextAlignment(align)?;
        fmt.SetParagraphAlignment(paragraph_align)?;
        fmt.SetWordWrapping(wrapping)?;
        let trimming_sign: IDWriteInlineObject = factory.CreateEllipsisTrimmingSign(&fmt)?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        fmt.SetTrimming(&trimming, &trimming_sign)?;
    }
    Ok(fmt)
}

fn mode_content_reveal_alpha(pill: PillRectDip, mode: IslandMode, target_h: f32) -> f32 {
    match mode {
        IslandMode::Hidden => 0.0,
        IslandMode::Slim => {
            let w_fade = ((235.0 - pill.width) / 55.0).clamp(0.0, 1.0);
            let h_fade = ((46.0 - pill.height) / 18.0).clamp(0.0, 1.0);
            w_fade.min(h_fade)
        }
        IslandMode::Compact => {
            let w_fade = ((pill.width - 176.0) / 72.0).clamp(0.0, 1.0);
            let h_fade = ((88.0 - pill.height) / 42.0).clamp(0.0, 1.0);
            w_fade.min(h_fade)
        }
        IslandMode::Expanded | IslandMode::Notice => {
            let grow_fade = ((pill.height - 58.0) / 52.0).clamp(0.0, 1.0);
            let shrink_fade = ((245.0 - pill.height) / 60.0).clamp(0.0, 1.0);
            grow_fade.min(shrink_fade)
        }
        IslandMode::Settings => ((pill.height - 175.0) / 80.0).clamp(0.0, 1.0),
        IslandMode::AudioDevices => {
            let start = (target_h * 0.45).min(120.0);
            let span = (target_h * 0.35).max(36.0);
            ((pill.height - start) / span).clamp(0.0, 1.0)
        }
    }
}

fn virtual_pill_anchored(
    pill: PillRectDip,
    target_w: f32,
    target_h: f32,
    center_y: bool,
) -> PillRectDip {
    let top = if center_y {
        pill.top + ((pill.height - target_h) * 0.5).clamp(-10.0, 0.0)
    } else {
        pill.top
    };
    PillRectDip {
        left: pill.left + (pill.width - target_w) * 0.5,
        top,
        width: target_w,
        height: target_h,
        corner_radius: pill.corner_radius,
    }
}
