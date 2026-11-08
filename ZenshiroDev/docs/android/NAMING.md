# Android device names and identity (phase A1)

Code: `ZenshiroDev/crates/zenlink-android/src/naming.rs`, `identity.rs`, `discovery.rs`.

## Name resolution

1. Properties are read (`getprop`, read-only) **only** for devices in state `device`.
   Unauthorized, offline, recovery and no-permission devices get the placeholder
   "Android device (<adb serial>)" and name source "placeholder".
2. Marketing name, first non-empty of:
   `ro.product.marketname`, `ro.product.vendor.marketname`,
   `ro.product.odm.marketname`, `ro.vendor.oplus.market.name`,
   `ro.oppo.market.name`, `ro.vivo.market.name`, `ro.config.marketing_name`.
   Only `ro.product.marketname` is named by the master prompt. The others are read
   only when present; **which vendors and Android versions set them is not verified
   on hardware** yet.
3. Otherwise `ro.product.model`. If it is one word with a digit it is a model code
   ("Samsung SM-A576B"), source "model code"; else source "model" ("Google Pixel 8").
4. Brand: `ro.product.brand` (carries sub-brands such as Redmi and POCO), else
   `ro.product.manufacturer`. Canonical spelling from a short table (Samsung,
   OPPO, vivo, realme, TECNO, POCO, HONOR, ...). The brand is put in front unless
   the name already starts with a known brand; doubled prefixes are removed.
5. No model-code database: a code is never turned into a product name.
6. Cleaning: control and zero-width characters, U+FFFD and surrounding quotes
   removed, whitespace collapsed, placeholders ("unknown", "null", "n/a",
   "default", ...) treated as missing.
7. A user nickname is shown first, the detected name below it (`naming::label`).
8. If properties cannot be read from an authorized device, the `model:` token of
   `adb devices -l` is shown as reported (ADB replaces spaces and `-` with `_`).

## Identity

- Strong: `android/<serial>` from `ro.serialno`, else `ro.boot.serialno`, else the
  serial the transport reveals (USB serial, or the one inside the Android 11+
  wireless-debugging mDNS name `adb-<serial>-<suffix>._adb-tls-connect._tcp`).
  Same key over USB and Wi-Fi.
- Rejected serials: shorter than 6, non-alphanumeric, one repeated character, or
  shared vendor defaults such as `0123456789ABCDEF`.
- Weak (no usable serial): `android-weak/<model>/<adb serial>`, or
  `android-weak/<model>/wifi` for `adb connect` devices. Never an IP address or a
  transport id. Weak keys are never merged across transports.
- One device on USB and Wi-Fi at once appears once; USB is preferred.

## Readiness

`Ready` (authorized, API ≥ 21), `NeedsAuthorization`, `Offline`, `NoPermission`,
`NotAndroidSystem`, `TooOld { sdk }`, `Unknown` (authorized, properties not read).
Audio needs API ≥ 30.

## Tests

All fixtures are synthetic. Covered: Samsung-style codes, TECNO marketing name and
board id, Xiaomi/Redmi/POCO, missing marketname, duplicate brand prefix, tablet,
unauthorized/offline, identical models, USB-to-Wi-Fi reconnect, malformed Unicode
and whitespace, and a fake ADB server (`adb::fake`). Real-device names are
**unverified** until tested on hardware.
