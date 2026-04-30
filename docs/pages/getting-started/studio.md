# Soyuz Studio

The desktop workbench for writing and previewing Soyuz assets.

## Interface Overview

- Explorer: navigate workspace files and open Rhai scripts.
- Editor tabs: work across scripts, settings, Preview, Export, and docs.
- Activity bar: jump to Preview, Export, Terminal, and Settings.
- Preview tab: run the current script in a docked 3D surface.
- Export tab: configure mesh resolution and export format.
- Output panel: inspect runtime logs and preview/export messages.

## Preview Behavior

Preview opens inside Soyuz Studio by default. Use Refresh to rerun the current script in the tab, Stop to end the preview process, and Pop Out only when you explicitly want a separate native preview window.

Docked native preview is Linux/X11-first. When docking is unavailable, Soyuz keeps the Preview tab open and reports the reason instead of silently opening a window.

## Keyboard Shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+Enter` | Open or refresh Preview tab |
| `Ctrl+S` | Save file |
| `Ctrl+O` | Open file |
| `Ctrl+N` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+P` | Open command palette |
| `Ctrl+\`` | Toggle terminal |
