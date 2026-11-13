# CrushClone-rs

A standalone, zero-allocation Rust VST3 and CLAP audio plugin port of CamelCrusher / [`doxiedevops/CrushClone`](https://github.com/doxiedevops/CrushClone), built with [`nih-plug`](https://github.com/robbert-vdh/nih-plug) and `nih_plug_vizia`.

> ⚠️ **VIBECODED WARNING:** This project (DSP algorithms and VST3/CLAP plugin code) was vibecoded with AI assistance. Read the code, verify it in your DAW with a limiter on your master bus, and use at your own risk!
>
> **Trademark Notice:** *CamelCrusher* is a trademark of its respective owner. `CrushClone-rs` is an independent open-source reimplementation and is not affiliated with or endorsed by Camel Audio or Apple.

---

## Building & Installing the Plugin

### From GitHub Actions / Pre-Built Releases
Cross-platform builds for **macOS (`arm64`)**, **Windows (`x86_64`)**, and **Linux (`x86_64`)** are compiled automatically via [`.github/workflows/build.yml`](./.github/workflows/build.yml).

Download them from the [Releases](../../releases) page. Each release is built from a version tag (e.g. `v0.1.0`).

Each platform `.zip` contains `CrushClone-rs.vst3`, `CrushClone-rs.clap`, this README, and `LICENSE`.

#### Publishing a new release (maintainers)
```bash
git tag v0.1.0
git push origin v0.1.0
```
Once all three platform builds succeed, the workflow creates the `v0.1.0` GitHub Release with auto-generated release notes and the platform archives attached. Tags containing a hyphen (e.g. `v0.1.0-beta.1`) are marked as pre-releases.

### Building Locally from Source
```bash
# Run DSP unit tests
cargo test -p crushclone-dsp

# Bundle VST3 and CLAP plugins in release mode
cargo xtask bundle crushclone-rs --release
```

Bundled artifacts are output to `target/bundled/crushclone-rs.vst3` and `target/bundled/crushclone-rs.clap`.

### Installing & Bypassing macOS Gatekeeper ("Apple Jail")
On macOS, copy the bundles to `/Library/Audio/Plug-Ins/` (all users) or `~/Library/Audio/Plug-Ins/` (current user):

```bash
sudo mkdir -p /Library/Audio/Plug-Ins/VST3 /Library/Audio/Plug-Ins/CLAP
sudo cp -r target/bundled/crushclone-rs.vst3 /Library/Audio/Plug-Ins/VST3/
sudo cp -r target/bundled/crushclone-rs.clap /Library/Audio/Plug-Ins/CLAP/
```

Because downloaded release bundles are not notarized with a paid Apple Developer ID certificate, macOS Gatekeeper may quarantine them. Strip the quarantine attribute and apply a local ad-hoc signature if needed:

```bash
xattr -cr ~/Library/Audio/Plug-Ins/VST3/"CrushClone-rs.vst3"
xattr -cr ~/Library/Audio/Plug-Ins/CLAP/"CrushClone-rs.clap"
codesign --force --deep --sign - ~/Library/Audio/Plug-Ins/VST3/"CrushClone-rs.vst3"
codesign --force --deep --sign - ~/Library/Audio/Plug-Ins/CLAP/"CrushClone-rs.clap"
```

On **Windows**, copy `CrushClone-rs.vst3` to `C:\Program Files\Common Files\VST3\` and `CrushClone-rs.clap` to `C:\Program Files\Common Files\CLAP\`.
On **Linux**, copy them to `~/.vst3/` and `~/.clap/`.

---

## Signal Chain & Parameters

1. **Distortion (`4x` Oversampled via `Oversampler4x`):**
   - **Tube:** Asymmetric `tanh` waveshaper with DC bias (`0.08 * drive`) for even-harmonic warmth.
   - **Mech:** Pre-emphasis `200 Hz` low-shelf and `4 kHz` high-shelf EQ feeding a `[-1, 1]` hard clipper with a cubic soft knee.
   - **On:** Bypass toggle for the distortion stage.
2. **Filter (`LadderFilter`):**
   - Trapezoidal State Variable TPT low-pass filter (`Cutoff`: `20 Hz – 20 kHz`, `Resonance`: `0% – 100%`, `On` toggle defaulting to OFF).
3. **Compressor (`CompressorModule`):**
   - **Amount:** One-knob soft-knee dynamics (`0 dB` to `-30 dB` threshold, `1:1` to `8:1` ratio, adaptive attack/release, and up to `+12 dB` makeup gain) with a live **GR** meter.
   - **Phat:** Parallel New York compression blend (`20% – 65%`) with a `+8 dB` @ `120 Hz` low-shelf boost.
   - **On:** Bypass toggle for the compressor stage.
4. **Master / Mix & Factory Presets:**
   - **Master:** Output gain (`-24 dB` to `+12 dB`), **Mix:** parallel dry/wet (`0% – 100%`), plus a `[-2.0, 2.0]` safety hard limiter.
   - **Factory Presets:** All 12 factory presets from `CrushClone` (`Default`, `DRM BigBadBeat`, `BAS FatSub`, `GTR Crunch`, `SYN WarmPad`, `VOX Gritty`, `MAS Loudener`, `DRM LoFiHit`, `SYN Screamer`, `Gentle Warmth`, `Telephone`, `Destroy Everything`) selectable from the header bar.

---

## Workspace Structure

```text
CrushClone-rs/
├── .github/workflows/
│   └── build.yml                  # Cross-platform CI build (macOS, Windows, Linux)
├── crates/
│   ├── crushclone-dsp/            # Zero-allocation Rust DSP library & factory presets
│   └── crushclone-rs/             # "CrushClone-rs" VST3/CLAP plugin & Vizia GUI
└── xtask/                         # nih_plug_xtask bundler
```

---

## License

Licensed under the **GNU General Public License v3.0 or later (GPL-3.0-or-later)**. See [`LICENSE`](./LICENSE) for details.

---

## Acknowledgements

- **[`doxiedevops/CrushClone`](https://github.com/doxiedevops/CrushClone)** for the original open-source C++ / JUCE DSP implementation and factory presets.
- **[`nih-plug`](https://github.com/robbert-vdh/nih-plug)** by Robbert van der Helm for the Rust VST3/CLAP plugin framework and `nih_plug_vizia` GUI integration.
