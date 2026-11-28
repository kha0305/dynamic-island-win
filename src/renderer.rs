use std::mem::{ManuallyDrop, size_of};

use windows::Foundation::{PropertyValue, Size};
use windows::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use windows::Graphics::Effects::{
    IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource, IGraphicsEffectSource_Impl,
};
use windows::System::DispatcherQueueController;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{
    CompositionBrush, CompositionDrawingSurface, CompositionEffectSourceParameter,
    CompositionGraphicsDevice, CompositionRoundedRectangleGeometry, CompositionStretch,
    CompositionSurfaceBrush, Compositor, ContainerVisual, SpriteVisual, Visual,
};
use windows::Win32::Foundation::{E_BOUNDS, E_INVALIDARG, HWND, POINT, TRUE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BORDER_MODE_HARD, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1GaussianBlur, CLSID_D2D1Saturation, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_BRUSH_PROPERTIES, D2D1_CAP_STYLE_ROUND, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_GAMMA_2_2, D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED, D2D1_GAUSSIANBLUR_PROP_BORDER_MODE,
    D2D1_GAUSSIANBLUR_PROP_OPTIMIZATION, D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION,
    D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1, D2D1_LINE_JOIN_ROUND,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_SATURATION_PROP_SATURATION, D2D1_STROKE_STYLE_PROPERTIES,
    D2D1CreateFactory, ID2D1Bitmap, ID2D1Bitmap1, ID2D1BitmapBrush, ID2D1Device, ID2D1DeviceContext,
    ID2D1Factory, ID2D1Factory1, ID2D1Geometry, ID2D1Image, ID2D1Layer, ID2D1LinearGradientBrush,
    ID2D1PathGeometry, ID2D1RadialGradientBrush, ID2D1RenderTarget, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN, D3D_DRIVER_TYPE_WARP,
    D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteInlineObject,
    IDWriteTextFormat, IDWriteTextLayout,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_HOSTBACKDROPBRUSH, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_GPU_PREFERENCE_MINIMUM_POWER,
    DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter1, IDXGIDevice, IDXGIFactory2, IDXGIFactory6,
    IDXGISurface, IDXGISwapChain1,
};
use windows::Win32::System::WinRT::Composition::{
    ICompositionDrawingSurfaceInterop, ICompositorDesktopInterop, ICompositorInterop,
};
use windows::Win32::System::WinRT::Graphics::Direct2D::{
    GRAPHICS_EFFECT_PROPERTY_MAPPING, GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT,
    IGraphicsEffectD2D1Interop, IGraphicsEffectD2D1Interop_Impl,
};
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_ASTA, DQTAT_COM_NONE, DQTAT_COM_STA,
    DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
};
use windows::Win32::UI::Controls::MARGINS;
use windows::core::{BOOL, GUID, HSTRING, Interface, PCWSTR, implement};
use windows_numerics::{Matrix3x2, Vector2, Vector3};

use crate::hud::{AudioOutputDevice, VolumeSnapshot};
use crate::media::{ArtworkUpdate, DecodedArtwork};
use crate::state::{
    AppConfig, HudContent, IslandMode, IslandNoticeView, MAX_AUDIO_DEVICES_VISIBLE,
    MediaDisplayState, NoticeKind, PillRectDip, UiAction, UiHitbox, UiRectDip,
    audio_devices_height_for_count, compute_surface_size_px, current_windows_filetime_ticks,
    fill_audio_devices_hitboxes, fill_interactive_hitboxes, fill_notice_hitboxes,
    fill_volume_hud_hitboxes, format_duration_ticks, settings_section_tops,
    target_dimensions_for_mode, target_dimensions_for_state,
};
use crate::tray::to_wide_null;
use crate::updater::APP_VERSION;

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
}

struct BrushPalette {
    bg: ID2D1SolidColorBrush,
    border: ID2D1SolidColorBrush,
    glass_fill: ID2D1LinearGradientBrush,
    glass_sheen: ID2D1LinearGradientBrush,
    glass_border: ID2D1LinearGradientBrush,
    glass_inner_rim: ID2D1LinearGradientBrush,
    glass_edge_shadow: ID2D1SolidColorBrush,
    surface_card: ID2D1SolidColorBrush,
    surface_hover: ID2D1SolidColorBrush,
    card_border: ID2D1SolidColorBrush,
    text_primary: ID2D1SolidColorBrush,
    text_secondary: ID2D1SolidColorBrush,
    text_shadow: ID2D1SolidColorBrush,
    accent: ID2D1SolidColorBrush,
    accent_soft: ID2D1SolidColorBrush,
    sys_accent: ID2D1SolidColorBrush,
    sys_accent_soft: ID2D1SolidColorBrush,
    switch_off: ID2D1SolidColorBrush,
    knob: ID2D1SolidColorBrush,
    danger_bg: ID2D1SolidColorBrush,
    danger_hover: ID2D1SolidColorBrush,
    danger_border: ID2D1SolidColorBrush,
    power_ok: ID2D1SolidColorBrush,
    power_warn: ID2D1SolidColorBrush,
    art_bg: ID2D1SolidColorBrush,
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

struct IconGeometries {
    round_stroke: ID2D1StrokeStyle,
    play_triangle: ID2D1PathGeometry,
    prev_triangle: ID2D1PathGeometry,
    next_triangle: ID2D1PathGeometry,
    speaker_cone: ID2D1PathGeometry,
}

#[derive(Clone, Copy, PartialEq)]
struct BackdropState {
    visible: bool,
    left_px: f32,
    top_px: f32,
    width_px: f32,
    height_px: f32,
    radius_px: f32,
    opacity: f32,
}

impl BackdropState {
    const HIDDEN: Self = Self {
        visible: false,
        left_px: 0.0,
        top_px: 0.0,
        width_px: 0.0,
        height_px: 0.0,
        radius_px: 0.0,
        opacity: 0.0,
    };

    fn approx_eq(self, other: Self) -> bool {
        if !self.visible && !other.visible {
            return true;
        }
        self.visible == other.visible
            && (self.left_px - other.left_px).abs() < 0.01
            && (self.top_px - other.top_px).abs() < 0.01
            && (self.width_px - other.width_px).abs() < 0.01
            && (self.height_px - other.height_px).abs() < 0.01
            && (self.radius_px - other.radius_px).abs() < 0.01
            && (self.opacity - other.opacity).abs() < 0.002
    }
}

struct WinRtBackdropLayer {
    visual: SpriteVisual,
    geometry: CompositionRoundedRectangleGeometry,
    last_state: BackdropState,
    blur_effect_active: bool,
}

enum CompositionHost {
    WinRtGlass {
        _dispatcher_queue: DispatcherQueueController,
        _compositor: Compositor,
        _target: DesktopWindowTarget,
        root: ContainerVisual,
        backdrop: Option<WinRtBackdropLayer>,
        graphics_device: CompositionGraphicsDevice,
        _drawing_surface: CompositionDrawingSurface,
        surface_brush: CompositionSurfaceBrush,
        surface_interop: ICompositionDrawingSurfaceInterop,
        content_visual: SpriteVisual,
    },
    DirectCompFallback {
        device: IDCompositionDevice,
        _target: IDCompositionTarget,
        _visual: IDCompositionVisual,
        swap_chain: IDXGISwapChain1,
        target_bitmap: Option<ID2D1Bitmap1>,
    },
}

impl CompositionHost {
    fn new(
        hwnd: HWND,
        d3d_device: &ID3D11Device,
        dxgi_device: &IDXGIDevice,
        d2d_device: &ID2D1Device,
        factory6: &IDXGIFactory6,
        surface_w_px: f32,
        surface_h_px: f32,
    ) -> windows::core::Result<Self> {
        if let Ok(winrt) =
            try_init_winrt_glass(hwnd, d2d_device, surface_w_px, surface_h_px)
        {
            return Ok(winrt);
        }

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: surface_w_px.round() as u32,
            Height: surface_h_px.round() as u32,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: 0,
        };
        let factory2: IDXGIFactory2 = factory6.cast()?;
        let swap_chain = unsafe {
            factory2.CreateSwapChainForComposition(d3d_device, &swap_chain_desc, None)
        }?;

        let device: IDCompositionDevice = unsafe { DCompositionCreateDevice(dxgi_device) }?;
        let target = unsafe { device.CreateTargetForHwnd(hwnd, true) }?;
        let visual = unsafe { device.CreateVisual() }?;
        unsafe {
            visual.SetContent(&swap_chain)?;
            target.SetRoot(&visual)?;
            device.Commit()?;
        }
        Ok(Self::DirectCompFallback {
            device,
            _target: target,
            _visual: visual,
            swap_chain,
            target_bitmap: None,
        })
    }

    fn has_live_backdrop(&self) -> bool {
        matches!(
            self,
            Self::WinRtGlass {
                backdrop: Some(_),
                ..
            }
        )
    }

    fn update_backdrop(&mut self, pill: PillRectDip, opacity: f32, dpi_scale: f32) {
        let Self::WinRtGlass {
            backdrop: Some(layer),
            ..
        } = self
        else {
            return;
        };

        let visible = opacity > 0.02 && pill.height >= 2.0 && pill.width >= 3.0;
        if !visible {
            if layer.last_state.visible {
                let _ = layer.visual.SetIsVisible(false);
                let _ = layer.visual.SetOpacity(0.0);
                layer.last_state = BackdropState::HIDDEN;
            }
            return;
        }

        let inset = 0.5_f32;
        let left_px = (pill.left + inset) * dpi_scale;
        let top_px = (pill.top + inset) * dpi_scale;
        let width_px = ((pill.width - inset * 2.0) * dpi_scale).max(1.0);
        let height_px = ((pill.height - inset * 2.0) * dpi_scale).max(1.0);
        let max_radius_px = width_px.min(height_px) * 0.5;
        let radius_px = (pill.corner_radius * dpi_scale).clamp(0.5, max_radius_px);
        let clamped_opacity = opacity.clamp(0.0, 1.0);

        let next = BackdropState {
            visible: true,
            left_px,
            top_px,
            width_px,
            height_px,
            radius_px,
            opacity: clamped_opacity,
        };
        if layer.last_state.approx_eq(next) {
            return;
        }

        if !layer.last_state.visible {
            let _ = layer.visual.SetIsVisible(true);
        }
        if !layer.last_state.visible || (layer.last_state.opacity - clamped_opacity).abs() >= 0.002 {
            let _ = layer.visual.SetOpacity(clamped_opacity);
        }
        if !layer.last_state.visible
            || (layer.last_state.left_px - left_px).abs() >= 0.01
            || (layer.last_state.top_px - top_px).abs() >= 0.01
        {
            let _ = layer.visual.SetOffset(Vector3 {
                X: left_px,
                Y: top_px,
                Z: 0.0,
            });
        }
        if !layer.last_state.visible
            || (layer.last_state.width_px - width_px).abs() >= 0.01
            || (layer.last_state.height_px - height_px).abs() >= 0.01
        {
            let sz = Vector2 {
                X: width_px,
                Y: height_px,
            };
            let _ = layer.visual.SetSize(sz);
            let _ = layer.geometry.SetSize(sz);
        }
        if !layer.last_state.visible || (layer.last_state.radius_px - radius_px).abs() >= 0.01 {
            let _ = layer.geometry.SetCornerRadius(Vector2 {
                X: radius_px,
                Y: radius_px,
            });
        }
        layer.last_state = next;
    }

    fn hide_backdrop(&mut self) {
        if let Self::WinRtGlass {
            backdrop: Some(layer),
            ..
        } = self
            && layer.last_state.visible
        {
            let _ = layer.visual.SetIsVisible(false);
            let _ = layer.visual.SetOpacity(0.0);
            layer.last_state = BackdropState::HIDDEN;
        }
    }

    fn resize_content(
        &mut self,
        d2d_context: &ID2D1DeviceContext,
        surface_w_px: f32,
        surface_h_px: f32,
    ) -> windows::core::Result<()> {
        match self {
            Self::WinRtGlass {
                root,
                backdrop,
                graphics_device,
                _drawing_surface,
                surface_brush,
                surface_interop,
                content_visual,
                ..
            } => {
                // Tạo mới bề mặt theo đúng kích thước pixel vật lý và gán lại brush để tránh kẹt tỷ lệ 1x khi đổi DPI/màn hình.
                let new_surface = graphics_device.CreateDrawingSurface(
                    Size {
                        Width: surface_w_px,
                        Height: surface_h_px,
                    },
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    DirectXAlphaMode::Premultiplied,
                )?;
                let new_interop: ICompositionDrawingSurfaceInterop = new_surface.cast()?;
                surface_brush.SetSurface(&new_surface)?;
                surface_brush.SetStretch(CompositionStretch::Fill)?;
                surface_brush.SetHorizontalAlignmentRatio(0.0)?;
                surface_brush.SetVerticalAlignmentRatio(0.0)?;
                let sz = Vector2 {
                    X: surface_w_px,
                    Y: surface_h_px,
                };
                root.SetSize(sz)?;
                content_visual.SetSize(sz)?;
                *_drawing_surface = new_surface;
                *surface_interop = new_interop;
                if let Some(layer) = backdrop {
                    layer.last_state = BackdropState::HIDDEN;
                }
                Ok(())
            }
            Self::DirectCompFallback {
                device,
                swap_chain,
                target_bitmap,
                ..
            } => {
                unsafe {
                    d2d_context.SetTarget(None);
                }
                *target_bitmap = None;
                unsafe {
                    swap_chain.ResizeBuffers(
                        2,
                        surface_w_px.round() as u32,
                        surface_h_px.round() as u32,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        Default::default(),
                    )?;
                }
                *target_bitmap = Some(attach_swapchain_backbuffer(d2d_context, swap_chain)?);
                unsafe { device.Commit() }
            }
        }
    }
}

fn create_dispatcher_queue_current_thread() -> windows::core::Result<DispatcherQueueController> {
    let mut last_err = None;
    for apartment in [DQTAT_COM_NONE, DQTAT_COM_STA, DQTAT_COM_ASTA] {
        let options = DispatcherQueueOptions {
            dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: apartment,
        };
        match unsafe { CreateDispatcherQueueController(options) } {
            Ok(ctrl) => return Ok(ctrl),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(windows::core::Error::from_thread))
}

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct HostBackdropSaturationEffect {
    saturation: f32,
    source: CompositionEffectSourceParameter,
}

impl IGraphicsEffectSource_Impl for HostBackdropSaturationEffect_Impl {}

impl IGraphicsEffect_Impl for HostBackdropSaturationEffect_Impl {
    fn Name(&self) -> windows::core::Result<HSTRING> {
        Ok(HSTRING::from("HostBackdropSaturation"))
    }

    fn SetName(&self, _name: &HSTRING) -> windows::core::Result<()> {
        Ok(())
    }
}

impl IGraphicsEffectD2D1Interop_Impl for HostBackdropSaturationEffect_Impl {
    fn GetEffectId(&self) -> windows::core::Result<GUID> {
        Ok(CLSID_D2D1Saturation)
    }

    fn GetNamedPropertyMapping(
        &self,
        name: &PCWSTR,
        index: *mut u32,
        mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> windows::core::Result<()> {
        if index.is_null() || mapping.is_null() {
            return Err(windows::core::Error::from(E_INVALIDARG));
        }
        let prop = unsafe { name.to_string() }.unwrap_or_default();
        if prop == "Saturation" {
            unsafe {
                *index = D2D1_SATURATION_PROP_SATURATION.0 as u32;
                *mapping = GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT;
            }
            Ok(())
        } else {
            Err(windows::core::Error::from(E_INVALIDARG))
        }
    }

    fn GetPropertyCount(&self) -> windows::core::Result<u32> {
        Ok(1)
    }

    fn GetProperty(&self, index: u32) -> windows::core::Result<windows::Foundation::IPropertyValue> {
        if index == D2D1_SATURATION_PROP_SATURATION.0 as u32 {
            PropertyValue::CreateSingle(self.saturation)?.cast()
        } else {
            Err(windows::core::Error::from(E_BOUNDS))
        }
    }

    fn GetSource(&self, index: u32) -> windows::core::Result<IGraphicsEffectSource> {
        if index == 0 {
            self.source.cast()
        } else {
            Err(windows::core::Error::from(E_BOUNDS))
        }
    }

    fn GetSourceCount(&self) -> windows::core::Result<u32> {
        Ok(1)
    }
}

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct HostBackdropBlurEffect {
    blur_sigma: f32,
    source: IGraphicsEffectSource,
}

impl IGraphicsEffectSource_Impl for HostBackdropBlurEffect_Impl {}

impl IGraphicsEffect_Impl for HostBackdropBlurEffect_Impl {
    fn Name(&self) -> windows::core::Result<HSTRING> {
        Ok(HSTRING::from("HostBackdropBlur"))
    }

    fn SetName(&self, _name: &HSTRING) -> windows::core::Result<()> {
        Ok(())
    }
}

impl IGraphicsEffectD2D1Interop_Impl for HostBackdropBlurEffect_Impl {
    fn GetEffectId(&self) -> windows::core::Result<GUID> {
        Ok(CLSID_D2D1GaussianBlur)
    }

    fn GetNamedPropertyMapping(
        &self,
        name: &PCWSTR,
        index: *mut u32,
        mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> windows::core::Result<()> {
        if index.is_null() || mapping.is_null() {
            return Err(windows::core::Error::from(E_INVALIDARG));
        }
        let prop = unsafe { name.to_string() }.unwrap_or_default();
        if prop == "BlurAmount" {
            unsafe {
                *index = D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32;
                *mapping = GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT;
            }
            Ok(())
        } else {
            Err(windows::core::Error::from(E_INVALIDARG))
        }
    }

    fn GetPropertyCount(&self) -> windows::core::Result<u32> {
        Ok(3)
    }

    fn GetProperty(&self, index: u32) -> windows::core::Result<windows::Foundation::IPropertyValue> {
        if index == D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32 {
            PropertyValue::CreateSingle(self.blur_sigma)?.cast()
        } else if index == D2D1_GAUSSIANBLUR_PROP_OPTIMIZATION.0 as u32 {
            PropertyValue::CreateUInt32(D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED.0 as u32)?.cast()
        } else if index == D2D1_GAUSSIANBLUR_PROP_BORDER_MODE.0 as u32 {
            PropertyValue::CreateUInt32(D2D1_BORDER_MODE_HARD.0 as u32)?.cast()
        } else {
            Err(windows::core::Error::from(E_BOUNDS))
        }
    }

    fn GetSource(&self, index: u32) -> windows::core::Result<IGraphicsEffectSource> {
        if index == 0 {
            Ok(self.source.clone())
        } else {
            Err(windows::core::Error::from(E_BOUNDS))
        }
    }

    fn GetSourceCount(&self) -> windows::core::Result<u32> {
        Ok(1)
    }
}

fn create_blurred_host_backdrop_brush(
    compositor: &Compositor,
) -> windows::core::Result<(CompositionBrush, bool)> {
    let backdrop_brush = compositor.CreateHostBackdropBrush()?;
    let param_name = HSTRING::from("backdrop");
    let source_param = CompositionEffectSourceParameter::Create(&param_name)?;

    // Chuỗi Saturation -> GaussianBlur tạo độ trong và sắc màu rực nhẹ chuẩn Liquid Glass.
    let saturated: IGraphicsEffectSource = HostBackdropSaturationEffect {
        saturation: 1.32,
        source: source_param.clone(),
    }
    .into();
    let vibrant_blur: IGraphicsEffect = HostBackdropBlurEffect {
        blur_sigma: 7.5,
        source: saturated,
    }
    .into();

    if let Ok(factory) = compositor.CreateEffectFactory(&vibrant_blur)
        && let Ok(effect_brush) = factory.CreateBrush()
        && effect_brush
            .SetSourceParameter(&param_name, &backdrop_brush)
            .is_ok()
    {
        return Ok((effect_brush.cast()?, true));
    }

    let direct_source: IGraphicsEffectSource = source_param.cast()?;
    let fallback_blur: IGraphicsEffect = HostBackdropBlurEffect {
        blur_sigma: 7.5,
        source: direct_source,
    }
    .into();
    if let Ok(factory) = compositor.CreateEffectFactory(&fallback_blur)
        && let Ok(effect_brush) = factory.CreateBrush()
        && effect_brush
            .SetSourceParameter(&param_name, &backdrop_brush)
            .is_ok()
    {
        return Ok((effect_brush.cast()?, true));
    }

    Ok((backdrop_brush.cast()?, false))
}

fn try_init_winrt_glass(
    hwnd: HWND,
    d2d_device: &ID2D1Device,
    surface_w_px: f32,
    surface_h_px: f32,
) -> windows::core::Result<CompositionHost> {
    let use_host_backdrop = TRUE;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_HOSTBACKDROPBRUSH,
            (&raw const use_host_backdrop).cast(),
            size_of::<BOOL>() as u32,
        );
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }

    let dispatcher_queue = create_dispatcher_queue_current_thread()?;
    let compositor = Compositor::new()?;
    let interop: ICompositorInterop = compositor.cast()?;
    let graphics_device = unsafe { interop.CreateGraphicsDevice(d2d_device) }?;
    let drawing_surface = graphics_device.CreateDrawingSurface(
        Size {
            Width: surface_w_px,
            Height: surface_h_px,
        },
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        DirectXAlphaMode::Premultiplied,
    )?;
    let surface_interop: ICompositionDrawingSurfaceInterop = drawing_surface.cast()?;
    let surface_brush = compositor.CreateSurfaceBrushWithSurface(&drawing_surface)?;
    surface_brush.SetStretch(CompositionStretch::Fill)?;
    surface_brush.SetHorizontalAlignmentRatio(0.0)?;
    surface_brush.SetVerticalAlignmentRatio(0.0)?;

    let root = compositor.CreateContainerVisual()?;
    root.SetSize(Vector2 {
        X: surface_w_px,
        Y: surface_h_px,
    })?;

    let backdrop = match create_blurred_host_backdrop_brush(&compositor) {
        Ok((backdrop_brush, blurred)) => {
            let visual = compositor.CreateSpriteVisual()?;
            visual.SetBrush(&backdrop_brush)?;
            let geometry = compositor.CreateRoundedRectangleGeometry()?;
            geometry.SetOffset(Vector2 { X: 0.0, Y: 0.0 })?;
            let clip = compositor.CreateGeometricClipWithGeometry(&geometry)?;
            visual.SetClip(&clip)?;
            visual.SetIsVisible(false)?;
            visual.SetOpacity(0.0)?;
            let backdrop_base: Visual = visual.cast()?;
            root.Children()?.InsertAtTop(&backdrop_base)?;
            Some(WinRtBackdropLayer {
                visual,
                geometry,
                last_state: BackdropState::HIDDEN,
                blur_effect_active: blurred,
            })
        }
        Err(_) => None,
    };

    let content_visual = compositor.CreateSpriteVisual()?;
    content_visual.SetBrush(&surface_brush)?;
    content_visual.SetSize(Vector2 {
        X: surface_w_px,
        Y: surface_h_px,
    })?;
    let content_base: Visual = content_visual.cast()?;
    root.Children()?.InsertAtTop(&content_base)?;

    let desktop_interop: ICompositorDesktopInterop = compositor.cast()?;
    let target = unsafe { desktop_interop.CreateDesktopWindowTarget(hwnd, true) }?;
    target.SetRoot(&root)?;

    Ok(CompositionHost::WinRtGlass {
        _dispatcher_queue: dispatcher_queue,
        _compositor: compositor,
        _target: target,
        root,
        backdrop,
        graphics_device,
        _drawing_surface: drawing_surface,
        surface_brush,
        surface_interop,
        content_visual,
    })
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

    fn create_clamped_bitmap_brush(
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

    fn apply_media_theme_colors(&mut self, accent: [f32; 3], glow_alpha: f32) {
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

    fn upload_artwork_bitmap(&self, art: &DecodedArtwork) -> windows::core::Result<ID2D1Bitmap1> {
        self.upload_bgra_bitmap(art.width, art.height, &art.bgra)
    }

    fn upload_bgra_bitmap(
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
        self.comp_host
            .update_backdrop(input.pill, input.pill_opacity, scale);

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
                self.draw_scene(canvas, input, has_backdrop);
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
                self.draw_scene(canvas, input, false);
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

    unsafe fn draw_slim(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let (sw, sh, _, _) = target_dimensions_for_mode(IslandMode::Slim);
        let p = virtual_pill_anchored(input.pill, sw, sh, false);
        let cy = p.top + p.height * 0.5;

        if !input.media.has_session {
            // Chế độ chờ tối giản chuẩn Notch: chấm trạng thái dịu nhẹ + vạch định vị tâm siêu mảnh.
            let dot = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: p.left + 11.0,
                    top: cy - 2.2,
                    right: p.left + 15.4,
                    bottom: cy + 2.2,
                },
                radiusX: 2.2,
                radiusY: 2.2,
            };
            let cx = p.left + p.width * 0.5;
            let notch_bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx - 14.0,
                    top: cy - 1.2,
                    right: cx + 14.0,
                    bottom: cy + 1.2,
                },
                radiusX: 1.2,
                radiusY: 1.2,
            };
            unsafe {
                canvas
                    .ctx
                    .FillRoundedRectangle(&dot, &self.brushes.sys_accent_soft);
                canvas
                    .ctx
                    .FillRoundedRectangle(&notch_bar, &self.brushes.card_border);
            }
            return;
        }

        let art_size = 10.0;
        let art_x = p.left + 8.0;
        let art_y = p.top + (p.height - art_size) * 0.5;
        unsafe {
            self.draw_artwork_or_placeholder(
                canvas,
                art_x,
                art_y,
                art_size,
                art_size * 0.5,
                input.media.is_playing,
            );
        }

        let vis_active = input.config.visualizer_enabled && input.media.is_playing;
        let track_left = art_x + art_size + 7.0;
        let right_reserve = if vis_active { 25.0 } else { 22.0 };
        let track_right = (p.left + p.width - right_reserve).max(track_left + 12.0);
        let track_h = 2.4;

        let bg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: track_left,
                top: cy - track_h * 0.5,
                right: track_right,
                bottom: cy + track_h * 0.5,
            },
            radiusX: track_h * 0.5,
            radiusY: track_h * 0.5,
        };
        unsafe {
            canvas
                .ctx
                .FillRoundedRectangle(&bg_bar, &self.brushes.switch_off);
        }

        let track_w = (track_right - track_left).max(1.0);
        let tl = input.media.timeline;
        let (fill_w, fill_brush) = if tl.available && tl.duration_ticks() > 0 {
            let ratio =
                tl.progress_ratio(current_windows_filetime_ticks(), input.media.is_playing);
            ((track_w * ratio).max(track_h), &self.brushes.accent)
        } else {
            (track_w, &self.brushes.accent_soft)
        };
        let fg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: track_left,
                top: cy - track_h * 0.5,
                right: (track_left + fill_w).min(track_right),
                bottom: cy + track_h * 0.5,
            },
            radiusX: track_h * 0.5,
            radiusY: track_h * 0.5,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&fg_bar, fill_brush);
        }

        if vis_active {
            unsafe {
                self.draw_visualizer_bars_count(
                    canvas,
                    p.left + p.width - 20.0,
                    cy,
                    input.visualizer_phase,
                    2.5,
                    6.5,
                    4.0,
                    2.2,
                    3,
                );
            }
        } else if !input.media.is_playing {
            let pause_cx = p.left + p.width - 14.0;
            for dx in [-1.8_f32, 1.8_f32] {
                let bar = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: pause_cx + dx - 0.85,
                        top: cy - 3.0,
                        right: pause_cx + dx + 0.85,
                        bottom: cy + 3.0,
                    },
                    radiusX: 0.85,
                    radiusY: 0.85,
                };
                unsafe {
                    canvas
                        .ctx
                        .FillRoundedRectangle(&bar, &self.brushes.text_secondary);
                }
            }
        }
    }

    unsafe fn draw_compact(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        if let Some(hud) = input.active_hud {
            unsafe {
                self.draw_compact_hud(canvas, input, hud);
            }
            return;
        }
        let (cw, ch, _, _) = target_dimensions_for_mode(IslandMode::Compact);
        let p = virtual_pill_anchored(input.pill, cw, ch, true);
        let art_size = 26.0;
        let art_x = p.left + 7.0;
        let art_y = p.top + (p.height - art_size) * 0.5;
        unsafe {
            self.draw_artwork_with_rim(
                canvas,
                art_x,
                art_y,
                art_size,
                art_size * 0.5,
                input.media.is_playing,
                true,
            );
        }

        let content_left = art_x + art_size + 9.0;
        let content_right = (p.left + p.width - 38.0).max(content_left + 40.0);
        let cy = p.top + p.height * 0.5;
        let right_cap_cx = p.left + p.width - 20.0;

        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let accent = self.brushes.accent.clone();

        if input.media.has_session {
            let sub_label = if !input.media.artist.trim().is_empty() {
                input.media.artist.as_str()
            } else if !input.media.source_app.trim().is_empty() {
                input.media.source_app.as_str()
            } else if input.media.is_playing {
                "Đang phát"
            } else {
                "Đã tạm dừng"
            };
            unsafe {
                self.draw_text(
                    canvas,
                    &input.media.title,
                    TextFormatSlot::Compact,
                    &text_primary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 4.5,
                        right: content_right,
                        bottom: p.top + 20.5,
                    },
                );
                self.draw_text(
                    canvas,
                    sub_label,
                    TextFormatSlot::CompactSub,
                    &text_secondary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 19.5,
                        right: content_right,
                        bottom: p.top + 32.0,
                    },
                );
            }

            let tl = input.media.timeline;
            if tl.available && tl.duration_ticks() > 0 {
                let ratio = tl.progress_ratio(
                    current_windows_filetime_ticks(),
                    input.media.is_playing,
                );
                let bar_top = p.top + p.height - 5.4;
                let bar_bot = p.top + p.height - 3.4;
                let bg = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: content_left,
                        top: bar_top,
                        right: content_right,
                        bottom: bar_bot,
                    },
                    radiusX: 1.0,
                    radiusY: 1.0,
                };
                unsafe {
                    canvas
                        .ctx
                        .FillRoundedRectangle(&bg, &self.brushes.switch_off);
                }
                let track_w = (content_right - content_left).max(1.0);
                let fill_w = (track_w * ratio).clamp(0.0, track_w);
                if fill_w > 0.8 {
                    let fg = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: content_left,
                            top: bar_top,
                            right: (content_left + fill_w.max(2.0)).min(content_right),
                            bottom: bar_bot,
                        },
                        radiusX: 1.0,
                        radiusY: 1.0,
                    };
                    let fill_brush = if input.media.is_playing {
                        &self.brushes.accent
                    } else {
                        &self.brushes.text_secondary
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&fg, fill_brush);
                    }
                }
            }

            if input.config.visualizer_enabled && input.media.is_playing {
                unsafe {
                    self.draw_visualizer_bars(
                        canvas,
                        right_cap_cx - 9.1,
                        cy,
                        input.visualizer_phase,
                    );
                }
            } else if !input.media.is_playing {
                for dx in [-2.8_f32, 2.8_f32] {
                    let bar = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: right_cap_cx + dx - 1.2,
                            top: cy - 5.2,
                            right: right_cap_cx + dx + 1.2,
                            bottom: cy + 5.2,
                        },
                        radiusX: 1.2,
                        radiusY: 1.2,
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&bar, &text_secondary);
                    }
                }
            } else {
                unsafe {
                    self.draw_centered_triangle(
                        canvas,
                        &self.icons.play_triangle,
                        right_cap_cx + 0.5,
                        cy,
                        2.1,
                        &accent,
                    );
                }
            }
        } else {
            unsafe {
                self.draw_text(
                    canvas,
                    "Win Dynamic Island",
                    TextFormatSlot::Compact,
                    &text_primary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 5.0,
                        right: content_right,
                        bottom: p.top + 21.0,
                    },
                );
                self.draw_text(
                    canvas,
                    "Bấm để mở rộng hoặc cuộn chỉnh loa",
                    TextFormatSlot::CompactSub,
                    &text_secondary,
                    UiRectDip {
                        left: content_left,
                        top: p.top + 20.0,
                        right: content_right,
                        bottom: p.top + 34.0,
                    },
                );
            }
            let outer = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: right_cap_cx - 5.5,
                    top: cy - 5.5,
                    right: right_cap_cx + 5.5,
                    bottom: cy + 5.5,
                },
                radiusX: 5.5,
                radiusY: 5.5,
            };
            let inner = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: right_cap_cx - 2.6,
                    top: cy - 2.6,
                    right: right_cap_cx + 2.6,
                    bottom: cy + 2.6,
                },
                radiusX: 2.6,
                radiusY: 2.6,
            };
            unsafe {
                canvas
                    .ctx
                    .FillRoundedRectangle(&outer, &self.brushes.accent_soft);
                canvas.ctx.FillRoundedRectangle(&inner, &accent);
            }
        }
    }

    unsafe fn draw_compact_hud(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
        hud: HudContent,
    ) {
        let (cw, ch, _, _) = target_dimensions_for_mode(IslandMode::Compact);
        let p = virtual_pill_anchored(input.pill, cw, ch, true);
        match hud {
            HudContent::Volume {
                level,
                muted,
                show_device_name,
            } => {
                let display_level = input.volume_scrub_ratio.unwrap_or(level).clamp(0.0, 1.0);
                let is_dragging = input.volume_scrub_ratio.is_some();
                fill_volume_hud_hitboxes(p, &mut self.hitbox_scratch);
                let mute_rect = self
                    .hitbox_scratch
                    .iter()
                    .find(|hb| hb.action == UiAction::ToggleVolumeMute)
                    .map(|hb| hb.rect)
                    .unwrap_or(UiRectDip {
                        left: p.left + 6.0,
                        top: p.top + 3.0,
                        right: p.left + 36.0,
                        bottom: p.top + p.height - 3.0,
                    });
                let slider_rect = self
                    .hitbox_scratch
                    .iter()
                    .find(|hb| hb.action == UiAction::SeekVolume)
                    .map(|hb| hb.rect)
                    .unwrap_or(UiRectDip {
                        left: p.left + 44.0,
                        top: p.top + 2.0,
                        right: p.left + p.width - 50.0,
                        bottom: p.top + p.height - 2.0,
                    });

                let mute_hovered = input.hovered_action == Some(UiAction::ToggleVolumeMute);
                if mute_hovered {
                    let pill_btn = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: mute_rect.left,
                            top: mute_rect.top,
                            right: mute_rect.right,
                            bottom: mute_rect.bottom,
                        },
                        radiusX: 12.0,
                        radiusY: 12.0,
                    };
                    unsafe {
                        canvas
                            .ctx
                            .FillRoundedRectangle(&pill_btn, &self.brushes.surface_hover);
                    }
                }

                let cx = mute_rect.left + mute_rect.width() * 0.5;
                let cy = mute_rect.top + mute_rect.height() * 0.5;
                let speaker_brush = if muted {
                    &self.brushes.danger_border
                } else {
                    &self.brushes.text_primary
                };
                unsafe {
                    self.draw_speaker_icon(canvas, cx, cy, display_level, muted, speaker_brush);
                }

                let dev_label = if show_device_name && !is_dragging {
                    input
                        .current_audio_device_name
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                } else {
                    None
                };

                let slider_hovered = input.hovered_action == Some(UiAction::SeekVolume);
                if let Some(dev_name) = dev_label {
                    let text_primary = self.brushes.text_primary.clone();
                    unsafe {
                        self.draw_text(
                            canvas,
                            dev_name,
                            TextFormatSlot::SmallLeft,
                            &text_primary,
                            UiRectDip {
                                left: slider_rect.left,
                                top: p.top + 4.5,
                                right: slider_rect.right,
                                bottom: p.top + p.height - 14.5,
                            },
                        );
                    }
                    let bar_h = if slider_hovered { 3.8 } else { 2.8 };
                    let bar_cy = p.top + p.height - 6.4;
                    let bar_top = bar_cy - bar_h * 0.5;
                    let bar_bot = bar_cy + bar_h * 0.5;
                    let bar_bg = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: slider_rect.left,
                            top: bar_top,
                            right: slider_rect.right,
                            bottom: bar_bot,
                        },
                        radiusX: bar_h * 0.5,
                        radiusY: bar_h * 0.5,
                    };
                    unsafe {
                        canvas
                            .ctx
                            .FillRoundedRectangle(&bar_bg, &self.brushes.switch_off);
                    }
                    if display_level > 0.004 {
                        let fill_w = (slider_rect.width() * display_level).max(bar_h);
                        let bar_fg = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: slider_rect.left,
                                top: bar_top,
                                right: (slider_rect.left + fill_w).min(slider_rect.right),
                                bottom: bar_bot,
                            },
                            radiusX: bar_h * 0.5,
                            radiusY: bar_h * 0.5,
                        };
                        let fill_brush = if muted {
                            &self.brushes.danger_border
                        } else {
                            &self.brushes.accent
                        };
                        unsafe {
                            canvas.ctx.FillRoundedRectangle(&bar_fg, fill_brush);
                        }
                    }
                    if slider_hovered {
                        let knob_r = 3.8;
                        let knob_x = (slider_rect.left + slider_rect.width() * display_level)
                            .clamp(slider_rect.left + knob_r, slider_rect.right - knob_r);
                        let halo_r = knob_r + 2.0;
                        let halo = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - halo_r,
                                top: bar_cy - halo_r,
                                right: knob_x + halo_r,
                                bottom: bar_cy + halo_r,
                            },
                            radiusX: halo_r,
                            radiusY: halo_r,
                        };
                        let knob = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - knob_r,
                                top: bar_cy - knob_r,
                                right: knob_x + knob_r,
                                bottom: bar_cy + knob_r,
                            },
                            radiusX: knob_r,
                            radiusY: knob_r,
                        };
                        unsafe {
                            canvas
                                .ctx
                                .FillRoundedRectangle(&halo, &self.brushes.accent_soft);
                            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
                        }
                    }
                } else {
                    let track_h = if is_dragging {
                        6.5
                    } else if slider_hovered {
                        5.5
                    } else {
                        4.4
                    };
                    let bg_bar = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: slider_rect.left,
                            top: cy - track_h * 0.5,
                            right: slider_rect.right,
                            bottom: cy + track_h * 0.5,
                        },
                        radiusX: track_h * 0.5,
                        radiusY: track_h * 0.5,
                    };
                    unsafe {
                        canvas
                            .ctx
                            .FillRoundedRectangle(&bg_bar, &self.brushes.switch_off);
                    }

                    if display_level > 0.004 {
                        let fill_w = (slider_rect.width() * display_level).max(track_h);
                        let fg_bar = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: slider_rect.left,
                                top: cy - track_h * 0.5,
                                right: (slider_rect.left + fill_w).min(slider_rect.right),
                                bottom: cy + track_h * 0.5,
                            },
                            radiusX: track_h * 0.5,
                            radiusY: track_h * 0.5,
                        };
                        let fill_brush = if muted {
                            &self.brushes.danger_border
                        } else {
                            &self.brushes.accent
                        };
                        unsafe {
                            canvas.ctx.FillRoundedRectangle(&fg_bar, fill_brush);
                        }
                    }

                    if slider_hovered || is_dragging {
                        let knob_r = if is_dragging { 6.2 } else { 5.2 };
                        let knob_x = (slider_rect.left + slider_rect.width() * display_level)
                            .clamp(slider_rect.left + knob_r, slider_rect.right - knob_r);
                        let halo_r = knob_r + 3.0;
                        let halo = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - halo_r,
                                top: cy - halo_r,
                                right: knob_x + halo_r,
                                bottom: cy + halo_r,
                            },
                            radiusX: halo_r,
                            radiusY: halo_r,
                        };
                        let knob = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: knob_x - knob_r,
                                top: cy - knob_r,
                                right: knob_x + knob_r,
                                bottom: cy + knob_r,
                            },
                            radiusX: knob_r,
                            radiusY: knob_r,
                        };
                        unsafe {
                            canvas
                                .ctx
                                .FillRoundedRectangle(&halo, &self.brushes.accent_soft);
                            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
                        }
                    }
                }

                let label = if muted {
                    String::from("Tắt")
                } else {
                    format!("{}%", (display_level * 100.0).round() as u32)
                };
                let pct_brush = if muted {
                    self.brushes.danger_border.clone()
                } else {
                    self.brushes.text_primary.clone()
                };
                unsafe {
                    self.draw_text(
                        canvas,
                        &label,
                        TextFormatSlot::SmallRight,
                        &pct_brush,
                        UiRectDip {
                            left: slider_rect.right + 4.0,
                            top: p.top + 4.0,
                            right: p.left + p.width - 12.0,
                            bottom: p.top + p.height - 4.0,
                        },
                    );
                }
            }
            HudContent::Power {
                on_ac,
                percent,
                low_battery,
            } => {
                let cy = p.top + p.height * 0.5;
                let fill_brush = if on_ac {
                    &self.brushes.power_ok
                } else if low_battery {
                    &self.brushes.power_warn
                } else {
                    &self.brushes.accent
                };
                let pct_ratio = percent.map_or(0.5, |v| (v as f32 / 100.0).clamp(0.08, 1.0));
                unsafe {
                    self.draw_battery_icon(canvas, p.left + 21.0, cy, pct_ratio, on_ac, fill_brush);
                }

                let status_text = if on_ac {
                    "Đang cắm sạc"
                } else if low_battery {
                    "Pin yếu"
                } else {
                    "Đang dùng pin"
                };
                let pct_label = percent.map_or_else(|| String::from("--%"), |v| format!("{v}%"));
                let text_primary = self.brushes.text_primary.clone();
                let accent_brush = fill_brush.clone();
                unsafe {
                    self.draw_text(
                        canvas,
                        status_text,
                        TextFormatSlot::Compact,
                        &text_primary,
                        UiRectDip {
                            left: p.left + 38.0,
                            top: p.top + 4.0,
                            right: p.left + p.width - 54.0,
                            bottom: p.top + p.height - 4.0,
                        },
                    );
                    self.draw_text(
                        canvas,
                        &pct_label,
                        TextFormatSlot::SmallRight,
                        &accent_brush,
                        UiRectDip {
                            left: p.left + p.width - 54.0,
                            top: p.top + 4.0,
                            right: p.left + p.width - 12.0,
                            bottom: p.top + p.height - 4.0,
                        },
                    );
                }
            }
        }
    }

    unsafe fn draw_speaker_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        level: f32,
        muted: bool,
        brush: &ID2D1SolidColorBrush,
    ) {
        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 7.5,
                top: cy - 2.6,
                right: cx - 3.6,
                bottom: cy + 2.6,
            },
            radiusX: 1.0,
            radiusY: 1.0,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&box_rect, brush);
        }
        if let Ok(base) = self.icons.speaker_cone.cast::<ID2D1Geometry>() {
            canvas.set_local_translation(cx - 2.4, cy);
            unsafe {
                canvas.ctx.FillGeometry(&base, brush, None);
                canvas
                    .ctx
                    .DrawGeometry(&base, brush, 1.4, &self.icons.round_stroke);
            }
            canvas.reset_transform();
        }
        if muted {
            unsafe {
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + 2.0,
                        Y: cy - 3.2,
                    },
                    Vector2 {
                        X: cx + 7.2,
                        Y: cy + 3.2,
                    },
                    brush,
                    1.7,
                    Some(&self.icons.round_stroke),
                );
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + 7.2,
                        Y: cy - 3.2,
                    },
                    Vector2 {
                        X: cx + 2.0,
                        Y: cy + 3.2,
                    },
                    brush,
                    1.7,
                    Some(&self.icons.round_stroke),
                );
            }
        } else {
            if level > 0.02 {
                let w1 = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + 2.4,
                        top: cy - 2.6,
                        right: cx + 4.1,
                        bottom: cy + 2.6,
                    },
                    radiusX: 0.85,
                    radiusY: 0.85,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&w1, brush);
                }
            }
            if level >= 0.45 {
                let w2 = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + 5.3,
                        top: cy - 4.6,
                        right: cx + 7.0,
                        bottom: cy + 4.6,
                    },
                    radiusX: 0.85,
                    radiusY: 0.85,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&w2, brush);
                }
            }
        }
    }

    unsafe fn draw_battery_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        ratio: f32,
        on_ac: bool,
        fill_brush: &ID2D1SolidColorBrush,
    ) {
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 10.0,
                top: cy - 5.5,
                right: cx + 7.0,
                bottom: cy + 5.5,
            },
            radiusX: 2.5,
            radiusY: 2.5,
        };
        let nub = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx + 7.5,
                top: cy - 2.4,
                right: cx + 9.6,
                bottom: cy + 2.4,
            },
            radiusX: 1.0,
            radiusY: 1.0,
        };
        let inner_max_w = 13.4_f32;
        let inner_w = (inner_max_w * ratio.clamp(0.08, 1.0)).max(2.0);
        let fill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 8.2,
                top: cy - 3.7,
                right: cx - 8.2 + inner_w,
                bottom: cy + 3.7,
            },
            radiusX: 1.4,
            radiusY: 1.4,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&fill, fill_brush);
            canvas.ctx.DrawRoundedRectangle(
                &body,
                &self.brushes.text_secondary,
                1.25,
                None,
            );
            canvas
                .ctx
                .FillRoundedRectangle(&nub, &self.brushes.text_secondary);
            if on_ac {
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx - 1.2,
                        Y: cy - 3.2,
                    },
                    Vector2 {
                        X: cx - 3.0,
                        Y: cy + 0.2,
                    },
                    &self.brushes.text_primary,
                    1.3,
                    Some(&self.icons.round_stroke),
                );
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx - 3.0,
                        Y: cy + 0.2,
                    },
                    Vector2 {
                        X: cx + 0.6,
                        Y: cy + 0.2,
                    },
                    &self.brushes.text_primary,
                    1.3,
                    Some(&self.icons.round_stroke),
                );
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + 0.6,
                        Y: cy + 0.2,
                    },
                    Vector2 {
                        X: cx - 1.2,
                        Y: cy + 3.4,
                    },
                    &self.brushes.text_primary,
                    1.3,
                    Some(&self.icons.round_stroke),
                );
            }
        }
    }

    unsafe fn draw_expanded(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let p = virtual_pill_anchored(input.pill, 424.0, 160.0, false);
        let art_size = 64.0;
        let art_x = p.left + 18.0;
        let art_y = p.top + 16.0;
        unsafe {
            self.draw_artwork_or_placeholder(
                canvas,
                art_x,
                art_y,
                art_size,
                14.0,
                input.media.is_playing,
            );
        }

        let text_left = art_x + art_size + 14.0;
        let text_right = p.left + p.width - 18.0;

        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let accent = self.brushes.accent.clone();

        unsafe {
            self.draw_text(
                canvas,
                &input.media.title,
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: text_left,
                    top: p.top + 16.0,
                    right: text_right,
                    bottom: p.top + 38.0,
                },
            );
            self.draw_text(
                canvas,
                &input.media.artist,
                TextFormatSlot::Subtitle,
                &text_secondary,
                UiRectDip {
                    left: text_left,
                    top: p.top + 40.0,
                    right: text_right,
                    bottom: p.top + 58.0,
                },
            );
        }

        let status_label = if input.media.has_session {
            if input.media.source_app.is_empty() {
                if input.media.is_playing {
                    "Đang phát"
                } else {
                    "Đã tạm dừng"
                }
            } else {
                input.media.source_app.as_str()
            }
        } else {
            "Bấm Cài đặt hoặc chuột phải mở menu"
        };
        unsafe {
            self.draw_text(
                canvas,
                status_label,
                TextFormatSlot::SmallLeft,
                &accent,
                UiRectDip {
                    left: text_left,
                    top: p.top + 60.0,
                    right: text_right,
                    bottom: p.top + 78.0,
                },
            );
        }

        let now_ft = current_windows_filetime_ticks();
        let tl = input.media.timeline;
        let is_scrubbing = input.timeline_scrub_ratio.is_some();
        let (pos_label, dur_label, ratio) = if input.media.has_session && tl.available {
            let dur = tl.duration_ticks();
            if let Some(scrub) = input.timeline_scrub_ratio {
                let clamped = scrub.clamp(0.0, 1.0);
                let pos = tl.seek_ticks_for_ratio(clamped).unwrap_or(0);
                (
                    format_duration_ticks(pos),
                    format_duration_ticks(dur),
                    clamped,
                )
            } else {
                let pos = tl.current_position_ticks(now_ft, input.media.is_playing);
                (
                    format_duration_ticks(pos),
                    format_duration_ticks(dur),
                    tl.progress_ratio(now_ft, input.media.is_playing),
                )
            }
        } else {
            (String::from("--:--"), String::from("--:--"), 0.0)
        };

        unsafe {
            self.draw_text(
                canvas,
                &pos_label,
                TextFormatSlot::SmallLeft,
                &text_secondary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 85.5,
                    right: p.left + 58.0,
                    bottom: p.top + 105.5,
                },
            );
            self.draw_text(
                canvas,
                &dur_label,
                TextFormatSlot::SmallRight,
                &text_secondary,
                UiRectDip {
                    left: p.left + p.width - 58.0,
                    top: p.top + 85.5,
                    right: p.left + p.width - 18.0,
                    bottom: p.top + 105.5,
                },
            );
        }

        fill_interactive_hitboxes(p, IslandMode::Expanded, &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            if hb.action == UiAction::SeekTimeline {
                unsafe {
                    self.draw_timeline_bar(
                        canvas,
                        hb.rect,
                        ratio,
                        input.media.has_session && tl.available,
                        hovered || is_scrubbing,
                        is_scrubbing,
                    );
                }
                continue;
            }
            let fill = if hovered {
                &self.brushes.surface_hover
            } else if hb.action == UiAction::MediaPlayPause {
                &self.brushes.accent_soft
            } else {
                &self.brushes.surface_card
            };
            let radius = hb.rect.height() * 0.5;
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: hb.rect.left,
                    top: hb.rect.top,
                    right: hb.rect.right,
                    bottom: hb.rect.bottom,
                },
                radiusX: radius,
                radiusY: radius,
            };
            unsafe {
                canvas.ctx.FillRoundedRectangle(&card, fill);
            }
            if hb.action == UiAction::OpenSettings {
                let icon_brush = text_secondary.clone();
                let icon_cx = hb.rect.left + 21.0;
                let icon_cy = hb.rect.top + hb.rect.height() * 0.5;
                unsafe {
                    self.draw_gear_icon(canvas, icon_cx, icon_cy, &icon_brush);
                    self.draw_text(
                        canvas,
                        "Cài đặt",
                        TextFormatSlot::Button,
                        &text_primary,
                        UiRectDip {
                            left: hb.rect.left + 32.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 10.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                }
            } else if hb.action == UiAction::OpenAudioDevices {
                let speaker_brush = if input.volume_snapshot.muted {
                    self.brushes.danger_border.clone()
                } else {
                    accent.clone()
                };
                let dev_label = input
                    .current_audio_device_name
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Âm thanh");
                unsafe {
                    self.draw_speaker_icon(
                        canvas,
                        hb.rect.left + 14.0,
                        hb.rect.top + hb.rect.height() * 0.5,
                        if input.volume_snapshot.available {
                            input.volume_snapshot.level
                        } else {
                            0.6
                        },
                        input.volume_snapshot.muted,
                        &speaker_brush,
                    );
                    self.draw_text(
                        canvas,
                        dev_label,
                        TextFormatSlot::SmallLeft,
                        &text_primary,
                        UiRectDip {
                            left: hb.rect.left + 26.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 8.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                }
            } else {
                let icon_brush = if input.media.has_session {
                    &text_primary
                } else {
                    &text_secondary
                };
                unsafe {
                    self.draw_media_action_icon(
                        canvas,
                        hb.action,
                        hb.rect,
                        input.media.is_playing,
                        icon_brush,
                    );
                }
            }
        }
    }

    unsafe fn draw_gear_icon(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let ring = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 3.6,
                top: cy - 3.6,
                right: cx + 3.6,
                bottom: cy + 3.6,
            },
            radiusX: 3.6,
            radiusY: 3.6,
        };
        let core = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - 1.3,
                top: cy - 1.3,
                right: cx + 1.3,
                bottom: cy + 1.3,
            },
            radiusX: 1.3,
            radiusY: 1.3,
        };
        unsafe {
            canvas.ctx.DrawRoundedRectangle(&ring, brush, 1.45, None);
            canvas.ctx.FillRoundedRectangle(&core, brush);
            for idx in 0..8 {
                let angle = idx as f32 * std::f32::consts::FRAC_PI_4;
                let (sin_a, cos_a) = angle.sin_cos();
                canvas.ctx.DrawLine(
                    Vector2 {
                        X: cx + cos_a * 3.7,
                        Y: cy + sin_a * 3.7,
                    },
                    Vector2 {
                        X: cx + cos_a * 5.6,
                        Y: cy + sin_a * 5.6,
                    },
                    brush,
                    1.55,
                    Some(&self.icons.round_stroke),
                );
            }
        }
    }

    unsafe fn draw_media_action_icon(
        &self,
        canvas: DrawCanvas<'_>,
        action: UiAction,
        rect: UiRectDip,
        is_playing: bool,
        brush: &ID2D1SolidColorBrush,
    ) {
        let cx = rect.left + rect.width() * 0.5;
        let cy = rect.top + rect.height() * 0.5;
        match action {
            UiAction::MediaPlayPause if is_playing => {
                for offset in [-3.4f32, 3.4f32] {
                    let bar = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx + offset - 1.7,
                            top: cy - 6.4,
                            right: cx + offset + 1.7,
                            bottom: cy + 6.4,
                        },
                        radiusX: 1.7,
                        radiusY: 1.7,
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&bar, brush);
                    }
                }
            }
            UiAction::MediaPlayPause => unsafe {
                self.draw_centered_triangle(
                    canvas,
                    &self.icons.play_triangle,
                    cx + 0.8,
                    cy,
                    2.6,
                    brush,
                );
            },
            UiAction::MediaPrev => {
                let bar = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx - 6.0,
                        top: cy - 5.6,
                        right: cx - 3.6,
                        bottom: cy + 5.6,
                    },
                    radiusX: 1.2,
                    radiusY: 1.2,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&bar, brush);
                    self.draw_centered_triangle(
                        canvas,
                        &self.icons.prev_triangle,
                        cx + 1.8,
                        cy,
                        2.2,
                        brush,
                    );
                }
            }
            UiAction::MediaNext => {
                let bar = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + 3.6,
                        top: cy - 5.6,
                        right: cx + 6.0,
                        bottom: cy + 5.6,
                    },
                    radiusX: 1.2,
                    radiusY: 1.2,
                };
                unsafe {
                    canvas.ctx.FillRoundedRectangle(&bar, brush);
                    self.draw_centered_triangle(
                        canvas,
                        &self.icons.next_triangle,
                        cx - 1.8,
                        cy,
                        2.2,
                        brush,
                    );
                }
            }
            _ => {}
        }
    }

    unsafe fn draw_centered_triangle(
        &self,
        canvas: DrawCanvas<'_>,
        geom: &ID2D1PathGeometry,
        cx: f32,
        cy: f32,
        round_stroke_w: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let Ok(base) = geom.cast::<ID2D1Geometry>() else {
            return;
        };
        canvas.set_local_translation(cx, cy);
        unsafe {
            canvas.ctx.FillGeometry(&base, brush, None);
            canvas
                .ctx
                .DrawGeometry(&base, brush, round_stroke_w, &self.icons.round_stroke);
        }
        canvas.reset_transform();
    }

    unsafe fn draw_timeline_bar(
        &self,
        canvas: DrawCanvas<'_>,
        rect: UiRectDip,
        ratio: f32,
        active: bool,
        hovered: bool,
        dragging: bool,
    ) {
        let track_h = if dragging && active {
            6.5
        } else if hovered && active {
            5.5
        } else {
            4.0
        };
        let cy = rect.top + rect.height() * 0.5;
        let bg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.left,
                top: cy - track_h * 0.5,
                right: rect.right,
                bottom: cy + track_h * 0.5,
            },
            radiusX: track_h * 0.5,
            radiusY: track_h * 0.5,
        };
        unsafe {
            canvas
                .ctx
                .FillRoundedRectangle(&bg_bar, &self.brushes.switch_off);
        }

        if !active {
            return;
        }

        let fill_w = (rect.width() * ratio.clamp(0.0, 1.0)).max(track_h);
        let fill_right = (rect.left + fill_w).min(rect.right);
        let fg_bar = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.left,
                top: cy - track_h * 0.5,
                right: fill_right,
                bottom: cy + track_h * 0.5,
            },
            radiusX: track_h * 0.5,
            radiusY: track_h * 0.5,
        };
        unsafe {
            canvas
                .ctx
                .FillRoundedRectangle(&fg_bar, &self.brushes.accent);
        }

        let knob_r = if dragging {
            6.5
        } else if hovered {
            5.5
        } else {
            4.0
        };
        let knob_x = (rect.left + rect.width() * ratio.clamp(0.0, 1.0))
            .clamp(rect.left + knob_r, rect.right - knob_r);
        if hovered || dragging {
            let halo_r = knob_r + 3.5;
            let halo = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: knob_x - halo_r,
                    top: cy - halo_r,
                    right: knob_x + halo_r,
                    bottom: cy + halo_r,
                },
                radiusX: halo_r,
                radiusY: halo_r,
            };
            unsafe {
                canvas
                    .ctx
                    .FillRoundedRectangle(&halo, &self.brushes.accent_soft);
            }
        }
        let knob = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: knob_x - knob_r,
                top: cy - knob_r,
                right: knob_x + knob_r,
                bottom: cy + knob_r,
            },
            radiusX: knob_r,
            radiusY: knob_r,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
        }
    }

    unsafe fn draw_settings(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let p = virtual_pill_anchored(input.pill, 424.0, 360.0, false);
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let sys_accent = self.brushes.sys_accent.clone();
        let version_label = format!("v{APP_VERSION}");

        unsafe {
            self.draw_text(
                canvas,
                "Bảng điều khiển nhanh",
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 10.0,
                    right: p.left + 216.0,
                    bottom: p.top + 34.0,
                },
            );
            self.draw_text(
                canvas,
                &version_label,
                TextFormatSlot::CompactSub,
                &text_secondary,
                UiRectDip {
                    left: p.left + 206.0,
                    top: p.top + 12.0,
                    right: p.left + p.width - 96.0,
                    bottom: p.top + 34.0,
                },
            );
        }

        let sec_tops = settings_section_tops(p);
        for (sec_top, title) in sec_tops
            .into_iter()
            .zip(["Hiển thị", "HUD & Âm thanh", "Hệ thống"])
        {
            unsafe {
                self.draw_text(
                    canvas,
                    title,
                    TextFormatSlot::CompactSub,
                    &text_secondary,
                    UiRectDip {
                        left: p.left + 18.0,
                        top: sec_top,
                        right: p.left + p.width - 18.0,
                        bottom: sec_top + 14.0,
                    },
                );
            }
        }

        fill_interactive_hitboxes(p, IslandMode::Settings, &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            let is_danger = hb.action == UiAction::ExitApp;
            let has_pending_update =
                hb.action == UiAction::CheckUpdate && input.available_update_tag.is_some();
            let fill = if is_danger {
                if hovered {
                    &self.brushes.danger_hover
                } else {
                    &self.brushes.danger_bg
                }
            } else if hovered {
                &self.brushes.surface_hover
            } else if has_pending_update {
                &self.brushes.sys_accent_soft
            } else {
                &self.brushes.surface_card
            };

            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: hb.rect.left,
                    top: hb.rect.top,
                    right: hb.rect.right,
                    bottom: hb.rect.bottom,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            unsafe {
                canvas.ctx.FillRoundedRectangle(&card, fill);
                if is_danger {
                    canvas.ctx.DrawRoundedRectangle(
                        &card,
                        &self.brushes.danger_border,
                        1.0,
                        None,
                    );
                } else if has_pending_update {
                    canvas
                        .ctx
                        .DrawRoundedRectangle(&card, &self.brushes.sys_accent, 0.95, None);
                } else {
                    canvas.ctx.DrawRoundedRectangle(
                        &card,
                        &self.brushes.card_border,
                        0.85,
                        None,
                    );
                }
            }

            match hb.action {
                UiAction::BackToMedia => unsafe {
                    self.draw_text(
                        canvas,
                        "Quay lại",
                        TextFormatSlot::ButtonCenter,
                        &text_primary,
                        hb.rect,
                    );
                },
                UiAction::ToggleNotchCollapse => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Gọn mép 4s",
                        input.config.notch_collapse_after_4s,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleHudVolume => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "HUD Âm lượng",
                        input.config.hud_volume_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleHudWheel => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Cuộn chỉnh loa",
                        input.config.hud_wheel_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleHudPower => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "HUD Pin / Sạc",
                        input.config.hud_power_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::OpenAudioDevices => unsafe {
                    self.draw_text(
                        canvas,
                        "Thiết bị phát",
                        TextFormatSlot::Button,
                        &text_primary,
                        UiRectDip {
                            left: hb.rect.left + 10.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 46.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                    self.draw_text(
                        canvas,
                        "Chọn",
                        TextFormatSlot::SmallRight,
                        &sys_accent,
                        UiRectDip {
                            left: hb.rect.right - 48.0,
                            top: hb.rect.top,
                            right: hb.rect.right - 10.0,
                            bottom: hb.rect.bottom,
                        },
                    );
                },
                UiAction::ToggleAutoHideIdle => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Tự ẩn khi rảnh",
                        input.config.auto_hide_when_idle,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleAutoHideFullscreen => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Ẩn toàn màn hình",
                        input.config.auto_hide_fullscreen,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleVisualizer => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Sóng nhạc động",
                        input.config.visualizer_enabled,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::ToggleAutostart => unsafe {
                    self.draw_toggle_tile(
                        canvas,
                        hb.rect,
                        "Mở cùng Windows",
                        input.config.autostart,
                        &text_primary,
                        &sys_accent,
                    );
                },
                UiAction::CheckUpdate => {
                    let (label, right_tag, right_brush) = match input.available_update_tag {
                        Some(tag) => (String::from("Cập nhật mới"), tag, &sys_accent),
                        None => (
                            String::from("Kiểm tra cập nhật"),
                            "Kiểm tra",
                            &text_secondary,
                        ),
                    };
                    unsafe {
                        self.draw_text(
                            canvas,
                            &label,
                            TextFormatSlot::Button,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 10.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 56.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                        self.draw_text(
                            canvas,
                            right_tag,
                            TextFormatSlot::SmallRight,
                            right_brush,
                            UiRectDip {
                                left: hb.rect.right - 64.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 10.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                    }
                }
                UiAction::CycleMonitor => {
                    let label = format!("Màn hình: {}", input.monitor_label);
                    unsafe {
                        self.draw_text(
                            canvas,
                            &label,
                            TextFormatSlot::Button,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 12.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 72.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                        self.draw_text(
                            canvas,
                            "Đổi",
                            TextFormatSlot::SmallRight,
                            &sys_accent,
                            UiRectDip {
                                left: hb.rect.right - 64.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 10.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                    }
                }
                UiAction::HideIsland => unsafe {
                    self.draw_text(
                        canvas,
                        "Ẩn tạm",
                        TextFormatSlot::ButtonCenter,
                        &text_primary,
                        hb.rect,
                    );
                },
                UiAction::ExitApp => unsafe {
                    self.draw_text(
                        canvas,
                        "Thoát ứng dụng",
                        TextFormatSlot::ButtonCenter,
                        &text_primary,
                        hb.rect,
                    );
                },
                _ => {}
            }
        }
    }

    unsafe fn draw_audio_devices(
        &mut self,
        canvas: DrawCanvas<'_>,
        input: &RenderFrameInput<'_>,
    ) {
        let target_h = audio_devices_height_for_count(input.audio_devices.len());
        let p = virtual_pill_anchored(input.pill, 424.0, target_h, false);
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let sys_accent = self.brushes.sys_accent.clone();

        let total = input.audio_devices.len();
        let max_scroll = total.saturating_sub(MAX_AUDIO_DEVICES_VISIBLE);
        let start = input.audio_device_scroll.min(max_scroll);
        let end = (start + MAX_AUDIO_DEVICES_VISIBLE).min(total);
        let visible_count = end.saturating_sub(start);

        let vol_status = if !input.volume_snapshot.available {
            String::from("Chưa có dịch vụ âm thanh")
        } else if input.volume_snapshot.muted {
            format!(
                "Đang tắt tiếng ({}%)",
                (input.volume_snapshot.level * 100.0).round() as u32
            )
        } else {
            format!(
                "Âm lượng thiết bị hiện tại: {}%",
                (input.volume_snapshot.level * 100.0).round() as u32
            )
        };

        unsafe {
            self.draw_text(
                canvas,
                "Thiết bị phát âm thanh",
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 10.0,
                    right: p.left + p.width - 96.0,
                    bottom: p.top + 32.0,
                },
            );
            self.draw_text(
                canvas,
                &vol_status,
                TextFormatSlot::SmallLeft,
                &text_secondary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 32.0,
                    right: p.left + p.width - 96.0,
                    bottom: p.top + 49.0,
                },
            );
        }

        fill_audio_devices_hitboxes(p, visible_count, &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: hb.rect.left,
                    top: hb.rect.top,
                    right: hb.rect.right,
                    bottom: hb.rect.bottom,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };

            match hb.action {
                UiAction::BackToMedia => {
                    let fill = if hovered {
                        &self.brushes.surface_hover
                    } else {
                        &self.brushes.surface_card
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&card, fill);
                        canvas.ctx.DrawRoundedRectangle(
                            &card,
                            &self.brushes.card_border,
                            0.85,
                            None,
                        );
                        self.draw_text(
                            canvas,
                            "Quay lại",
                            TextFormatSlot::ButtonCenter,
                            &text_primary,
                            hb.rect,
                        );
                    }
                }
                UiAction::SelectAudioDevice(rel_idx) => {
                    let Some(dev) = input.audio_devices.get(start + rel_idx) else {
                        continue;
                    };
                    let is_default = dev.is_default;
                    let fill = if hovered {
                        &self.brushes.surface_hover
                    } else if is_default {
                        &self.brushes.sys_accent_soft
                    } else {
                        &self.brushes.surface_card
                    };
                    let border_brush = if is_default {
                        &self.brushes.sys_accent
                    } else {
                        &self.brushes.card_border
                    };
                    let border_w = if is_default { 1.1 } else { 0.85 };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&card, fill);
                        canvas.ctx.DrawRoundedRectangle(&card, border_brush, border_w, None);
                    }

                    let icon_brush = if is_default {
                        if input.volume_snapshot.muted {
                            self.brushes.danger_border.clone()
                        } else {
                            sys_accent.clone()
                        }
                    } else {
                        text_secondary.clone()
                    };
                    let icon_level = if is_default {
                        input.volume_snapshot.level
                    } else {
                        0.65
                    };
                    let icon_muted = is_default && input.volume_snapshot.muted;
                    let name_text = dev.name.clone();
                    let cy = hb.rect.top + hb.rect.height() * 0.5;
                    let text_right = if is_default {
                        hb.rect.right - 82.0
                    } else {
                        hb.rect.right - 14.0
                    };
                    unsafe {
                        self.draw_speaker_icon(
                            canvas,
                            hb.rect.left + 19.0,
                            cy,
                            icon_level,
                            icon_muted,
                            &icon_brush,
                        );
                        self.draw_text(
                            canvas,
                            &name_text,
                            TextFormatSlot::Button,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 35.0,
                                top: hb.rect.top,
                                right: text_right,
                                bottom: hb.rect.bottom,
                            },
                        );
                        if is_default {
                            self.draw_text(
                                canvas,
                                "Đang dùng",
                                TextFormatSlot::SmallRight,
                                &sys_accent,
                                UiRectDip {
                                    left: hb.rect.right - 78.0,
                                    top: hb.rect.top,
                                    right: hb.rect.right - 12.0,
                                    bottom: hb.rect.bottom,
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        if total == 0 {
            let empty_card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: p.left + 16.0,
                    top: p.top + 56.0,
                    right: p.left + p.width - 16.0,
                    bottom: p.top + 96.0,
                },
                radiusX: 10.0,
                radiusY: 10.0,
            };
            unsafe {
                canvas
                    .ctx
                    .FillRoundedRectangle(&empty_card, &self.brushes.surface_card);
                self.draw_text(
                    canvas,
                    "Chưa phát hiện thiết bị phát âm thanh",
                    TextFormatSlot::ButtonCenter,
                    &text_secondary,
                    UiRectDip {
                        left: empty_card.rect.left,
                        top: empty_card.rect.top,
                        right: empty_card.rect.right,
                        bottom: empty_card.rect.bottom,
                    },
                );
            }
        } else if total > MAX_AUDIO_DEVICES_VISIBLE {
            let hint = format!("Cuộn chuột để xem thêm ({}-{total}/{total})", start + 1);
            let _ = end;
            unsafe {
                self.draw_text(
                    canvas,
                    &hint,
                    TextFormatSlot::Small,
                    &text_secondary,
                    UiRectDip {
                        left: p.left + 16.0,
                        top: p.top + target_h - 22.0,
                        right: p.left + p.width - 16.0,
                        bottom: p.top + target_h - 6.0,
                    },
                );
            }
        }
    }

    unsafe fn draw_notice(&mut self, canvas: DrawCanvas<'_>, input: &RenderFrameInput<'_>) {
        let Some(notice) = input.notice else {
            return;
        };
        let p = virtual_pill_anchored(input.pill, 416.0, 136.0, false);
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let accent_brush = match notice.kind {
            NoticeKind::UpToDate => self.brushes.power_ok.clone(),
            NoticeKind::Error => self.brushes.danger_border.clone(),
            NoticeKind::Checking | NoticeKind::UpdateAvailable | NoticeKind::Downloading => {
                self.brushes.accent.clone()
            }
        };
        let badge_fill = if notice.kind == NoticeKind::Error {
            self.brushes.danger_bg.clone()
        } else {
            self.brushes.surface_card.clone()
        };

        let cx = p.left + 35.0;
        let cy = p.top + 42.0;
        let badge_r = 17.5;
        let badge = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - badge_r,
                top: cy - badge_r,
                right: cx + badge_r,
                bottom: cy + badge_r,
            },
            radiusX: badge_r,
            radiusY: badge_r,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&badge, &badge_fill);
            canvas
                .ctx
                .DrawRoundedRectangle(&badge, &accent_brush, 1.25, None);
            self.draw_notice_glyph(canvas, cx, cy, notice.kind, &accent_brush);
            self.draw_text(
                canvas,
                notice.title,
                TextFormatSlot::Title,
                &text_primary,
                UiRectDip {
                    left: p.left + 64.0,
                    top: p.top + 14.0,
                    right: p.left + p.width - 18.0,
                    bottom: p.top + 36.0,
                },
            );
            self.draw_text(
                canvas,
                notice.message,
                TextFormatSlot::BodyLeft,
                &text_secondary,
                UiRectDip {
                    left: p.left + 64.0,
                    top: p.top + 38.0,
                    right: p.left + p.width - 18.0,
                    bottom: p.top + 88.0,
                },
            );
        }

        fill_notice_hitboxes(p, notice.secondary_label.is_some(), &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: hb.rect.left,
                    top: hb.rect.top,
                    right: hb.rect.right,
                    bottom: hb.rect.bottom,
                },
                radiusX: 11.0,
                radiusY: 11.0,
            };
            match hb.action {
                UiAction::NoticePrimary => {
                    let is_err = notice.kind == NoticeKind::Error;
                    let fill = if is_err {
                        if hovered {
                            &self.brushes.danger_hover
                        } else {
                            &self.brushes.danger_bg
                        }
                    } else if hovered {
                        &self.brushes.surface_hover
                    } else {
                        &self.brushes.accent_soft
                    };
                    let border = if is_err {
                        &self.brushes.danger_border
                    } else {
                        &self.brushes.accent
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&card, fill);
                        canvas.ctx.DrawRoundedRectangle(&card, border, 1.0, None);
                        self.draw_text(
                            canvas,
                            notice.primary_label,
                            TextFormatSlot::ButtonCenter,
                            &text_primary,
                            hb.rect,
                        );
                    }
                }
                UiAction::NoticeSecondary => {
                    let label = notice.secondary_label.unwrap_or("Đóng");
                    let fill = if hovered {
                        &self.brushes.surface_hover
                    } else {
                        &self.brushes.surface_card
                    };
                    unsafe {
                        canvas.ctx.FillRoundedRectangle(&card, fill);
                        canvas.ctx.DrawRoundedRectangle(
                            &card,
                            &self.brushes.card_border,
                            0.85,
                            None,
                        );
                        self.draw_text(
                            canvas,
                            label,
                            TextFormatSlot::ButtonCenter,
                            &text_secondary,
                            hb.rect,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    unsafe fn draw_notice_glyph(
        &self,
        canvas: DrawCanvas<'_>,
        cx: f32,
        cy: f32,
        kind: NoticeKind,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = Some(&self.icons.round_stroke);
        unsafe {
            match kind {
                NoticeKind::UpToDate => {
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 5.5, Y: cy + 0.4 },
                        Vector2 { X: cx - 1.5, Y: cy + 4.4 },
                        brush,
                        2.1,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 1.5, Y: cy + 4.4 },
                        Vector2 { X: cx + 6.0, Y: cy - 4.4 },
                        brush,
                        2.1,
                        stroke,
                    );
                }
                NoticeKind::UpdateAvailable | NoticeKind::Downloading => {
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx, Y: cy - 6.2 },
                        Vector2 { X: cx, Y: cy + 3.4 },
                        brush,
                        2.0,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 4.4, Y: cy - 0.6 },
                        Vector2 { X: cx, Y: cy + 3.8 },
                        brush,
                        2.0,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx + 4.4, Y: cy - 0.6 },
                        Vector2 { X: cx, Y: cy + 3.8 },
                        brush,
                        2.0,
                        stroke,
                    );
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx - 5.5, Y: cy + 6.8 },
                        Vector2 { X: cx + 5.5, Y: cy + 6.8 },
                        brush,
                        1.8,
                        stroke,
                    );
                }
                NoticeKind::Checking => {
                    let dot = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx - 1.3,
                            top: cy - 6.2,
                            right: cx + 1.3,
                            bottom: cy - 3.6,
                        },
                        radiusX: 1.3,
                        radiusY: 1.3,
                    };
                    canvas.ctx.FillRoundedRectangle(&dot, brush);
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx, Y: cy - 1.2 },
                        Vector2 { X: cx, Y: cy + 6.0 },
                        brush,
                        2.1,
                        stroke,
                    );
                }
                NoticeKind::Error => {
                    canvas.ctx.DrawLine(
                        Vector2 { X: cx, Y: cy - 6.0 },
                        Vector2 { X: cx, Y: cy + 1.8 },
                        brush,
                        2.2,
                        stroke,
                    );
                    let dot = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: cx - 1.4,
                            top: cy + 4.4,
                            right: cx + 1.4,
                            bottom: cy + 7.2,
                        },
                        radiusX: 1.4,
                        radiusY: 1.4,
                    };
                    canvas.ctx.FillRoundedRectangle(&dot, brush);
                }
            }
        }
    }

    unsafe fn draw_toggle_tile(
        &mut self,
        canvas: DrawCanvas<'_>,
        rect: UiRectDip,
        label: &str,
        enabled: bool,
        text_brush: &ID2D1SolidColorBrush,
        track_on_brush: &ID2D1SolidColorBrush,
    ) {
        let text_rect = UiRectDip {
            left: rect.left + 10.0,
            top: rect.top,
            right: rect.right - 44.0,
            bottom: rect.bottom,
        };
        unsafe {
            self.draw_text(canvas, label, TextFormatSlot::Button, text_brush, text_rect);
        }

        let sw_w = 30.0;
        let sw_h = 16.0;
        let sw_x = rect.right - sw_w - 10.0;
        let sw_y = rect.top + (rect.height() - sw_h) * 0.5;
        let track = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: sw_x,
                top: sw_y,
                right: sw_x + sw_w,
                bottom: sw_y + sw_h,
            },
            radiusX: sw_h * 0.5,
            radiusY: sw_h * 0.5,
        };
        let track_brush = if enabled {
            track_on_brush
        } else {
            &self.brushes.switch_off
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&track, track_brush);
        }

        let knob_size = 12.0;
        let knob_x = if enabled {
            sw_x + sw_w - knob_size - 2.0
        } else {
            sw_x + 2.0
        };
        let knob_y = sw_y + 2.0;
        let knob = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: knob_x,
                top: knob_y,
                right: knob_x + knob_size,
                bottom: knob_y + knob_size,
            },
            radiusX: knob_size * 0.5,
            radiusY: knob_size * 0.5,
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&knob, &self.brushes.knob);
        }
    }

    unsafe fn draw_artwork_or_placeholder(
        &self,
        canvas: DrawCanvas<'_>,
        x: f32,
        y: f32,
        size: f32,
        radius: f32,
        is_playing: bool,
    ) {
        unsafe {
            self.draw_artwork_with_rim(canvas, x, y, size, radius, is_playing, false);
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_artwork_with_rim(
        &self,
        canvas: DrawCanvas<'_>,
        x: f32,
        y: f32,
        size: f32,
        radius: f32,
        is_playing: bool,
        soft_rim: bool,
    ) {
        let Some(art_brush) = &self.artwork_brush else {
            unsafe {
                self.draw_placeholder_art(canvas, x, y, size, radius, is_playing);
            }
            return;
        };

        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + size,
                bottom: y + size,
            },
            radiusX: radius,
            radiusY: radius,
        };

        let (src_l, src_t, src_r, src_b) = match compute_center_crop_rect(self.artwork_size_px) {
            Some(r) => (r.left, r.top, r.right, r.bottom),
            None => (
                0.0,
                0.0,
                self.artwork_size_px.0.max(1) as f32,
                self.artwork_size_px.1.max(1) as f32,
            ),
        };
        let mat = bitmap_rect_transform(src_l, src_t, src_r, src_b, box_rect.rect);
        unsafe {
            art_brush.SetTransform(&mat);
            canvas.ctx.FillRoundedRectangle(&box_rect, art_brush);
            if soft_rim {
                canvas
                    .ctx
                    .DrawRoundedRectangle(&box_rect, &self.brushes.glass_inner_rim, 1.0, None);
            } else {
                canvas
                    .ctx
                    .DrawRoundedRectangle(&box_rect, &self.brushes.border, 1.0, None);
            }
        }
    }

    unsafe fn draw_placeholder_art(
        &self,
        canvas: DrawCanvas<'_>,
        x: f32,
        y: f32,
        size: f32,
        radius: f32,
        is_playing: bool,
    ) {
        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + size,
                bottom: y + size,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            canvas
                .ctx
                .FillRoundedRectangle(&box_rect, &self.brushes.art_bg);
            canvas
                .ctx
                .DrawRoundedRectangle(&box_rect, &self.brushes.border, 1.0, None);
        }

        let core_size = size * 0.34;
        let core_x = x + (size - core_size) * 0.5;
        let core_y = y + (size - core_size) * 0.5;
        let core = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: core_x,
                top: core_y,
                right: core_x + core_size,
                bottom: core_y + core_size,
            },
            radiusX: core_size * 0.5,
            radiusY: core_size * 0.5,
        };
        let brush = if is_playing {
            &self.brushes.accent
        } else {
            &self.brushes.text_secondary
        };
        unsafe {
            canvas.ctx.FillRoundedRectangle(&core, brush);
        }
    }

    unsafe fn draw_visualizer_bars(
        &self,
        canvas: DrawCanvas<'_>,
        start_x: f32,
        center_y: f32,
        phase: f32,
    ) {
        unsafe {
            self.draw_visualizer_bars_count(
                canvas,
                start_x,
                center_y,
                phase,
                3.2,
                11.6,
                4.0,
                2.2,
                5,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_visualizer_bars_count(
        &self,
        canvas: DrawCanvas<'_>,
        start_x: f32,
        center_y: f32,
        phase: f32,
        min_h: f32,
        amp_h: f32,
        step_x: f32,
        bar_w: f32,
        count: usize,
    ) {
        const ENVELOPE_5: [f32; 5] = [0.55, 0.88, 1.0, 0.88, 0.55];
        for (i, &env5) in ENVELOPE_5.iter().enumerate().take(count) {
            let env = if count == 5 { env5 } else { 1.0 };
            let wave = ((phase + i as f32 * 1.35).sin() * 0.5 + 0.5).clamp(0.22, 1.0);
            let h = min_h + wave * amp_h * env;
            let bx = start_x + i as f32 * step_x;
            let bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: bx,
                    top: center_y - h * 0.5,
                    right: bx + bar_w,
                    bottom: center_y + h * 0.5,
                },
                radiusX: bar_w * 0.5,
                radiusY: bar_w * 0.5,
            };
            unsafe {
                canvas.ctx.FillRoundedRectangle(&bar, &self.brushes.accent);
            }
        }
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
        let w_tenth = (rect.width() * 10.0).round() as i32;
        let h_tenth = (rect.height() * 10.0).round() as i32;

        let layout = if let Some(idx) = self.text_cache.iter().position(|e| {
            e.slot == slot && e.w_tenth == w_tenth && e.h_tenth == h_tenth && e.text == text
        }) {
            self.text_cache[idx].layout.clone()
        } else {
            self.utf16_scratch.clear();
            self.utf16_scratch.extend(text.encode_utf16());
            let format = self.fonts.by_slot(slot);
            let Ok(created) = (unsafe {
                self.dwrite_factory.CreateTextLayout(
                    &self.utf16_scratch,
                    format,
                    rect.width(),
                    rect.height(),
                )
            }) else {
                return;
            };
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
            created
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

fn attach_swapchain_backbuffer(
    d2d_context: &ID2D1DeviceContext,
    swap_chain: &IDXGISwapChain1,
) -> windows::core::Result<ID2D1Bitmap1> {
    let surface: IDXGISurface = unsafe { swap_chain.GetBuffer(0) }?;
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        ..Default::default()
    };

    let bitmap = unsafe { d2d_context.CreateBitmapFromDxgiSurface(&surface, Some(&props)) }?;
    let image: ID2D1Image = bitmap.cast()?;
    unsafe {
        d2d_context.SetTarget(&image);
    }
    Ok(bitmap)
}

fn create_icon_geometries(factory: &ID2D1Factory1) -> windows::core::Result<IconGeometries> {
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

fn boost_accent_for_readability(rgb: [f32; 3]) -> [f32; 3] {
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

fn build_ambient_aura_bgra(art: &DecodedArtwork) -> ([u8; 16 * 10 * 4], u32, u32) {
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

fn extract_vibrant_accent_rgb(bgra: &[u8]) -> [f32; 3] {
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

fn create_ambient_glow_brush(
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

fn create_brushes(ctx: &ID2D1DeviceContext) -> windows::core::Result<BrushPalette> {
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

fn create_text_formats(factory: &IDWriteFactory) -> windows::core::Result<TextFormats> {
    Ok(TextFormats {
        title: make_format(
            factory,
            13.5,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        subtitle: make_format(
            factory,
            11.5,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        body_left: make_format(
            factory,
            11.5,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
            DWRITE_WORD_WRAPPING_WRAP,
        )?,
        compact: make_format(
            factory,
            11.8,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        compact_sub: make_format(
            factory,
            10.0,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        button: make_format(
            factory,
            11.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        button_center: make_format(
            factory,
            11.5,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        small: make_format(
            factory,
            10.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        small_left: make_format(
            factory,
            10.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )?,
        small_right: make_format(
            factory,
            10.5,
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

fn bitmap_rect_transform(
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

fn compute_center_crop_rect((w, h): (u32, u32)) -> Option<D2D_RECT_F> {
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

fn create_low_power_d3d11_device(
    factory6: &IDXGIFactory6,
) -> windows::core::Result<(ID3D11Device, String)> {
    let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
    let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;

    if let Ok(adapter) = unsafe {
        factory6.EnumAdapterByGpuPreference::<IDXGIAdapter1>(0, DXGI_GPU_PREFERENCE_MINIMUM_POWER)
    } {
        let name = read_adapter_name(&adapter);
        let mut device = None;
        let hr = unsafe {
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                Default::default(),
                flags,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
        };
        if hr.is_ok()
            && let Some(dev) = device
        {
            return Ok((dev, name));
        }
    }

    let mut device = None;
    let hw_hr = unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            flags,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    };
    if hw_hr.is_ok()
        && let Some(dev) = device
    {
        return Ok((dev, String::from("Default Hardware Adapter")));
    }

    let mut warp_device = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            Default::default(),
            flags,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut warp_device),
            None,
            None,
        )?;
    }
    let dev = warp_device.ok_or_else(windows::core::Error::from_thread)?;
    Ok((dev, String::from("Microsoft Basic Render (WARP)")))
}

fn read_adapter_name(adapter: &IDXGIAdapter1) -> String {
    let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
        return String::from("Unknown Adapter");
    };
    let end = desc
        .Description
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(desc.Description.len());
    String::from_utf16_lossy(&desc.Description[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::IslandAnimator;
    use windows::Win32::Graphics::Direct2D::{
        D2D1_BITMAP_OPTIONS_CPU_READ, D2D1_MAP_OPTIONS_READ,
    };
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
        WS_EX_TOOLWINDOW, WS_POPUP,
    };

    #[test]
    fn low_power_adapter_prefers_integrated_gpu() {
        let factory6: IDXGIFactory6 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
                .expect("CreateDXGIFactory2 must succeed");
        let (_dev, name) =
            create_low_power_d3d11_device(&factory6).expect("Low-power D3D11 device must succeed");
        println!("Selected DXGI minimum-power adapter: {name}");
        assert!(!name.is_empty());
        assert!(
            !name.to_ascii_uppercase().contains("NVIDIA"),
            "Adapter duoc chon khong duoc la GPU roi NVIDIA: {name}"
        );
    }

    #[test]
    fn winrt_glass_backdrop_initializes_and_renders() {
        struct ComGuard(bool);
        impl Drop for ComGuard {
            fn drop(&mut self) {
                if self.0 {
                    unsafe { CoUninitialize() };
                }
            }
        }

        let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
        let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW");
        let class_name = to_wide_null("STATIC");
        let title = to_wide_null("GlassTestWindow");
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                0,
                0,
                444,
                200,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
        }
        .expect("CreateWindowExW");

        let mut renderer = IslandRenderer::new(hwnd, 1.0).expect("IslandRenderer::new");
        println!(
            "WinRT Glass backdrop active: {} | Blur effect active: {} | GPU: {}",
            renderer.has_glass_backdrop(),
            renderer.has_blur_effect(),
            renderer.adapter_name()
        );
        assert!(
            renderer.has_glass_backdrop(),
            "HostBackdropBrush phai hoat dong tren Windows 11"
        );
        assert!(
            renderer.has_blur_effect(),
            "GaussianBlur CompositionEffectBrush phai hoat dong tren Windows 11"
        );

        let media = MediaDisplayState::default();
        let sample_devices = vec![
            AudioOutputDevice {
                id: String::from("dev-1"),
                name: String::from("Speakers (Realtek Audio)"),
                is_default: true,
            },
            AudioOutputDevice {
                id: String::from("dev-2"),
                name: String::from("Headphones (USB DAC)"),
                is_default: false,
            },
        ];
        let vol_snap = VolumeSnapshot {
            level: 0.64,
            muted: false,
            available: true,
        };
        for mode in [
            IslandMode::Slim,
            IslandMode::Compact,
            IslandMode::Expanded,
            IslandMode::Settings,
            IslandMode::AudioDevices,
            IslandMode::Notice,
            IslandMode::Hidden,
        ] {
            let pill = IslandAnimator::new(mode).current_pill();
            let sample_notice = if mode == IslandMode::Notice {
                Some(IslandNoticeView {
                    kind: NoticeKind::UpToDate,
                    title: "Đã là bản mới nhất",
                    message: "Bạn đang sử dụng phiên bản mới nhất (v0.1.3).",
                    primary_label: "Đã hiểu",
                    secondary_label: None,
                })
            } else {
                None
            };
            let input = RenderFrameInput {
                pill,
                mode,
                pill_opacity: if mode == IslandMode::Hidden { 0.0 } else { 1.0 },
                content_alpha: if mode == IslandMode::Hidden { 0.0 } else { 1.0 },
                config: AppConfig::default(),
                media: &media,
                active_hud: None,
                volume_scrub_ratio: None,
                hovered_action: None,
                timeline_scrub_ratio: None,
                monitor_label: "M1",
                visualizer_phase: 0.4,
                audio_devices: &sample_devices,
                audio_device_scroll: 0,
                volume_snapshot: vol_snap,
                current_audio_device_name: Some("Speakers (Realtek Audio)"),
                notice: sample_notice,
                available_update_tag: Some("v0.2.2"),
            };
            renderer.render(&input).expect("renderer.render");
        }
        for (kind, sec) in [
            (NoticeKind::Checking, None),
            (NoticeKind::UpdateAvailable, Some("Để sau")),
            (NoticeKind::Downloading, None),
            (NoticeKind::Error, Some("Thử lại")),
        ] {
            let notice_input = RenderFrameInput {
                pill: IslandAnimator::new(IslandMode::Notice).current_pill(),
                mode: IslandMode::Notice,
                pill_opacity: 1.0,
                content_alpha: 1.0,
                config: AppConfig::default(),
                media: &media,
                active_hud: None,
                volume_scrub_ratio: None,
                hovered_action: Some(UiAction::NoticePrimary),
                timeline_scrub_ratio: None,
                monitor_label: "M1",
                visualizer_phase: 0.0,
                audio_devices: &[],
                audio_device_scroll: 0,
                volume_snapshot: vol_snap,
                current_audio_device_name: None,
                notice: Some(IslandNoticeView {
                    kind,
                    title: "Kiểm tra thông báo",
                    message: "Nội dung thông báo mẫu nhiều dòng hiển thị đẹp trên thẻ kính khói.",
                    primary_label: "Cập nhật ngay",
                    secondary_label: sec,
                }),
                available_update_tag: None,
            };
            renderer
                .render(&notice_input)
                .expect("render IslandNoticeView");
        }
        let sample_art = DecodedArtwork {
            width: 32,
            height: 20,
            bgra: [30, 50, 210, 255].repeat(32 * 20),
        };
        renderer.update_artwork(ArtworkUpdate::Set(sample_art));
        assert!(renderer.artwork_aura_bitmap.is_some());
        let mut active_media = media.clone();
        active_media.has_session = true;
        active_media.is_playing = true;
        active_media.timeline = crate::state::TimelineState {
            available: true,
            start_ticks: 0,
            end_ticks: 100_000_000,
            base_position_ticks: 45_000_000,
            last_updated_filetime_ticks: 0,
        };
        let slim_active_input = RenderFrameInput {
            pill: IslandAnimator::new(IslandMode::Slim).current_pill(),
            mode: IslandMode::Slim,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &active_media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: None,
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.4,
            audio_devices: &sample_devices,
            audio_device_scroll: 0,
            volume_snapshot: vol_snap,
            current_audio_device_name: Some("Speakers (Realtek Audio)"),
            notice: None,
            available_update_tag: None,
        };
        renderer
            .render(&slim_active_input)
            .expect("render active Slim Notch");
        let compact_active_input = RenderFrameInput {
            pill: IslandAnimator::new(IslandMode::Compact).current_pill(),
            mode: IslandMode::Compact,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &active_media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: None,
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.4,
            audio_devices: &sample_devices,
            audio_device_scroll: 0,
            volume_snapshot: vol_snap,
            current_audio_device_name: Some("Speakers (Realtek Audio)"),
            notice: None,
            available_update_tag: None,
        };
        renderer
            .render(&compact_active_input)
            .expect("render active Compact pill");
        let mut paused_media = active_media.clone();
        paused_media.is_playing = false;
        let compact_paused_input = RenderFrameInput {
            media: &paused_media,
            ..compact_active_input
        };
        renderer
            .render(&compact_paused_input)
            .expect("render paused Compact pill");
        let expanded_input = RenderFrameInput {
            pill: IslandAnimator::new(IslandMode::Expanded).current_pill(),
            mode: IslandMode::Expanded,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &active_media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: None,
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.4,
            audio_devices: &sample_devices,
            audio_device_scroll: 0,
            volume_snapshot: vol_snap,
            current_audio_device_name: Some("Speakers (Realtek Audio)"),
            notice: None,
            available_update_tag: None,
        };
        renderer
            .render(&expanded_input)
            .expect("render with artwork aura");
        let mut dev_anim = IslandAnimator::new(IslandMode::Slim);
        dev_anim.set_target_mode_with_audio_count(IslandMode::AudioDevices, sample_devices.len());
        let mut t = std::time::Instant::now();
        for _ in 0..300 {
            t += std::time::Duration::from_millis(8);
            if !dev_anim.step(t) {
                break;
            }
        }
        let audio_dev_input = RenderFrameInput {
            pill: dev_anim.current_pill(),
            mode: IslandMode::AudioDevices,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &active_media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: None,
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.0,
            audio_devices: &sample_devices,
            audio_device_scroll: 0,
            volume_snapshot: vol_snap,
            current_audio_device_name: Some("Speakers (Realtek Audio)"),
            notice: None,
            available_update_tag: None,
        };
        renderer
            .render(&audio_dev_input)
            .expect("render compact dynamic AudioDevices");
        for hud in [
            HudContent::Volume {
                level: 0.64,
                muted: false,
                show_device_name: false,
            },
            HudContent::Volume {
                level: 0.64,
                muted: false,
                show_device_name: true,
            },
            HudContent::Volume {
                level: 0.64,
                muted: true,
                show_device_name: false,
            },
            HudContent::Power {
                on_ac: true,
                percent: Some(84),
                low_battery: false,
            },
            HudContent::Power {
                on_ac: false,
                percent: Some(12),
                low_battery: true,
            },
        ] {
            let hud_input = RenderFrameInput {
                pill: IslandAnimator::new(IslandMode::Compact).current_pill(),
                mode: IslandMode::Compact,
                pill_opacity: 1.0,
                content_alpha: 1.0,
                config: AppConfig::default(),
                media: &active_media,
                active_hud: Some(hud),
                volume_scrub_ratio: None,
                hovered_action: Some(UiAction::SeekVolume),
                timeline_scrub_ratio: None,
                monitor_label: "M1",
                visualizer_phase: 0.0,
                audio_devices: &sample_devices,
                audio_device_scroll: 0,
                volume_snapshot: vol_snap,
                current_audio_device_name: Some("Headphones (USB DAC)"),
                notice: None,
                available_update_tag: None,
            };
            renderer.render(&hud_input).expect("render HUD Compact");
        }
        renderer.update_artwork(ArtworkUpdate::Clear);
        assert!(renderer.artwork_aura_bitmap.is_none());

        renderer.hide_backdrop();
        drop(renderer);
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }

    #[test]
    fn artwork_aura_and_accent_match_video_colors() {
        // Nửa trái đỏ ấm, nửa phải xanh lam -> lưới màu và điểm nhấn phải giữ đúng tương quan màu.
        let w = 16_usize;
        let h = 10_usize;
        let mut bgra = Vec::with_capacity(w * h * 4);
        for _y in 0..h {
            for x in 0..w {
                if x < w / 2 {
                    bgra.extend_from_slice(&[28, 45, 220, 255]); // Đỏ ấm (BGR)
                } else {
                    bgra.extend_from_slice(&[215, 130, 30, 255]); // Xanh dương (BGR)
                }
            }
        }
        let art = DecodedArtwork {
            width: w as u32,
            height: h as u32,
            bgra,
        };
        let (aura, aw, ah) = build_ambient_aura_bgra(&art);
        assert_eq!((aw, ah), (16, 10));
        // Cột trái ưu tiên đỏ (R > B), cột phải ưu tiên xanh (B > R).
        let left_idx = (5 * 16 + 1) * 4;
        let right_idx = (5 * 16 + 14) * 4;
        assert!(aura[left_idx + 2] > aura[left_idx]);
        assert!(aura[right_idx] > aura[right_idx + 2]);

        let accent = boost_accent_for_readability(extract_vibrant_accent_rgb(&art.bgra));
        assert!(accent.iter().copied().fold(0.0_f32, f32::max) >= 0.70);

        let white_art = DecodedArtwork {
            width: 8,
            height: 8,
            bgra: [255, 255, 255, 255].repeat(64),
        };
        let (white_aura, _, _) = build_ambient_aura_bgra(&white_art);
        assert!(
            white_aura[2] <= 168 && white_aura[1] <= 168 && white_aura[0] <= 168,
            "Aura từ ảnh trắng phải được hãm sáng để không tẩy nhạt chữ: {:?}",
            &white_aura[0..4]
        );
    }

    #[test]
    fn smoked_glass_preserves_translucency_and_contrast_over_white() {
        fn srgb_to_rel_lum(r: f32, g: f32, b: f32) -> f32 {
            let lin = |c: f32| {
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
        }

        struct ComGuard(bool);
        impl Drop for ComGuard {
            fn drop(&mut self) {
                if self.0 {
                    unsafe { CoUninitialize() };
                }
            }
        }

        let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
        let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW");
        let class_name = to_wide_null("STATIC");
        let title = to_wide_null("ContrastTestWindow");
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP,
                0,
                0,
                444,
                260,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
        }
        .expect("CreateWindowExW");

        let mut renderer = IslandRenderer::new(hwnd, 1.0).expect("IslandRenderer::new");
        let size_px = compute_surface_size_px(1.0);
        let bmp_size = D2D_SIZE_U {
            width: size_px.0 as u32,
            height: size_px.1 as u32,
        };
        let target_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            ..Default::default()
        };
        let gpu_bmp = unsafe {
            renderer
                .d2d_context
                .CreateBitmap(bmp_size, None, 0, &target_props)
        }
        .expect("Create offscreen target bitmap");
        let target_img: ID2D1Image = gpu_bmp.cast().expect("ID2D1Image");
        let pill = IslandAnimator::new(IslandMode::Settings).current_pill();
        let media = MediaDisplayState::default();
        let input = RenderFrameInput {
            pill,
            mode: IslandMode::Settings,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &media,
            active_hud: None,
            volume_scrub_ratio: None,
            hovered_action: None,
            timeline_scrub_ratio: None,
            monitor_label: "1 - Chính",
            visualizer_phase: 0.0,
            audio_devices: &[],
            audio_device_scroll: 0,
            volume_snapshot: VolumeSnapshot::default(),
            current_audio_device_name: None,
            notice: None,
            available_update_tag: None,
        };
        let clear = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        unsafe {
            let off_ctx = renderer.d2d_context.clone();
            let canvas = DrawCanvas {
                ctx: &off_ctx,
                scale: 1.0,
                offset_x_px: 0.0,
                offset_y_px: 0.0,
            };
            off_ctx.SetTarget(&target_img);
            off_ctx.BeginDraw();
            off_ctx.SetDpi(96.0, 96.0);
            off_ctx.SetTransform(&canvas.base_transform());
            off_ctx.Clear(Some(&clear));
            renderer.draw_pill_shell(canvas, pill, 1.0, true, false);
            renderer.draw_clipped_contents(canvas, &input, 1.0);
            off_ctx.EndDraw(None, None).expect("EndDraw");
        }

        let cpu_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };
        let cpu_bmp = unsafe {
            renderer
                .d2d_context
                .CreateBitmap(bmp_size, None, 0, &cpu_props)
        }
        .expect("Create CPU_READ bitmap");
        let src_bmp: ID2D1Bitmap = gpu_bmp.cast().expect("ID2D1Bitmap");
        unsafe {
            cpu_bmp
                .CopyFromBitmap(None, &src_bmp, None)
                .expect("CopyFromBitmap");
        }
        let mapped = unsafe { cpu_bmp.Map(D2D1_MAP_OPTIONS_READ) }.expect("Map");
        let pitch = mapped.pitch as usize;
        let total_bytes = pitch * bmp_size.height as usize;
        let buf = unsafe { std::slice::from_raw_parts(mapped.bits, total_bytes) };

        let sample_over_white = |x_f: f32, y_f: f32| -> (f32, f32, f32, f32) {
            let x = (x_f.round() as usize).min(bmp_size.width as usize - 1);
            let y = (y_f.round() as usize).min(bmp_size.height as usize - 1);
            let off = y * pitch + x * 4;
            let b_p = buf[off] as f32 / 255.0;
            let g_p = buf[off + 1] as f32 / 255.0;
            let r_p = buf[off + 2] as f32 / 255.0;
            let a = buf[off + 3] as f32 / 255.0;
            // Hòa trộn PREMULTIPLIED lên nền trắng thuần (1.0, 1.0, 1.0)
            (r_p + (1.0 - a), g_p + (1.0 - a), b_p + (1.0 - a), a)
        };

        let glass = sample_over_white(pill.left + pill.width - 18.0, pill.top + 45.0);
        let card = sample_over_white(pill.left + pill.width - 20.0, pill.top + 14.0);
        let exit = sample_over_white(pill.left + pill.width - 24.0, pill.top + pill.height - 16.0);

        unsafe {
            let _ = cpu_bmp.Unmap();
        }
        renderer.hide_backdrop();
        drop(renderer);
        unsafe {
            let _ = DestroyWindow(hwnd);
        }

        let glass_contrast = (1.0 + 0.05) / (srgb_to_rel_lum(glass.0, glass.1, glass.2) + 0.05);
        println!(
            "Over pure #FFFFFF -> glass={glass:?} (contrast={glass_contrast:.2}:1), card={card:?}, exit={exit:?}"
        );

        assert!(
            glass.3 >= 0.55 && glass.3 <= 0.80,
            "Kính phải giữ độ xuyên sáng vừa phải (0.55..0.80), hiện tại: {:.3}",
            glass.3
        );
        assert!(
            glass_contrast >= 4.5,
            "Độ tương phản chữ trắng trên kính đặt lên nền #FFFFFF phải >= 4.5:1, hiện tại: {glass_contrast:.2}"
        );
        assert!(
            card.0 <= 0.32 && card.1 <= 0.34 && card.2 <= 0.38,
            "Thẻ cài đặt trên nền trắng phải đầm tối, không tẩy xám sáng: {card:?}"
        );
        assert!(
            exit.0 > exit.1 * 1.45 && exit.0 <= 0.62,
            "Nút Thoát ứng dụng trên nền trắng phải giữ đỏ trầm, không bạc hồng: {exit:?}"
        );
    }
}
