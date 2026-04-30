use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        bail!("missing xtask command");
    };

    match command.as_str() {
        "studio-smoke" => run_studio_smoke(SmokeOptions::parse(args)?),
        "theme" => run_theme(ThemeOptions::parse(args)?),
        "docs" => run_docs(DocsOptions::parse(args)?),
        _ => {
            print_usage();
            bail!("unknown xtask command: {command}");
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThemeMode {
    Generate,
    Check,
}

#[derive(Debug)]
struct ThemeOptions {
    mode: ThemeMode,
    website: Option<PathBuf>,
}

impl ThemeOptions {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let Some(mode_arg) = args.next() else {
            bail!("missing theme command: expected generate or check");
        };
        let mode = match mode_arg.as_str() {
            "generate" => ThemeMode::Generate,
            "check" => ThemeMode::Check,
            _ => bail!("unknown theme command: {mode_arg}"),
        };

        let mut website = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--website" => {
                    let Some(value) = args.next() else {
                        bail!("missing value for --website");
                    };
                    website = Some(PathBuf::from(value));
                }
                _ => bail!("unknown theme argument: {arg}"),
            }
        }

        Ok(Self { mode, website })
    }
}

struct ThemeArtifact {
    path: PathBuf,
    contents: String,
}

struct ThemeTokens {
    value: serde_json::Value,
}

impl ThemeTokens {
    fn read(root: &Path) -> Result<Self> {
        let path = root.join("theme/soyuz-graphite.tokens.json");
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("read theme tokens {}", path.display()))?;
        let value = serde_json::from_str(&contents)
            .with_context(|| format!("parse theme tokens {}", path.display()))?;
        Ok(Self { value })
    }

    fn at(&self, path: &[&str]) -> Result<&serde_json::Value> {
        let mut current = &self.value;
        for key in path {
            current = current
                .get(*key)
                .with_context(|| format!("missing theme token {}", path.join(".")))?;
        }
        Ok(current)
    }

    fn str(&self, path: &[&str]) -> Result<&str> {
        self.at(path)?
            .as_str()
            .with_context(|| format!("theme token {} must be a string", path.join(".")))
    }

    fn num(&self, path: &[&str]) -> Result<f64> {
        self.at(path)?
            .as_f64()
            .with_context(|| format!("theme token {} must be a number", path.join(".")))
    }

    fn bool(&self, path: &[&str]) -> Result<bool> {
        self.at(path)?
            .as_bool()
            .with_context(|| format!("theme token {} must be a boolean", path.join(".")))
    }

    fn vec3(&self, path: &[&str]) -> Result<[f64; 3]> {
        let value = self.at(path)?;
        let values = value
            .as_array()
            .with_context(|| format!("theme token {} must be an array", path.join(".")))?;
        if values.len() != 3 {
            bail!("theme token {} must contain 3 numbers", path.join("."));
        }
        Ok([
            values[0]
                .as_f64()
                .with_context(|| format!("theme token {}[0] must be a number", path.join(".")))?,
            values[1]
                .as_f64()
                .with_context(|| format!("theme token {}[1] must be a number", path.join(".")))?,
            values[2]
                .as_f64()
                .with_context(|| format!("theme token {}[2] must be a number", path.join(".")))?,
        ])
    }
}

fn run_theme(options: ThemeOptions) -> Result<()> {
    let root = workspace_root()?;
    let website = resolve_website_path(&root, options.website)?;
    let tokens = ThemeTokens::read(&root)?;
    let artifacts = theme_artifacts(&root, &website, &tokens)?;

    match options.mode {
        ThemeMode::Generate => {
            for artifact in artifacts {
                write_artifact(&artifact)?;
            }
        }
        ThemeMode::Check => {
            let mut stale = Vec::new();
            for artifact in artifacts {
                let existing = fs::read_to_string(&artifact.path).unwrap_or_default();
                if existing != artifact.contents {
                    stale.push(artifact.path);
                }
            }
            if !stale.is_empty() {
                for path in &stale {
                    eprintln!("stale generated theme artifact: {}", path.display());
                }
                bail!("theme artifacts are stale; run cargo run -p xtask -- theme generate");
            }
            check_raw_ui_colors(&root, &website)?;
        }
    }

    Ok(())
}

fn resolve_website_path(root: &Path, path: Option<PathBuf>) -> Result<PathBuf> {
    let path = path.unwrap_or_else(|| root.join("../soyuz-website"));
    let path = if path.is_absolute() {
        path
    } else if path == Path::new("../soyuz-website") {
        root.join(path)
    } else {
        std::env::current_dir()
            .context("read current directory")?
            .join(path)
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve website path {}", path.display()))?;
    if !path.join("package.json").is_file() {
        bail!(
            "website path does not look like soyuz-website: {}",
            path.display()
        );
    }
    Ok(path)
}

fn theme_artifacts(
    root: &Path,
    website: &Path,
    tokens: &ThemeTokens,
) -> Result<Vec<ThemeArtifact>> {
    Ok(vec![
        ThemeArtifact {
            path: root.join("app/assets/theme.css"),
            contents: generate_app_theme_css(tokens)?,
        },
        ThemeArtifact {
            path: root.join("crates/soyuz-render/src/theme_generated.rs"),
            contents: generate_render_theme_rs(tokens)?,
        },
        ThemeArtifact {
            path: website.join("src/styles/generated-theme.css"),
            contents: generate_website_theme_css(tokens)?,
        },
        ThemeArtifact {
            path: website.join("src/lib/theme.generated.ts"),
            contents: generate_website_theme_ts(tokens)?,
        },
    ])
}

fn write_artifact(artifact: &ThemeArtifact) -> Result<()> {
    if let Some(parent) = artifact.path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create theme artifact dir {}", parent.display()))?;
    }
    fs::write(&artifact.path, &artifact.contents)
        .with_context(|| format!("write theme artifact {}", artifact.path.display()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DocsMode {
    Generate,
    Check,
}

#[derive(Debug)]
struct DocsOptions {
    mode: DocsMode,
    website: Option<PathBuf>,
}

impl DocsOptions {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let Some(mode_arg) = args.next() else {
            bail!("missing docs command: expected generate or check");
        };
        let mode = match mode_arg.as_str() {
            "generate" => DocsMode::Generate,
            "check" => DocsMode::Check,
            _ => bail!("unknown docs command: {mode_arg}"),
        };

        let mut website = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--website" => {
                    let Some(value) = args.next() else {
                        bail!("missing value for --website");
                    };
                    website = Some(PathBuf::from(value));
                }
                _ => bail!("unknown docs argument: {arg}"),
            }
        }

        Ok(Self { mode, website })
    }
}

#[derive(Debug, Deserialize)]
struct DocsManifest {
    site: DocsSite,
    sections: Vec<DocsSection>,
}

#[derive(Debug, Deserialize)]
struct DocsSite {
    title: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct DocsSection {
    id: String,
    title: String,
    description: String,
    pages: Vec<DocsPage>,
}

#[derive(Clone, Debug, Deserialize)]
struct DocsPage {
    id: String,
    href: String,
    title: String,
    description: String,
    source: Option<String>,
    kind: DocsPageKind,
    #[serde(default)]
    api_categories: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum DocsPageKind {
    Markdown,
    Api,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiSpec {
    categories: Vec<ApiCategorySpec>,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiCategorySpec {
    id: String,
    title: String,
    functions: Vec<ApiFunctionSpec>,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiFunctionSpec {
    name: String,
    signature: String,
    description: String,
    example: String,
    kind: String,
    alias_of: Option<String>,
}

#[derive(Serialize)]
struct ApiManifestJson<'a> {
    categories: Vec<ApiCategoryJson<'a>>,
}

#[derive(Serialize)]
struct ApiCategoryJson<'a> {
    id: &'a str,
    title: &'a str,
    functions: Vec<ApiFunctionJson<'a>>,
}

#[derive(Serialize)]
struct ApiFunctionJson<'a> {
    name: &'a str,
    signature: &'a str,
    description: &'a str,
    example: &'a str,
    kind: &'a str,
    #[serde(rename = "aliasOf", skip_serializing_if = "Option::is_none")]
    alias_of: Option<&'a str>,
}

fn run_docs(options: DocsOptions) -> Result<()> {
    let root = workspace_root()?;
    let website = resolve_website_path(&root, options.website)?;
    let docs = read_docs_manifest(&root)?;
    let api = read_api_spec(&root)?;
    validate_docs_manifest(&root, &docs, &api)?;
    validate_runtime_mappings(&api)?;

    let artifacts = docs_artifacts(&root, &website, &docs, &api)?;
    match options.mode {
        DocsMode::Generate => {
            for artifact in artifacts {
                write_artifact(&artifact)?;
            }
        }
        DocsMode::Check => {
            let mut stale = Vec::new();
            for artifact in artifacts {
                let existing = fs::read_to_string(&artifact.path).unwrap_or_default();
                if existing != artifact.contents {
                    stale.push(artifact.path);
                }
            }
            if !stale.is_empty() {
                for path in &stale {
                    eprintln!("stale generated docs artifact: {}", path.display());
                }
                bail!("docs artifacts are stale; run cargo run -p xtask -- docs generate");
            }
            run_docs_snippet_tests(&root)?;
        }
    }

    Ok(())
}

fn read_docs_manifest(root: &Path) -> Result<DocsManifest> {
    let path = root.join("docs/docs.toml");
    let contents = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("parse {}", path.display()))
}

fn read_api_spec(root: &Path) -> Result<ApiSpec> {
    let path = root.join("docs/api/manifest.toml");
    let contents = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("parse {}", path.display()))
}

fn validate_docs_manifest(root: &Path, docs: &DocsManifest, api: &ApiSpec) -> Result<()> {
    let mut section_ids = BTreeSet::new();
    let mut page_ids = BTreeSet::new();
    let mut hrefs = BTreeSet::new();
    let api_categories = api
        .categories
        .iter()
        .map(|category| category.id.as_str())
        .collect::<BTreeSet<_>>();

    if docs.site.title.trim().is_empty() {
        bail!("docs site title must not be empty");
    }
    if docs.site.description.trim().is_empty() {
        bail!("docs site description must not be empty");
    }

    for section in &docs.sections {
        if !section_ids.insert(section.id.as_str()) {
            bail!("duplicate docs section id: {}", section.id);
        }
        if section.title.trim().is_empty() || section.description.trim().is_empty() {
            bail!(
                "docs section {} must have title and description",
                section.id
            );
        }
        if section.pages.is_empty() {
            bail!("docs section {} must contain at least one page", section.id);
        }

        for page in &section.pages {
            if !page_ids.insert(page.id.as_str()) {
                bail!("duplicate docs page id: {}", page.id);
            }
            if !hrefs.insert(page.href.as_str()) {
                bail!("duplicate docs page href: {}", page.href);
            }
            if !page.href.starts_with("/docs") {
                bail!("docs page {} href must start with /docs", page.id);
            }

            match page.kind {
                DocsPageKind::Markdown => {
                    let Some(source) = &page.source else {
                        bail!("markdown docs page {} is missing source", page.id);
                    };
                    let source_path = root.join("docs/pages").join(source);
                    if !source_path.is_file() {
                        bail!(
                            "markdown docs page {} source does not exist: {}",
                            page.id,
                            source_path.display()
                        );
                    }
                    if !page.api_categories.is_empty() {
                        bail!(
                            "markdown docs page {} must not list api_categories",
                            page.id
                        );
                    }
                }
                DocsPageKind::Api => {
                    if page.source.is_some() {
                        bail!("api docs page {} must not list a markdown source", page.id);
                    }
                    if page.api_categories.is_empty() {
                        bail!("api docs page {} must list api_categories", page.id);
                    }
                    for category in &page.api_categories {
                        if !api_categories.contains(category.as_str()) {
                            bail!(
                                "api docs page {} references missing API category {}",
                                page.id,
                                category
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn validate_runtime_mappings(api: &ApiSpec) -> Result<()> {
    for category in &api.categories {
        for function in &category.functions {
            match category.id.as_str() {
                "primitives" | "operations" | "modifiers" | "transforms" | "math" => {
                    let _registrations = sdf_registration_lines(category, function)?;
                }
                "environment" => {
                    let _registration = env_registration_line(function)?;
                }
                _ => bail!(
                    "unknown API category for runtime generation: {}",
                    category.id
                ),
            }
        }
    }
    Ok(())
}

fn docs_artifacts(
    root: &Path,
    website: &Path,
    docs: &DocsManifest,
    api: &ApiSpec,
) -> Result<Vec<ThemeArtifact>> {
    Ok(vec![
        ThemeArtifact {
            path: root.join("SOYUZ_API.json"),
            contents: generate_api_manifest_json(api)?,
        },
        ThemeArtifact {
            path: root.join("crates/soyuz-script/src/api_generated.rs"),
            contents: rustfmt_source(&generate_api_generated_rs(api)?)?,
        },
        ThemeArtifact {
            path: root.join("app/src/docs_generated.rs"),
            contents: rustfmt_source(&generate_app_docs_rs(docs, api))?,
        },
        ThemeArtifact {
            path: root.join("SOYUZ_COOKBOOK.md"),
            contents: generate_cookbook_markdown(root, docs)?,
        },
        ThemeArtifact {
            path: website.join("src/lib/docs.generated.ts"),
            contents: generate_website_docs_ts(docs),
        },
    ])
}

fn generate_api_manifest_json(api: &ApiSpec) -> Result<String> {
    let manifest = ApiManifestJson {
        categories: api
            .categories
            .iter()
            .map(|category| ApiCategoryJson {
                id: &category.id,
                title: &category.title,
                functions: category
                    .functions
                    .iter()
                    .map(|function| ApiFunctionJson {
                        name: &function.name,
                        signature: &function.signature,
                        description: &function.description,
                        example: &function.example,
                        kind: &function.kind,
                        alias_of: function.alias_of.as_deref(),
                    })
                    .collect(),
            })
            .collect(),
    };
    Ok(format!("{}\n", serde_json::to_string_pretty(&manifest)?))
}

fn generate_website_docs_ts(docs: &DocsManifest) -> String {
    let markdown_pages = docs
        .sections
        .iter()
        .flat_map(|section| section.pages.iter())
        .filter(|page| page.kind == DocsPageKind::Markdown)
        .collect::<Vec<_>>();

    let mut output = String::from(
        "/* @generated by cargo run -p xtask -- docs generate. Do not edit directly. */\n\n",
    );
    for page in &markdown_pages {
        let Some(source) = page.source.as_deref() else {
            continue;
        };
        output.push_str(&format!(
            "import {} from '@soyuz-docs/docs/pages/{}?raw';\n",
            ts_doc_ident(&page.id),
            source
        ));
    }

    output.push_str(
        "\nexport type DocsPage = {\n\
         \tid: string;\n\
         \thref: string;\n\
         \ttitle: string;\n\
         \tdescription: string;\n\
         \tsection: string;\n\
         \tkind: 'markdown' | 'api';\n\
         \tcontent?: string;\n\
         \tapiCategories?: string[];\n\
         };\n\n",
    );

    output.push_str("export const docsSite = ");
    output.push_str(&format!(
        "{{ title: {}, description: {} }} as const;\n\n",
        json_string(&docs.site.title),
        json_string(&docs.site.description)
    ));

    output.push_str("export const docsPages: DocsPage[] = [\n");
    for section in &docs.sections {
        for page in &section.pages {
            let kind = match page.kind {
                DocsPageKind::Markdown => "markdown",
                DocsPageKind::Api => "api",
            };
            output.push_str("\t{\n");
            output.push_str(&format!(
                "\t\tid: {},\n\t\thref: {},\n\t\ttitle: {},\n\t\tdescription: {},\n\t\tsection: {},\n\t\tkind: {},\n",
                json_string(&page.id),
                json_string(&page.href),
                json_string(&page.title),
                json_string(&page.description),
                json_string(&section.title),
                json_string(kind),
            ));
            match page.kind {
                DocsPageKind::Markdown => {
                    output.push_str(&format!("\t\tcontent: {},\n", ts_doc_ident(&page.id)));
                }
                DocsPageKind::Api => {
                    output.push_str(&format!(
                        "\t\tapiCategories: {},\n",
                        json_string_array(&page.api_categories)
                    ));
                }
            }
            output.push_str("\t},\n");
        }
    }
    output.push_str("];\n\n");

    output.push_str("export const docsSections = [\n");
    for section in &docs.sections {
        output.push_str("\t{\n");
        output.push_str(&format!(
            "\t\tid: {},\n\t\ttitle: {},\n\t\tdescription: {},\n\t\titems: docsPages.filter((page) => page.section === {}),\n",
            json_string(&section.id),
            json_string(&section.title),
            json_string(&section.description),
            json_string(&section.title),
        ));
        output.push_str("\t},\n");
    }
    output.push_str("];\n\n");

    output.push_str(
        "export function getDocPage(href: string): DocsPage | undefined {\n\
         \treturn docsPages.find((page) => page.href === href || page.id === href);\n\
         }\n",
    );

    output
}

fn generate_app_docs_rs(docs: &DocsManifest, api: &ApiSpec) -> String {
    let mut output = String::from(
        "// @generated by cargo run -p xtask -- docs generate. Do not edit directly.\n\n\
         pub struct EmbeddedDoc {\n\
         \tpub id: &'static str,\n\
         \tpub title: &'static str,\n\
         \tpub section: &'static str,\n\
         \tpub href: &'static str,\n\
         \tpub content: &'static str,\n\
         }\n\n\
         pub static DOCS: &[EmbeddedDoc] = &[\n",
    );

    for section in &docs.sections {
        for page in &section.pages {
            output.push_str("\tEmbeddedDoc {\n");
            output.push_str(&format!(
                "\t\tid: {:?},\n\t\ttitle: {:?},\n\t\tsection: {:?},\n\t\thref: {:?},\n",
                page.id, page.title, section.title, page.href
            ));
            match page.kind {
                DocsPageKind::Markdown => {
                    if let Some(source) = page.source.as_deref() {
                        output.push_str(&format!(
                            "\t\tcontent: include_str!(\"../../docs/pages/{source}\"),\n"
                        ));
                    }
                }
                DocsPageKind::Api => {
                    output.push_str(&format!(
                        "\t\tcontent: {:?},\n",
                        generate_api_doc_markdown(page, api)
                    ));
                }
            }
            output.push_str("\t},\n");
        }
    }

    output.push_str(
        "\tEmbeddedDoc {\n\
         \t\tid: \"readme\",\n\
         \t\ttitle: \"README\",\n\
         \t\tsection: \"Project\",\n\
         \t\thref: \"/README.md\",\n\
         \t\tcontent: include_str!(\"../../README.md\"),\n\
         \t},\n",
    );

    output.push_str(
        "];\n\n\
         pub fn doc_by_id(id: &str) -> Option<&'static EmbeddedDoc> {\n\
         \tDOCS.iter().find(|doc| doc.id == id)\n\
         }\n\n\
         pub fn doc_title(id: &str) -> Option<&'static str> {\n\
         \tdoc_by_id(id).map(|doc| doc.title)\n\
         }\n",
    );

    output
}

fn generate_api_doc_markdown(page: &DocsPage, api: &ApiSpec) -> String {
    let mut output = format!("# {}\n\n{}\n", page.title, page.description);
    for category_id in &page.api_categories {
        if let Some(category) = api
            .categories
            .iter()
            .find(|category| &category.id == category_id)
        {
            output.push_str(&format!("\n## {}\n\n", category.title));
            for function in &category.functions {
                output.push_str(&format!(
                    "### `{}`\n\n`{}`\n\n{}\n\n```rhai\n{}\n```\n\n",
                    function.name, function.signature, function.description, function.example
                ));
            }
        }
    }
    output
}

fn generate_cookbook_markdown(root: &Path, docs: &DocsManifest) -> Result<String> {
    let mut output = String::from(
        "<!-- @generated by cargo run -p xtask -- docs generate. Do not edit directly. -->\n\n",
    );
    for section in &docs.sections {
        if section.id != "cookbook" {
            continue;
        }
        for page in &section.pages {
            let Some(source) = &page.source else {
                continue;
            };
            let path = root.join("docs/pages").join(source);
            let contents =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            output.push_str(&contents);
            if !output.ends_with('\n') {
                output.push('\n');
            }
            output.push('\n');
        }
    }
    Ok(output)
}

fn generate_api_generated_rs(api: &ApiSpec) -> Result<String> {
    let mut sdf_lines = Vec::new();
    let mut env_lines = Vec::new();
    for category in &api.categories {
        for function in &category.functions {
            if category.id == "environment" {
                env_lines.push(env_registration_line(function)?);
            } else {
                sdf_lines.extend(sdf_registration_lines(category, function)?);
            }
        }
    }

    Ok(format!(
        "// @generated by cargo run -p xtask -- docs generate. Do not edit directly.\n\
         #![allow(clippy::missing_panics_doc)]\n\
         #![allow(clippy::unwrap_used)]\n\n\
         use rhai::Engine;\n\
         use soyuz_sdf::Environment;\n\
         use std::sync::{{Arc, Mutex}};\n\n\
         use crate::env_api;\n\
         use crate::sdf_api::{{self, RhaiSdf}};\n\n\
         pub const API_MANIFEST_JSON: &str = include_str!(\"../../../SOYUZ_API.json\");\n\n\
         pub fn register_sdf_api_generated(engine: &mut Engine) {{\n\
         \tengine\n\
         \t\t.register_type_with_name::<RhaiSdf>(\"Sdf\")\n\
         \t\t.register_fn(\"to_string\", |sdf: &mut RhaiSdf| format!(\"{{:?}}\", sdf.op));\n\
         {sdf_body}\
         }}\n\n\
         #[allow(clippy::too_many_lines)]\n\
         pub fn register_env_api_generated(engine: &mut Engine, env: &Arc<Mutex<Environment>>) {{\n\
         {env_body}\
         }}\n",
        sdf_body = indent_lines(&sdf_lines.join("\n"), "\t"),
        env_body = indent_lines(&env_lines.join("\n"), "\t"),
    ))
}

fn sdf_registration_lines(
    category: &ApiCategorySpec,
    function: &ApiFunctionSpec,
) -> Result<Vec<String>> {
    let name = function.name.as_str();
    let lines = match category.id.as_str() {
        "primitives" => {
            let mut lines = vec![format!("engine.register_fn({name:?}, sdf_api::{name});")];
            match name {
                "sphere" => {
                    lines.push("engine.register_fn(\"sphere\", sdf_api::sphere_int);".into());
                }
                "cube" => {
                    lines.push("engine.register_fn(\"cube\", sdf_api::cube_int);".into());
                }
                _ => {}
            }
            lines
        }
        "operations" | "modifiers" | "transforms" => {
            vec![format!("engine.register_fn({name:?}, RhaiSdf::{name});")]
        }
        "math" => {
            let target = match name {
                "PI" => "pi",
                "TAU" => "tau",
                "deg" => "deg_to_rad",
                "rad" => "rad_to_deg",
                _ => bail!("missing math runtime mapping for {}", function.name),
            };
            vec![format!("engine.register_fn({name:?}, sdf_api::{target});")]
        }
        _ => bail!("unknown SDF API category {}", category.id),
    };
    Ok(lines)
}

#[allow(clippy::too_many_lines)]
fn env_registration_line(function: &ApiFunctionSpec) -> Result<String> {
    let name = function.name.as_str();
    if name == "rgb_hex" {
        return Ok("engine.register_fn(\"rgb_hex\", env_api::rgb_hex);".into());
    }
    if name == "set_material_color_hex" {
        return Ok("let e = env.clone();\n\
             engine.register_fn(\"set_material_color_hex\", move |hex: &str| {\n\
             \tif let Some((r, g, b)) = env_api::parse_hex_color(hex) {\n\
             \t\te.lock().unwrap().material_color = [r, g, b];\n\
             \t}\n\
             });"
        .into());
    }
    if name == "set_background_color" {
        return Ok("let e = env.clone();\n\
             engine.register_fn(\"set_background_color\", move |r: f64, g: f64, b: f64| {\n\
             \tlet color = [r as f32, g as f32, b as f32];\n\
             \tlet mut e = e.lock().unwrap();\n\
             \te.sky_horizon = color;\n\
             \te.sky_zenith = color;\n\
             });"
        .into());
    }
    if let Some(field) = env_vec3_field(name) {
        return Ok(format!(
            "let e = env.clone();\n\
             engine.register_fn({name:?}, move |x: f64, y: f64, z: f64| {{\n\
             \te.lock().unwrap().{field} = [x as f32, y as f32, z as f32];\n\
             }});"
        ));
    }
    if let Some(field) = env_f64_field(name) {
        return Ok(format!(
            "let e = env.clone();\n\
             engine.register_fn({name:?}, move |value: f64| {{\n\
             \te.lock().unwrap().{field} = value as f32;\n\
             }});"
        ));
    }
    if let Some(field) = env_bool_field(name) {
        return Ok(format!(
            "let e = env.clone();\n\
             engine.register_fn({name:?}, move |enabled: bool| {{\n\
             \te.lock().unwrap().{field} = enabled;\n\
             }});"
        ));
    }
    match name {
        "env_default" => Ok("let e = env.clone();\n\
             engine.register_fn(\"env_default\", move || {\n\
             \t*e.lock().unwrap() = Environment::default();\n\
             });"
        .into()),
        "env_studio" => Ok(env_preset_body(
            "env_studio",
            &[
                ("sun_direction", "[1.0, 1.0, 0.5]"),
                ("sun_color", "[1.0, 1.0, 1.0]"),
                ("sun_intensity", "0.8"),
                ("ambient_color", "[0.3, 0.3, 0.35]"),
                ("ambient_intensity", "1.0"),
                ("sky_horizon", "[0.9, 0.9, 0.95]"),
                ("sky_zenith", "[0.8, 0.85, 0.95]"),
                ("fog_color", "[0.85, 0.85, 0.9]"),
                ("fog_density", "0.005"),
            ],
        )),
        "env_sunset" => Ok(env_preset_body(
            "env_sunset",
            &[
                ("sun_direction", "[1.0, 0.2, 0.3]"),
                ("sun_color", "[1.0, 0.6, 0.3]"),
                ("sun_intensity", "1.2"),
                ("ambient_color", "[0.3, 0.2, 0.3]"),
                ("ambient_intensity", "0.8"),
                ("sky_horizon", "[1.0, 0.7, 0.5]"),
                ("sky_zenith", "[0.4, 0.3, 0.5]"),
                ("fog_color", "[0.9, 0.6, 0.4]"),
                ("fog_density", "0.02"),
            ],
        )),
        "env_night" => Ok(env_preset_body(
            "env_night",
            &[
                ("sun_direction", "[0.5, 0.8, 0.2]"),
                ("sun_color", "[0.7, 0.8, 1.0]"),
                ("sun_intensity", "0.3"),
                ("ambient_color", "[0.05, 0.07, 0.15]"),
                ("ambient_intensity", "1.0"),
                ("sky_horizon", "[0.1, 0.1, 0.2]"),
                ("sky_zenith", "[0.02, 0.02, 0.08]"),
                ("fog_color", "[0.05, 0.05, 0.1]"),
                ("fog_density", "0.03"),
            ],
        )),
        "env_daylight" => Ok(env_preset_body(
            "env_daylight",
            &[
                ("sun_direction", "[0.5, 0.8, 0.3]"),
                ("sun_color", "[1.0, 0.98, 0.95]"),
                ("sun_intensity", "1.0"),
                ("ambient_color", "[0.2, 0.25, 0.35]"),
                ("ambient_intensity", "1.0"),
                ("sky_horizon", "[0.7, 0.8, 0.9]"),
                ("sky_zenith", "[0.3, 0.5, 0.8]"),
                ("fog_color", "[0.6, 0.65, 0.7]"),
                ("fog_density", "0.01"),
            ],
        )),
        "env_clay" => Ok(env_preset_body(
            "env_clay",
            &[
                ("sun_direction", "[0.5, 1.0, 0.5]"),
                ("sun_color", "[1.0, 1.0, 1.0]"),
                ("sun_intensity", "0.6"),
                ("ambient_color", "[0.5, 0.5, 0.5]"),
                ("ambient_intensity", "1.0"),
                ("material_color", "[0.85, 0.85, 0.85]"),
                ("material_shininess", "8.0"),
                ("specular_intensity", "0.1"),
                ("shadows_enabled", "false"),
                ("ao_enabled", "true"),
                ("ao_intensity", "2.0"),
                ("sky_horizon", "[0.95, 0.95, 0.95]"),
                ("sky_zenith", "[0.9, 0.9, 0.95]"),
                ("fog_density", "0.0"),
            ],
        )),
        _ => bail!("missing environment runtime mapping for {}", function.name),
    }
}

fn env_vec3_field(name: &str) -> Option<&'static str> {
    match name {
        "set_sun_direction" => Some("sun_direction"),
        "set_sun_color" => Some("sun_color"),
        "set_ambient_color" => Some("ambient_color"),
        "set_material_color" => Some("material_color"),
        "set_sky_horizon" => Some("sky_horizon"),
        "set_sky_zenith" => Some("sky_zenith"),
        "set_fog_color" => Some("fog_color"),
        _ => None,
    }
}

fn env_f64_field(name: &str) -> Option<&'static str> {
    match name {
        "set_sun_intensity" => Some("sun_intensity"),
        "set_ambient_intensity" => Some("ambient_intensity"),
        "set_material_shininess" => Some("material_shininess"),
        "set_specular_intensity" => Some("specular_intensity"),
        "set_fog_density" => Some("fog_density"),
        "set_ao_intensity" => Some("ao_intensity"),
        "set_shadow_softness" => Some("shadow_softness"),
        _ => None,
    }
}

fn env_bool_field(name: &str) -> Option<&'static str> {
    match name {
        "set_ao_enabled" => Some("ao_enabled"),
        "set_shadows_enabled" => Some("shadows_enabled"),
        _ => None,
    }
}

fn env_preset_body(name: &str, assignments: &[(&str, &str)]) -> String {
    let mut output = format!(
        "let e = env.clone();\nengine.register_fn({name:?}, move || {{\n\tlet mut e = e.lock().unwrap();\n"
    );
    for (field, value) in assignments {
        output.push_str(&format!("\te.{field} = {value};\n"));
    }
    output.push_str("});");
    output
}

fn run_docs_snippet_tests(root: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["test", "-p", "soyuz-script", "--test", "docs_snippets"])
        .status()
        .context("run docs snippet tests")?;
    if status.success() {
        Ok(())
    } else {
        bail!("docs snippet tests failed with status {status}")
    }
}

fn ts_doc_ident(id: &str) -> String {
    format!(
        "doc_{}",
        id.chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect::<String>()
    )
}

fn json_string(value: &str) -> String {
    match serde_json::to_string(value) {
        Ok(value) => value,
        Err(_) => "\"\"".into(),
    }
}

fn json_string_array(values: &[String]) -> String {
    let items = values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

fn indent_lines(value: &str, indent: &str) -> String {
    if value.trim().is_empty() {
        return String::new();
    }
    let mut output = String::new();
    for line in value.lines() {
        output.push_str(indent);
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn rustfmt_source(source: &str) -> Result<String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("spawn rustfmt for generated Rust docs artifact")?;

    child
        .stdin
        .as_mut()
        .context("open rustfmt stdin")?
        .write_all(source.as_bytes())
        .context("write generated Rust source to rustfmt")?;

    let output = child
        .wait_with_output()
        .context("wait for generated Rust rustfmt")?;
    if !output.status.success() {
        bail!("rustfmt failed for generated Rust docs artifact");
    }

    String::from_utf8(output.stdout).context("generated Rust source should be valid UTF-8")
}

fn generate_app_theme_css(tokens: &ThemeTokens) -> Result<String> {
    let mut css = String::from(
        "/* @generated by cargo run -p xtask -- theme generate. Do not edit directly. */\n\
         \n\
         :root {\n",
    );

    let vars = [
        ("--theme-id", &["meta", "id"][..]),
        ("--theme-name", &["meta", "name"][..]),
        ("--bg-primary", &["workbench", "bgPrimary"][..]),
        ("--bg-secondary", &["workbench", "bgSecondary"][..]),
        ("--bg-tertiary", &["workbench", "bgTertiary"][..]),
        ("--bg-elevated", &["workbench", "bgElevated"][..]),
        ("--bg-input", &["workbench", "bgInput"][..]),
        ("--text-primary", &["workbench", "textPrimary"][..]),
        ("--text-secondary", &["workbench", "textSecondary"][..]),
        ("--text-muted", &["workbench", "textMuted"][..]),
        ("--accent", &["workbench", "accent"][..]),
        ("--accent-hover", &["workbench", "accentHover"][..]),
        ("--success", &["color", "success"][..]),
        ("--warning", &["color", "warning"][..]),
        ("--error", &["color", "error"][..]),
        ("--border", &["workbench", "border"][..]),
        ("--border-strong", &["workbench", "borderStrong"][..]),
        ("--selection-bg", &["workbench", "selection"][..]),
        (
            "--button-text-on-accent",
            &["workbench", "buttonTextOnAccent"][..],
        ),
        ("--hover-bg", &["workbench", "hoverOverlay"][..]),
        ("--subtle-bg", &["workbench", "subtleOverlay"][..]),
        ("--strong-bg", &["workbench", "strongOverlay"][..]),
        ("--shadow-medium", &["workbench", "shadowMedium"][..]),
        ("--shadow-subtle", &["workbench", "shadowSubtle"][..]),
        ("--focus-ring", &["workbench", "focusRing"][..]),
        ("--drop-target-bg", &["workbench", "dropTarget"][..]),
        ("--error-bg", &["workbench", "errorBg"][..]),
        ("--error-subtle-bg", &["workbench", "errorSubtle"][..]),
        ("--warning-subtle-bg", &["workbench", "warningSubtle"][..]),
        ("--danger-bg", &["workbench", "dangerBg"][..]),
        ("--danger-text", &["workbench", "dangerText"][..]),
        ("--toolbar-stop-bg", &["workbench", "toolbarStop"][..]),
        (
            "--toolbar-stop-hover-bg",
            &["workbench", "toolbarStopHover"][..],
        ),
        (
            "--window-close-hover-bg",
            &["workbench", "windowCloseHover"][..],
        ),
        ("--status-bar-bg", &["workbench", "statusBarBg"][..]),
        ("--status-bar-border", &["workbench", "statusBarBorder"][..]),
        ("--status-bar-text", &["workbench", "statusBarText"][..]),
        ("--status-bar-muted", &["workbench", "statusBarMuted"][..]),
        ("--menu-bg", &["workbench", "menuBg"][..]),
        ("--menu-hover-bg", &["workbench", "menuHover"][..]),
        ("--menu-active-bg", &["workbench", "menuActive"][..]),
        ("--menu-disabled-text", &["workbench", "menuDisabled"][..]),
        ("--preview-canvas-bg", &["preview", "canvasBg"][..]),
        (
            "--preview-placeholder-text",
            &["preview", "placeholderText"][..],
        ),
        ("--syntax-keyword", &["syntax", "keyword"][..]),
        ("--syntax-builtin", &["syntax", "builtin"][..]),
        ("--syntax-string", &["syntax", "string"][..]),
        ("--syntax-number", &["syntax", "number"][..]),
        ("--syntax-comment", &["syntax", "comment"][..]),
        ("--syntax-operator", &["syntax", "operator"][..]),
    ];

    for (name, path) in vars {
        css.push_str(&format!("    {name}: {};\n", tokens.str(path)?));
    }
    css.push_str("}\n");
    Ok(css)
}

#[allow(clippy::too_many_lines)]
fn generate_website_theme_css(tokens: &ThemeTokens) -> Result<String> {
    let mut css = String::from(
        "/* @generated by cargo run -p xtask -- theme generate. Do not edit directly. */\n\
         \n\
         @theme {\n",
    );

    let theme_vars = [
        ("--color-bg", &["color", "bg"][..]),
        ("--color-bg-alt", &["color", "bgAlt"][..]),
        ("--color-bg-code", &["color", "bgCode"][..]),
        ("--color-surface", &["color", "surface"][..]),
        ("--color-surface-raised", &["color", "surfaceRaised"][..]),
        ("--color-text", &["color", "text"][..]),
        ("--color-text-muted", &["color", "textMuted"][..]),
        ("--color-text-subtle", &["color", "textSubtle"][..]),
        ("--color-border", &["color", "border"][..]),
        ("--color-border-light", &["color", "borderLight"][..]),
        ("--color-accent", &["color", "accent"][..]),
        ("--color-accent-hover", &["color", "accentHover"][..]),
        ("--color-accent-light", &["color", "accentLight"][..]),
        ("--color-sdf", &["color", "sdf"][..]),
        ("--color-sdf-muted", &["color", "sdfMuted"][..]),
        ("--color-warning", &["color", "warning"][..]),
        ("--color-error", &["color", "error"][..]),
        ("--color-shadow", &["color", "shadow"][..]),
        (
            "--font-sans",
            &[
                "literal",
                "Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif",
            ][..],
        ),
        (
            "--font-mono",
            &[
                "literal",
                "'JetBrains Mono', 'SF Mono', 'Fira Code', Consolas, monospace",
            ][..],
        ),
        ("--shadow-sm", &["website", "shadowSm"][..]),
        ("--shadow-md", &["website", "shadowMd"][..]),
        ("--shadow-lg", &["website", "shadowLg"][..]),
        ("--shadow-accent", &["website", "shadowAccent"][..]),
        ("--breakpoint-xs", &["literal", "480px"][..]),
        ("--breakpoint-sm", &["literal", "640px"][..]),
        ("--breakpoint-md", &["literal", "768px"][..]),
        ("--breakpoint-lg", &["literal", "900px"][..]),
        ("--breakpoint-xl", &["literal", "1100px"][..]),
        ("--breakpoint-2xl", &["literal", "1280px"][..]),
    ];

    for (name, path) in theme_vars {
        css.push_str(&format!(
            "    {name}: {};\n",
            token_or_literal(tokens, path)?
        ));
    }
    css.push_str("}\n\n:root {\n");

    let root_vars = [
        (
            "--color-primary-text-on-accent",
            &["color", "primaryTextOnAccent"][..],
        ),
        ("--color-grid-line", &["website", "gridLine"][..]),
        ("--syntax-keyword", &["syntax", "keyword"][..]),
        ("--syntax-builtin", &["syntax", "builtin"][..]),
        ("--syntax-string", &["syntax", "string"][..]),
        ("--syntax-number", &["syntax", "number"][..]),
        ("--syntax-comment", &["syntax", "comment"][..]),
        ("--syntax-operator", &["syntax", "operator"][..]),
        ("--status-ready-bg", &["status", "readyBg"][..]),
        ("--status-ready-text", &["status", "readyText"][..]),
        ("--status-success-bg", &["status", "successBg"][..]),
        ("--status-success-text", &["status", "successText"][..]),
        ("--status-warning-bg", &["status", "warningBg"][..]),
        ("--status-warning-text", &["status", "warningText"][..]),
        ("--status-error-bg", &["status", "errorBg"][..]),
        ("--status-error-text", &["status", "errorText"][..]),
        ("--preview-canvas-bg", &["preview", "canvasBg"][..]),
        (
            "--preview-placeholder-text",
            &["preview", "placeholderText"][..],
        ),
        ("--renderer-bg", &["preview", "rendererBg"][..]),
        (
            "--renderer-overlay-bg",
            &["preview", "rendererOverlayBg"][..],
        ),
        (
            "--renderer-error-detail",
            &["preview", "rendererErrorDetail"][..],
        ),
        (
            "--renderer-hint-code-bg",
            &["preview", "rendererHintCodeBg"][..],
        ),
        (
            "--renderer-control-text",
            &["preview", "rendererControlText"][..],
        ),
        ("--diagram-box", &["diagram", "box"][..]),
        ("--diagram-line", &["diagram", "line"][..]),
        ("--diagram-highlight", &["diagram", "highlight"][..]),
        ("--diagram-text", &["diagram", "text"][..]),
        ("--diagram-shadow", &["diagram", "shadow"][..]),
    ];

    for (name, path) in root_vars {
        css.push_str(&format!("    {name}: {};\n", tokens.str(path)?));
    }
    css.push_str("}\n");
    Ok(css)
}

fn token_or_literal<'a>(tokens: &'a ThemeTokens, path: &[&'a str]) -> Result<&'a str> {
    if path.first() == Some(&"literal") {
        Ok(path[1])
    } else {
        tokens.str(path)
    }
}

fn generate_website_theme_ts(tokens: &ThemeTokens) -> Result<String> {
    let clear = hex_to_rgb(tokens.str(&["renderer", "clearColor"])?)?;
    let bool_num = |value: bool| if value { "1.0" } else { "0.0" };

    Ok(format!(
        "/* @generated by cargo run -p xtask -- theme generate. Do not edit directly. */\n\
         \n\
         export const soyuzTheme = {{\n\
         \tmeta: {{ id: {id:?}, name: {name:?} }},\n\
         \tcolors: {{\n\
         \t\taccent: {accent:?},\n\
         \t\tsdf: {sdf:?},\n\
         \t\ttext: {text:?},\n\
         \t\ttextMuted: {text_muted:?},\n\
         \t\tborder: {border:?},\n\
         \t}},\n\
         \tstatus: {{\n\
         \t\tready: {{ background: {ready_bg:?}, text: {ready_text:?} }},\n\
         \t\tsuccess: {{ background: {success_bg:?}, text: {success_text:?} }},\n\
         \t\twarning: {{ background: {warning_bg:?}, text: {warning_text:?} }},\n\
         \t\terror: {{ background: {error_bg:?}, text: {error_text:?} }},\n\
         \t}},\n\
         \tdiagram: {{ box: {diagram_box:?}, line: {diagram_line:?}, highlight: {diagram_highlight:?}, text: {diagram_text:?}, shadow: {diagram_shadow:?} }},\n\
         \trenderer: {{\n\
         \t\tclearValue: {{ r: {clear_r}, g: {clear_g}, b: {clear_b}, a: 1.0 }},\n\
         \t\tenvironment: {{\n\
         \t\t\tsunDirection: {sun_direction},\n\
         \t\t\tsunColor: {sun_color},\n\
         \t\t\tsunIntensity: {sun_intensity},\n\
         \t\t\tambientColor: {ambient_color},\n\
         \t\t\tambientIntensity: {ambient_intensity},\n\
         \t\t\tmaterialColor: {material_color},\n\
         \t\t\tmaterialShininess: {material_shininess},\n\
         \t\t\tspecularIntensity: {specular_intensity},\n\
         \t\t\tskyHorizon: {sky_horizon},\n\
         \t\t\tskyZenith: {sky_zenith},\n\
         \t\t\tfogColor: {fog_color},\n\
         \t\t\tfogDensity: {fog_density},\n\
         \t\t\taoEnabled: {ao_enabled},\n\
         \t\t\taoIntensity: {ao_intensity},\n\
         \t\t\tshadowsEnabled: {shadows_enabled},\n\
         \t\t\tshadowSoftness: {shadow_softness},\n\
         \t\t}},\n\
         \t}},\n\
         }} as const;\n\
         \n\
         export type SoyuzTheme = typeof soyuzTheme;\n",
        id = tokens.str(&["meta", "id"])?,
        name = tokens.str(&["meta", "name"])?,
        accent = tokens.str(&["color", "accent"])?,
        sdf = tokens.str(&["color", "sdf"])?,
        text = tokens.str(&["color", "text"])?,
        text_muted = tokens.str(&["color", "textMuted"])?,
        border = tokens.str(&["color", "border"])?,
        ready_bg = tokens.str(&["status", "readyBg"])?,
        ready_text = tokens.str(&["status", "readyText"])?,
        success_bg = tokens.str(&["status", "successBg"])?,
        success_text = tokens.str(&["status", "successText"])?,
        warning_bg = tokens.str(&["status", "warningBg"])?,
        warning_text = tokens.str(&["status", "warningText"])?,
        error_bg = tokens.str(&["status", "errorBg"])?,
        error_text = tokens.str(&["status", "errorText"])?,
        diagram_box = tokens.str(&["diagram", "box"])?,
        diagram_line = tokens.str(&["diagram", "line"])?,
        diagram_highlight = tokens.str(&["diagram", "highlight"])?,
        diagram_text = tokens.str(&["diagram", "text"])?,
        diagram_shadow = tokens.str(&["diagram", "shadow"])?,
        clear_r = fmt_float(clear[0]),
        clear_g = fmt_float(clear[1]),
        clear_b = fmt_float(clear[2]),
        sun_direction = fmt_array(tokens.vec3(&["renderer", "sunDirection"])?),
        sun_color = fmt_array(tokens.vec3(&["renderer", "sunColor"])?),
        sun_intensity = fmt_float(tokens.num(&["renderer", "sunIntensity"])?),
        ambient_color = fmt_array(tokens.vec3(&["renderer", "ambientColor"])?),
        ambient_intensity = fmt_float(tokens.num(&["renderer", "ambientIntensity"])?),
        material_color = fmt_array(tokens.vec3(&["renderer", "materialColor"])?),
        material_shininess = fmt_float(tokens.num(&["renderer", "materialShininess"])?),
        specular_intensity = fmt_float(tokens.num(&["renderer", "specularIntensity"])?),
        sky_horizon = fmt_array(tokens.vec3(&["renderer", "skyHorizon"])?),
        sky_zenith = fmt_array(tokens.vec3(&["renderer", "skyZenith"])?),
        fog_color = fmt_array(tokens.vec3(&["renderer", "fogColor"])?),
        fog_density = fmt_float(tokens.num(&["renderer", "fogDensity"])?),
        ao_enabled = bool_num(tokens.bool(&["renderer", "aoEnabled"])?),
        ao_intensity = fmt_float(tokens.num(&["renderer", "aoIntensity"])?),
        shadows_enabled = bool_num(tokens.bool(&["renderer", "shadowsEnabled"])?),
        shadow_softness = fmt_float(tokens.num(&["renderer", "shadowSoftness"])?),
    ))
}

fn generate_render_theme_rs(tokens: &ThemeTokens) -> Result<String> {
    let clear = hex_to_rgb(tokens.str(&["renderer", "clearColor"])?)?;

    Ok(format!(
        concat!(
            "// @generated by cargo run -p xtask -- theme generate. Do not edit directly.\n",
            "#![allow(clippy::unreadable_literal)]\n",
            "\n",
            "use soyuz_sdf::Environment;\n",
            "\n",
            "pub const RENDER_CLEAR_COLOR: wgpu::Color = wgpu::Color {{\n",
            "    r: {clear_r},\n",
            "    g: {clear_g},\n",
            "    b: {clear_b},\n",
            "    a: 1.0,\n",
            "}};\n",
            "\n",
            "#[must_use]\n",
            "pub fn default_environment() -> Environment {{\n",
            "    Environment {{\n",
            "        sun_direction: {sun_direction},\n",
            "        sun_color: {sun_color},\n",
            "        sun_intensity: {sun_intensity},\n",
            "        ambient_color: {ambient_color},\n",
            "        ambient_intensity: {ambient_intensity},\n",
            "        material_color: {material_color},\n",
            "        material_shininess: {material_shininess},\n",
            "        specular_intensity: {specular_intensity},\n",
            "        sky_horizon: {sky_horizon},\n",
            "        sky_zenith: {sky_zenith},\n",
            "        fog_color: {fog_color},\n",
            "        fog_density: {fog_density},\n",
            "        ao_enabled: {ao_enabled},\n",
            "        ao_intensity: {ao_intensity},\n",
            "        shadows_enabled: {shadows_enabled},\n",
            "        shadow_softness: {shadow_softness},\n",
            "    }}\n",
            "}}\n"
        ),
        clear_r = fmt_float(clear[0]),
        clear_g = fmt_float(clear[1]),
        clear_b = fmt_float(clear[2]),
        sun_direction = fmt_array(tokens.vec3(&["renderer", "sunDirection"])?),
        sun_color = fmt_array(tokens.vec3(&["renderer", "sunColor"])?),
        sun_intensity = fmt_float(tokens.num(&["renderer", "sunIntensity"])?),
        ambient_color = fmt_array(tokens.vec3(&["renderer", "ambientColor"])?),
        ambient_intensity = fmt_float(tokens.num(&["renderer", "ambientIntensity"])?),
        material_color = fmt_array(tokens.vec3(&["renderer", "materialColor"])?),
        material_shininess = fmt_float(tokens.num(&["renderer", "materialShininess"])?),
        specular_intensity = fmt_float(tokens.num(&["renderer", "specularIntensity"])?),
        sky_horizon = fmt_array(tokens.vec3(&["renderer", "skyHorizon"])?),
        sky_zenith = fmt_array(tokens.vec3(&["renderer", "skyZenith"])?),
        fog_color = fmt_array(tokens.vec3(&["renderer", "fogColor"])?),
        fog_density = fmt_float(tokens.num(&["renderer", "fogDensity"])?),
        ao_enabled = tokens.bool(&["renderer", "aoEnabled"])?,
        ao_intensity = fmt_float(tokens.num(&["renderer", "aoIntensity"])?),
        shadows_enabled = tokens.bool(&["renderer", "shadowsEnabled"])?,
        shadow_softness = fmt_float(tokens.num(&["renderer", "shadowSoftness"])?),
    ))
}

fn hex_to_rgb(hex: &str) -> Result<[f64; 3]> {
    let hex = hex
        .strip_prefix('#')
        .with_context(|| format!("expected hex color, got {hex}"))?;
    if hex.len() != 6 {
        bail!("expected 6-digit hex color, got #{hex}");
    }
    let red = u8::from_str_radix(&hex[0..2], 16).context("parse red channel")?;
    let green = u8::from_str_radix(&hex[2..4], 16).context("parse green channel")?;
    let blue = u8::from_str_radix(&hex[4..6], 16).context("parse blue channel")?;
    Ok([
        f64::from(red) / 255.0,
        f64::from(green) / 255.0,
        f64::from(blue) / 255.0,
    ])
}

fn fmt_array(values: [f64; 3]) -> String {
    format!(
        "[{}, {}, {}]",
        fmt_float(values[0]),
        fmt_float(values[1]),
        fmt_float(values[2])
    )
}

fn fmt_float(value: f64) -> String {
    let mut value = format!("{value:.6}");
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.push('0');
    }
    value
}

fn check_raw_ui_colors(root: &Path, website: &Path) -> Result<()> {
    let mut violations = Vec::new();
    for (path, prefix) in [
        (root.join("app/src"), "app/src/"),
        (root.join("app/assets"), "app/assets/"),
        (
            root.join("crates/soyuz-render/src"),
            "crates/soyuz-render/src/",
        ),
        (website.join("src"), "soyuz-website/src/"),
        (website.join("static"), "soyuz-website/static/"),
    ] {
        collect_raw_color_violations(&path, &path, prefix, &mut violations)?;
    }

    if !violations.is_empty() {
        for violation in &violations {
            eprintln!("{violation}");
        }
        bail!("raw UI color literals found outside generated theme artifacts");
    }

    Ok(())
}

fn collect_raw_color_violations(
    base: &Path,
    current: &Path,
    prefix: &str,
    violations: &mut Vec<String>,
) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("read directory {}", current.display()))?
    {
        let entry = entry.context("read directory entry")?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if path.is_dir() {
            if should_skip_theme_dir(&name) {
                continue;
            }
            collect_raw_color_violations(base, &path, prefix, violations)?;
            continue;
        }

        if !is_theme_checked_file(&path) {
            continue;
        }

        let rel = path
            .strip_prefix(base)
            .with_context(|| format!("strip prefix for {}", path.display()))?;
        let rel_display = format!("{prefix}{}", rel.display());
        if is_theme_color_allowlisted(&rel_display) {
            continue;
        }

        let contents =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        for (index, line) in contents.lines().enumerate() {
            if line_contains_raw_color(line) {
                violations.push(format!("{}:{}: {}", rel_display, index + 1, line.trim()));
            }
        }
    }

    Ok(())
}

fn should_skip_theme_dir(name: &str) -> bool {
    matches!(
        name,
        "target" | "node_modules" | ".svelte-kit" | "build" | "dist" | ".git"
    )
}

fn is_theme_checked_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("css" | "svelte" | "ts" | "tsx" | "js" | "rs")
    )
}

fn is_theme_color_allowlisted(path: &str) -> bool {
    path == "app/assets/theme.css"
        || path == "app/src/docs_generated.rs"
        || path == "crates/soyuz-render/src/theme_generated.rs"
        || path == "crates/soyuz-render/src/text_overlay.rs"
        || path == "theme/soyuz-graphite.tokens.json"
        || path == "soyuz-website/src/styles/generated-theme.css"
        || path == "soyuz-website/src/lib/theme.generated.ts"
        || path == "soyuz-website/src/routes/about/+page.svelte"
        || path == "soyuz-website/src/lib/assets/favicon.svg"
        || path == "soyuz-website/static/favicon.svg"
}

fn line_contains_raw_color(line: &str) -> bool {
    contains_hex_color(line) || contains_css_color_function(line)
}

fn contains_hex_color(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }

        let previous = index.checked_sub(1).map(|idx| bytes[idx]);
        if previous == Some(b'{') {
            index += 1;
            continue;
        }

        let mut end = index + 1;
        while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
            end += 1;
        }
        let len = end - index - 1;
        if matches!(len, 3 | 6 | 8) {
            return true;
        }
        index = end.max(index + 1);
    }
    false
}

fn contains_css_color_function(line: &str) -> bool {
    for needle in ["rgb(", "rgba("] {
        let mut offset = 0;
        while let Some(found) = line[offset..].find(needle) {
            let index = offset + found;
            let previous = index
                .checked_sub(1)
                .and_then(|idx| line.as_bytes().get(idx));
            if !matches!(
                previous,
                Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
            ) {
                return true;
            }
            offset = index + needle.len();
        }
    }
    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokeMode {
    Fast,
    Graphics,
}

#[derive(Debug)]
struct SmokeOptions {
    mode: SmokeMode,
    studio_bin: Option<PathBuf>,
}

impl SmokeOptions {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut mode = SmokeMode::Fast;
        let mut studio_bin = None;
        let mut args = args.peekable();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--mode" => {
                    let Some(value) = args.next() else {
                        bail!("missing value for --mode");
                    };
                    mode = match value.as_str() {
                        "fast" => SmokeMode::Fast,
                        "graphics" => SmokeMode::Graphics,
                        _ => bail!("invalid --mode value: {value}"),
                    };
                }
                "--studio-bin" => {
                    let Some(value) = args.next() else {
                        bail!("missing value for --studio-bin");
                    };
                    studio_bin = Some(PathBuf::from(value));
                }
                _ => bail!("unknown studio-smoke argument: {arg}"),
            }
        }

        Ok(Self { mode, studio_bin })
    }
}

fn run_studio_smoke(options: SmokeOptions) -> Result<()> {
    let root = workspace_root()?;
    let smoke_script = root.join("examples/smoke.rhai");
    ensure_file(&smoke_script)?;

    let studio_bin = if let Some(path) = options.studio_bin {
        path
    } else {
        run_status(
            "dx check",
            Command::new("dx").current_dir(&root).args([
                "check",
                "--desktop",
                "--package",
                "soyuz-app",
                "--bin",
                "soyuz-studio",
                "--features",
                "desktop",
            ]),
        )?;
        run_status(
            "dx build",
            Command::new("dx").current_dir(&root).args([
                "build",
                "--desktop",
                "--package",
                "soyuz-app",
                "--bin",
                "soyuz-studio",
                "--features",
                "desktop",
            ]),
        )?;
        default_dx_studio_bin(&root)
    };

    ensure_file(&studio_bin)?;

    match options.mode {
        SmokeMode::Fast => run_fast_smoke(&studio_bin, &smoke_script),
        SmokeMode::Graphics => run_graphics_smoke(&studio_bin, &smoke_script),
    }
}

fn run_fast_smoke(studio_bin: &Path, smoke_script: &Path) -> Result<()> {
    run_status(
        "preview check-only",
        Command::new(studio_bin)
            .args(["--preview", "--script"])
            .arg(smoke_script)
            .arg("--check-only"),
    )?;

    let temp_dir = tempfile::tempdir().context("create smoke tempdir")?;
    let export_path = temp_dir.path().join("smoke.stl");
    run_status(
        "export check",
        Command::new(studio_bin)
            .arg("--export-check")
            .arg("--script")
            .arg(smoke_script)
            .arg("--out")
            .arg(&export_path)
            .arg("--resolution")
            .arg("16"),
    )?;

    let metadata = std::fs::metadata(&export_path)
        .with_context(|| format!("read exported smoke file {}", export_path.display()))?;
    if metadata.len() == 0 {
        bail!("exported smoke file is empty: {}", export_path.display());
    }

    Ok(())
}

fn run_graphics_smoke(studio_bin: &Path, smoke_script: &Path) -> Result<()> {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        bail!("graphics smoke requires DISPLAY or WAYLAND_DISPLAY");
    }

    let mut child = Command::new(studio_bin)
        .arg("--preview")
        .arg("--script")
        .arg(smoke_script)
        .spawn()
        .context("spawn graphics preview")?;

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().context("poll graphics preview")? {
            bail!("graphics preview exited early with status {status}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    child.kill().context("stop graphics preview")?;
    let _status = child.wait().context("wait for graphics preview exit")?;
    Ok(())
}

fn run_status(label: &str, command: &mut Command) -> Result<()> {
    reject_runtime_cargo_or_preview_helper(label, command)?;

    let status = command
        .status()
        .with_context(|| format!("run {label}: {command:?}"))?;
    if !status.success() {
        bail!("{label} failed with status {status}");
    }

    Ok(())
}

fn reject_runtime_cargo_or_preview_helper(label: &str, command: &Command) -> Result<()> {
    let program = command.get_program().to_string_lossy();
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    if label != "dx check" && label != "dx build" && program == "cargo" {
        bail!("{label} must not invoke cargo at runtime");
    }
    if program.contains("soyuz-preview") || args.iter().any(|arg| arg.contains("soyuz-preview")) {
        bail!("{label} must not invoke soyuz-preview");
    }

    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    let output = Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        bail!("cargo metadata failed with status {}", output.status);
    }

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse cargo metadata")?;
    let root = metadata
        .get("workspace_root")
        .and_then(serde_json::Value::as_str)
        .context("cargo metadata did not include workspace_root")?;
    Ok(PathBuf::from(root))
}

fn default_dx_studio_bin(root: &Path) -> PathBuf {
    let exe = if cfg!(windows) {
        "soyuz-studio.exe"
    } else {
        "soyuz-studio"
    };
    root.join("target")
        .join("dx")
        .join("soyuz-studio")
        .join("debug")
        .join(platform_dir())
        .join("app")
        .join(exe)
}

fn platform_dir() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn ensure_file(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        bail!("expected file does not exist: {}", path.display())
    }
}

fn print_usage() {
    let program = std::env::args_os()
        .next()
        .unwrap_or_else(|| OsString::from("xtask"));
    eprintln!(
        "Usage:\n  {} theme generate [--website PATH]\n  {} theme check [--website PATH]\n  {} docs generate [--website PATH]\n  {} docs check [--website PATH]\n  {} studio-smoke --mode fast [--studio-bin PATH]\n  {} studio-smoke --mode graphics [--studio-bin PATH]",
        Path::new(&program).display(),
        Path::new(&program).display(),
        Path::new(&program).display(),
        Path::new(&program).display(),
        Path::new(&program).display(),
        Path::new(&program).display(),
    );
}
