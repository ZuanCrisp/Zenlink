// ZenLink language layer: Bahasa Indonesia (default) and English.
//
// The UI is written in English. In Indonesian, every visible text node and
// title/aria-label/placeholder whose text is in the dictionary is replaced,
// including text the app sets later (a MutationObserver watches the page),
// and confirm() prompts. Switching back restores the English originals.

const ID = {
  // top bar & idle
  "Off air": "Tidak tayang",
  "Live": "Tayang",
  "Connecting": "Menghubungkan",
  "Preparing": "Menyiapkan",
  "Starting": "Memulai",
  "Failed": "Gagal",
  "Connected": "Terhubung",
  "Couldn't connect": "Gagal terhubung",
  "Start casting": "Mulai mirroring",
  "Stop": "Berhenti",
  "Settings": "Pengaturan",
  "Settings (Ctrl+,)": "Pengaturan (Ctrl+,)",
  "Minimize": "Perkecil",
  "Maximize": "Perbesar",
  "Close": "Tutup",
  "No device connected": "Belum ada perangkat",
  "Rename device": "Ganti nama perangkat",
  "Device name": "Nama perangkat",
  "Plug in your iPhone or iPad": "Sambungkan iPhone atau iPad kamu",
  "Use a USB cable. Screen and sound come through the cable, the same way TikTok LIVE Studio casts iOS.":
    "Pakai kabel USB. Layar dan suara lewat kabel, sama seperti TikTok LIVE Studio.",
  "Use a USB cable, unlock the device and tap Trust when it asks. Set up Wi-Fi once to cast without the cable.":
    "Pakai kabel USB, buka kunci perangkat, lalu ketuk Percayai. Atur Wi-Fi sekali agar bisa tanpa kabel.",
  "Open Connection & Drivers": "Buka Koneksi & Driver",
  "Choose mode": "Pilih mode",
  "Tap Trust": "Ketuk Percayai",
  "Tap Trust on the device": "Ketuk Percayai di perangkat",
  "Unlock it and confirm Trust This Computer with your passcode.":
    "Buka kunci, lalu konfirmasi Percayai Komputer Ini dengan kode sandi.",
  "Unlock it, then press Start casting.": "Buka kunci, lalu tekan Mulai mirroring.",
  "Connected over Wi-Fi. Unlock it, then press Start casting.":
    "Terhubung lewat Wi-Fi. Buka kunci, lalu tekan Mulai mirroring.",
  "Ready": "Siap",
  "Driver": "Driver",
  "Unsafe driver": "Driver tidak aman",
  "Choose a connection mode": "Pilih mode koneksi",
  "Pick Apple Driver or USB Direct to continue.": "Pilih Apple Driver atau USB Direct untuk lanjut.",
  "Remove the libusb0 filter first": "Hapus filter libusb0 dulu",
  "Install the USB Direct driver": "Pasang driver USB Direct",
  "libusb-win32 isn't installed": "libusb-win32 belum terpasang",
  "Restore the Apple driver": "Kembalikan driver Apple",
  "Your iPhone/iPad is using another app's USB driver": "iPhone/iPad kamu memakai driver USB aplikasi lain",
  // connecting checklist
  "Detect device": "Deteksi perangkat",
  "Found through Apple Mobile Device Service": "Ditemukan lewat Apple Mobile Device Service",
  "Secure connection": "Koneksi aman",
  "Opening an encrypted USB tunnel to the device": "Membuka tunnel USB terenkripsi ke perangkat",
  "Prepare the device": "Siapkan perangkat",
  "Keep it unlocked. The first time, a small developer image is installed.":
    "Biarkan tidak terkunci. Pertama kali, image developer kecil akan dipasang.",
  "Start stream": "Mulai stream",
  "Receiving the screen in HEVC": "Menerima layar dalam HEVC",
  "Try again": "Coba lagi",
  // bottom bar
  "Mute": "Bisukan",
  "Mute (M)": "Bisukan (M)",
  "Volume": "Volume",
  "Device": "Perangkat",
  "Resolution": "Resolusi",
  "Quality": "Kualitas",
  "Audio": "Audio",
  "Device controls": "Kontrol perangkat",
  "Gestures and buttons (G)": "Gestur & tombol (G)",
  "Home": "Home",
  "Home button": "Tombol Home",
  "Switch theme": "Ganti tema",
  "Full screen": "Layar penuh",
  "Full screen (F11)": "Layar penuh (F11)",
  "Exit full screen": "Keluar layar penuh",
  // gesture panel
  "Gestures": "Gestur",
  "Go to the Home Screen": "Ke Layar Utama",
  "Show recent apps": "Tampilkan aplikasi terbaru",
  "App switcher": "Pengalih aplikasi",
  "Swipe down from the top left": "Usap ke bawah dari kiri atas",
  "Notifications": "Notifikasi",
  "Swipe down from the top right": "Usap ke bawah dari kanan atas",
  "Control Center": "Pusat Kontrol",
  "Swipe from the right edge": "Usap dari tepi kanan",
  "Swipe": "Usap",
  "Swipe from the left edge (Back)": "Usap dari tepi kiri (Kembali)",
  "Buttons": "Tombol",
  "Lock": "Kunci",
  "Gestures and buttons need Apple mode. USB Direct carries picture and sound only.":
    "Gesture hanya tersedia di mode Apple Driver.",
  "Gestures and buttons need Apple mode.": "Gesture hanya tersedia di mode Apple Driver.",
  // chooser
  "How should ZenLink connect?": "Bagaimana ZenLink terhubung?",
  "Choose the USB connection mode. Nothing connects and no driver changes until you continue.":
    "Pilih mode koneksi. Tidak ada yang tersambung dan tidak ada driver yang diubah sebelum kamu lanjut.",
  "USB connection mode": "Mode koneksi USB",
  "Recommended": "Disarankan",
  "Standard": "Standar",
  "for Live Streaming": "untuk Live Streaming",
  "Standard mode with Apple's driver. Built for compatibility, a stable connection and ease of use.":
    "Mode standar menggunakan driver Apple. Mengutamakan kompatibilitas, koneksi stabil, dan kemudahan penggunaan.",
  "Best quality for live streaming: high bitrate (measured 12–30 Mbps), up to ~56 fps and very low latency, like TikTok LIVE Studio. Needs the libusb0 driver.":
    "Kualitas terbaik untuk livestream: bitrate tinggi (diukur 12–30 Mbps), hingga ~56 fps, latensi sangat rendah, seperti TikTok LIVE Studio. Memerlukan driver libusb0.",
  "HEVC, device-limited to about 4–6 Mbps": "HEVC, dibatasi perangkat sekitar 4–6 Mbps",
  "Mouse touch, keyboard, gestures and buttons": "Sentuh dengan mouse, keyboard, gestur dan tombol",
  "Apple's own driver, nothing to install": "Driver Apple sendiri, tanpa instalasi",
  "Cable or Wi-Fi (after a one-time setup with the cable)": "Kabel atau Wi-Fi (setelah atur sekali lewat kabel)",
  "QuickTime capture over USB, like TikTok LIVE Studio (H.264, higher bitrate)":
    "Capture QuickTime lewat USB, seperti TikTok LIVE Studio (H.264, bitrate lebih tinggi)",
  "Picture and sound only: no touch, keyboard, gestures or buttons":
    "Hanya gambar dan suara: tanpa sentuh, keyboard, gestur, atau tombol",
  "No bitrate setting: the device picks it": "Tanpa pengaturan bitrate: perangkat yang menentukan",
  "Switches the device to the libusb0 driver; Apple apps (iTunes, Apple Devices) can't see it until you switch back":
    "Mengganti driver perangkat ke libusb0; aplikasi Apple (iTunes, Apple Devices) tidak melihatnya sampai dikembalikan",
  "Cable only (no Wi-Fi)": "Hanya kabel (tanpa Wi-Fi)",
  "Remember my choice": "Ingat pilihan saya",
  "You can change the mode any time in Settings → Connection & Drivers. Changing it may need an app restart and reconnecting the USB cable.":
    "Mode koneksi dapat diubah kapan saja melalui Pengaturan → Koneksi & Driver. Mengganti mode mungkin memerlukan restart aplikasi dan mencabut lalu menyambungkan kembali kabel USB.",
  "Cancel": "Batal",
  "Continue": "Lanjut",
  // settings
  "Close settings": "Tutup pengaturan",
  "Close (Esc)": "Tutup (Esc)",
  "Connection & Drivers": "Koneksi & Driver",
  "No mode chosen": "Belum memilih mode",
  "Change": "Ubah",
  "Apple Driver": "Apple Driver",
  "Apple Driver (Standard)": "Apple Driver (Standar)",
  "Remembered for next start.": "Diingat untuk start berikutnya.",
  "Asked again at next start.": "Ditanyakan lagi saat start berikutnya.",
  "Apple USB driver": "Driver USB Apple",
  "USB Direct on this device": "USB Direct di perangkat ini",
  "Capture backend": "Backend capture",
  "Bitrate (measured)": "Bitrate (terukur)",
  "Check drivers": "Periksa driver",
  "Install USB Direct driver": "Pasang driver USB Direct",
  "Restore Apple driver": "Kembalikan driver Apple",
  "Remove libusb0 filter": "Hapus filter libusb0",
  "Install manually with Zadig": "Pasang manual dengan Zadig",
  "Switch back to Apple driver": "Kembali ke driver Apple",
  "Switch to USB Direct driver (libusb0)": "Ganti ke driver USB Direct (libusb0)",
  "Not available": "Tidak tersedia",
  "Not connected": "Tidak terhubung",
  "Not installed": "Belum terpasang",
  "Unknown driver state": "Status driver tidak diketahui",
  "Installed (libusb0 is the device driver)": "Terpasang (libusb0 sebagai driver perangkat)",
  "UNSAFE: libusb0 filter on Apple driver": "TIDAK AMAN: filter libusb0 di driver Apple",
  "Wireless (Wi-Fi)": "Nirkabel (Wi-Fi)",
  "Apple Driver mode can also cast over Wi-Fi. Set it up once with the cable connected and the device unlocked; then unplug. The PC and the device must be on the same network. Wi-Fi adds a little latency.":
    "Mode Apple Driver juga bisa lewat Wi-Fi. Atur sekali dengan kabel tersambung dan perangkat tidak terkunci, lalu cabut. PC dan perangkat harus di jaringan yang sama. Wi-Fi menambah sedikit latensi.",
  "Set up Wi-Fi": "Atur Wi-Fi",
  "Each mode uses its own driver on the device: Apple mode the Apple driver, USB Direct the libusb0 driver (installed as the device's driver, never stacked as a filter). Switching asks for administrator permission; then unplug the cable and plug it in again. A new mode applies after ZenLink restarts.":
    "Tiap mode memakai driver sendiri: mode Apple memakai driver Apple, USB Direct memakai driver libusb0 (dipasang sebagai driver perangkat, tidak pernah sebagai filter). Mengganti meminta izin administrator; lalu cabut dan colok lagi kabelnya. Mode baru berlaku setelah ZenLink restart.",
  "Video quality": "Kualitas video",
  "Best graphics": "Grafis terbaik",
  "Device maximum, same as Xcode": "Maksimum perangkat, sama seperti Xcode",
  "Normal": "Normal",
  "Up to 4 Mbps, steadier": "Hingga 4 Mbps, lebih stabil",
  "Best performance": "Performa terbaik",
  "Performance": "Performa",
  "Up to 2 Mbps, lightest": "Hingga 2 Mbps, paling ringan",
  "Control": "Kontrol",
  "Controls": "Kontrol",
  "Touch with the mouse": "Sentuh dengan mouse",
  "Type with the PC keyboard": "Ketik dengan keyboard PC",
  "Gesture panel": "Panel gestur",
  "Always show": "Selalu tampil",
  "Auto-hide": "Sembunyi otomatis",
  "Hidden": "Disembunyikan",
  "Passcode for Alt+U": "Kode sandi untuk Alt+U",
  "Wake & unlock types this passcode. Stored encrypted for your Windows account only.":
    "Bangunkan & buka kunci mengetik kode sandi ini. Disimpan terenkripsi hanya untuk akun Windows kamu.",
  "Not set": "Belum diatur",
  "Save": "Simpan",
  "Scroll speed": "Kecepatan gulir",
  "Slow": "Lambat",
  "Fast": "Cepat",
  "Display": "Tampilan",
  "Picture fit": "Ukuran gambar",
  "Fit": "Pas",
  "Fill": "Penuh",
  "1:1 pixels": "Piksel 1:1",
  "Fit window to device": "Sesuaikan jendela ke perangkat",
  "Resizes the window to the device's shape when it connects or turns":
    "Mengubah ukuran jendela mengikuti bentuk perangkat saat terhubung atau diputar",
  "Scaling": "Penskalaan",
  "Smooth": "Halus",
  "Sharp": "Tajam",
  "Rotation": "Rotasi",
  "Auto follows the device. Alt+H switches.": "Auto mengikuti perangkat. Alt+H untuk mengganti.",
  "Show stats bar": "Tampilkan bar statistik",
  "Animated background": "Latar animasi",
  "Sound": "Suara",
  "Play sound": "Putar suara",
  "Audio comes from the device over the cable": "Audio dari perangkat lewat kabel",
  "On this PC": "Di PC ini",
  "Device only": "Hanya perangkat",
  "Lip sync": "Sinkron bibir",
  "The picture waits for the sound (Windows needs ~10–50 ms to play audio), so both arrive together":
    "Gambar menunggu suara (Windows butuh ~10–50 ms memutar audio), agar keduanya sinkron",
  "Audio buffer": "Buffer audio",
  "Lower is tighter lip-sync; higher rides out hiccups": "Lebih rendah = lebih sinkron; lebih tinggi = lebih tahan gangguan",
  "Low": "Rendah",
  "High": "Tinggi",
  "Behaviour": "Perilaku",
  "Advanced": "Lanjutan",
  "Start casting automatically": "Mulai mirroring otomatis",
  "When an iPhone or iPad is plugged in": "Saat iPhone atau iPad disambungkan",
  "Close to the tray": "Tutup ke system tray",
  "The close button hides ZenLink in the system tray (it keeps casting, using little memory). Quit from the tray icon.":
    "Tombol tutup menyembunyikan ZenLink di system tray (tetap mirroring, hemat memori). Keluar lewat ikon tray.",
  "Always on top": "Selalu di atas",
  "Theme": "Tema",
  "Dark": "Gelap",
  "Light": "Terang",
  "Language": "Bahasa",
  "Shortcuts": "Pintasan",
  "Leave full screen / close a panel": "Keluar layar penuh / tutup panel",
  "Gestures and buttons panel": "Panel gestur & tombol",
  "Rotate: Auto / 90° / 270°": "Putar: Auto / 90° / 270°",
  "Start / stop casting": "Mulai / hentikan mirroring",
  "Copy / cut / paste on the device": "Salin / potong / tempel di perangkat",
  "Select all / undo on the device": "Pilih semua / urungkan di perangkat",
  "Type the PC clipboard text on the device": "Ketik teks clipboard PC di perangkat",
  "Show / hide the on-screen keyboard": "Tampilkan / sembunyikan keyboard layar",
  "Lock the device": "Kunci perangkat",
  "Wake & unlock (types the saved passcode)": "Bangunkan & buka kunci (mengetik kode sandi tersimpan)",
  "Stealth dim: device screen to 0% / restore": "Redup senyap: layar perangkat 0% / pulihkan",
  "Copy the text on the device screen to the PC (OCR)": "Salin teks di layar perangkat ke PC (OCR)",
  "Clear the copied-text history": "Hapus riwayat teks yang disalin",
  "Back (swipe from the left edge)": "Kembali (usap dari tepi kiri)",
  "Right-click Home": "Klik kanan Home",
  "Device shortcuts work in Apple Driver mode while casting. Space, M and G apply when keys aren't being sent to the device.":
    "Pintasan perangkat berfungsi di mode Apple Driver saat mirroring. Space, M dan G berlaku saat tombol tidak dikirim ke perangkat.",
  "About": "Tentang",
  "Support ZenLink": "Dukung ZenLink",
  "Like the app? Support development on Sociabuzz.": "Suka aplikasinya? Dukung pengembangan di Sociabuzz.",
  "Working…": "Memproses…",
  // toasts
  "Start casting first.": "Mulai mirroring dulu.",
  "Connect the device first.": "Sambungkan perangkat dulu.",
  "Driver status refreshed.": "Status driver diperbarui.",
  "New quality applies the next time you start casting.": "Kualitas baru berlaku saat mirroring berikutnya.",
  "No picture to read yet.": "Belum ada gambar untuk dibaca.",
  "No text found on the screen.": "Tidak ada teks di layar.",
  "Reading the text on the screen…": "Membaca teks di layar…",
  "Nothing new on this screen; the copied text is unchanged.": "Tidak ada yang baru; teks yang disalin tidak berubah.",
  "Copy history cleared. The next Alt+C starts fresh.": "Riwayat salin dihapus. Alt+C berikutnya mulai baru.",
  "Text read, but Windows didn't allow copying it to the clipboard.": "Teks terbaca, tapi Windows tidak mengizinkan menyalin ke clipboard.",
  "Setting up Wi-Fi… keep the device unlocked and on the cable.": "Mengatur Wi-Fi… biarkan perangkat tidak terkunci dan tetap di kabel.",
  "Wi-Fi setup failed.": "Pengaturan Wi-Fi gagal.",
  "Stealth dim needs Apple Driver mode while casting.": "Redup senyap butuh mode Apple Driver saat mirroring.",
  "Stop casting before changing drivers.": "Hentikan mirroring sebelum mengganti driver.",
  "This device's driver doesn't match the chosen mode yet. See Connection & Drivers.":
    "Driver perangkat belum cocok dengan mode yang dipilih. Lihat Koneksi & Driver.",
  "Device brightness restored (~25%).": "Kecerahan perangkat dipulihkan (~25%).",
  "Passcode removed.": "Kode sandi dihapus.",
  "Silent": "Senyap",
};

// Text that includes a name or number.
const ID_PATTERNS = [
  [/^(.+) is ready$/, "$1 siap"],
  [/^(.+) · Wi-Fi$/, "$1 · Wi-Fi"],
  [/^(.+) is saved and applies after a restart\.$/, "$1 tersimpan dan berlaku setelah restart."],
];

const KEY = "zenlink.lang";
let lang = "id";
try { lang = localStorage.getItem(KEY) || "id"; } catch (_) { /* private mode */ }

const original = new WeakMap();      // text node -> English text
const originalAttr = new WeakMap();  // element -> {attr: English}
const ATTRS = ["title", "aria-label", "placeholder"];

function toId(en) {
  const trimmed = en.trim();
  if (!trimmed) return null;
  let out = ID[trimmed];
  if (out === undefined) {
    for (const [re, rep] of ID_PATTERNS) {
      if (re.test(trimmed)) { out = trimmed.replace(re, rep); break; }
    }
  }
  if (out === undefined) return null;
  return en.replace(trimmed, out);
}

function translateText(node) {
  const known = original.get(node);
  // The app replaced the text since we last saw it: that is the new English.
  const current = node.data;
  let en = known;
  if (known === undefined || (current !== known && current !== toId(known))) {
    en = current;
    original.set(node, en);
  }
  const want = lang === "id" ? (toId(en) ?? en) : en;
  if (node.data !== want) node.data = want;
}

function translateAttrs(el) {
  let saved = originalAttr.get(el);
  for (const a of ATTRS) {
    if (!el.hasAttribute(a)) continue;
    const cur = el.getAttribute(a);
    saved = saved || {};
    if (saved[a] === undefined || (cur !== saved[a] && cur !== toId(saved[a]))) saved[a] = cur;
    const want = lang === "id" ? (toId(saved[a]) ?? saved[a]) : saved[a];
    if (cur !== want) el.setAttribute(a, want);
  }
  if (saved) originalAttr.set(el, saved);
}

function walk(root) {
  if (root.nodeType === Node.TEXT_NODE) return translateText(root);
  if (root.nodeType !== Node.ELEMENT_NODE) return;
  if (root.closest && root.closest("script, style, svg, [data-no-i18n]")) return;
  translateAttrs(root);
  const it = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT, {
    acceptNode: (n) => (n.nodeType === Node.ELEMENT_NODE && n.matches("script, style, svg, [data-no-i18n]"))
      ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT,
  });
  for (let n = it.nextNode(); n; n = it.nextNode()) {
    if (n.nodeType === Node.TEXT_NODE) translateText(n);
    else translateAttrs(n);
  }
}

let busy = false;
const observer = new MutationObserver((records) => {
  if (busy) return;
  busy = true;
  try {
    for (const r of records) {
      if (r.type === "characterData") translateText(r.target);
      else if (r.type === "attributes") translateAttrs(r.target);
      else r.addedNodes.forEach(walk);
    }
  } finally {
    busy = false;
  }
});

const nativeConfirm = window.confirm.bind(window);
window.confirm = (msg) => nativeConfirm(lang === "id" ? (toId(String(msg)) ?? msg) : msg);

function apply() {
  document.documentElement.lang = lang;
  document.querySelectorAll("[data-lang]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.lang === lang)));
  busy = true;
  try { walk(document.body); } finally { busy = false; }
}

export function setLang(next) {
  lang = next === "en" ? "en" : "id";
  try { localStorage.setItem(KEY, lang); } catch (_) { /* ignore */ }
  apply();
}

export const getLang = () => lang;
export const t = (en) => (lang === "id" ? (toId(en) ?? en) : en);

export function init() {
  apply();
  observer.observe(document.body, {
    subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS,
  });
  document.querySelectorAll("[data-lang]").forEach((b) => b.addEventListener("click", () => setLang(b.dataset.lang)));
}
