//! Client for the ADB host ("smart socket") protocol.
//!
//! ZenLink does not talk USB to the phone itself. The official Google `adb`
//! server owns the USB and Wi-Fi transports (it is bundled unmodified, see
//! docs/android/ADB.md), and this client speaks its documented host protocol
//! (AOSP `packages/modules/adb/protocol.txt`, `SERVICES.TXT`) over TCP:
//!
//! - request: 4 lowercase hex digits (payload length) + payload;
//! - reply: `OKAY`, or `FAIL` + 4 hex digits + message;
//! - `host:*` services answer with a length-prefixed payload;
//! - after `host:transport:<serial>` the same socket is connected to the
//!   device, and a device service such as `shell:<cmd>` streams until EOF.
//!
//! The server may belong to another tool (Android Studio, scrcpy, a phone
//! suite). [`check_server`] reports a version mismatch instead of killing it.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

/// Default ADB server port (`ANDROID_ADB_SERVER_PORT` overrides it, as in `adb`).
pub const DEFAULT_PORT: u16 = 5037;

/// Host protocol version of the bundled platform-tools 37.0.x `adb`.
pub const BUNDLED_SERVER_VERSION: u32 = 41;

/// Largest length a 4-hex prefix can carry.
const MAX_FRAME: usize = 0xFFFF;

#[derive(Debug)]
pub enum AdbError {
    Io(io::Error),
    /// The server answered `FAIL` with this message.
    Fail(String),
    /// The server sent something that is not the host protocol.
    Protocol(String),
}

impl std::fmt::Display for AdbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "adb: {e}"),
            Self::Fail(m) => write!(f, "adb: {m}"),
            Self::Protocol(m) => write!(f, "adb protocol: {m}"),
        }
    }
}

impl std::error::Error for AdbError {}

impl From<io::Error> for AdbError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, AdbError>;

/// Writes one request frame.
pub fn write_request<W: Write>(w: &mut W, payload: &str) -> Result<()> {
    if payload.len() > MAX_FRAME {
        return Err(AdbError::Protocol(format!(
            "request too long ({} bytes)",
            payload.len()
        )));
    }
    let mut frame = format!("{:04x}", payload.len()).into_bytes();
    frame.extend_from_slice(payload.as_bytes());
    w.write_all(&frame)?;
    w.flush()?;
    Ok(())
}

fn read_hex4<R: Read>(r: &mut R) -> Result<usize> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    let s = std::str::from_utf8(&b).map_err(|_| AdbError::Protocol(format!("bad length {b:?}")))?;
    usize::from_str_radix(s, 16).map_err(|_| AdbError::Protocol(format!("bad length {s:?}")))
}

/// Reads one length-prefixed payload.
pub fn read_frame<R: Read>(r: &mut R) -> Result<Vec<u8>> {
    let n = read_hex4(r)?;
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Reads `OKAY`, or turns `FAIL <msg>` into [`AdbError::Fail`].
pub fn read_status<R: Read>(r: &mut R) -> Result<()> {
    let mut s = [0u8; 4];
    r.read_exact(&mut s)?;
    match &s {
        b"OKAY" => Ok(()),
        b"FAIL" => {
            let msg = read_frame(r)?;
            Err(AdbError::Fail(String::from_utf8_lossy(&msg).into_owned()))
        }
        other => Err(AdbError::Protocol(format!(
            "expected OKAY/FAIL, got {:?}",
            String::from_utf8_lossy(other)
        ))),
    }
}

/// Which device a device service goes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target<'a> {
    Serial(&'a str),
    /// `transport_id:` from `devices -l`; survives serial ambiguity.
    TransportId(u64),
}

impl Target<'_> {
    fn request(&self) -> String {
        match self {
            Target::Serial(s) => format!("host:transport:{s}"),
            Target::TransportId(id) => format!("host:transport-id:{id}"),
        }
    }

    /// Prefix for host services aimed at one device (`forward`, `killforward`).
    fn host_prefix(&self) -> String {
        match self {
            Target::Serial(s) => format!("host-serial:{s}:"),
            Target::TransportId(id) => format!("host-transport-id:{id}:"),
        }
    }
}

/// `STAT` on an open sync socket: the remote file's mode, `None` when it
/// does not exist (the reply's mode, size and mtime are all 0).
pub fn sync_stat<S: Read + Write>(s: &mut S, remote: &str) -> Result<Option<u32>> {
    if remote.len() > 1024 {
        return Err(AdbError::Protocol("remote path too long".into()));
    }
    let mut req = Vec::with_capacity(8 + remote.len());
    req.extend_from_slice(b"STAT");
    req.extend_from_slice(&(remote.len() as u32).to_le_bytes());
    req.extend_from_slice(remote.as_bytes());
    s.write_all(&req)?;
    let mut r = [0u8; 16];
    s.read_exact(&mut r)?;
    if &r[..4] != b"STAT" {
        return Err(AdbError::Protocol("sync: expected STAT".into()));
    }
    let mode = u32::from_le_bytes([r[4], r[5], r[6], r[7]]);
    Ok((mode != 0).then_some(mode))
}

/// Largest DATA chunk of the sync protocol (`SYNC_DATA_MAX`).
const SYNC_DATA_MAX: usize = 64 * 1024;

/// Sends one file over an open `sync:` socket (`SEND`, `DATA`..., `DONE`),
/// waits for the device's reply and ends the sync session with `QUIT`.
/// `mode` is the Unix file mode, e.g. `0o100644`.
pub fn sync_send<S: Read + Write>(
    s: &mut S,
    remote: &str,
    mode: u32,
    data: &[u8],
    mtime: u32,
) -> Result<()> {
    sync_send_from(
        s,
        remote,
        mode,
        &mut io::Cursor::new(data),
        mtime,
        &mut |_| {},
    )
}

/// [`sync_send`] streaming from `src` (a large file never sits in memory),
/// calling `progress(bytes sent)` after every chunk.
pub fn sync_send_from<S: Read + Write>(
    s: &mut S,
    remote: &str,
    mode: u32,
    src: &mut dyn Read,
    mtime: u32,
    progress: &mut dyn FnMut(u64),
) -> Result<()> {
    let spec = format!("{remote},{mode}");
    if spec.len() > 1024 {
        return Err(AdbError::Protocol("remote path too long".into()));
    }
    let mut head = Vec::with_capacity(8 + spec.len());
    head.extend_from_slice(b"SEND");
    head.extend_from_slice(&(spec.len() as u32).to_le_bytes());
    head.extend_from_slice(spec.as_bytes());
    s.write_all(&head)?;
    let mut d = vec![0u8; 8 + SYNC_DATA_MAX];
    let mut sent = 0u64;
    loop {
        // Fill a whole chunk (a short read is not the end of the file).
        let mut n = 0;
        while n < SYNC_DATA_MAX {
            match src.read(&mut d[8 + n..])? {
                0 => break,
                k => n += k,
            }
        }
        if n == 0 {
            break;
        }
        d[..4].copy_from_slice(b"DATA");
        d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        s.write_all(&d[..8 + n])?;
        sent += n as u64;
        progress(sent);
        if n < SYNC_DATA_MAX {
            break;
        }
    }
    let mut done = Vec::with_capacity(8);
    done.extend_from_slice(b"DONE");
    done.extend_from_slice(&mtime.to_le_bytes());
    s.write_all(&done)?;
    s.flush()?;
    let mut reply = [0u8; 8];
    s.read_exact(&mut reply)?;
    let len = u32::from_le_bytes([reply[4], reply[5], reply[6], reply[7]]) as usize;
    let result = match &reply[..4] {
        b"OKAY" => Ok(()),
        b"FAIL" => {
            let mut msg = vec![0u8; len.min(MAX_FRAME)];
            s.read_exact(&mut msg)?;
            Err(AdbError::Fail(String::from_utf8_lossy(&msg).into_owned()))
        }
        other => Err(AdbError::Protocol(format!(
            "sync: expected OKAY/FAIL, got {:?}",
            String::from_utf8_lossy(other)
        ))),
    };
    let _ = s.write_all(b"QUIT\0\0\0\0");
    result
}

#[derive(Clone, Debug)]
pub struct AdbClient {
    pub addr: SocketAddr,
    pub timeout: Duration,
}

impl Default for AdbClient {
    fn default() -> Self {
        let port = std::env::var("ANDROID_ADB_SERVER_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        Self::new(port)
    }
}

impl AdbClient {
    pub fn new(port: u16) -> Self {
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            timeout: Duration::from_secs(5),
        }
    }

    fn connect(&self) -> Result<TcpStream> {
        let s = TcpStream::connect_timeout(&self.addr, self.timeout)?;
        s.set_read_timeout(Some(self.timeout))?;
        s.set_write_timeout(Some(self.timeout))?;
        s.set_nodelay(true)?;
        Ok(s)
    }

    /// Sends a `host:` request and returns its payload.
    pub fn host_query(&self, req: &str) -> Result<Vec<u8>> {
        let mut s = self.connect()?;
        write_request(&mut s, req)?;
        read_status(&mut s)?;
        read_frame(&mut s)
    }

    /// The server's host protocol version (`host:version`).
    pub fn server_version(&self) -> Result<u32> {
        let v = self.host_query("host:version")?;
        let s = String::from_utf8_lossy(&v);
        u32::from_str_radix(s.trim(), 16)
            .map_err(|_| AdbError::Protocol(format!("bad version {s:?}")))
    }

    /// `host:devices-l` text, for [`crate::devices::parse_list`].
    pub fn devices_l(&self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.host_query("host:devices-l")?).into_owned())
    }

    /// Opens `host:track-devices-l`. Each [`DeviceTracker::next_list`] blocks
    /// until the device list changes (the first one comes at once).
    pub fn track_devices(&self) -> Result<DeviceTracker> {
        let mut s = self.connect()?;
        write_request(&mut s, "host:track-devices-l")?;
        read_status(&mut s)?;
        // Updates may be minutes apart; the caller decides when to give up.
        s.set_read_timeout(None)?;
        Ok(DeviceTracker { stream: s })
    }

    /// Opens a socket connected to a device service (e.g. `shell:getprop`).
    pub fn open_service(&self, target: &Target, service: &str) -> Result<TcpStream> {
        let mut s = self.connect()?;
        write_request(&mut s, &target.request())?;
        read_status(&mut s)?;
        write_request(&mut s, service)?;
        read_status(&mut s)?;
        Ok(s)
    }

    /// Runs a read-only shell command and returns its output (stdout and
    /// stderr mixed, as with the legacy shell service), at most `limit` bytes.
    pub fn shell(&self, target: &Target, cmd: &str, limit: usize) -> Result<Vec<u8>> {
        let s = self.open_service(target, &format!("shell:{cmd}"))?;
        let mut out = Vec::new();
        s.take(limit as u64).read_to_end(&mut out)?;
        Ok(out)
    }

    /// An open `sync:` socket with read and write timeouts of at least 15 s,
    /// for [`sync_stat`] and [`sync_send_from`].
    pub fn open_sync(&self, target: &Target) -> Result<TcpStream> {
        let s = self.open_service(target, "sync:")?;
        let slow = self.timeout.max(Duration::from_secs(15));
        s.set_read_timeout(Some(slow))?;
        s.set_write_timeout(Some(slow))?;
        Ok(s)
    }

    /// Copies `data` to `remote` on the device (`adb push`).
    pub fn push(&self, target: &Target, data: &[u8], remote: &str, mode: u32) -> Result<()> {
        let mut s = self.open_service(target, "sync:")?;
        // A slow USB 2 link moves the 700 KB scrcpy server in well under this.
        let slow = self.timeout.max(Duration::from_secs(15));
        s.set_read_timeout(Some(slow))?;
        s.set_write_timeout(Some(slow))?;
        let mtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);
        sync_send(&mut s, remote, mode, data, mtime)
    }

    /// `adb reverse <remote> <local>`: connections the device opens to
    /// `remote` (e.g. `localabstract:scrcpy_0000abcd`) arrive at `local`
    /// (e.g. `tcp:27183`) on this PC.
    pub fn reverse(&self, target: &Target, remote: &str, local: &str) -> Result<()> {
        let mut s = self.open_service(target, &format!("reverse:forward:{remote};{local}"))?;
        read_status(&mut s)
    }

    /// `adb reverse --remove <remote>`.
    pub fn reverse_remove(&self, target: &Target, remote: &str) -> Result<()> {
        let mut s = self.open_service(target, &format!("reverse:killforward:{remote}"))?;
        read_status(&mut s)
    }

    /// `adb forward <local> <remote>`: connections to `local` on this PC
    /// (e.g. `tcp:27183`) go to `remote` on the device.
    pub fn forward(&self, target: &Target, local: &str, remote: &str) -> Result<()> {
        let mut s = self.connect()?;
        let req = format!("{}forward:{local};{remote}", target.host_prefix());
        write_request(&mut s, &req)?;
        // The first OKAY says the server took the request, the second that the forward exists.
        read_status(&mut s)?;
        read_status(&mut s)
    }

    /// `adb forward --remove <local>`.
    pub fn forward_remove(&self, target: &Target, local: &str) -> Result<()> {
        let mut s = self.connect()?;
        write_request(
            &mut s,
            &format!("{}killforward:{local}", target.host_prefix()),
        )?;
        read_status(&mut s)
    }

    /// All system properties of a device (`getprop`).
    pub fn getprop(&self, target: &Target) -> Result<crate::props::Props> {
        Ok(crate::props::Props::parse_bytes(&self.shell(
            target,
            "getprop",
            1 << 20,
        )?))
    }
}

pub struct DeviceTracker {
    stream: TcpStream,
}

impl DeviceTracker {
    pub fn next_list(&mut self) -> Result<String> {
        Ok(String::from_utf8_lossy(&read_frame(&mut self.stream)?).into_owned())
    }

    /// Unblocks a thread waiting in [`Self::next_list`].
    pub fn shutdown(&self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerStatus {
    /// Nothing listens on the port: ZenLink may start its bundled server.
    NotRunning,
    /// A server with the bundled version runs (ours or another tool's): share it.
    Compatible(u32),
    /// Another tool's server with a different version runs. Starting our `adb`
    /// would kill it, so ZenLink must ask the user first (master prompt section 16).
    Mismatch { found: u32, expected: u32 },
    /// Something answers on the port but not with the ADB host protocol.
    Foreign(String),
}

pub fn check_server(client: &AdbClient, expected: u32) -> ServerStatus {
    match client.server_version() {
        Ok(v) if v == expected => ServerStatus::Compatible(v),
        Ok(v) => ServerStatus::Mismatch { found: v, expected },
        Err(AdbError::Io(e))
            if matches!(
                e.kind(),
                io::ErrorKind::ConnectionRefused
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::NotConnected
            ) =>
        {
            ServerStatus::NotRunning
        }
        Err(e) => ServerStatus::Foreign(e.to_string()),
    }
}

/// Starts the bundled `adb` server (`adb start-server`), without a console window.
///
/// Call it only after [`check_server`] returned [`ServerStatus::NotRunning`].
pub fn start_server(adb: &Path, port: u16) -> io::Result<std::process::ExitStatus> {
    let mut cmd = std::process::Command::new(adb);
    cmd.args(["-P", &port.to_string(), "start-server"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.status()
}

#[cfg(test)]
pub(crate) mod fake {
    //! An in-process stand-in for the `adb` server: answers scripted requests.

    use super::*;
    use std::collections::HashMap;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    pub enum Reply {
        /// `OKAY` + length-prefixed payload (host services).
        Frame(Vec<u8>),
        /// `OKAY`, then stay connected for the next request on this socket.
        Okay,
        /// `OKAY` + raw bytes, then close (device services such as shell).
        Stream(Vec<u8>),
        Fail(String),
        /// `OKAY`, then these frames in order (track-devices).
        Frames(Vec<Vec<u8>>),
        /// These exact bytes, then close (e.g. `OKAYOKAY` for forward).
        Raw(Vec<u8>),
    }

    pub struct FakeAdb {
        pub port: u16,
        pub log: Arc<Mutex<Vec<String>>>,
    }

    pub fn frame(s: &str) -> Reply {
        Reply::Frame(s.as_bytes().to_vec())
    }

    impl FakeAdb {
        pub fn start(script: HashMap<String, Reply>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let log = Arc::new(Mutex::new(Vec::new()));
            let log2 = log.clone();
            std::thread::spawn(move || {
                for conn in listener.incoming() {
                    let Ok(mut c) = conn else { break };
                    let script = script.clone();
                    let log = log2.clone();
                    std::thread::spawn(move || {
                        // After `host:transport*:X`, device services are looked up as `service@X` first.
                        let mut transport: Option<String> = None;
                        loop {
                            let Ok(req) = read_frame(&mut c) else { return };
                            let req = String::from_utf8(req).unwrap();
                            log.lock().unwrap().push(req.clone());
                            let scoped = transport
                                .as_ref()
                                .and_then(|t| script.get(&format!("{req}@{t}")));
                            let reply = scoped
                                .or_else(|| script.get(&req))
                                .cloned()
                                .unwrap_or(Reply::Fail(format!("unknown service {req}")));
                            if let Some(t) = req
                                .strip_prefix("host:transport-id:")
                                .or_else(|| req.strip_prefix("host:transport:"))
                            {
                                transport = Some(t.to_string());
                            }
                            let mut out = Vec::new();
                            let close = match reply {
                                Reply::Okay => {
                                    out.extend_from_slice(b"OKAY");
                                    false
                                }
                                Reply::Frame(p) => {
                                    out.extend_from_slice(b"OKAY");
                                    out.extend_from_slice(format!("{:04x}", p.len()).as_bytes());
                                    out.extend_from_slice(&p);
                                    true
                                }
                                Reply::Stream(p) => {
                                    out.extend_from_slice(b"OKAY");
                                    out.extend_from_slice(&p);
                                    true
                                }
                                Reply::Frames(fs) => {
                                    out.extend_from_slice(b"OKAY");
                                    for p in fs {
                                        out.extend_from_slice(
                                            format!("{:04x}", p.len()).as_bytes(),
                                        );
                                        out.extend_from_slice(&p);
                                    }
                                    true
                                }
                                Reply::Raw(p) => {
                                    out.extend_from_slice(&p);
                                    true
                                }
                                Reply::Fail(m) => {
                                    out.extend_from_slice(b"FAIL");
                                    out.extend_from_slice(format!("{:04x}", m.len()).as_bytes());
                                    out.extend_from_slice(m.as_bytes());
                                    true
                                }
                            };
                            if c.write_all(&out).is_err() || close {
                                return;
                            }
                        }
                    });
                }
            });
            Self { port, log }
        }

        pub fn client(&self) -> AdbClient {
            let mut c = AdbClient::new(self.port);
            c.timeout = Duration::from_secs(2);
            c
        }

        pub fn requests(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::*;
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn framing_round_trip() {
        let mut buf = Vec::new();
        write_request(&mut buf, "host:version").unwrap();
        assert_eq!(buf, b"000chost:version");
        assert_eq!(read_frame(&mut &buf[..]).unwrap(), b"host:version");
        assert!(read_status(&mut &b"OKAY"[..]).is_ok());
        match read_status(&mut &b"FAIL0010device not found"[..]) {
            Err(AdbError::Fail(m)) => assert_eq!(m, "device not found"),
            r => panic!("{r:?}"),
        }
        assert!(matches!(
            read_status(&mut &b"HTTP"[..]),
            Err(AdbError::Protocol(_))
        ));
        assert!(matches!(read_frame(&mut &b"zz"[..]), Err(AdbError::Io(_))));
        assert!(matches!(
            read_frame(&mut &b"00g1x"[..]),
            Err(AdbError::Protocol(_))
        ));
    }

    #[test]
    fn version_and_server_check() {
        let fake = FakeAdb::start(HashMap::from([("host:version".to_string(), frame("0029"))]));
        assert_eq!(fake.client().server_version().unwrap(), 41);
        assert_eq!(
            check_server(&fake.client(), 41),
            ServerStatus::Compatible(41)
        );
        assert_eq!(
            check_server(&fake.client(), 40),
            ServerStatus::Mismatch {
                found: 41,
                expected: 40
            }
        );
    }

    #[test]
    fn not_running_when_port_is_closed() {
        // Bind and drop to get a port nothing listens on.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut c = AdbClient::new(port);
        c.timeout = Duration::from_millis(500);
        assert_eq!(check_server(&c, 41), ServerStatus::NotRunning);
    }

    #[test]
    fn shell_goes_through_transport() {
        let fake = FakeAdb::start(HashMap::from([
            ("host:transport:R5CX10ABCDE".to_string(), Reply::Okay),
            (
                "shell:getprop".to_string(),
                Reply::Stream(b"[ro.product.model]: [SM-A576B]\r\n".to_vec()),
            ),
            (
                "host:transport:gone".to_string(),
                Reply::Fail("device 'gone' not found".into()),
            ),
        ]));
        let p = fake
            .client()
            .getprop(&Target::Serial("R5CX10ABCDE"))
            .unwrap();
        assert_eq!(p.get("ro.product.model"), Some("SM-A576B"));
        assert_eq!(
            fake.requests(),
            ["host:transport:R5CX10ABCDE", "shell:getprop"]
        );
        match fake.client().getprop(&Target::Serial("gone")) {
            Err(AdbError::Fail(m)) => assert!(m.contains("not found")),
            r => panic!("{r:?}"),
        }
    }

    /// Records writes and replays a scripted reply, like one sync socket.
    struct Duplex {
        sent: Vec<u8>,
        reply: io::Cursor<Vec<u8>>,
    }

    impl Read for Duplex {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.reply.read(b)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.sent.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn sync_send_chunks_and_reads_reply() {
        let data = vec![7u8; SYNC_DATA_MAX + 10];
        let mut d = Duplex {
            sent: Vec::new(),
            reply: io::Cursor::new(b"OKAY\0\0\0\0".to_vec()),
        };
        sync_send(&mut d, "/data/local/tmp/x.jar", 0o100644, &data, 5).unwrap();
        let spec = b"/data/local/tmp/x.jar,33188";
        let s = &d.sent;
        assert_eq!(&s[..4], b"SEND");
        assert_eq!(&s[4..8], &(spec.len() as u32).to_le_bytes());
        assert_eq!(&s[8..8 + spec.len()], spec);
        let mut i = 8 + spec.len();
        for want in [SYNC_DATA_MAX, 10] {
            assert_eq!(&s[i..i + 4], b"DATA");
            assert_eq!(&s[i + 4..i + 8], &(want as u32).to_le_bytes());
            i += 8 + want;
        }
        assert_eq!(&s[i..i + 8], b"DONE\x05\0\0\0");
        assert_eq!(&s[i + 8..], b"QUIT\0\0\0\0");

        let mut msg = b"FAIL\x0b\0\0\0".to_vec();
        msg.extend_from_slice(b"no space!!!");
        let mut d = Duplex {
            sent: Vec::new(),
            reply: io::Cursor::new(msg),
        };
        match sync_send(&mut d, "/x", 0o100644, b"abc", 0) {
            Err(AdbError::Fail(m)) => assert_eq!(m, "no space!!!"),
            r => panic!("{r:?}"),
        }
    }

    #[test]
    fn reverse_and_forward_requests() {
        let ok2 = || Reply::Raw(b"OKAYOKAY".to_vec());
        let fake = FakeAdb::start(HashMap::from([
            ("host:transport:SER".to_string(), Reply::Okay),
            (
                "reverse:forward:localabstract:scrcpy_0000abcd;tcp:27183".to_string(),
                ok2(),
            ),
            (
                "reverse:killforward:localabstract:scrcpy_0000abcd".to_string(),
                ok2(),
            ),
            (
                "host-serial:SER:forward:tcp:27183;localabstract:scrcpy_0000abcd".to_string(),
                ok2(),
            ),
            (
                "host-transport-id:3:killforward:tcp:27183".to_string(),
                ok2(),
            ),
        ]));
        let c = fake.client();
        let t = Target::Serial("SER");
        c.reverse(&t, "localabstract:scrcpy_0000abcd", "tcp:27183")
            .unwrap();
        c.reverse_remove(&t, "localabstract:scrcpy_0000abcd")
            .unwrap();
        c.forward(&t, "tcp:27183", "localabstract:scrcpy_0000abcd")
            .unwrap();
        c.forward_remove(&Target::TransportId(3), "tcp:27183")
            .unwrap();
        // A device that cannot reverse answers FAIL, so the caller can fall back to forward.
        assert!(c
            .reverse(&Target::Serial("old"), "localabstract:x", "tcp:1")
            .is_err());
    }

    #[test]
    fn transport_id_target() {
        assert_eq!(Target::TransportId(7).request(), "host:transport-id:7");
    }

    #[test]
    fn track_devices_streams_updates() {
        let fake = FakeAdb::start(HashMap::from([(
            "host:track-devices-l".to_string(),
            Reply::Frames(vec![
                b"".to_vec(),
                b"X1 device usb:1-1 transport_id:1\n".to_vec(),
            ]),
        )]));
        let mut t = fake.client().track_devices().unwrap();
        assert_eq!(t.next_list().unwrap(), "");
        assert!(t.next_list().unwrap().starts_with("X1 device"));
        assert!(t.next_list().is_err(), "closed stream ends tracking");
    }
}
