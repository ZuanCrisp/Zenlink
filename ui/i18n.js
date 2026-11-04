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
  "Pick Apple Driver or ZenLink Studio to continue.": "Pilih Apple Driver atau ZenLink Studio untuk lanjut.",
  "Remove the libusb0 filter first": "Hapus filter libusb0 dulu",
  "Install the ZenLink Studio driver": "Pasang driver ZenLink Studio",
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
  "Gestures and buttons need Apple mode. ZenLink Studio carries picture and sound only.":
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
  "Apple Driver connection": "Koneksi Apple Driver",
  "Wired (cable)": "Kabel",
  "Wireless (Wi-Fi)": "Nirkabel (Wi-Fi)",
  "Wired uses the cable. Wireless casts over Wi-Fi after the one-time setup below.": "Kabel memakai kabel USB. Nirkabel memakai Wi-Fi setelah setup sekali di bawah.",
  "Wi-Fi": "Wi-Fi",
  "Wi-Fi isn't set up yet": "Wi-Fi belum diatur",
  "Wireless is chosen. Connect the cable, unlock the device and press Set up Wi-Fi in Settings once, then unplug. The PC and the device must be on the same network. Or switch to Wired.": "Mode nirkabel dipilih. Sambungkan kabel, buka kunci perangkat, lalu tekan Atur Wi-Fi di Pengaturan sekali, setelah itu cabut kabel. PC dan perangkat harus di jaringan yang sama. Atau pilih Kabel.",
  "Stop casting before switching between wired and wireless.": "Hentikan casting sebelum pindah kabel/nirkabel.",
  "Wireless: ZenLink casts over Wi-Fi.": "Nirkabel: ZenLink cast lewat Wi-Fi.",
  "Wired: ZenLink casts over the cable.": "Kabel: ZenLink cast lewat kabel.",
  "The device isn't on Wi-Fi yet. Press Set up Wi-Fi in Settings with the cable connected, keep both on the same network, or switch to Wired.": "Perangkat belum tersambung Wi-Fi. Tekan Atur Wi-Fi di Pengaturan saat kabel tersambung, pastikan di jaringan yang sama, atau pilih Kabel.",
  "Connection": "Koneksi",
  "Cable (USB) or Wi-Fi (AirPlay screen mirroring, no driver needed)": "Kabel (USB) atau Wi-Fi (AirPlay Screen Mirroring, tanpa driver)",
  "Cable: full native resolution, the device's whole screen (measured 2360×1640 on iPad 11th gen), H.264 at high bitrate like TikTok LIVE Studio": "Kabel: resolusi native penuh, seluruh layar perangkat (terukur 2360×1640 di iPad gen 11), H.264 bitrate tinggi seperti TikTok LIVE Studio",
  "Cable needs the libusb0 driver; Apple apps (iTunes, Apple Devices) can't see the device until you switch back": "Kabel butuh driver libusb0; aplikasi Apple (iTunes, Apple Devices) tidak melihat perangkat sampai dikembalikan",
  "Wired uses the cable. Wireless casts over Wi-Fi.": "Kabel memakai kabel USB. Nirkabel lewat Wi-Fi.",
  "Wired uses the cable and the libusb0 driver. Wireless is AirPlay: on the device open Control Center → Screen Mirroring → ZenLink.": "Kabel memakai kabel USB dan driver libusb0. Nirkabel memakai AirPlay: di perangkat buka Pusat Kontrol → Pencerminan Layar → ZenLink.",
  "Wired uses the cable. Wireless casts over Wi-Fi after the one-time setup below.": "Kabel memakai kabel USB. Nirkabel lewat Wi-Fi setelah setup sekali di bawah.",
  "Starting the AirPlay receiver": "Menyalakan penerima AirPlay",
  "Allow ZenLink on the network": "Izinkan ZenLink di jaringan",
  "Approve the Windows permission prompt to let your iPhone/iPad reach ZenLink over Wi-Fi.": "Setujui izin Windows agar iPhone/iPad bisa menjangkau ZenLink lewat Wi-Fi.",
  "ZenLink is allowed on the local network. Start casting, then pick ZenLink in Screen Mirroring.": "ZenLink diizinkan di jaringan lokal. Mulai casting, lalu pilih ZenLink di Pencerminan Layar.",
  "Windows didn't allow it (the permission prompt was declined).": "Windows tidak mengizinkan (permintaan izin ditolak).",
  "The firewall rule wasn't added. Try again.": "Aturan firewall belum ditambahkan. Coba lagi.",
  "Same Wi-Fi": "Wi-Fi yang sama",
  "Connect the iPhone/iPad to the same network as this PC.": "Sambungkan iPhone/iPad ke jaringan yang sama dengan PC ini.",
  "Open Control Center": "Buka Pusat Kontrol",
  "Swipe down from the top-right corner of the screen.": "Usap ke bawah dari pojok kanan atas layar.",
  "Tap Screen Mirroring": "Ketuk Pencerminan Layar",
  "Choose": "Pilih",
  ". The picture appears here.": ". Gambar akan muncul di sini.",
  "Waiting for your iPhone or iPad": "Menunggu iPhone atau iPad kamu",
  "Mirror over Wi-Fi with AirPlay": "Cerminkan lewat Wi-Fi dengan AirPlay",
  "ZenLink is visible on your network now. Follow these steps on the device:": "ZenLink sudah terlihat di jaringan. Ikuti langkah ini di perangkat:",
  "Press Start casting, then follow these steps on the device:": "Tekan Mulai casting, lalu ikuti langkah ini di perangkat:",
  "ZenLink doesn't appear in the list? Settings → Connection & Drivers → Allow ZenLink on the network.": "ZenLink tidak muncul? Pengaturan → Koneksi & Driver → Izinkan ZenLink di jaringan.",
  "No touch control over AirPlay; picture and sound only.": "AirPlay tanpa kontrol sentuh; hanya gambar dan suara.",
  "(TikTok LIVE Studio's wired cast). Apple's service can't see it until that driver is removed: close TikTok LIVE Studio, unplug the cable, uninstall its iOS driver, then plug in again.": "(cast kabel TikTok LIVE Studio). Layanan Apple tidak bisa melihatnya sampai driver itu dilepas: tutup TikTok LIVE Studio, cabut kabel, uninstall driver iOS-nya, lalu colok lagi.",
  "Latency": "Latensi",
  "Resolution · FPS": "Resolusi · FPS",
  "Apple mode: the device's encoder runs at about 4–6 Mbps at most; these presets set a ceiling under it. Applies the next time you start casting.": "Mode Apple: encoder perangkat maksimal sekitar 4–6 Mbps; preset ini memberi batas di bawahnya. Berlaku saat casting berikutnya.",
  "Gestures and buttons beside the picture (Apple Driver mode)": "Gesture dan tombol di samping gambar (mode Apple Driver)",
  "Swipe up from the bottom edge for Home, pause before letting go for the App Switcher. Mouse Back button or Alt+Left goes back. Trackpad scrolls and swipes pages.": "Usap ke atas dari tepi bawah untuk Home, tahan sebentar sebelum dilepas untuk Pengalih Aplikasi. Tombol Back mouse atau Alt+Kiri untuk kembali. Trackpad untuk scroll dan geser halaman.",
  "Auto": "Otomatis",
  "Alt + Left": "Alt + Kiri",
  "App Switcher": "Pengalih Aplikasi",
  "by Zenvy Zenshiro · GPL-3.0": "oleh Zenvy Zenshiro · GPL-3.0",
  "The device uses the ZenLink Studio driver, so Apple mode can't see it. Open Connection & Drivers and press Restore Apple driver, then reconnect the cable.": "Perangkat memakai driver ZenLink Studio, jadi mode Apple tidak bisa melihatnya. Buka Koneksi & Driver, tekan Kembali ke driver Apple, lalu colok ulang kabelnya.",
  "Apple Mobile Device Service isn't running. Install Apple Devices from the Microsoft Store.": "Apple Mobile Device Service tidak berjalan. Pasang Apple Devices dari Microsoft Store.",
  "The device didn't take it.": "Perangkat tidak menerimanya.",
  "Device screen dimmed to 0%. The picture here stays full quality. Alt+D restores it.": "Layar perangkat diredupkan ke 0%. Gambar di sini tetap kualitas penuh. Alt+D untuk mengembalikan.",
  "Saved (hidden) — type to replace": "Tersimpan (tersembunyi) — ketik untuk mengganti",
  "Device connected": "Perangkat tersambung",
  "Your iPhone/iPad chose ZenLinkStudio in Screen Mirroring": "iPhone/iPad kamu memilih ZenLinkStudio di Pencerminan Layar",
  "Receive video": "Terima video",
  "Decrypting the AirPlay stream": "Mendekripsi stream AirPlay",
  "Receiving the screen over Wi-Fi": "Menerima layar lewat Wi-Fi",
  "Found on USB through the libusb0 driver": "Ditemukan di USB lewat driver libusb0",
  "Open the device": "Buka perangkat",
  "Switching it to its screen-capture mode": "Mengalihkan ke mode tangkap layar",
  "Check trust": "Cek kepercayaan",
  "Keep it unlocked while ZenLink proves this PC is trusted": "Biarkan tidak terkunci sementara ZenLink membuktikan PC ini dipercaya",
  "Receiving the screen over ZenLink Studio": "Menerima layar lewat ZenLink Studio",
  "Casting will stop. The new connection mode applies after ZenLink restarts. Continue?": "Casting akan berhenti. Mode koneksi baru berlaku setelah ZenLink restart. Lanjutkan?",
  "Running": "Berjalan",
  "Stopped": "Berhenti",
  "Replaced by libusb0": "Diganti libusb0",
  "Unknown": "Tidak diketahui",
  "Installed (libusb0.sys + libusb0.dll)": "Terpasang (libusb0.sys + libusb0.dll)",
  "Bundled with ZenLink (installs with the driver)": "Disertakan dengan ZenLink (terpasang bersama driver)",
  "Ready to install (package is built on install)": "Siap dipasang (paket dibuat saat pemasangan)",
  "Not in use (ZenLink Studio driver)": "Tidak dipakai (driver ZenLink Studio)",
  "In use, with an unsafe libusb0 filter": "Dipakai, dengan filter libusb0 yang tidak aman",
  "Idle": "Diam",
  "Restore Apple's driver on this device?\n\nZenLink Studio stops working until its driver is installed again. A backup is saved first. Windows will ask for administrator permission. Unplug and reconnect the cable afterwards.": "Kembalikan driver Apple di perangkat ini?\n\nZenLink Studio tidak berfungsi sampai drivernya dipasang lagi. Cadangan disimpan dulu. Windows akan meminta izin administrator. Cabut dan colok ulang kabel setelahnya.",
  "Remove the libusb0 filter from this device?\n\nIt is stacked on Apple's driver, which can crash Windows. Apple's driver stays. A backup is saved first. Windows will ask for administrator permission.": "Lepas filter libusb0 dari perangkat ini?\n\nFilter ini menumpuk di driver Apple dan bisa membuat Windows crash. Driver Apple tetap ada. Cadangan disimpan dulu. Windows akan meminta izin administrator.",
  "Unplug the cable and plug it in again while ZenLink restarts.": "Cabut kabel lalu colok lagi saat ZenLink restart.",
  "Restart ZenLink now": "Restart ZenLink sekarang",
  "Changing the device's driver…": "Mengganti driver perangkat…",
  "Approve the Windows permission prompt. A PowerShell window shows each step; keep the cable plugged in until it finishes.": "Setujui izin Windows. Jendela PowerShell menampilkan tiap langkah; biarkan kabel tersambung sampai selesai.",
  "Driver changed": "Driver diganti",
  "Driver not changed": "Driver tidak diganti",
  "AirPlay picture": "Gambar AirPlay",
  "Smooth: H.264, lowest delay (best for live streaming). Sharp: HEVC, more detail but more delay.": "Halus: H.264, delay paling rendah (terbaik untuk live). Tajam: HEVC, lebih detail tapi delay lebih besar.",
  "Smooth": "Halus",
  "Sharp": "Tajam",
  "Sharp (HEVC): more detail, more delay. Start Screen Mirroring again on the device.": "Tajam (HEVC): lebih detail, delay lebih besar. Mulai Pencerminan Layar lagi di perangkat.",
  "Smooth (H.264): lowest delay. Start Screen Mirroring again on the device.": "Halus (H.264): delay paling rendah. Mulai Pencerminan Layar lagi di perangkat.",
  "Live stats in the header": "Statistik live di header",
  "FPS, video delay and audio delay next to the device name while casting": "FPS, delay video dan delay audio di samping nama perangkat saat casting",
  "AirPlay pairing and FairPlay key exchange": "Pairing AirPlay dan pertukaran kunci FairPlay",
  "Connecting": "Menyambungkan",
  "The device opened screen capture but sent nothing. Restart the iPhone/iPad (turn it off and on), unlock it, keep it on the home screen, then press Start casting again.": "Perangkat membuka tangkap layar tapi tidak mengirim apa pun. Restart iPhone/iPad (matikan lalu nyalakan), buka kuncinya, biarkan di layar utama, lalu tekan Mulai mirroring lagi.",
  "The device did not come back in screen-capture mode. Restart the iPhone/iPad (turn it off and on), plug it in again, unlock it and retry.": "Perangkat tidak kembali ke mode tangkap layar. Restart iPhone/iPad (matikan lalu nyalakan), colok lagi, buka kuncinya lalu coba lagi.",
  "The device did not offer its screen-capture configuration. Unlock it and try again. If it keeps failing, restart the iPhone/iPad.": "Perangkat tidak menyediakan konfigurasi tangkap layar. Buka kuncinya lalu coba lagi. Kalau masih gagal, restart iPhone/iPad.",
  "ZenLink Studio: the device encodes the picture itself. There is no bitrate control in this mode; turn on Show stats bar to see FPS and delay beside the device name.": "ZenLink Studio: perangkat sendiri yang mengenkode gambar. Tidak ada pengaturan bitrate di mode ini; nyalakan Tampilkan bar statistik untuk melihat FPS dan delay di samping nama perangkat.",
  "No device": "Tidak ada perangkat",
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
  "ZenLink Studio on this device": "ZenLink Studio di perangkat ini",
  "Capture backend": "Backend capture",
  "Bitrate (measured)": "Bitrate (terukur)",
  "Check drivers": "Periksa driver",
  "Install ZenLink Studio driver": "Pasang driver ZenLink Studio",
  "Restore Apple driver": "Kembalikan driver Apple",
  "Remove libusb0 filter": "Hapus filter libusb0",
  "Install manually with Zadig": "Pasang manual dengan Zadig",
  "Switch back to Apple driver": "Kembali ke driver Apple",
  "Switch to ZenLink Studio driver (libusb0)": "Ganti ke driver ZenLink Studio",
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
  "Each mode uses its own driver on the device: Apple mode the Apple driver, ZenLink Studio the libusb0 driver (installed as the device's driver, never stacked as a filter). Switching asks for administrator permission; then unplug the cable and plug it in again. A new mode applies after ZenLink restarts.":
    "Tiap mode memakai driver sendiri: mode Apple memakai driver Apple, ZenLink Studio memakai driver libusb0 (dipasang sebagai driver perangkat, tidak pernah sebagai filter). Mengganti meminta izin administrator; lalu cabut dan colok lagi kabelnya. Mode baru berlaku setelah ZenLink restart.",
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
  [/^On your iPhone\/iPad open Control Center, tap Screen Mirroring and choose “(.+)”\. Both must be on the same Wi-Fi network\.$/,
    "Di iPhone/iPad buka Pusat Kontrol, ketuk Pencerminan Layar, lalu pilih “$1”. Keduanya harus di Wi-Fi yang sama."],
  [/^(.+) is connecting over AirPlay…$/, "$1 sedang tersambung lewat AirPlay…"],
  [/^Connecting to (.+)$/, "Menyambungkan ke $1"],
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
