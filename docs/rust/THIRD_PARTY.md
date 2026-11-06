# Third-party references (ZenLink Next)

Motion constants and Liquid Glass optics are adapted from these projects.
No code is copied verbatim; values and formulas are re-implemented.

- **shayann07/liquidglass** (Apache-2.0, see its NOTICE): press/release
  spring fits (press 1.04 swell, ~4-10 % release bounce), glow 90/260 ms,
  "materialize" curves in 320 ms `cubic-bezier(0.05, 0.7, 0.1, 1)` and out
  240 ms `cubic-bezier(0.3, 0, 0.8, 0.15)`; exact Snell deviation and the
  padded-backdrop contract for the planned refraction layer.
  https://github.com/shayann07/liquidglass
- **al1h3n/liquid-rust** (MIT OR Apache-2.0): squircle SDF, bevel slope,
  Snell refraction with dispersion and frost pyramid, the basis for the
  planned HLSL refraction shader; closed-form `Spring`.
  https://github.com/al1h3n/liquid-rust
- **TomLo-FStack/win-liquid-glass** (no licence: read only, nothing used):
  reference for the D3D11 + DXGI Desktop Duplication + DirectComposition
  capture path. https://github.com/TomLo-FStack/win-liquid-glass
