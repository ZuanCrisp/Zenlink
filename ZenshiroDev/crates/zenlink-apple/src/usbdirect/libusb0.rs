//! libusb-win32 (libusb0.dll from System32, paired with the libusb0.sys driver ZenLink Studio
//! installs). The same library the backend used through pyusb's libusb0 backend; struct layouts
//! follow pyusb (packed, 512-byte paths on Windows).

use std::ffi::{c_char, c_int, c_uint, c_void};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use libloading::Library;

const PATH: usize = 512;
/// libusb0 returns -ETIMEDOUT (116) when a transfer times out.
pub const TIMEOUT: i32 = -116;

#[repr(C, packed)]
struct EndpointDesc {
    b_length: u8,
    b_descriptor_type: u8,
    b_endpoint_address: u8,
    bm_attributes: u8,
    w_max_packet_size: u16,
    b_interval: u8,
    b_refresh: u8,
    b_synch_address: u8,
    extra: *const u8,
    extralen: c_int,
}

#[repr(C, packed)]
struct InterfaceDesc {
    b_length: u8,
    b_descriptor_type: u8,
    b_interface_number: u8,
    b_alternate_setting: u8,
    b_num_endpoints: u8,
    b_interface_class: u8,
    b_interface_sub_class: u8,
    b_interface_protocol: u8,
    i_interface: u8,
    endpoint: *const EndpointDesc,
    extra: *const u8,
    extralen: c_int,
}

#[repr(C, packed)]
struct Interface {
    altsetting: *const InterfaceDesc,
    num_altsetting: c_int,
}

#[repr(C, packed)]
struct ConfigDesc {
    b_length: u8,
    b_descriptor_type: u8,
    w_total_length: u16,
    b_num_interfaces: u8,
    b_configuration_value: u8,
    i_configuration: u8,
    bm_attributes: u8,
    b_max_power: u8,
    interface: *const Interface,
    extra: *const u8,
    extralen: c_int,
}

#[repr(C, packed)]
struct DeviceDesc {
    b_length: u8,
    b_descriptor_type: u8,
    bcd_usb: u16,
    b_device_class: u8,
    b_device_sub_class: u8,
    b_device_protocol: u8,
    b_max_packet_size0: u8,
    id_vendor: u16,
    id_product: u16,
    bcd_device: u16,
    i_manufacturer: u8,
    i_product: u8,
    i_serial_number: u8,
    b_num_configurations: u8,
}

#[repr(C, packed)]
struct UsbDevice {
    next: *const UsbDevice,
    prev: *const UsbDevice,
    filename: [c_char; PATH],
    bus: *const UsbBus,
    descriptor: DeviceDesc,
    config: *const ConfigDesc,
    dev: *const c_void,
    devnum: u8,
    num_children: u8,
    children: *const *const UsbDevice,
}

#[repr(C, packed)]
struct UsbBus {
    next: *const UsbBus,
    prev: *const UsbBus,
    dirname: [c_char; PATH],
    devices: *const UsbDevice,
    location: u32,
    root_dev: *const UsbDevice,
}

type Handle = *mut c_void;

struct Api {
    _lib: Library,
    find_busses: unsafe extern "C" fn() -> c_int,
    find_devices: unsafe extern "C" fn() -> c_int,
    get_busses: unsafe extern "C" fn() -> *const UsbBus,
    open: unsafe extern "C" fn(*const UsbDevice) -> Handle,
    close: unsafe extern "C" fn(Handle) -> c_int,
    set_configuration: unsafe extern "C" fn(Handle, c_int) -> c_int,
    claim_interface: unsafe extern "C" fn(Handle, c_int) -> c_int,
    release_interface: unsafe extern "C" fn(Handle, c_int) -> c_int,
    bulk_write: unsafe extern "C" fn(Handle, c_int, *const u8, c_int, c_int) -> c_int,
    bulk_read: unsafe extern "C" fn(Handle, c_int, *mut u8, c_int, c_int) -> c_int,
    control_msg:
        unsafe extern "C" fn(Handle, c_int, c_int, c_int, c_int, *mut u8, c_int, c_int) -> c_int,
    clear_halt: unsafe extern "C" fn(Handle, c_uint) -> c_int,
    get_string_simple: unsafe extern "C" fn(Handle, c_int, *mut c_char, usize) -> c_int,
}

// The function pointers and library handle are immutable after loading.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

static API: OnceLock<Result<Api, String>> = OnceLock::new();
/// libusb0's bus list is global: one enumeration at a time.
static SCAN: Mutex<()> = Mutex::new(());

pub fn dll_path() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(root).join("System32").join("libusb0.dll")
}

fn api() -> Result<&'static Api, String> {
    API.get_or_init(|| unsafe {
        let path = dll_path();
        if !path.exists() {
            return Err(
                "libusb0.dll is not installed. Open Settings > Connection & Drivers.".into(),
            );
        }
        let lib =
            Library::new(&path).map_err(|e| format!("libusb0.dll could not be loaded: {e}"))?;
        macro_rules! f {
            ($n:literal) => {
                *lib.get($n).map_err(|e| format!("libusb0.dll: {e}"))?
            };
        }
        let init: unsafe extern "C" fn() = f!(b"usb_init\0");
        init();
        Ok(Api {
            find_busses: f!(b"usb_find_busses\0"),
            find_devices: f!(b"usb_find_devices\0"),
            get_busses: f!(b"usb_get_busses\0"),
            open: f!(b"usb_open\0"),
            close: f!(b"usb_close\0"),
            set_configuration: f!(b"usb_set_configuration\0"),
            claim_interface: f!(b"usb_claim_interface\0"),
            release_interface: f!(b"usb_release_interface\0"),
            bulk_write: f!(b"usb_bulk_write\0"),
            bulk_read: f!(b"usb_bulk_read\0"),
            control_msg: f!(b"usb_control_msg\0"),
            clear_halt: f!(b"usb_clear_halt\0"),
            get_string_simple: f!(b"usb_get_string_simple\0"),
            _lib: lib,
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub address: u8,
    pub max_packet: u16,
}

#[derive(Clone, Debug)]
pub struct Iface {
    pub number: u8,
    pub class: u8,
    pub subclass: u8,
    pub endpoints: Vec<Endpoint>,
}

impl Iface {
    pub fn ep_in(&self) -> Option<&Endpoint> {
        self.endpoints.iter().find(|e| e.address & 0x80 != 0)
    }

    pub fn ep_out(&self) -> Option<&Endpoint> {
        self.endpoints.iter().find(|e| e.address & 0x80 == 0)
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub value: u8,
    pub interfaces: Vec<Iface>,
}

/// One USB device as libusb0 lists it (a snapshot of its descriptors).
pub struct Device {
    ptr: *const UsbDevice,
    pub configs: Vec<Config>,
}

unsafe impl Send for Device {}

unsafe fn configs(dev: &UsbDevice) -> Vec<Config> {
    let n = dev.descriptor.b_num_configurations as usize;
    let base = dev.config;
    if base.is_null() {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let c = &*base.add(i);
            let ifaces = (0..c.b_num_interfaces as usize)
                .filter_map(|j| {
                    let it = &*c.interface.add(j);
                    if it.altsetting.is_null() || it.num_altsetting < 1 {
                        return None;
                    }
                    let a = &*it.altsetting;
                    let eps = (0..a.b_num_endpoints as usize)
                        .map(|k| {
                            let e = &*a.endpoint.add(k);
                            Endpoint {
                                address: e.b_endpoint_address,
                                max_packet: e.w_max_packet_size,
                            }
                        })
                        .collect();
                    Some(Iface {
                        number: a.b_interface_number,
                        class: a.b_interface_class,
                        subclass: a.b_interface_sub_class,
                        endpoints: eps,
                    })
                })
                .collect();
            Config {
                value: c.b_configuration_value,
                interfaces: ifaces,
            }
        })
        .collect()
}

/// Apple devices on libusb0 with their serial numbers (dashes removed, upper case).
pub fn apple_devices() -> Result<Vec<(String, Device)>, String> {
    let api = api()?;
    let _scan = SCAN.lock().unwrap();
    let mut out = Vec::new();
    unsafe {
        (api.find_busses)();
        (api.find_devices)();
        let mut bus = (api.get_busses)();
        while !bus.is_null() {
            let mut dev = (*bus).devices;
            while !dev.is_null() {
                let d = &*dev;
                let desc = std::ptr::addr_of!(d.descriptor).read_unaligned();
                if desc.id_vendor == 0x05AC {
                    let serial = serial_of(api, dev, desc.i_serial_number);
                    out.push((
                        serial,
                        Device {
                            ptr: dev,
                            configs: configs(d),
                        },
                    ));
                }
                dev = d.next;
            }
            bus = (*bus).next;
        }
    }
    Ok(out)
}

unsafe fn serial_of(api: &Api, dev: *const UsbDevice, index: u8) -> String {
    let h = (api.open)(dev);
    if h.is_null() {
        return String::new();
    }
    let mut buf = [0 as c_char; 256];
    let n = (api.get_string_simple)(h, index as c_int, buf.as_mut_ptr(), buf.len());
    (api.close)(h);
    if n <= 0 {
        return String::new();
    }
    let bytes: Vec<u8> = buf[..n as usize].iter().map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes)
        .trim_matches(['\0', ' ', '\t'])
        .replace('-', "")
        .to_uppercase()
}

pub fn find(serial: &str) -> Result<Option<Device>, String> {
    let want = serial.replace('-', "").to_uppercase();
    Ok(apple_devices()?
        .into_iter()
        .find(|(s, _)| *s == want)
        .map(|(_, d)| d))
}

/// An open device handle; closed on drop.
pub struct Open {
    h: Handle,
}

unsafe impl Send for Open {}
unsafe impl Sync for Open {}

fn check(what: &str, r: c_int) -> Result<c_int, i32> {
    if r < 0 {
        log::debug!("libusb0 {what}: {r}");
        Err(r)
    } else {
        Ok(r)
    }
}

impl Device {
    pub fn open(&self) -> Result<Open, String> {
        let api = api()?;
        let h = unsafe { (api.open)(self.ptr) };
        if h.is_null() {
            return Err("libusb0 could not open the device".into());
        }
        Ok(Open { h })
    }
}

impl Open {
    pub fn set_configuration(&self, value: u8) -> Result<(), i32> {
        let api = api().map_err(|_| -1)?;
        check("set_configuration", unsafe {
            (api.set_configuration)(self.h, value as c_int)
        })
        .map(drop)
    }

    /// GET_CONFIGURATION over the control pipe (libusb0 has no getter).
    pub fn active_configuration(&self) -> Option<u8> {
        let mut b = [0u8; 1];
        (self.control(0x80, 0x08, 0, 0, &mut b, 1000).ok()? == 1).then_some(b[0])
    }

    pub fn claim(&self, iface: u8) -> Result<(), i32> {
        let api = api().map_err(|_| -1)?;
        check("claim_interface", unsafe {
            (api.claim_interface)(self.h, iface as c_int)
        })
        .map(drop)
    }

    pub fn release(&self, iface: u8) {
        if let Ok(api) = api() {
            unsafe { (api.release_interface)(self.h, iface as c_int) };
        }
    }

    pub fn clear_halt(&self, ep: u8) -> Result<(), i32> {
        let api = api().map_err(|_| -1)?;
        check("clear_halt", unsafe {
            (api.clear_halt)(self.h, ep as c_uint)
        })
        .map(drop)
    }

    pub fn control(
        &self,
        req_type: u8,
        req: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
        timeout: u32,
    ) -> Result<usize, i32> {
        let api = api().map_err(|_| -1)?;
        let r = unsafe {
            (api.control_msg)(
                self.h,
                req_type as c_int,
                req as c_int,
                value as c_int,
                index as c_int,
                data.as_mut_ptr(),
                data.len() as c_int,
                timeout as c_int,
            )
        };
        check("control_msg", r).map(|n| n as usize)
    }

    pub fn write(&self, ep: u8, data: &[u8], timeout: u32) -> Result<usize, i32> {
        let api = api().map_err(|_| -1)?;
        let r = unsafe {
            (api.bulk_write)(
                self.h,
                ep as c_int,
                data.as_ptr(),
                data.len() as c_int,
                timeout as c_int,
            )
        };
        check("bulk_write", r).map(|n| n as usize)
    }

    /// Bulk read; `Err(TIMEOUT)` when nothing came in time.
    pub fn read(&self, ep: u8, buf: &mut [u8], timeout: u32) -> Result<usize, i32> {
        let api = api().map_err(|_| -1)?;
        let r = unsafe {
            (api.bulk_read)(
                self.h,
                ep as c_int,
                buf.as_mut_ptr(),
                buf.len() as c_int,
                timeout as c_int,
            )
        };
        if r == TIMEOUT {
            return Err(TIMEOUT);
        }
        check("bulk_read", r).map(|n| n as usize)
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        if let Ok(api) = api() {
            unsafe { (api.close)(self.h) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_pyusb() {
        // ctypes sizes of pyusb's packed structures on 64-bit Windows.
        assert_eq!(std::mem::size_of::<EndpointDesc>(), 9 + 8 + 4);
        assert_eq!(std::mem::size_of::<InterfaceDesc>(), 9 + 8 + 8 + 4);
        assert_eq!(std::mem::size_of::<ConfigDesc>(), 9 + 8 + 8 + 4);
        assert_eq!(std::mem::size_of::<DeviceDesc>(), 18);
        assert_eq!(
            std::mem::offset_of!(UsbDevice, descriptor),
            8 + 8 + PATH + 8
        );
    }
}
