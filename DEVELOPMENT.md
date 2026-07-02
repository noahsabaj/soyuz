# Development workflow (Dioxus 0.8)

Soyuz Studio is a Dioxus desktop app. Beyond `cargo build`, the Dioxus CLI
(`dx`, pinned in CI to `0.8.0-alpha.0`) unlocks a much faster inner loop.

## Fast iteration with `dx serve`

```bash
dx serve
```

- **RSX hot-reload** — edits to `rsx!` markup, `asset!()`-referenced CSS/SVG/JS,
  and other static assets apply **without a rebuild** and **without losing app
  state** (open tabs, cursor, etc.). This is the single biggest day-to-day win:
  tweak `app/assets/*.css` or a component's markup and see it live.

- **Rust hot-patching (Subsecond)**:

  ```bash
  dx serve --hotpatch
  ```

  Edit Rust (event handlers, component bodies) and have it patched into the
  running app without a full rebuild or state loss.

  > **Workspace caveat.** Subsecond currently hot-patches only the *tip* crate
  > (`soyuz-app`). Edits in the dependency crates — `soyuz-engine`,
  > `soyuz-render`, `soyuz-script`, `soyuz-core`, `soyuz-sdf` — will **not** be
  > hot-patched; those still require a normal rebuild. Since much of Soyuz's
  > logic lives in those crates, `--hotpatch` mainly accelerates UI work in
  > `app/`.

## Debugging

While `dx serve` is running, press **`d`** to attach an LLDB instance
(VS Code-compatible) to the running app.

## Assets

UI assets go through the `asset!()` macro (manganis) — they are content-hashed,
optimized, and bundled. See `app/src/assets.rs`. Reference stylesheets via
`document::Stylesheet { href: assets::* }` and scripts via
`document::Script { src: asset!("/assets/…js") }` rather than inline
`<style>`/`<script>` blocks, so they participate in hashing and hot-reload.

## CI parity

`.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy` with
`-D warnings`, `xtask docs check --soyuz-only` / `theme check --soyuz-only`
(soyuz-side generated artifacts + docs snippet tests), `cargo test`,
`cargo build --release`, and `cargo run -p xtask -- studio-smoke --mode fast`
(which invokes `dx check` / `dx build`); a separate job builds `soyuz-wasm`
with wasm-pack. The website-integrated variant of the artifact checks
additionally validates the website's generated files, so it needs
soyuz-website checked out as a sibling — which soyuz CI doesn't have (the
repo is private). That variant runs in the website repo's deploy workflow
(via `npm run check`, which invokes xtask with `--website`). Run everything
locally before pushing (without `--soyuz-only`, the checks cover the website
sibling too):

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo run -p xtask -- docs check
cargo run -p xtask -- theme check
```
