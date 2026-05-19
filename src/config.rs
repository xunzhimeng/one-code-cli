use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::error::{OccError, OccResult};
use crate::output;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArgsStrategy {
    #[default]
    Builtin,
    Append,
    Override,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptVia {
    Stdin,
    Arg,
    File,
    FileIndirection,
    ArgOrFileIndirection,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnvMode {
    #[default]
    Inherit,
    Strict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_proxy_env_keys")]
    pub env_keys: Vec<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            env_keys: default_proxy_env_keys(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimeoutsConfig {
    pub default_run: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(rename = "cli_type")]
    pub backend: String,
    pub command: Option<String>,
    pub path: Option<PathBuf>,
    pub model: Option<String>,
    #[serde(alias = "reasoning_effort")]
    pub effort: Option<String>,
    pub default_timeout: Option<String>,
    pub config_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "is_inherit_env_mode")]
    pub env_mode: EnvMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_allowlist: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub args_strategy: ArgsStrategy,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    pub prompt_via: Option<PromptVia>,
    #[serde(default)]
    pub resume_args: Vec<String>,
    #[serde(default)]
    pub interactive_args: Vec<String>,
    #[serde(default)]
    pub non_interactive_args: Vec<String>,
    #[serde(skip, default)]
    pub builtin: bool,
}

impl Profile {
    fn builtin_new(name: &str, prompt_via: Option<PromptVia>) -> Self {
        Self {
            name: name.to_string(),
            aliases: Vec::new(),
            backend: name.to_string(),
            command: Some(default_command_name(name).to_string()),
            path: None,
            model: None,
            effort: None,
            default_timeout: None,
            config_dir: None,
            env_mode: EnvMode::Inherit,
            env_allowlist: Vec::new(),
            env: BTreeMap::new(),
            args_strategy: ArgsStrategy::Builtin,
            args: Vec::new(),
            extra_args: Vec::new(),
            prompt_via,
            resume_args: Vec::new(),
            interactive_args: Vec::new(),
            non_interactive_args: Vec::new(),
            builtin: true,
        }
    }
}

fn is_inherit_env_mode(mode: &EnvMode) -> bool {
    *mode == EnvMode::Inherit
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigFile {
    pub version: Option<u32>,
    #[serde(rename = "default_agent")]
    pub default_profile: Option<String>,
    pub doc_root: Option<PathBuf>,
    pub proxy: Option<ProxyConfig>,
    pub timeouts: Option<TimeoutsConfig>,
    #[serde(rename = "cli_type_defaults", default)]
    pub backend_defaults: BTreeMap<String, String>,
    #[serde(rename = "cli_type_aliases", default)]
    pub backend_aliases: BTreeMap<String, String>,
    #[serde(rename = "agents", default)]
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EffectiveConfig {
    pub version: u32,
    #[serde(rename = "default_agent")]
    pub default_profile: Option<String>,
    pub doc_root: PathBuf,
    pub proxy: ProxyConfig,
    pub timeouts: TimeoutsConfig,
    #[serde(rename = "cli_type_defaults")]
    pub backend_defaults: BTreeMap<String, String>,
    #[serde(rename = "cli_type_aliases")]
    pub backend_aliases: BTreeMap<String, String>,
    #[serde(rename = "agents")]
    pub profiles: Vec<Profile>,
    pub loaded_paths: Vec<PathBuf>,
    pub search_paths: Vec<PathBuf>,
}

impl EffectiveConfig {
    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| {
            profile.name == name || profile.aliases.iter().any(|alias| alias == name)
        })
    }

    pub fn profiles_for_backend<'a>(
        &'a self,
        backend: &'a str,
    ) -> impl Iterator<Item = &'a Profile> + 'a {
        self.profiles
            .iter()
            .filter(move |profile| profile.backend == backend)
    }

    pub fn resolve_profile(
        &self,
        profile: Option<&str>,
        backend: Option<&str>,
    ) -> OccResult<Profile> {
        if let Some(name) = profile {
            return self.profile(name).cloned().ok_or_else(|| {
                OccError::new(
                    "profile_not_found",
                    format!("Agent '{}' was not found.", name),
                )
            });
        }

        if let Some(backend_name) = backend {
            let backend_name = self
                .backend_aliases
                .get(backend_name)
                .map(String::as_str)
                .unwrap_or(backend_name);
            if let Some(default_profile) = self.backend_defaults.get(backend_name) {
                return self.profile(default_profile).cloned().ok_or_else(|| {
                    OccError::new(
                        "profile_not_found",
                        format!(
                            "CLI '{}' default agent '{}' was not found.",
                            backend_name, default_profile
                        ),
                    )
                });
            }

            return self
                .profiles_for_backend(backend_name)
                .next()
                .cloned()
                .ok_or_else(|| {
                    OccError::new(
                        "backend_not_found",
                        format!("CLI '{}' was not found.", backend_name),
                    )
                });
        }

        let default_profile = self.default_profile.as_deref().ok_or_else(|| {
            let available: Vec<&str> = self.profiles.iter().map(|p| p.name.as_str()).collect();
            OccError::new(
                "no_cli_selected",
                format!(
                    "No --agent, --cli, or default_agent was provided.\n  \
                     Available agents: {}\n  \
                     Set default_agent in ~/.occ/config.toml or pass --cli <name>.\n  \
                     Run `occ config init --user` to create a config file.",
                    available.join(", ")
                ),
            )
        })?;

        self.profile(default_profile).cloned().ok_or_else(|| {
            OccError::new(
                "profile_not_found",
                format!("Default agent '{}' was not found.", default_profile),
            )
        })
    }

    pub fn resolved_doc_root(&self, cwd: &Path, override_doc_root: Option<&PathBuf>) -> PathBuf {
        let path = override_doc_root
            .cloned()
            .unwrap_or_else(|| self.doc_root.clone());
        if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        }
    }
}

pub fn load(config_arg: Option<&PathBuf>, cwd: &Path) -> OccResult<EffectiveConfig> {
    let search_paths = search_paths(cwd);
    let source_paths = if let Some(path) = config_arg {
        if !path.exists() {
            return Err(OccError::new(
                "config_not_found",
                format!(
                    "Config file '{}' was not found.",
                    output::display_path(path)
                ),
            ));
        }
        vec![path.clone()]
    } else {
        search_paths
            .iter()
            .filter(|path| path.exists())
            .cloned()
            .collect::<Vec<_>>()
    };

    let mut parsed = Vec::new();
    for path in source_paths.iter().rev() {
        let text = fs::read_to_string(path).map_err(|error| {
            OccError::io(
                "config_parse_failed",
                format!("Failed to read '{}'", output::display_path(path)),
                error,
            )
        })?;
        let mut config: ConfigFile = toml::from_str(&text).map_err(|error| {
            OccError::new(
                "config_parse_failed",
                format!(
                    "Failed to parse '{}': {}",
                    output::display_path(path),
                    error
                ),
            )
        })?;
        normalize_profiles(&mut config.profiles);
        parsed.push((path.clone(), config));
    }

    let mut version = 1;
    let mut default_profile = None;
    let mut doc_root = default_doc_root()?;
    let mut proxy = ProxyConfig::default();
    let mut timeouts = TimeoutsConfig::default();
    let mut backend_defaults = BTreeMap::new();
    let mut backend_aliases = BTreeMap::new();

    for (_, config) in &parsed {
        if let Some(value) = config.version {
            version = value;
        }
        if let Some(value) = &config.default_profile {
            default_profile = Some(value.clone());
        }
        if let Some(value) = &config.doc_root {
            doc_root = value.clone();
        }
        if let Some(value) = &config.proxy {
            proxy = value.clone();
        }
        if let Some(value) = &config.timeouts {
            timeouts = value.clone();
        }
        backend_defaults.extend(config.backend_defaults.clone());
        backend_aliases.extend(config.backend_aliases.clone());
    }

    let mut seen = BTreeSet::new();
    let mut profiles = Vec::new();
    for (_, config) in parsed.iter().rev() {
        for profile in &config.profiles {
            if seen.insert(profile.name.clone()) {
                profiles.push(profile.clone());
            }
        }
    }
    for profile in builtin_profiles() {
        if seen.insert(profile.name.clone()) {
            profiles.push(profile);
        }
    }

    Ok(EffectiveConfig {
        version,
        default_profile,
        doc_root,
        proxy,
        timeouts,
        backend_defaults,
        backend_aliases,
        profiles,
        loaded_paths: parsed.into_iter().map(|(path, _)| path).collect(),
        search_paths,
    })
}

pub fn search_paths(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = vec![cwd.join(".occ.toml"), cwd.join(".occ").join("config.toml")];
    if let Some(base_dirs) = BaseDirs::new() {
        paths.push(base_dirs.home_dir().join(".occ").join("config.toml"));
    }
    paths
}

pub fn default_project_config_path(cwd: &Path) -> PathBuf {
    cwd.join(".occ").join("config.toml")
}

pub fn default_user_config_path() -> OccResult<PathBuf> {
    BaseDirs::new()
        .map(|base_dirs| base_dirs.home_dir().join(".occ").join("config.toml"))
        .ok_or_else(|| {
            OccError::new(
                "home_not_found",
                "Unable to locate the user home directory.",
            )
        })
}

pub fn write_sample_config(path: &Path, force: bool) -> OccResult<()> {
    if path.exists() && !force {
        return Err(OccError::new(
            "config_already_exists",
            format!(
                "Config file '{}' already exists. Use --force to overwrite it.",
                output::display_path(path)
            ),
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to create '{}'", output::display_path(parent)),
                error,
            )
        })?;
    }

    fs::write(path, sample_config_toml()).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to write '{}'", output::display_path(path)),
            error,
        )
    })
}

pub fn read_config_file(path: &Path) -> OccResult<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile {
            version: Some(1),
            ..ConfigFile::default()
        });
    }

    let text = fs::read_to_string(path).map_err(|error| {
        OccError::io(
            "config_parse_failed",
            format!("Failed to read '{}'", output::display_path(path)),
            error,
        )
    })?;
    let mut config: ConfigFile = toml::from_str(&text).map_err(|error| {
        OccError::new(
            "config_parse_failed",
            format!(
                "Failed to parse '{}': {}",
                output::display_path(path),
                error
            ),
        )
    })?;
    if config.version.is_none() {
        config.version = Some(1);
    }
    normalize_profiles(&mut config.profiles);
    Ok(config)
}

pub fn write_config_file(path: &Path, config: &ConfigFile) -> OccResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to create '{}'", output::display_path(parent)),
                error,
            )
        })?;
    }
    let text = toml::to_string_pretty(config).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize config TOML: {}", error),
        )
    })?;
    fs::write(path, text).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to write '{}'", output::display_path(path)),
            error,
        )
    })
}

pub fn sample_config_toml() -> String {
    format!(
        r#"version = 1
default_agent = "claude"

[proxy]
enabled = true
env_keys = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY", "http_proxy", "https_proxy", "all_proxy", "no_proxy"]

[timeouts]
default_run = "none"

[cli_type_defaults]
claude = "claude"
codex = "codex"
opencode = "opencode"
gemini = "gemini"

[cli_type_aliases]
c = "claude"
x = "codex"
o = "opencode"
g = "gemini"

[[agents]]
name = "claude"
aliases = ["claude-cc"]
cli_type = "claude"
command = "{}"
args_strategy = "builtin"
prompt_via = "stdin"
non_interactive_args = ["--print", "--dangerously-skip-permissions"]
interactive_args = ["--dangerously-skip-permissions"]

# Same Claude Code CLI, separate agent config. Keep API tokens in shell env.
[[agents]]
name = "deepseek-cc"
cli_type = "claude"
command = "{}"
model = "deepseek-chat"
env = {{ ANTHROPIC_BASE_URL = "https://api.deepseek.com/anthropic" }}
args_strategy = "builtin"
prompt_via = "stdin"
non_interactive_args = ["--print", "--dangerously-skip-permissions"]
interactive_args = ["--dangerously-skip-permissions"]

[[agents]]
name = "codex"
cli_type = "codex"
command = "{}"
args_strategy = "builtin"
prompt_via = "stdin"
non_interactive_args = ["exec", "--dangerously-bypass-approvals-and-sandbox", "--skip-git-repo-check"]
interactive_args = ["--dangerously-bypass-approvals-and-sandbox", "--skip-git-repo-check"]

[[agents]]
name = "opencode"
cli_type = "opencode"
command = "{}"
args_strategy = "builtin"
prompt_via = "arg-or-file-indirection"
non_interactive_args = ["run", "--dangerously-skip-permissions"]

[[agents]]
name = "gemini"
cli_type = "gemini"
command = "{}"
args_strategy = "builtin"
prompt_via = "arg-or-file-indirection"
non_interactive_args = ["--yolo", "--skip-trust"]
interactive_args = ["--yolo", "--skip-trust"]
"#,
        default_command_name("claude"),
        default_command_name("claude"),
        default_command_name("codex"),
        default_command_name("opencode"),
        default_command_name("gemini")
    )
}

fn default_doc_root() -> OccResult<PathBuf> {
    BaseDirs::new()
        .map(|base_dirs| base_dirs.home_dir().join(".occ"))
        .ok_or_else(|| {
            OccError::new(
                "home_not_found",
                "Unable to locate the user home directory for the default doc_root.",
            )
        })
}

pub fn editable_config_file(config: &EffectiveConfig) -> ConfigFile {
    ConfigFile {
        version: Some(config.version),
        default_profile: config.default_profile.clone(),
        doc_root: Some(config.doc_root.clone()),
        proxy: Some(config.proxy.clone()),
        timeouts: Some(config.timeouts.clone()),
        backend_defaults: config.backend_defaults.clone(),
        backend_aliases: config.backend_aliases.clone(),
        profiles: config.profiles.clone(),
    }
}

pub fn editable_config_toml(config: &EffectiveConfig) -> OccResult<String> {
    let file = editable_config_file(config);
    toml::to_string_pretty(&file).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize editable config: {}", error),
        )
    })
}

pub fn builtin_profiles() -> Vec<Profile> {
    vec![
        Profile::builtin_new("claude", Some(PromptVia::Stdin)),
        Profile::builtin_new("codex", Some(PromptVia::Stdin)),
        Profile::builtin_new("opencode", None),
        Profile::builtin_new("gemini", None),
    ]
}

fn normalize_profiles(profiles: &mut [Profile]) {
    for profile in profiles {
        profile.builtin = false;
    }
}

fn default_command_name(command: &str) -> &str {
    if cfg!(windows) {
        match command {
            "claude" => "claude.cmd",
            "codex" => "codex.cmd",
            "opencode" => "opencode.cmd",
            "gemini" => "gemini.cmd",
            _ => command,
        }
    } else {
        command
    }
}

fn default_true() -> bool {
    true
}

pub fn default_proxy_env_keys() -> Vec<String> {
    [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
    ]
    .iter()
    .map(|value| (*value).to_string())
    .collect()
}
