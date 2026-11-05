//! D3D11 device shared by the decoder (through the DXGI device manager) and
//! the renderer, plus the ring of NV12 textures that hands frames between them.

use std::sync::Mutex;

use windows::core::{Interface, Result};
use windows::Win32::Foundation::{HMODULE, RECT};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1};
use windows::Win32::Graphics::Direct3D10::ID3D10Multithread;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{IMFDXGIDeviceManager, MFCreateDXGIDeviceManager};

pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub video_device: ID3D11VideoDevice,
    pub video_context: ID3D11VideoContext,
    pub manager: IMFDXGIDeviceManager,
}

// The device is free-threaded and the immediate context is guarded by
// ID3D10Multithread (SetMultithreadProtected), so both threads may use them.
unsafe impl Send for Gpu {}
unsafe impl Sync for Gpu {}

impl Gpu {
    pub fn create() -> Result<Self> {
        unsafe {
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let device: ID3D11Device = device.unwrap();
            let context: ID3D11DeviceContext = context.unwrap();
            let mt: ID3D10Multithread = context.cast()?;
            let _ = mt.SetMultithreadProtected(true);
            let mut token = 0u32;
            let mut manager = None;
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = manager.unwrap();
            manager.ResetDevice(&device, token)?;
            Ok(Self {
                video_device: device.cast()?,
                video_context: context.cast()?,
                device,
                context,
                manager,
            })
        }
    }

    pub fn nv12_texture(&self, w: u32, h: u32, dynamic: bool) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_NV12,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: if dynamic { D3D11_USAGE_DYNAMIC } else { D3D11_USAGE_DEFAULT },
            // Video-processor inputs must be render-target capable on some drivers (AMD).
            BindFlags: if dynamic {
                D3D11_BIND_SHADER_RESOURCE.0 as u32
            } else {
                (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32
            },
            CPUAccessFlags: if dynamic { D3D11_CPU_ACCESS_WRITE.0 as u32 } else { 0 },
            MiscFlags: 0,
        };
        let mut tex = None;
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut tex))? };
        Ok(tex.unwrap())
    }
}

pub struct Slot {
    pub tex: ID3D11Texture2D,
    pub w: u32,
    pub h: u32,
    /// Visible part of the coded picture (decoders pad to 16 / 32 lines).
    pub src: RECT,
}

/// Triple buffer: the decoder writes a slot that is neither the newest nor
/// the one on screen, then publishes it as the newest.
#[derive(Default)]
pub struct Ring {
    pub slots: [Option<Slot>; 3],
    pub latest: Option<usize>,
    pub showing: Option<usize>,
    pub seq: u64,
    /// The newest frame has not been shown yet.
    pub fresh: bool,
}

unsafe impl Send for Ring {}

pub type SharedRing = std::sync::Arc<Mutex<Ring>>;

impl Ring {
    pub fn free_slot(&self) -> usize {
        (0..3).find(|i| Some(*i) != self.latest && Some(*i) != self.showing).unwrap_or(0)
    }
}
