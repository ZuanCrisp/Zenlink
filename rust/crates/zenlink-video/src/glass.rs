//! Liquid Glass layer: a click-through window kept directly under the app
//! window that shows the real background, lightly blurred and refracted at
//! the glass edges (shader in glass.hlsl).
//!
//! Background source (hybrid):
//! - `Desktop`: DXGI Desktop Duplication of the monitor, so windows behind
//!   ZenLink refract too. The app and this layer are excluded from capture
//!   (WDA_EXCLUDEFROMCAPTURE) to avoid sampling themselves; that also hides
//!   ZenLink from screen recorders, so:
//! - `Wallpaper`: used automatically while OBS (or another recorder) runs:
//!   the wallpaper image only, nothing is excluded, OBS sees ZenLink.
//!
//! OBS comes first: this runs on its own D3D11 device at the lowest GPU
//! thread priority, renders only when something changed, at most 60 fps
//! (30 while mirroring), and never touches the video device.

use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::{s, w, Interface, Result, PCWSTR};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{
    ID3DBlob, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST, D3D_DRIVER_TYPE_UNKNOWN,
    D3D_FEATURE_LEVEL_11_0,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Where the glass reads the background from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Source {
    Off = 0,
    Desktop = 1,
    Wallpaper = 2,
}

/// A glass shape, physical px relative to the app window's top-left.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Shape {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    pub bezel: f32,
    pub strength: f32,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Params {
    pub enabled: bool,
    /// 0..1: 0 = clear, 1 = heavily frosted.
    pub thick: f32,
    /// Refraction at the edges on/off.
    pub lens: bool,
    pub dark: bool,
    /// Lower the frame cap (mirroring is running).
    pub busy: bool,
    pub scale: f32,
    /// First shape = the window body (its outline is the glass outline).
    pub shapes: Vec<Shape>,
    /// "auto", "desktop" or "wallpaper".
    pub source: String,
}

pub struct Glass {
    tx: Sender<Params>,
    mode: Arc<AtomicU8>,
}

impl Glass {
    /// Starts the glass thread for the app window `main` (HWND as isize).
    pub fn new(main: isize) -> Self {
        let (tx, rx) = mpsc::channel();
        let mode = Arc::new(AtomicU8::new(Source::Off as u8));
        let m = mode.clone();
        std::thread::Builder::new()
            .name("zl-glass".into())
            .spawn(move || {
                if let Err(e) = run(HWND(main as _), rx, &m) {
                    log::warn!("glass: stopped: {e:?}");
                }
                m.store(Source::Off as u8, Relaxed);
            })
            .ok();
        Self { tx, mode }
    }

    pub fn update(&self, p: Params) {
        let _ = self.tx.send(p);
    }

    /// The source in use right now (`Off` if the layer could not start).
    pub fn source(&self) -> Source {
        match self.mode.load(Relaxed) {
            1 => Source::Desktop,
            2 => Source::Wallpaper,
            _ => Source::Off,
        }
    }
}

/// Screen recorders that must see ZenLink: their presence switches the
/// glass to the wallpaper source (nothing excluded from capture).
const RECORDERS: &[&str] = &[
    "obs64.exe",
    "obs32.exe",
    "obs.exe",
    "streamlabs obs.exe",
    "streamlabs desktop.exe",
    "xsplit.core.exe",
];

fn recorder_running() -> bool {
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return false;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = false;
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let n = String::from_utf16_lossy(
                    &e.szExeFile[..e.szExeFile.iter().position(|&c| c == 0).unwrap_or(0)],
                )
                .to_lowercase();
                if RECORDERS.contains(&n.as_str()) {
                    found = true;
                    break;
                }
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
        found
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Cb {
    win: [f32; 2],
    src: [f32; 2],
    pad: [f32; 2],
    sigma: f32,
    dark: f32,
    dir: [f32; 4],
    light: [f32; 4],
    count: i32,
    lens: f32,
    rim_k: f32,
    scale: f32,
    rc: [[f32; 4]; 8],
    rp: [[f32; 4]; 8],
}

struct Target {
    tex: ID3D11Texture2D,
    rtv: ID3D11RenderTargetView,
    srv: ID3D11ShaderResourceView,
    w: u32,
    h: u32,
}

struct Gfx {
    dev: ID3D11Device,
    ctx: ID3D11DeviceContext,
    vs: ID3D11VertexShader,
    ps_blur: ID3D11PixelShader,
    ps_comp: ID3D11PixelShader,
    cb: ID3D11Buffer,
    smp: ID3D11SamplerState,
    swap: Option<IDXGISwapChain1>,
    sw: (u32, u32),
    _dcomp: IDCompositionDevice,
    _target: IDCompositionTarget,
    visual: IDCompositionVisual,
    dcomp: IDCompositionDevice,
    // Background region (window + pad), and two blur buffers.
    src: Option<Target>,
    a: Option<Target>,
    b: Option<Target>,
}

const PAD: i32 = 48;

fn compile(
    code: &str,
    entry: windows::core::PCSTR,
    target: windows::core::PCSTR,
) -> Result<ID3DBlob> {
    unsafe {
        let mut blob = None;
        let mut err = None;
        let r = D3DCompile(
            code.as_ptr() as _,
            code.len(),
            None,
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut blob,
            Some(&mut err),
        );
        if let Err(e) = r {
            if let Some(err) = err {
                let msg = std::slice::from_raw_parts(
                    err.GetBufferPointer() as *const u8,
                    err.GetBufferSize(),
                );
                log::warn!("glass: shader: {}", String::from_utf8_lossy(msg));
            }
            return Err(e);
        }
        Ok(blob.unwrap())
    }
}

fn blob_bytes(b: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()) }
}

impl Gfx {
    fn new(adapter: &IDXGIAdapter1, layer: HWND) -> Result<Self> {
        unsafe {
            let mut dev = None;
            let mut ctx = None;
            D3D11CreateDevice(
                adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut dev),
                None,
                Some(&mut ctx),
            )?;
            let dev: ID3D11Device = dev.unwrap();
            let ctx: ID3D11DeviceContext = ctx.unwrap();
            // OBS first: this device yields to everything else on the GPU.
            let dxgi: IDXGIDevice = dev.cast()?;
            let _ = dxgi.SetGPUThreadPriority(-7);

            let code = include_str!("glass.hlsl");
            let vsb = compile(code, s!("vs"), s!("vs_5_0"))?;
            let pbb = compile(code, s!("blur"), s!("ps_5_0"))?;
            let pcb = compile(code, s!("comp"), s!("ps_5_0"))?;
            let mut vs = None;
            dev.CreateVertexShader(blob_bytes(&vsb), None, Some(&mut vs))?;
            let mut ps_blur = None;
            dev.CreatePixelShader(blob_bytes(&pbb), None, Some(&mut ps_blur))?;
            let mut ps_comp = None;
            dev.CreatePixelShader(blob_bytes(&pcb), None, Some(&mut ps_comp))?;

            let mut cb = None;
            dev.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: (std::mem::size_of::<Cb>() as u32).div_ceil(16) * 16,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut cb),
            )?;
            let mut smp = None;
            dev.CreateSamplerState(
                &D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MaxLOD: f32::MAX,
                    ..Default::default()
                },
                Some(&mut smp),
            )?;

            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi)?;
            let target = dcomp.CreateTargetForHwnd(layer, true)?;
            let visual = dcomp.CreateVisual()?;
            target.SetRoot(&visual)?;
            Ok(Self {
                dev,
                ctx,
                vs: vs.unwrap(),
                ps_blur: ps_blur.unwrap(),
                ps_comp: ps_comp.unwrap(),
                cb: cb.unwrap(),
                smp: smp.unwrap(),
                swap: None,
                sw: (0, 0),
                _dcomp: dcomp.clone(),
                _target: target,
                visual,
                dcomp,
                src: None,
                a: None,
                b: None,
            })
        }
    }

    fn target(&self, w: u32, h: u32, rt: bool) -> Result<Target> {
        unsafe {
            let mut tex = None;
            self.dev.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: w.max(1),
                    Height: h.max(1),
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_SHADER_RESOURCE.0
                        | if rt { D3D11_BIND_RENDER_TARGET.0 } else { 0 })
                        as u32,
                    ..Default::default()
                },
                None,
                Some(&mut tex),
            )?;
            let tex: ID3D11Texture2D = tex.unwrap();
            let mut srv = None;
            self.dev
                .CreateShaderResourceView(&tex, None, Some(&mut srv))?;
            let mut rtv = None;
            // The source copy target needs an RTV only to be cleared.
            self.dev
                .CreateRenderTargetView(&tex, None, Some(&mut rtv))
                .ok();
            Ok(Target {
                rtv: rtv.unwrap_or_else(unreachable_rtv),
                srv: srv.unwrap(),
                tex,
                w,
                h,
            })
        }
    }

    fn ensure_sizes(&mut self, ww: u32, wh: u32, f: u32) -> Result<()> {
        let (sw, sh) = (ww + 2 * PAD as u32, wh + 2 * PAD as u32);
        if self.src.as_ref().map(|t| (t.w, t.h)) != Some((sw, sh)) {
            self.src = Some(self.target(sw, sh, true)?);
            unsafe {
                self.ctx
                    .ClearRenderTargetView(&self.src.as_ref().unwrap().rtv, &[0.0, 0.0, 0.0, 1.0])
            };
        }
        let (bw, bh) = (sw.div_ceil(f), sh.div_ceil(f));
        if self.a.as_ref().map(|t| (t.w, t.h)) != Some((bw, bh)) {
            self.a = Some(self.target(bw, bh, true)?);
            self.b = Some(self.target(bw, bh, true)?);
        }
        if self.sw != (ww, wh) || self.swap.is_none() {
            unsafe {
                if let Some(sc) = &self.swap {
                    sc.ResizeBuffers(
                        2,
                        ww,
                        wh,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )?;
                } else {
                    let dxgi: IDXGIDevice = self.dev.cast()?;
                    let factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
                    let sc = factory.CreateSwapChainForComposition(
                        &self.dev,
                        &DXGI_SWAP_CHAIN_DESC1 {
                            Width: ww,
                            Height: wh,
                            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                            SampleDesc: DXGI_SAMPLE_DESC {
                                Count: 1,
                                Quality: 0,
                            },
                            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                            BufferCount: 2,
                            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                            ..Default::default()
                        },
                        None,
                    )?;
                    self.visual.SetContent(&sc)?;
                    self.dcomp.Commit()?;
                    self.swap = Some(sc);
                }
            }
            self.sw = (ww, wh);
        }
        Ok(())
    }

    fn pass(
        &self,
        ps: &ID3D11PixelShader,
        input: &ID3D11ShaderResourceView,
        out: &ID3D11RenderTargetView,
        w: u32,
        h: u32,
        cb: &Cb,
    ) {
        unsafe {
            self.ctx
                .UpdateSubresource(&self.cb, 0, None, cb as *const _ as _, 0, 0);
            self.ctx
                .OMSetRenderTargets(Some(&[Some(out.clone())]), None);
            self.ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: w as f32,
                Height: h as f32,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            self.ctx
                .IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.ctx.VSSetShader(&self.vs, None);
            self.ctx.PSSetShader(ps, None);
            self.ctx
                .PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            self.ctx.PSSetSamplers(0, Some(&[Some(self.smp.clone())]));
            self.ctx
                .PSSetShaderResources(0, Some(&[Some(input.clone())]));
            self.ctx.Draw(3, 0);
            self.ctx.PSSetShaderResources(0, Some(&[None]));
            self.ctx.OMSetRenderTargets(None, None);
        }
    }

    /// Blur `src` by `sigma` px, then composite the glass into the swapchain.
    fn render(&mut self, p: &Params, ww: u32, wh: u32) -> Result<()> {
        let sigma = p.thick * 28.0 * p.scale.max(1.0) / 1.5;
        let f: u32 = if sigma > 10.0 {
            4
        } else if sigma > 3.0 {
            2
        } else {
            1
        };
        self.ensure_sizes(ww, wh, f)?;
        let src = self.src.as_ref().unwrap();
        let (a, b) = (self.a.as_ref().unwrap(), self.b.as_ref().unwrap());
        let mut cb = Cb {
            win: [ww as f32, wh as f32],
            src: [src.w as f32, src.h as f32],
            pad: [PAD as f32, PAD as f32],
            dark: if p.dark { 1.0 } else { 0.0 },
            light: [-0.6, -0.8, 0.0, 0.0],
            lens: if p.lens { 1.0 } else { 0.0 },
            rim_k: if p.dark { 0.28 } else { 0.45 },
            scale: p.scale,
            ..Default::default()
        };
        for (i, s) in p.shapes.iter().take(8).enumerate() {
            cb.rc[i] = [s.x + s.w / 2.0, s.y + s.h / 2.0, s.w / 2.0, s.h / 2.0];
            cb.rp[i] = [
                s.radius.min(s.w / 2.0).min(s.h / 2.0),
                s.bezel,
                s.strength,
                0.0,
            ];
        }
        cb.count = p.shapes.len().min(8) as i32;
        let input = if sigma >= 0.5 {
            let s = sigma / f as f32;
            let mut h = cb;
            h.sigma = s;
            h.dir = [f as f32 / src.w as f32, 0.0, 0.0, 0.0];
            self.pass(&self.ps_blur, &src.srv, &a.rtv, a.w, a.h, &h);
            let mut v = cb;
            v.sigma = s;
            v.dir = [0.0, 1.0 / a.h as f32, 0.0, 0.0];
            self.pass(&self.ps_blur, &a.srv, &b.rtv, b.w, b.h, &v);
            b.srv.clone()
        } else {
            src.srv.clone()
        };
        unsafe {
            let sc = self.swap.as_ref().unwrap();
            let back: ID3D11Texture2D = sc.GetBuffer(0)?;
            let mut rtv = None;
            self.dev
                .CreateRenderTargetView(&back, None, Some(&mut rtv))?;
            let rtv = rtv.unwrap();
            self.pass(&self.ps_comp, &input, &rtv, ww, wh, &cb);
            sc.Present(1, DXGI_PRESENT(0)).ok()?;
        }
        Ok(())
    }
}

fn unreachable_rtv() -> ID3D11RenderTargetView {
    panic!("render target view could not be created")
}

/// The background for one monitor: either a live duplication or a static
/// wallpaper texture of the monitor's size.
enum Backdrop {
    Dup {
        dup: IDXGIOutputDuplication,
        frame: Option<ID3D11Texture2D>,
        origin: (i32, i32),
    },
    Paper {
        tex: ID3D11Texture2D,
        origin: (i32, i32),
        size: (i32, i32),
        key: String,
    },
}

fn find_output(main: HWND) -> Result<(IDXGIAdapter1, IDXGIOutput1, RECT)> {
    unsafe {
        let mon = MonitorFromWindow(main, MONITOR_DEFAULTTONEAREST);
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut i = 0;
        while let Ok(ad) = factory.EnumAdapters1(i) {
            let mut j = 0;
            while let Ok(out) = ad.EnumOutputs(j) {
                let d = out.GetDesc()?;
                if d.Monitor == mon {
                    return Ok((ad, out.cast()?, d.DesktopCoordinates));
                }
                j += 1;
            }
            i += 1;
        }
        Err(windows::core::Error::from_hresult(DXGI_ERROR_NOT_FOUND))
    }
}

/// Current wallpaper file and fit style ("10" fill, "6" fit, "2" stretch,
/// "0" centre/tile, "22" span), from the registry.
fn wallpaper_info() -> Option<(String, u32)> {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let read = |name: PCWSTR| -> Option<String> {
        let mut buf = [0u16; 520];
        let mut n = (buf.len() * 2) as u32;
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Control Panel\\Desktop"),
                name,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as _),
                Some(&mut n),
            )
            .ok()
            .ok()?;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
        Some(String::from_utf16_lossy(&buf[..len]))
    };
    let path = read(w!("WallPaper"))?;
    let style = read(w!("WallpaperStyle"))
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(10);
    Some((path, style))
}

/// Wallpaper drawn the way Windows draws it, into a monitor-size BGRA buffer.
fn wallpaper_pixels(path: &str, style: u32, mw: i32, mh: i32) -> Option<Vec<u8>> {
    unsafe {
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        let dec = wic
            .CreateDecoderFromFilename(
                PCWSTR(wide.as_ptr()),
                None,
                windows::Win32::Foundation::GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )
            .ok()?;
        let frame = dec.GetFrame(0).ok()?;
        let conv = WICConvertBitmapSource(&GUID_WICPixelFormat32bppPBGRA, &frame).ok()?;
        let (mut iw, mut ih) = (0u32, 0u32);
        conv.GetSize(&mut iw, &mut ih).ok()?;
        let (fw, fh) = (iw as f32, ih as f32);
        let (sx, sy) = (mw as f32 / fw, mh as f32 / fh);
        // Scaled size of the image on the monitor.
        let (tw, th) = match style {
            2 => (mw as f32, mh as f32), // stretch
            6 => {
                let k = sx.min(sy);
                (fw * k, fh * k)
            } // fit
            0 => (fw, fh),               // centre
            _ => {
                let k = sx.max(sy);
                (fw * k, fh * k)
            } // fill / span
        };
        let (tw, th) = (tw.round().max(1.0) as u32, th.round().max(1.0) as u32);
        let scaler = wic.CreateBitmapScaler().ok()?;
        scaler
            .Initialize(&conv, tw, th, WICBitmapInterpolationModeFant)
            .ok()?;
        let mut img = vec![0u8; (tw * th * 4) as usize];
        scaler.CopyPixels(std::ptr::null(), tw * 4, &mut img).ok()?;
        // Background colour around a fit/centred image.
        let bg = windows::Win32::Graphics::Gdi::GetSysColor(
            windows::Win32::Graphics::Gdi::COLOR_BACKGROUND,
        );
        let (r, g, b) = (
            (bg & 0xff) as u8,
            ((bg >> 8) & 0xff) as u8,
            ((bg >> 16) & 0xff) as u8,
        );
        let mut out = vec![0u8; (mw * mh * 4) as usize];
        for px in out.chunks_exact_mut(4) {
            px.copy_from_slice(&[b, g, r, 255]);
        }
        let (ox, oy) = ((mw - tw as i32) / 2, (mh - th as i32) / 2);
        for y in 0..mh {
            let iy = y - oy;
            if iy < 0 || iy >= th as i32 {
                continue;
            }
            let x0 = ox.max(0);
            let x1 = (ox + tw as i32).min(mw);
            if x1 <= x0 {
                continue;
            }
            let s = ((iy as u32 * tw + (x0 - ox) as u32) * 4) as usize;
            let d = ((y * mw + x0) * 4) as usize;
            let n = ((x1 - x0) * 4) as usize;
            out[d..d + n].copy_from_slice(&img[s..s + n]);
        }
        Some(out)
    }
}

unsafe extern "system" fn layer_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCHITTEST {
        return LRESULT(HTTRANSPARENT as isize);
    }
    unsafe { DefWindowProcW(h, msg, wp, lp) }
}

fn create_layer() -> Result<HWND> {
    unsafe {
        let inst = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            lpfnWndProc: Some(layer_proc),
            hInstance: inst.into(),
            lpszClassName: w!("ZenLinkGlass"),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let h = CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            w!("ZenLinkGlass"),
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(inst.into()),
            None,
        )?;
        let corner: u32 = 1; // DWMWCP_DONOTROUND: the shader draws the outline
        let _ = DwmSetWindowAttribute(
            h,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as _,
            4,
        );
        Ok(h)
    }
}

fn run(main: HWND, rx: Receiver<Params>, mode: &AtomicU8) -> Result<()> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let layer = create_layer()?;
    let (adapter, mut output, mut mon) = find_output(main)?;
    let mut gfx = Gfx::new(&adapter, layer)?;
    let mut params: Option<Params> = None;
    let mut backdrop: Option<Backdrop> = None;
    let mut recorder = recorder_running();
    let mut recorder_at = Instant::now();
    let mut last_rect = RECT::default();
    let mut dirty = true;
    let mut shown = false;
    let mut last_frame = Instant::now() - Duration::from_secs(1);
    loop {
        // Window messages for the layer window.
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let busy = params.as_ref().map(|p| p.busy).unwrap_or(false);
        let interval = Duration::from_millis(if busy { 33 } else { 16 });
        match rx.recv_timeout(interval) {
            Ok(p) => {
                let mut latest = p;
                while let Ok(n) = rx.try_recv() {
                    latest = n;
                }
                if params.as_ref() != Some(&latest) {
                    params = Some(latest);
                    dirty = true;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if recorder_at.elapsed() > Duration::from_secs(2) {
            recorder_at = Instant::now();
            let r = recorder_running();
            if r != recorder {
                recorder = r;
                backdrop = None;
                dirty = true;
            }
        }
        let Some(p) = params.as_ref() else { continue };
        let visible = unsafe { IsWindowVisible(main).as_bool() && !IsIconic(main).as_bool() };
        if !p.enabled || !visible {
            if shown {
                unsafe {
                    let _ = ShowWindow(layer, SW_HIDE);
                    let _ = SetWindowDisplayAffinity(layer, WDA_NONE);
                }
                shown = false;
            }
            backdrop = None;
            mode.store(Source::Off as u8, Relaxed);
            continue;
        }
        let want = match p.source.as_str() {
            "desktop" => Source::Desktop,
            "wallpaper" => Source::Wallpaper,
            _ if recorder => Source::Wallpaper,
            _ => Source::Desktop,
        };

        // Follow the app window: same rect, directly below it.
        let mut rc = RECT::default();
        unsafe {
            let _ = GetWindowRect(main, &mut rc);
        }
        let (ww, wh) = (
            (rc.right - rc.left).max(1) as u32,
            (rc.bottom - rc.top).max(1) as u32,
        );
        let below_ok = unsafe { GetWindow(main, GW_HWNDNEXT).ok() == Some(layer) };
        if rc != last_rect || !below_ok || !shown {
            unsafe {
                let _ = SetWindowPos(
                    layer,
                    Some(main),
                    rc.left,
                    rc.top,
                    ww as i32,
                    wh as i32,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
                );
            }
            shown = true;
            if rc != last_rect {
                dirty = true;
            }
            last_rect = rc;
        }

        // Monitor changes: new output (and maybe a new adapter: restart).
        let mi = unsafe {
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(MonitorFromWindow(main, MONITOR_DEFAULTTONEAREST), &mut mi);
            mi.rcMonitor
        };
        if mi != mon {
            let (ad, out, m) = find_output(main)?;
            if unsafe { ad.GetDesc1()?.AdapterLuid != adapter.GetDesc1()?.AdapterLuid } {
                return Err(windows::core::Error::from_hresult(
                    DXGI_ERROR_DEVICE_REMOVED,
                ));
            }
            output = out;
            mon = m;
            backdrop = None;
            dirty = true;
        }

        // Background source.
        let kind_ok = matches!(
            (&backdrop, want),
            (Some(Backdrop::Dup { .. }), Source::Desktop)
                | (Some(Backdrop::Paper { .. }), Source::Wallpaper)
        );
        if !kind_ok {
            backdrop = None;
            unsafe {
                // Desktop: this layer must not see itself. Wallpaper: recorders may see it.
                let _ = SetWindowDisplayAffinity(
                    layer,
                    if want == Source::Desktop {
                        WDA_EXCLUDEFROMCAPTURE
                    } else {
                        WDA_NONE
                    },
                );
            }
            match want {
                Source::Desktop => {
                    match unsafe { output.DuplicateOutput(&gfx.dev) } {
                        Ok(dup) => {
                            backdrop = Some(Backdrop::Dup {
                                dup,
                                frame: None,
                                origin: (mon.left, mon.top),
                            })
                        }
                        Err(e) => {
                            log::warn!("glass: desktop duplication unavailable ({e:?}), using the wallpaper");
                            params.as_mut().unwrap().source = "wallpaper".into();
                            continue;
                        }
                    }
                }
                _ => {
                    let (mw, mh) = (mon.right - mon.left, mon.bottom - mon.top);
                    let (path, style) = wallpaper_info().unwrap_or_default();
                    let px = wallpaper_pixels(&path, style, mw, mh)
                        .unwrap_or_else(|| vec![0x20; (mw * mh * 4) as usize]);
                    let mut tex = None;
                    unsafe {
                        gfx.dev.CreateTexture2D(
                            &D3D11_TEXTURE2D_DESC {
                                Width: mw as u32,
                                Height: mh as u32,
                                MipLevels: 1,
                                ArraySize: 1,
                                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                                SampleDesc: DXGI_SAMPLE_DESC {
                                    Count: 1,
                                    Quality: 0,
                                },
                                Usage: D3D11_USAGE_DEFAULT,
                                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                                ..Default::default()
                            },
                            Some(&D3D11_SUBRESOURCE_DATA {
                                pSysMem: px.as_ptr() as _,
                                SysMemPitch: (mw * 4) as u32,
                                SysMemSlicePitch: 0,
                            }),
                            Some(&mut tex),
                        )?;
                    }
                    backdrop = Some(Backdrop::Paper {
                        tex: tex.unwrap(),
                        origin: (mon.left, mon.top),
                        size: (mw, mh),
                        key: format!("{path}|{style}"),
                    });
                }
            }
            dirty = true;
        }
        mode.store(want as u8, Relaxed);

        // New desktop frame?
        if let Some(Backdrop::Dup { dup, frame, .. }) = backdrop.as_mut() {
            unsafe {
                let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
                let mut res = None;
                match dup.AcquireNextFrame(0, &mut info, &mut res) {
                    Ok(()) => {
                        if info.LastPresentTime != 0 || frame.is_none() {
                            if let Some(r) = res {
                                let t: ID3D11Texture2D = r.cast()?;
                                // Keep our own copy so the frame can be released at once.
                                let mut d = D3D11_TEXTURE2D_DESC::default();
                                t.GetDesc(&mut d);
                                let need = match frame {
                                    Some(f) => {
                                        let mut fd = D3D11_TEXTURE2D_DESC::default();
                                        f.GetDesc(&mut fd);
                                        (fd.Width, fd.Height) != (d.Width, d.Height)
                                    }
                                    None => true,
                                };
                                if need {
                                    let mut c = None;
                                    gfx.dev.CreateTexture2D(
                                        &D3D11_TEXTURE2D_DESC {
                                            BindFlags: 0,
                                            MiscFlags: 0,
                                            CPUAccessFlags: 0,
                                            Usage: D3D11_USAGE_DEFAULT,
                                            ..d
                                        },
                                        None,
                                        Some(&mut c),
                                    )?;
                                    *frame = c;
                                }
                                gfx.ctx.CopyResource(frame.as_ref().unwrap(), &t);
                                dirty = true;
                            }
                        }
                        let _ = dup.ReleaseFrame();
                    }
                    Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {}
                    Err(_) => {
                        // Access lost (mode switch, secure desktop): reopen.
                        backdrop = None;
                        continue;
                    }
                }
            }
        }
        if let Some(Backdrop::Paper { key, .. }) = &backdrop {
            if recorder_at.elapsed() < interval {
                // Checked with the recorder poll: reload on wallpaper change.
                let now = wallpaper_info()
                    .map(|(p, s)| format!("{p}|{s}"))
                    .unwrap_or_default();
                if &now != key {
                    backdrop = None;
                    continue;
                }
            }
        }
        if !dirty || last_frame.elapsed() < interval {
            continue;
        }

        // Copy the window's region (plus margin) of the background.
        let (tex, origin, size) = match &backdrop {
            Some(Backdrop::Dup {
                frame: Some(f),
                origin,
                ..
            }) => (
                f.clone(),
                *origin,
                (mon.right - mon.left, mon.bottom - mon.top),
            ),
            Some(Backdrop::Paper {
                tex, origin, size, ..
            }) => (tex.clone(), *origin, *size),
            _ => continue,
        };
        let sf = p.scale;
        let _ = sf;
        gfx.ensure_sizes(ww, wh, 1)?;
        let src = gfx.src.as_ref().unwrap();
        let (x0, y0) = (rc.left - PAD - origin.0, rc.top - PAD - origin.1);
        let bx0 = x0.max(0);
        let by0 = y0.max(0);
        let bx1 = (x0 + src.w as i32).min(size.0);
        let by1 = (y0 + src.h as i32).min(size.1);
        if bx1 > bx0 && by1 > by0 {
            unsafe {
                gfx.ctx.CopySubresourceRegion(
                    &src.tex,
                    0,
                    (bx0 - x0) as u32,
                    (by0 - y0) as u32,
                    0,
                    &tex,
                    0,
                    Some(&D3D11_BOX {
                        left: bx0 as u32,
                        top: by0 as u32,
                        front: 0,
                        right: bx1 as u32,
                        bottom: by1 as u32,
                        back: 1,
                    }),
                );
            }
        }
        if let Err(e) = gfx.render(p, ww, wh) {
            log::warn!("glass: render: {e:?}");
        }
        dirty = false;
        last_frame = Instant::now();
    }
    unsafe {
        let _ = DestroyWindow(layer);
    }
    Ok(())
}
