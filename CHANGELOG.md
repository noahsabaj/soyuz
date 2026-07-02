# Changelog

All notable changes to Soyuz are documented in this file. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Soyuz adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

For releases before 0.7.0, see the
[GitHub releases page](https://github.com/noahsabaj/soyuz/releases).

## [0.7.2] - 2026-07-02

v0.7.1 was tagged but never published; its changes ship here.

### Fixed

- The docked preview renders on the first `Ctrl+Enter`. Opening the Preview
  tab unmounted the editor and silently cancelled the just-started preview
  task; the preview lifecycle now runs on an app-scoped task.
- Cold-GPU startup no longer shows a white pane: the renderer requests a
  Vulkan adapter first (GL/EGL enumeration could stall for seconds on a cold
  driver stack), and the embedded window stays hidden until its first frame
  is presented.
- The embedded preview honors fractional display scaling; its geometry was
  previously double-scaled on fractional-DPI setups (e.g. 160%).
- The docked preview embeds correctly on Wayland sessions launched with
  `GDK_BACKEND=x11`; the preview child now forces its own X11 backend instead
  of silently becoming a floating Wayland window.
- The command palette reliably overlays a running preview, with no flash on
  open or close: the preview parks offscreen while the palette is open, and
  all window moves are serialized through a single X connection.
- Keyboard shortcuts keep working after clicking or dragging inside the
  preview; the studio reclaims keyboard focus if preview interaction breaks
  it (never from another application).
- The command palette hotkey works from any focus state, including right
  after opening the preview or closing the palette with `Escape`.
- `smooth_union`/`smooth_subtract`/`smooth_intersect` with `k = 0` no longer
  produce NaN geometry; smoothness is clamped to a positive minimum.
- glTF export writes the actual `.bin` filename into its JSON instead of a
  hardcoded one.

### Changed

- `Ctrl+P` / `Ctrl+Shift+P` now toggle the command palette open and closed.
- The docked preview follows the host pane when the window or panes resize.
- The log panel is consistently named "Output" across the UI.
- `soyuz-core` slimmed by roughly 3,400 lines: unused SDF operation,
  primitive, transform, texture, and material modules removed. The
  Rhai → `SdfOp` pipeline is the single source of truth for geometry.

### Added

- Session, undo, and SDF property test suites (139 tests across the
  workspace).
- Timestamped diagnostic breadcrumbs from the studio and the preview child on
  stderr, so preview issues are debuggable from a launch log.
- Dependabot configuration and CI enforcement of the generated docs/theme
  artifacts.
- This changelog.

## [0.7.1] - 2026-07-01

Tagged but never published; first shipped as part of v0.7.2.

### Added

- Lightweight, notify-only update check against GitHub Releases.
- AppStream metainfo so Linux software centres show proper app info.
- Native `.rpm` packaging for Fedora/RHEL/openSUSE.

### Changed

- Explorer redesigned VS Code-style with file-type icons and a proper tree.
- A docked preview without an X11 parent (plain Wayland) auto-opens a pop-out
  window with an explanation instead of failing.

## [0.7.0] - 2026-07-01

### Changed

- Studio migrated to Dioxus 0.8 and fully adopted dioxus-primitives for an
  accessible, keyboard-navigable UI, plus a broad modernization and fix pass.
- WASM playground builds link correctly (rhai's `wasm-bindgen` feature).

[0.7.2]: https://github.com/noahsabaj/soyuz/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/noahsabaj/soyuz/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/noahsabaj/soyuz/compare/v0.6.0...v0.7.0
