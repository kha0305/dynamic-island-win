use std::mem::{ManuallyDrop, size_of};

use windows::Foundation::PropertyValue;
use windows::Graphics::Effects::{
    IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource, IGraphicsEffectSource_Impl,
};
use windows::System::DispatcherQueueController;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{
    CompositionBrush, CompositionEffectSourceParameter, CompositionRoundedRectangleGeometry,
    CompositionStretch, Compositor, ContainerVisual, SpriteVisual, Visual,
};
use windows::Win32::Foundation::{E_BOUNDS, E_INVALIDARG, HWND, TRUE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BORDER_MODE_HARD, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D1_GRADIENT_STOP, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1GaussianBlur, CLSID_D2D1Saturation, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_CAP_STYLE_ROUND, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2,
    D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED, D2D1_GAUSSIANBLUR_PROP_BORDER_MODE,
    D2D1_GAUSSIANBLUR_PROP_OPTIMIZATION, D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION,
    D2D1_INTERPOLATION_MODE_LINEAR, D2D1_LAYER_OPTIONS1_NONE, D2D1_LAYER_PARAMETERS1,
    D2D1_LINE_JOIN_ROUND, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT, D2D1_SATURATION_PROP_SATURATION,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1CreateFactory, ID2D1Bitmap, ID2D1Bitmap1, ID2D1Device,
    ID2D1DeviceContext, ID2D1Factory, ID2D1Factory1, ID2D1Geometry, ID2D1Image, ID2D1Layer,
    ID2D1LinearGradientBrush, ID2D1PathGeometry, ID2D1RadialGradientBrush, ID2D1RenderTarget,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
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
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory, IDWriteInlineObject, IDWriteTextFormat,
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
use windows::Win32::System::WinRT::Composition::{ICompositorDesktopInterop, ICompositorInterop};
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

use crate::media::{ArtworkUpdate, DecodedArtwork};
use crate::state::{
    AppConfig, IslandMode, MediaDisplayState, PillRectDip, UiAction, UiHitbox, UiRectDip,
    compute_surface_size_px, current_windows_filetime_ticks, fill_interactive_hitboxes,
    format_duration_ticks,
};
use crate::tray::to_wide_null;

pub struct RenderFrameInput<'a> {
    pub pill: PillRectDip,
    pub mode: IslandMode,
    pub pill_opacity: f32,
    pub content_alpha: f32,
    pub config: AppConfig,
    pub media: &'a MediaDisplayState,
    pub hovered_action: Option<UiAction>,
    pub timeline_scrub_ratio: Option<f32>,
    pub monitor_label: &'a str,
    pub visualizer_phase: f32,
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
    text_primary: ID2D1SolidColorBrush,
    text_secondary: ID2D1SolidColorBrush,
    text_shadow: ID2D1SolidColorBrush,
    accent: ID2D1SolidColorBrush,
    accent_soft: ID2D1SolidColorBrush,
    switch_off: ID2D1SolidColorBrush,
    knob: ID2D1SolidColorBrush,
    danger_bg: ID2D1SolidColorBrush,
    danger_hover: ID2D1SolidColorBrush,
    danger_border: ID2D1SolidColorBrush,
    art_bg: ID2D1SolidColorBrush,
}

struct TextFormats {
    title: IDWriteTextFormat,
    subtitle: IDWriteTextFormat,
    compact: IDWriteTextFormat,
    button: IDWriteTextFormat,
    button_center: IDWriteTextFormat,
    small: IDWriteTextFormat,
    small_left: IDWriteTextFormat,
    small_right: IDWriteTextFormat,
}

struct IconGeometries {
    round_stroke: ID2D1StrokeStyle,
    play_triangle: ID2D1PathGeometry,
    prev_triangle: ID2D1PathGeometry,
    next_triangle: ID2D1PathGeometry,
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
        compositor: Compositor,
        _target: DesktopWindowTarget,
        root: ContainerVisual,
        backdrop: Option<WinRtBackdropLayer>,
        content_visual: SpriteVisual,
    },
    DirectCompFallback {
        device: IDCompositionDevice,
        _target: IDCompositionTarget,
        _visual: IDCompositionVisual,
    },
}

impl CompositionHost {
    fn new(
        hwnd: HWND,
        dxgi_device: &IDXGIDevice,
        swap_chain: &IDXGISwapChain1,
        surface_w_px: f32,
        surface_h_px: f32,
    ) -> windows::core::Result<Self> {
        if let Ok(winrt) = try_init_winrt_glass(hwnd, swap_chain, surface_w_px, surface_h_px) {
            return Ok(winrt);
        }

        let device: IDCompositionDevice = unsafe { DCompositionCreateDevice(dxgi_device) }?;
        let target = unsafe { device.CreateTargetForHwnd(hwnd, true) }?;
        let visual = unsafe { device.CreateVisual() }?;
        unsafe {
            visual.SetContent(swap_chain)?;
            target.SetRoot(&visual)?;
            device.Commit()?;
        }
        Ok(Self::DirectCompFallback {
            device,
            _target: target,
            _visual: visual,
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
        &self,
        swap_chain: &IDXGISwapChain1,
        surface_w_px: f32,
        surface_h_px: f32,
    ) -> windows::core::Result<()> {
        match self {
            Self::WinRtGlass {
                compositor,
                root,
                content_visual,
                ..
            } => {
                let sz = Vector2 {
                    X: surface_w_px,
                    Y: surface_h_px,
                };
                root.SetSize(sz)?;
                let interop: ICompositorInterop = compositor.cast()?;
                let surface = unsafe { interop.CreateCompositionSurfaceForSwapChain(swap_chain) }?;
                let brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
                brush.SetStretch(CompositionStretch::Fill)?;
                content_visual.SetBrush(&brush)?;
                content_visual.SetSize(sz)?;
                Ok(())
            }
            Self::DirectCompFallback { device, .. } => unsafe { device.Commit() },
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
    swap_chain: &IDXGISwapChain1,
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
    let surface = unsafe { interop.CreateCompositionSurfaceForSwapChain(swap_chain) }?;
    let surface_brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
    surface_brush.SetStretch(CompositionStretch::Fill)?;

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
        compositor,
        _target: target,
        root,
        backdrop,
        content_visual,
    })
}

pub struct IslandRenderer {
    _d3d_device: ID3D11Device,
    d2d_factory: ID2D1Factory1,
    _d2d_device: ID2D1Device,
    d2d_context: ID2D1DeviceContext,
    dwrite_factory: IDWriteFactory,
    comp_host: CompositionHost,
    swap_chain: IDXGISwapChain1,
    target_bitmap: Option<ID2D1Bitmap1>,
    artwork_bitmap: Option<ID2D1Bitmap1>,
    artwork_aura_bitmap: Option<ID2D1Bitmap1>,
    artwork_size_px: (u32, u32),
    ambient_glow_brush: ID2D1RadialGradientBrush,
    artwork_layer: ID2D1Layer,
    clip_layer: ID2D1Layer,
    brushes: BrushPalette,
    fonts: TextFormats,
    icons: IconGeometries,
    utf16_scratch: Vec<u16>,
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

        let (surface_w, surface_h) = compute_surface_size_px(dpi_scale);
        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: surface_w as u32,
            Height: surface_h as u32,
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
            factory2.CreateSwapChainForComposition(&d3d_device, &swap_chain_desc, None)
        }?;

        let comp_host = CompositionHost::new(
            hwnd,
            &dxgi_device,
            &swap_chain,
            surface_w as f32,
            surface_h as f32,
        )?;

        let d2d_factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;
        let d2d_device = unsafe { d2d_factory.CreateDevice(&dxgi_device) }?;
        let d2d_context =
            unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE) }?;
        let clip_layer = unsafe { d2d_context.CreateLayer(None) }?;
        let artwork_layer = unsafe { d2d_context.CreateLayer(None) }?;
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;

        let brushes = create_brushes(&d2d_context)?;
        let ambient_glow_brush = create_ambient_glow_brush(&d2d_context, [0.24, 0.72, 0.98], 0.14)?;
        let fonts = create_text_formats(&dwrite_factory)?;
        let icons = create_icon_geometries(&d2d_factory)?;

        let mut renderer = Self {
            _d3d_device: d3d_device,
            d2d_factory,
            _d2d_device: d2d_device,
            d2d_context,
            dwrite_factory,
            comp_host,
            swap_chain,
            target_bitmap: None,
            artwork_bitmap: None,
            artwork_aura_bitmap: None,
            artwork_size_px: (0, 0),
            ambient_glow_brush,
            artwork_layer,
            clip_layer,
            brushes,
            fonts,
            icons,
            utf16_scratch: Vec::with_capacity(128),
            hitbox_scratch: Vec::with_capacity(8),
            adapter_name,
            dpi_scale,
        };
        renderer.attach_backbuffer(dpi_scale)?;
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
                self.artwork_bitmap = None;
                self.artwork_aura_bitmap = None;
                self.artwork_size_px = (0, 0);
                self.apply_media_theme_colors([0.24, 0.78, 1.0], 0.14);
            }
            ArtworkUpdate::Set(art) => {
                let sz = (art.width, art.height);
                let accent = boost_accent_for_readability(extract_vibrant_accent_rgb(&art.bgra));
                self.apply_media_theme_colors(accent, 0.28);

                let (aura_bgra, aura_w, aura_h) = build_ambient_aura_bgra(&art);
                self.artwork_aura_bitmap = self
                    .upload_bgra_bitmap(aura_w, aura_h, &aura_bgra)
                    .ok();

                match self.upload_artwork_bitmap(&art) {
                    Ok(bmp) => {
                        self.artwork_bitmap = Some(bmp);
                        self.artwork_size_px = sz;
                    }
                    Err(_) => {
                        self.artwork_bitmap = None;
                        self.artwork_size_px = (0, 0);
                    }
                }
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
        if (self.dpi_scale - dpi_scale).abs() < 0.001 {
            return Ok(());
        }
        self.dpi_scale = dpi_scale;
        unsafe {
            self.d2d_context.SetTarget(None);
        }
        self.target_bitmap = None;

        let (surface_w, surface_h) = compute_surface_size_px(dpi_scale);
        unsafe {
            self.swap_chain.ResizeBuffers(
                2,
                surface_w as u32,
                surface_h as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                Default::default(),
            )?;
        }
        self.attach_backbuffer(dpi_scale)?;
        self.comp_host
            .resize_content(&self.swap_chain, surface_w as f32, surface_h as f32)?;
        Ok(())
    }

    pub fn render(&mut self, input: &RenderFrameInput<'_>) -> windows::core::Result<()> {
        self.comp_host
            .update_backdrop(input.pill, input.pill_opacity, self.dpi_scale);

        let Some(bitmap) = self.target_bitmap.clone() else {
            return Ok(());
        };
        let target_image: ID2D1Image = bitmap.cast()?;
        let clear = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };

        unsafe {
            self.d2d_context.SetTarget(&target_image);
            self.d2d_context.BeginDraw();
            self.d2d_context.Clear(Some(&clear));

            if input.pill_opacity > 0.02 && input.pill.height >= 2.0 {
                let show_media_aura =
                    !matches!(input.mode, IslandMode::Slim | IslandMode::Settings)
                        && input.media.has_session;
                self.draw_pill_shell(
                    input.pill,
                    input.pill_opacity,
                    self.comp_host.has_live_backdrop(),
                    show_media_aura,
                );
                if input.content_alpha > 0.05 && input.mode != IslandMode::Hidden {
                    self.draw_clipped_contents(input);
                }
            }

            self.d2d_context.EndDraw(None, None)?;
            self.swap_chain.Present(0, DXGI_PRESENT(0)).ok()?;
        }
        Ok(())
    }

    unsafe fn draw_pill_shell(
        &self,
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
        let has_video_aura = show_media_aura && self.artwork_aura_bitmap.is_some();

        unsafe {
            if !has_backdrop {
                self.brushes.bg.SetOpacity(opacity);
                self.d2d_context
                    .FillRoundedRectangle(&rounded, &self.brushes.bg);
            }

            if has_video_aura && let Some(aura_bmp) = &self.artwork_aura_bitmap {
                // Giữ lớp nền tối mờ vừa đủ (~0.24-0.34) để nền trắng không tẩy nhạt màu video.
                self.brushes.glass_fill.SetStartPoint(start);
                self.brushes.glass_fill.SetEndPoint(end);
                self.brushes.glass_fill.SetOpacity(opacity);
                self.d2d_context
                    .FillRoundedRectangle(&rounded, &self.brushes.glass_fill);

                if let Ok(geom) = self.d2d_factory.CreateRoundedRectangleGeometry(&rounded)
                    && let Ok(geom_base) = geom.cast::<ID2D1Geometry>()
                    && let Ok(aura_base) = aura_bmp.cast::<ID2D1Bitmap>()
                {
                    let mut layer_params = D2D1_LAYER_PARAMETERS1 {
                        contentBounds: rounded.rect,
                        geometricMask: ManuallyDrop::new(Some(geom_base)),
                        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                        maskTransform: Matrix3x2::identity(),
                        opacity: opacity * 0.62,
                        opacityBrush: ManuallyDrop::new(None),
                        layerOptions: D2D1_LAYER_OPTIONS1_NONE,
                    };
                    self.d2d_context
                        .PushLayer(&layer_params, &self.artwork_layer);
                    ManuallyDrop::drop(&mut layer_params.geometricMask);
                    ManuallyDrop::drop(&mut layer_params.opacityBrush);
                    self.d2d_context.DrawBitmap(
                        &aura_base,
                        Some(&rounded.rect),
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        None,
                        None,
                    );
                    self.d2d_context.PopLayer();
                }
            } else {
                self.brushes.glass_fill.SetStartPoint(start);
                self.brushes.glass_fill.SetEndPoint(end);
                self.brushes.glass_fill.SetOpacity(opacity * 0.72);
                self.d2d_context
                    .FillRoundedRectangle(&rounded, &self.brushes.glass_fill);
            }

            self.ambient_glow_brush.SetCenter(glow_center);
            self.ambient_glow_brush
                .SetRadiusX((pill.width * 0.62).max(40.0));
            self.ambient_glow_brush
                .SetRadiusY((pill.height * 0.96).max(24.0));
            self.ambient_glow_brush.SetOpacity(opacity);
            self.d2d_context
                .FillRoundedRectangle(&rounded, &self.ambient_glow_brush);

            self.brushes.glass_sheen.SetStartPoint(start);
            self.brushes.glass_sheen.SetEndPoint(end);
            self.brushes.glass_sheen.SetOpacity(opacity);
            self.d2d_context
                .FillRoundedRectangle(&rounded, &self.brushes.glass_sheen);

            self.brushes.glass_inner_rim.SetStartPoint(start);
            self.brushes.glass_inner_rim.SetEndPoint(end);
            self.brushes.glass_inner_rim.SetOpacity(opacity);
            self.d2d_context.DrawRoundedRectangle(
                &inner_rim_rect,
                &self.brushes.glass_inner_rim,
                0.9,
                None,
            );

            self.brushes.glass_edge_shadow.SetOpacity(opacity);
            self.d2d_context.DrawRoundedRectangle(
                &rounded,
                &self.brushes.glass_edge_shadow,
                1.8,
                None,
            );

            self.brushes.glass_border.SetStartPoint(start);
            self.brushes.glass_border.SetEndPoint(end);
            self.brushes.glass_border.SetOpacity(opacity);
            self.d2d_context
                .DrawRoundedRectangle(&rounded, &self.brushes.glass_border, 1.15, None);
        }
    }

    unsafe fn draw_clipped_contents(&mut self, input: &RenderFrameInput<'_>) {
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
            opacity: input.content_alpha.clamp(0.0, 1.0),
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };

        unsafe {
            self.d2d_context.PushLayer(&layer_params, &self.clip_layer);
            // Giải phóng refcount ManuallyDrop ngay sau PushLayer để không rò rỉ đối tượng COM.
            ManuallyDrop::drop(&mut layer_params.geometricMask);
            ManuallyDrop::drop(&mut layer_params.opacityBrush);
        }

        match input.mode {
            IslandMode::Slim => unsafe { self.draw_slim(input) },
            IslandMode::Compact => unsafe { self.draw_compact(input) },
            IslandMode::Expanded => unsafe { self.draw_expanded(input) },
            IslandMode::Settings => unsafe { self.draw_settings(input) },
            IslandMode::Hidden => {}
        }

        unsafe {
            self.d2d_context.PopLayer();
        }
    }

    unsafe fn draw_slim(&mut self, input: &RenderFrameInput<'_>) {
        let p = input.pill;
        let dot_size = 8.0;
        let dot_x = p.left + 14.0;
        let dot_y = p.top + (p.height - dot_size) * 0.5;
        let dot = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: dot_x,
                top: dot_y,
                right: dot_x + dot_size,
                bottom: dot_y + dot_size,
            },
            radiusX: dot_size * 0.5,
            radiusY: dot_size * 0.5,
        };
        let brush = if input.media.has_session {
            &self.brushes.accent
        } else {
            &self.brushes.text_secondary
        };
        unsafe {
            self.d2d_context.FillRoundedRectangle(&dot, brush);
        }

        let small_left_fmt = self.fonts.small_left.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let text_primary = self.brushes.text_primary.clone();
        let (label, text_brush) = if input.media.has_session {
            (input.media.title.as_str(), &text_primary)
        } else {
            ("Dynamic Island", &text_secondary)
        };
        unsafe {
            self.draw_text(
                label,
                &small_left_fmt,
                text_brush,
                UiRectDip {
                    left: dot_x + dot_size + 10.0,
                    top: p.top + 3.0,
                    right: p.left + p.width - 14.0,
                    bottom: p.top + p.height - 3.0,
                },
            );
        }
    }

    unsafe fn draw_compact(&mut self, input: &RenderFrameInput<'_>) {
        let p = input.pill;
        let art_size = 22.0;
        let art_x = p.left + 10.0;
        let art_y = p.top + (p.height - art_size) * 0.5;
        unsafe {
            self.draw_artwork_or_placeholder(art_x, art_y, art_size, 7.0, input.media.is_playing);
        }

        let vis_w = if input.config.visualizer_enabled && input.media.is_playing {
            24.0
        } else {
            10.0
        };
        let text_left = art_x + art_size + 10.0;
        let text_right = (p.left + p.width - 12.0 - vis_w).max(text_left + 20.0);
        let label = if input.media.has_session {
            input.media.title.as_str()
        } else {
            "Sẵn sàng - Bấm để mở rộng"
        };
        let compact_fmt = self.fonts.compact.clone();
        let text_primary = self.brushes.text_primary.clone();
        unsafe {
            self.draw_text(
                label,
                &compact_fmt,
                &text_primary,
                UiRectDip {
                    left: text_left,
                    top: p.top + 4.0,
                    right: text_right,
                    bottom: p.top + p.height - 4.0,
                },
            );
        }

        if input.config.visualizer_enabled && input.media.is_playing {
            unsafe {
                self.draw_visualizer_bars(
                    p.left + p.width - 28.0,
                    p.top + p.height * 0.5,
                    input.visualizer_phase,
                );
            }
        }
    }

    unsafe fn draw_expanded(&mut self, input: &RenderFrameInput<'_>) {
        let p = input.pill;
        let art_size = 64.0;
        let art_x = p.left + 18.0;
        let art_y = p.top + 16.0;
        unsafe {
            self.draw_artwork_or_placeholder(art_x, art_y, art_size, 14.0, input.media.is_playing);
        }

        let text_left = art_x + art_size + 14.0;
        let text_right = p.left + p.width - 80.0;

        let title_fmt = self.fonts.title.clone();
        let subtitle_fmt = self.fonts.subtitle.clone();
        let small_fmt = self.fonts.small.clone();
        let small_left_fmt = self.fonts.small_left.clone();
        let small_right_fmt = self.fonts.small_right.clone();
        let text_primary = self.brushes.text_primary.clone();
        let text_secondary = self.brushes.text_secondary.clone();
        let accent = self.brushes.accent.clone();

        unsafe {
            self.draw_text(
                &input.media.title,
                &title_fmt,
                &text_primary,
                UiRectDip {
                    left: text_left,
                    top: p.top + 16.0,
                    right: text_right,
                    bottom: p.top + 38.0,
                },
            );
            self.draw_text(
                &input.media.artist,
                &subtitle_fmt,
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
                status_label,
                &small_left_fmt,
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
                &pos_label,
                &small_left_fmt,
                &text_secondary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 85.5,
                    right: p.left + 58.0,
                    bottom: p.top + 105.5,
                },
            );
            self.draw_text(
                &dur_label,
                &small_right_fmt,
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
                self.d2d_context.FillRoundedRectangle(&card, fill);
            }
            if hb.action == UiAction::OpenSettings {
                unsafe {
                    self.draw_text("Cài đặt", &small_fmt, &text_primary, hb.rect);
                }
            } else {
                let icon_brush = if input.media.has_session {
                    &text_primary
                } else {
                    &text_secondary
                };
                unsafe {
                    self.draw_media_action_icon(
                        hb.action,
                        hb.rect,
                        input.media.is_playing,
                        icon_brush,
                    );
                }
            }
        }
    }

    unsafe fn draw_media_action_icon(
        &self,
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
                        self.d2d_context.FillRoundedRectangle(&bar, brush);
                    }
                }
            }
            UiAction::MediaPlayPause => unsafe {
                self.draw_centered_triangle(&self.icons.play_triangle, cx + 0.8, cy, 2.6, brush);
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
                    self.d2d_context.FillRoundedRectangle(&bar, brush);
                    self.draw_centered_triangle(&self.icons.prev_triangle, cx + 1.8, cy, 2.2, brush);
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
                    self.d2d_context.FillRoundedRectangle(&bar, brush);
                    self.draw_centered_triangle(&self.icons.next_triangle, cx - 1.8, cy, 2.2, brush);
                }
            }
            _ => {}
        }
    }

    unsafe fn draw_centered_triangle(
        &self,
        geom: &ID2D1PathGeometry,
        cx: f32,
        cy: f32,
        round_stroke_w: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let Ok(base) = geom.cast::<ID2D1Geometry>() else {
            return;
        };
        let translate = Matrix3x2::translation(cx, cy);
        let identity = Matrix3x2::identity();
        unsafe {
            self.d2d_context.SetTransform(&translate);
            self.d2d_context.FillGeometry(&base, brush, None);
            self.d2d_context
                .DrawGeometry(&base, brush, round_stroke_w, &self.icons.round_stroke);
            self.d2d_context.SetTransform(&identity);
        }
    }

    unsafe fn draw_timeline_bar(
        &self,
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
            self.d2d_context
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
            self.d2d_context
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
                self.d2d_context
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
            self.d2d_context
                .FillRoundedRectangle(&knob, &self.brushes.knob);
        }
    }

    unsafe fn draw_settings(&mut self, input: &RenderFrameInput<'_>) {
        let p = input.pill;
        let title_fmt = self.fonts.title.clone();
        let btn_center_fmt = self.fonts.button_center.clone();
        let btn_fmt = self.fonts.button.clone();
        let small_fmt = self.fonts.small.clone();
        let text_primary = self.brushes.text_primary.clone();
        let accent = self.brushes.accent.clone();

        unsafe {
            self.draw_text(
                "Bảng điều khiển nhanh",
                &title_fmt,
                &text_primary,
                UiRectDip {
                    left: p.left + 18.0,
                    top: p.top + 12.0,
                    right: p.left + p.width - 96.0,
                    bottom: p.top + 36.0,
                },
            );
        }

        fill_interactive_hitboxes(p, IslandMode::Settings, &mut self.hitbox_scratch);
        let count = self.hitbox_scratch.len();
        for idx in 0..count {
            let hb = self.hitbox_scratch[idx];
            let hovered = input.hovered_action == Some(hb.action);
            let is_danger = hb.action == UiAction::ExitApp;
            let fill = if is_danger {
                if hovered {
                    &self.brushes.danger_hover
                } else {
                    &self.brushes.danger_bg
                }
            } else if hovered {
                &self.brushes.surface_hover
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
                self.d2d_context.FillRoundedRectangle(&card, fill);
                if is_danger {
                    self.d2d_context.DrawRoundedRectangle(
                        &card,
                        &self.brushes.danger_border,
                        1.0,
                        None,
                    );
                }
            }

            match hb.action {
                UiAction::BackToMedia => unsafe {
                    self.draw_text("Quay lại", &btn_center_fmt, &accent, hb.rect);
                },
                UiAction::ToggleAutoHideIdle => unsafe {
                    self.draw_toggle_tile(
                        hb.rect,
                        "Tự ẩn khi rảnh",
                        input.config.auto_hide_when_idle,
                        &btn_fmt,
                        &text_primary,
                    );
                },
                UiAction::ToggleAutoHideFullscreen => unsafe {
                    self.draw_toggle_tile(
                        hb.rect,
                        "Ẩn toàn màn hình",
                        input.config.auto_hide_fullscreen,
                        &btn_fmt,
                        &text_primary,
                    );
                },
                UiAction::ToggleVisualizer => unsafe {
                    self.draw_toggle_tile(
                        hb.rect,
                        "Sóng nhạc động",
                        input.config.visualizer_enabled,
                        &btn_fmt,
                        &text_primary,
                    );
                },
                UiAction::ToggleAutostart => unsafe {
                    self.draw_toggle_tile(
                        hb.rect,
                        "Mở cùng Windows",
                        input.config.autostart,
                        &btn_fmt,
                        &text_primary,
                    );
                },
                UiAction::CycleMonitor => {
                    let label = format!("Màn hình: {}", input.monitor_label);
                    unsafe {
                        self.draw_text(
                            &label,
                            &btn_fmt,
                            &text_primary,
                            UiRectDip {
                                left: hb.rect.left + 12.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 72.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                        self.draw_text(
                            "Đổi",
                            &small_fmt,
                            &accent,
                            UiRectDip {
                                left: hb.rect.right - 64.0,
                                top: hb.rect.top,
                                right: hb.rect.right - 12.0,
                                bottom: hb.rect.bottom,
                            },
                        );
                    }
                }
                UiAction::HideIsland => unsafe {
                    self.draw_text("Ẩn tạm", &btn_center_fmt, &text_primary, hb.rect);
                },
                UiAction::ExitApp => unsafe {
                    self.draw_text("Thoát ứng dụng", &btn_center_fmt, &text_primary, hb.rect);
                },
                _ => {}
            }
        }
    }

    unsafe fn draw_toggle_tile(
        &mut self,
        rect: UiRectDip,
        label: &str,
        enabled: bool,
        font: &IDWriteTextFormat,
        text_brush: &ID2D1SolidColorBrush,
    ) {
        let text_rect = UiRectDip {
            left: rect.left + 10.0,
            top: rect.top,
            right: rect.right - 44.0,
            bottom: rect.bottom,
        };
        unsafe {
            self.draw_text(label, font, text_brush, text_rect);
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
            &self.brushes.accent
        } else {
            &self.brushes.switch_off
        };
        unsafe {
            self.d2d_context.FillRoundedRectangle(&track, track_brush);
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
            self.d2d_context
                .FillRoundedRectangle(&knob, &self.brushes.knob);
        }
    }

    unsafe fn draw_artwork_or_placeholder(
        &self,
        x: f32,
        y: f32,
        size: f32,
        radius: f32,
        is_playing: bool,
    ) {
        let Some(art_bmp) = &self.artwork_bitmap else {
            unsafe {
                self.draw_placeholder_art(x, y, size, radius, is_playing);
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

        if let Ok(geom) = unsafe { self.d2d_factory.CreateRoundedRectangleGeometry(&box_rect) }
            && let Ok(geom_base) = geom.cast::<ID2D1Geometry>()
            && let Ok(bmp_base) = art_bmp.cast::<ID2D1Bitmap>()
        {
            let mut layer_params = D2D1_LAYER_PARAMETERS1 {
                contentBounds: box_rect.rect,
                geometricMask: ManuallyDrop::new(Some(geom_base)),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: Matrix3x2::identity(),
                opacity: 1.0,
                opacityBrush: ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS1_NONE,
            };

            unsafe {
                self.d2d_context
                    .PushLayer(&layer_params, &self.artwork_layer);
                ManuallyDrop::drop(&mut layer_params.geometricMask);
                ManuallyDrop::drop(&mut layer_params.opacityBrush);
                let src_rect = compute_center_crop_rect(self.artwork_size_px);
                self.d2d_context.DrawBitmap(
                    &bmp_base,
                    Some(&box_rect.rect),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    src_rect.as_ref().map(|r| r as *const D2D_RECT_F),
                    None,
                );
                self.d2d_context.PopLayer();
                self.d2d_context
                    .DrawRoundedRectangle(&box_rect, &self.brushes.border, 1.0, None);
            }
        } else {
            unsafe {
                self.draw_placeholder_art(x, y, size, radius, is_playing);
            }
        }
    }

    unsafe fn draw_placeholder_art(
        &self,
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
            self.d2d_context
                .FillRoundedRectangle(&box_rect, &self.brushes.art_bg);
            self.d2d_context
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
            self.d2d_context.FillRoundedRectangle(&core, brush);
        }
    }

    unsafe fn draw_visualizer_bars(&self, start_x: f32, center_y: f32, phase: f32) {
        for i in 0..3 {
            let wave = ((phase + i as f32 * 1.6).sin() * 0.5 + 0.5).clamp(0.2, 1.0);
            let h = 4.0 + wave * 12.0;
            let bx = start_x + i as f32 * 5.0;
            let bar = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: bx,
                    top: center_y - h * 0.5,
                    right: bx + 3.0,
                    bottom: center_y + h * 0.5,
                },
                radiusX: 1.5,
                radiusY: 1.5,
            };
            unsafe {
                self.d2d_context
                    .FillRoundedRectangle(&bar, &self.brushes.accent);
            }
        }
    }

    unsafe fn draw_text(
        &mut self,
        text: &str,
        format: &IDWriteTextFormat,
        brush: &ID2D1SolidColorBrush,
        rect: UiRectDip,
    ) {
        if rect.width() <= 1.0 || rect.height() <= 1.0 {
            return;
        }
        self.utf16_scratch.clear();
        self.utf16_scratch.extend(text.encode_utf16());
        let Ok(layout) = (unsafe {
            self.dwrite_factory.CreateTextLayout(
                &self.utf16_scratch,
                format,
                rect.width(),
                rect.height(),
            )
        }) else {
            return;
        };
        unsafe {
            self.d2d_context.DrawTextLayout(
                Vector2 {
                    X: rect.left,
                    Y: rect.top + 0.6,
                },
                &layout,
                &self.brushes.text_shadow,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
            self.d2d_context.DrawTextLayout(
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

    fn attach_backbuffer(&mut self, dpi_scale: f32) -> windows::core::Result<()> {
        let surface: IDXGISurface = unsafe { self.swap_chain.GetBuffer(0) }?;
        let dpi = 96.0 * dpi_scale;
        let props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: dpi,
            dpiY: dpi,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            ..Default::default()
        };

        let bitmap = unsafe {
            self.d2d_context
                .CreateBitmapFromDxgiSurface(&surface, Some(&props))
        }?;
        let image: ID2D1Image = bitmap.cast()?;
        unsafe {
            self.d2d_context.SetTarget(&image);
        }
        self.target_bitmap = Some(bitmap);
        Ok(())
    }
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

    Ok(IconGeometries {
        round_stroke,
        play_triangle,
        prev_triangle,
        next_triangle,
    })
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
        if peak > 0.78 {
            let scale = 0.78 / peak;
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
                    r: 0.04,
                    g: 0.055,
                    b: 0.09,
                    a: 0.24,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.45,
                color: D2D1_COLOR_F {
                    r: 0.025,
                    g: 0.035,
                    b: 0.06,
                    a: 0.30,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    r: 0.015,
                    g: 0.022,
                    b: 0.04,
                    a: 0.36,
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
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.34,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.24,
                color: D2D1_COLOR_F {
                    r: 0.92,
                    g: 0.96,
                    b: 1.0,
                    a: 0.08,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.54,
                color: D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.0,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.84,
                color: D2D1_COLOR_F {
                    r: 0.88,
                    g: 0.94,
                    b: 1.0,
                    a: 0.025,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.14,
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
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.86,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.34,
                color: D2D1_COLOR_F {
                    r: 0.90,
                    g: 0.96,
                    b: 1.0,
                    a: 0.30,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.68,
                color: D2D1_COLOR_F {
                    r: 0.85,
                    g: 0.93,
                    b: 1.0,
                    a: 0.24,
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.58,
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
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.24,
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
                    a: 0.16,
                },
            },
        ],
    )?;

    Ok(BrushPalette {
        bg: make(0.048, 0.054, 0.072, 0.86)?,
        border: make(1.0, 1.0, 1.0, 0.32)?,
        glass_fill,
        glass_sheen,
        glass_border,
        glass_inner_rim,
        glass_edge_shadow: make(0.03, 0.04, 0.08, 0.16)?,
        surface_card: make(0.04, 0.05, 0.09, 0.36)?,
        surface_hover: make(0.08, 0.11, 0.18, 0.50)?,
        text_primary: make(0.99, 0.99, 1.0, 1.0)?,
        text_secondary: make(0.92, 0.95, 1.0, 0.96)?,
        text_shadow: make(0.02, 0.03, 0.06, 0.44)?,
        accent: make(0.32, 0.80, 1.0, 1.0)?,
        accent_soft: make(0.14, 0.42, 0.62, 0.48)?,
        switch_off: make(0.03, 0.04, 0.08, 0.42)?,
        knob: make(0.99, 0.99, 1.0, 1.0)?,
        danger_bg: make(0.85, 0.20, 0.22, 0.32)?,
        danger_hover: make(0.92, 0.26, 0.26, 0.50)?,
        danger_border: make(0.98, 0.50, 0.50, 0.60)?,
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
        )?,
        subtitle: make_format(
            factory,
            11.5,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?,
        compact: make_format(
            factory,
            12.0,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?,
        button: make_format(
            factory,
            11.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?,
        button_center: make_format(
            factory,
            11.5,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?,
        small: make_format(
            factory,
            10.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?,
        small_left: make_format(
            factory,
            10.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?,
        small_right: make_format(
            factory,
            10.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?,
    })
}

fn make_format(
    factory: &IDWriteFactory,
    size: f32,
    weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
    align: DWRITE_TEXT_ALIGNMENT,
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
        fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
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
        for mode in [
            IslandMode::Slim,
            IslandMode::Compact,
            IslandMode::Expanded,
            IslandMode::Settings,
            IslandMode::Hidden,
        ] {
            let pill = IslandAnimator::new(mode).current_pill();
            let input = RenderFrameInput {
                pill,
                mode,
                pill_opacity: if mode == IslandMode::Hidden { 0.0 } else { 1.0 },
                content_alpha: if mode == IslandMode::Hidden { 0.0 } else { 1.0 },
                config: AppConfig::default(),
                media: &media,
                hovered_action: None,
                timeline_scrub_ratio: None,
                monitor_label: "M1",
                visualizer_phase: 0.4,
            };
            renderer.render(&input).expect("renderer.render");
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
        let expanded_input = RenderFrameInput {
            pill: IslandAnimator::new(IslandMode::Expanded).current_pill(),
            mode: IslandMode::Expanded,
            pill_opacity: 1.0,
            content_alpha: 1.0,
            config: AppConfig::default(),
            media: &active_media,
            hovered_action: None,
            timeline_scrub_ratio: None,
            monitor_label: "M1",
            visualizer_phase: 0.4,
        };
        renderer
            .render(&expanded_input)
            .expect("render with artwork aura");
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
    }
}
