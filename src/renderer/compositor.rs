use std::mem::size_of;

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
use windows::Win32::Foundation::{E_BOUNDS, E_INVALIDARG, HWND, TRUE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BORDER_MODE_HARD, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    CLSID_D2D1GaussianBlur, CLSID_D2D1Saturation, D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED,
    D2D1_GAUSSIANBLUR_PROP_BORDER_MODE, D2D1_GAUSSIANBLUR_PROP_OPTIMIZATION,
    D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION, D2D1_SATURATION_PROP_SATURATION, ID2D1Bitmap1,
    ID2D1Device, ID2D1DeviceContext, ID2D1Image,
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
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_HOSTBACKDROPBRUSH, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_GPU_PREFERENCE_MINIMUM_POWER, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter1, IDXGIDevice,
    IDXGIFactory2, IDXGIFactory6, IDXGISurface, IDXGISwapChain1,
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
use windows_numerics::{Vector2, Vector3};

use crate::state::PillRectDip;

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

pub(super) struct WinRtBackdropLayer {
    visual: SpriteVisual,
    geometry: CompositionRoundedRectangleGeometry,
    last_state: BackdropState,
    pub(super) blur_effect_active: bool,
}

pub(super) enum CompositionHost {
    WinRtGlass {
        _dispatcher_queue: DispatcherQueueController,
        _compositor: Compositor,
        _target: DesktopWindowTarget,
        root: ContainerVisual,
        backdrop: Option<WinRtBackdropLayer>,
        shelf_backdrop: Option<WinRtBackdropLayer>,
        left_anti_backdrop: Option<WinRtBackdropLayer>,
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
    pub(super) fn new(
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

    pub(super) fn has_live_backdrop(&self) -> bool {
        matches!(
            self,
            Self::WinRtGlass {
                backdrop: Some(_),
                ..
            }
        )
    }

    pub(super) fn update_backdrops(
        &mut self,
        pill: PillRectDip,
        shelf: Option<PillRectDip>,
        left_anti: Option<PillRectDip>,
        opacity: f32,
        dpi_scale: f32,
    ) {
        let Self::WinRtGlass {
            backdrop,
            shelf_backdrop,
            left_anti_backdrop,
            ..
        } = self
        else {
            return;
        };

        if let Some(layer) = backdrop.as_mut() {
            update_layer_geometry(layer, pill, opacity, dpi_scale);
        }
        if let Some(shelf_layer) = shelf_backdrop.as_mut() {
            if let Some(shelf_rect) = shelf {
                update_layer_geometry(shelf_layer, shelf_rect, opacity, dpi_scale);
            } else {
                hide_layer_geometry(shelf_layer);
            }
        }
        if let Some(left_layer) = left_anti_backdrop.as_mut() {
            if let Some(left_rect) = left_anti {
                update_layer_geometry(left_layer, left_rect, opacity, dpi_scale);
            } else {
                hide_layer_geometry(left_layer);
            }
        }
    }

    pub(super) fn hide_backdrop(&mut self) {
        if let Self::WinRtGlass {
            backdrop,
            shelf_backdrop,
            left_anti_backdrop,
            ..
        } = self
        {
            if let Some(layer) = backdrop.as_mut() {
                hide_layer_geometry(layer);
            }
            if let Some(shelf_layer) = shelf_backdrop.as_mut() {
                hide_layer_geometry(shelf_layer);
            }
            if let Some(left_layer) = left_anti_backdrop.as_mut() {
                hide_layer_geometry(left_layer);
            }
        }
    }

    pub(super) fn resize_content(
        &mut self,
        d2d_context: &ID2D1DeviceContext,
        surface_w_px: f32,
        surface_h_px: f32,
    ) -> windows::core::Result<()> {
        match self {
            Self::WinRtGlass {
                root,
                backdrop,
                shelf_backdrop,
                left_anti_backdrop,
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
                if let Some(layer) = shelf_backdrop {
                    layer.last_state = BackdropState::HIDDEN;
                }
                if let Some(layer) = left_anti_backdrop {
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

    let backdrop = create_winrt_backdrop_layer(&compositor, &root);
    let shelf_backdrop = create_winrt_backdrop_layer(&compositor, &root);
    let left_anti_backdrop = create_winrt_backdrop_layer(&compositor, &root);

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
        shelf_backdrop,
        left_anti_backdrop,
        graphics_device,
        _drawing_surface: drawing_surface,
        surface_brush,
        surface_interop,
        content_visual,
    })
}

fn create_winrt_backdrop_layer(
    compositor: &Compositor,
    root: &ContainerVisual,
) -> Option<WinRtBackdropLayer> {
    let (backdrop_brush, blurred) = create_blurred_host_backdrop_brush(compositor).ok()?;
    let visual = compositor.CreateSpriteVisual().ok()?;
    visual.SetBrush(&backdrop_brush).ok()?;
    let geometry = compositor.CreateRoundedRectangleGeometry().ok()?;
    geometry.SetOffset(Vector2 { X: 0.0, Y: 0.0 }).ok()?;
    let clip = compositor
        .CreateGeometricClipWithGeometry(&geometry)
        .ok()?;
    visual.SetClip(&clip).ok()?;
    visual.SetIsVisible(false).ok()?;
    visual.SetOpacity(0.0).ok()?;
    let backdrop_base: Visual = visual.cast().ok()?;
    root.Children().ok()?.InsertAtTop(&backdrop_base).ok()?;
    Some(WinRtBackdropLayer {
        visual,
        geometry,
        last_state: BackdropState::HIDDEN,
        blur_effect_active: blurred,
    })
}

fn update_layer_geometry(
    layer: &mut WinRtBackdropLayer,
    rect: PillRectDip,
    opacity: f32,
    dpi_scale: f32,
) {
    let visible = opacity > 0.02 && rect.height >= 2.0 && rect.width >= 3.0;
    if !visible {
        hide_layer_geometry(layer);
        return;
    }

    let inset = 0.5_f32;
    let left_px = (rect.left + inset) * dpi_scale;
    let top_px = (rect.top + inset) * dpi_scale;
    let width_px = ((rect.width - inset * 2.0) * dpi_scale).max(1.0);
    let height_px = ((rect.height - inset * 2.0) * dpi_scale).max(1.0);
    let max_radius_px = width_px.min(height_px) * 0.5;
    let radius_px = (rect.corner_radius * dpi_scale).clamp(0.5, max_radius_px);
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

fn hide_layer_geometry(layer: &mut WinRtBackdropLayer) {
    if layer.last_state.visible {
        let _ = layer.visual.SetIsVisible(false);
        let _ = layer.visual.SetOpacity(0.0);
        layer.last_state = BackdropState::HIDDEN;
    }
}

pub(super) fn attach_swapchain_backbuffer(
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

pub(super) fn create_low_power_d3d11_device(
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
