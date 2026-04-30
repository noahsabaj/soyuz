//! Dioxus-managed static assets.

use dioxus::prelude::*;

pub const THEME_CSS: Asset = asset!("/assets/theme.css");
pub const BASE_CSS: Asset = asset!("/assets/base.css");
pub const APP_CSS: Asset = asset!("/assets/style.css");
pub const TOOLBAR_CSS: Asset = asset!("/src/toolbar/toolbar.module.css");
pub const BROWSER_CSS: Asset = asset!("/src/browser/browser.module.css");
pub const PANE_CSS: Asset = asset!("/src/pane/pane.module.css");
pub const TERMINAL_CSS: Asset = asset!("/src/terminal/terminal.module.css");
pub const STATUSBAR_CSS: Asset = asset!("/src/statusbar/statusbar.module.css");
pub const PALETTE_CSS: Asset = asset!("/src/command_palette/palette.module.css");
pub const SETTINGS_CSS: Asset = asset!("/src/settings_panel/settings.module.css");
pub const MARKDOWN_CSS: Asset = asset!("/src/markdown_panel/markdown.module.css");
pub const EXPORT_CSS: Asset = asset!("/src/export/export.module.css");
pub const APP_ICON_32: Asset = asset!("/assets/icons/icon-32.png");
