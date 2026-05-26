use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct DetectedCli {
    pub source_path: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub model: Option<String>,
    pub effort: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct DetectedDefaults {
    pub claude: DetectedCli,
    pub codex: DetectedCli,
    pub opencode: DetectedCli,
    pub gemini: DetectedCli,
}

pub fn detect() -> DetectedDefaults {
    let home = BaseDirs::new().map(|b| b.home_dir().to_path_buf());
    DetectedDefaults {
        claude: home.as_deref().map(detect_claude).unwrap_or_default(),
        codex: home.as_deref().map(detect_codex).unwrap_or_default(),
        opencode: home.as_deref().map(detect_opencode).unwrap_or_default(),
        gemini: home.as_deref().map(detect_gemini).unwrap_or_default(),
    }
}

pub fn detect_for_cli(backend: &str, config_dir: Option<&Path>) -> Option<DetectedCli> {
    if let Some(config_dir) = config_dir {
        return match backend {
            "claude" => Some(detect_claude_config_dir(config_dir)),
            "codex" => Some(detect_codex_home(config_dir.to_path_buf())),
            "opencode" => Some(detect_opencode_config_dir(config_dir)),
            "gemini" => Some(detect_gemini(config_dir)),
            _ => None,
        };
    }

    let detected = detect();
    match backend {
        "claude" => Some(detected.claude),
        "codex" => Some(detected.codex),
        "opencode" => Some(detected.opencode),
        "gemini" => Some(detected.gemini),
        _ => None,
    }
}

fn detect_claude(home: &Path) -> DetectedCli {
    detect_claude_settings(home.join(".claude").join("settings.json"))
}

fn detect_claude_config_dir(config_dir: &Path) -> DetectedCli {
    detect_claude_settings(config_dir.join("settings.json"))
}

fn detect_claude_settings(path: PathBuf) -> DetectedCli {
    let mut out = DetectedCli::default();
    let Ok(text) = fs::read_to_string(&path) else {
        return out;
    };
    out.source_path = Some(path);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return out;
    };
    if let Some(env) = value.get("env").and_then(|v| v.as_object()) {
        for key in [
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_SMALL_FAST_MODEL",
            "CLAUDE_CODE_EFFORT_LEVEL",
            "ENABLE_TOOL_SEARCH",
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
            "CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK",
            "DISABLE_AUTOUPDATER",
        ] {
            if let Some(v) = env.get(key).and_then(|v| v.as_str()) {
                out.env.insert(key.to_string(), v.to_string());
            }
        }
    }
    if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
        out.model = Some(model.to_string());
    }
    if let Some(effort) = value.get("effortLevel").and_then(|v| v.as_str()) {
        out.effort = Some(effort.to_string());
    } else if let Some(effort) = out.env.get("CLAUDE_CODE_EFFORT_LEVEL") {
        out.effort = Some(effort.clone());
    }
    out
}

fn detect_codex(home: &Path) -> DetectedCli {
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    detect_codex_home(codex_home)
}

fn detect_codex_home(codex_home: PathBuf) -> DetectedCli {
    let mut out = DetectedCli::default();
    let path = codex_home.join("config.toml");
    if let Ok(text) = fs::read_to_string(&path) {
        out.source_path = Some(path);
        if let Ok(value) = text.parse::<toml::Value>() {
            if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                out.model = Some(model.to_string());
            }
            if let Some(effort) = value.get("model_reasoning_effort").and_then(|v| v.as_str()) {
                out.effort = Some(effort.to_string());
            }
            if let Some(provider) = value.get("model_provider").and_then(|v| v.as_str()) {
                out.env
                    .insert("CODEX_MODEL_PROVIDER".to_string(), provider.to_string());
                if let Some(table) = value
                    .get("model_providers")
                    .and_then(|v| v.as_table())
                    .and_then(|t| t.get(provider))
                    .and_then(|v| v.as_table())
                {
                    if let Some(base) = table.get("base_url").and_then(|v| v.as_str()) {
                        out.env
                            .insert("OPENAI_BASE_URL".to_string(), base.to_string());
                    }
                    if let Some(env_key) = table.get("env_key").and_then(|v| v.as_str()) {
                        out.env
                            .insert("CODEX_PROVIDER_ENV_KEY".to_string(), env_key.to_string());
                        if let Ok(value) = std::env::var(env_key) {
                            if !value.trim().is_empty() {
                                out.env.insert(env_key.to_string(), value);
                            }
                        }
                    }
                    if let Some(wire_api) = table.get("wire_api").and_then(|v| v.as_str()) {
                        out.env
                            .insert("CODEX_WIRE_API".to_string(), wire_api.to_string());
                    }
                }
            }
        }
    }
    let auth_path = codex_home.join("auth.json");
    if let Ok(text) = fs::read_to_string(&auth_path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(key) = v.get("OPENAI_API_KEY").and_then(|v| v.as_str()) {
                out.env
                    .insert("OPENAI_API_KEY".to_string(), key.to_string());
            }
        }
    }
    for key in [
        "OPENAI_API_KEY",
        "CODEX_MODEL_PROVIDER",
        "OPENAI_BASE_URL",
        "CODEX_PROVIDER_ENV_KEY",
        "CODEX_WIRE_API",
        "AZURE_OPENAI_API_KEY",
        "OPENAI_ORG_ID",
        "OPENAI_PROJECT_ID",
        "OPENAI_TIMEOUT_MS",
    ] {
        if out.env.contains_key(key) {
            continue;
        }
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                out.env.insert(key.to_string(), value);
            }
        }
    }
    out
}

fn detect_opencode(home: &Path) -> DetectedCli {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("OPENCODE_CONFIG") {
        candidates.push(PathBuf::from(path));
    }
    candidates.extend([
        home.join(".config").join("opencode").join("opencode.json"),
        home.join(".config").join("opencode").join("config.json"),
        home.join(".opencode").join("opencode.json"),
        home.join(".opencode").join("config.json"),
    ]);
    detect_opencode_candidates(candidates)
}

fn detect_opencode_config_dir(config_dir: &Path) -> DetectedCli {
    detect_opencode_candidates(vec![
        config_dir.join("opencode.json"),
        config_dir.join("config.json"),
    ])
}

fn detect_opencode_candidates(candidates: Vec<PathBuf>) -> DetectedCli {
    let mut out = DetectedCli::default();
    for candidate in candidates {
        if let Ok(text) = fs::read_to_string(&candidate) {
            out.source_path = Some(candidate);
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                    out.model = Some(model.to_string());
                }
                if let Some(small_model) = value.get("small_model").and_then(|v| v.as_str()) {
                    out.env
                        .insert("OPENCODE_SMALL_MODEL".to_string(), small_model.to_string());
                }
                detect_opencode_provider_config(&value, &mut out);
            }
            break;
        }
    }
    for key in [
        "OPENCODE_CONFIG",
        "OPENCODE_API_KEY",
        "OPENCODE_BASE_URL",
        "OPENCODE_PROVIDER_ID",
        "OPENCODE_PROVIDER_NPM",
        "OPENCODE_PROVIDER_NAME",
        "OPENCODE_SMALL_MODEL",
        "OPENCODE_TIMEOUT_MS",
        "OPENCODE_CHUNK_TIMEOUT_MS",
        "OPENCODE_SET_CACHE_KEY",
    ] {
        if out.env.contains_key(key) {
            continue;
        }
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                out.env.insert(key.to_string(), value);
            }
        }
    }
    out
}

fn detect_opencode_provider_config(value: &serde_json::Value, out: &mut DetectedCli) {
    let Some(provider) = value.get("provider").and_then(|v| v.as_object()) else {
        return;
    };
    let selected_provider = out
        .model
        .as_deref()
        .and_then(|model| model.split_once('/').map(|(provider, _)| provider))
        .filter(|provider_id| provider.contains_key(*provider_id))
        .or_else(|| provider.keys().next().map(String::as_str));
    let Some(provider_id) = selected_provider else {
        return;
    };
    let Some(conf) = provider.get(provider_id) else {
        return;
    };
    out.env
        .insert("OPENCODE_PROVIDER_ID".to_string(), provider_id.to_string());

    if let Some(npm) = conf.get("npm").and_then(|v| v.as_str()) {
        out.env
            .insert("OPENCODE_PROVIDER_NPM".to_string(), npm.to_string());
    }
    if let Some(name) = conf.get("name").and_then(|v| v.as_str()) {
        out.env
            .insert("OPENCODE_PROVIDER_NAME".to_string(), name.to_string());
    }

    let options = conf.get("options").unwrap_or(conf);
    if let Some(api_key) = options.get("apiKey").and_then(|v| v.as_str()) {
        let value = env_reference_name(api_key)
            .and_then(|key| std::env::var(key).ok())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| api_key.to_string());
        out.env.insert("OPENCODE_API_KEY".to_string(), value);
    }
    if let Some(base_url) = options.get("baseURL").and_then(|v| v.as_str()) {
        out.env
            .insert("OPENCODE_BASE_URL".to_string(), base_url.to_string());
    }
    if let Some(timeout) = options.get("timeout") {
        if let Some(value) = json_scalar_string(timeout) {
            out.env.insert("OPENCODE_TIMEOUT_MS".to_string(), value);
        }
    }
    if let Some(chunk_timeout) = options.get("chunkTimeout") {
        if let Some(value) = json_scalar_string(chunk_timeout) {
            out.env
                .insert("OPENCODE_CHUNK_TIMEOUT_MS".to_string(), value);
        }
    }
    if let Some(set_cache_key) = options.get("setCacheKey") {
        if let Some(value) = json_scalar_string(set_cache_key) {
            out.env.insert("OPENCODE_SET_CACHE_KEY".to_string(), value);
        }
    }

    let selected_model = out
        .model
        .as_deref()
        .and_then(|model| model.split_once('/').map(|(_, model)| model));
    if let Some(models) = conf.get("models").and_then(|v| v.as_object()) {
        if let Some((model_id, model_conf)) = selected_model
            .and_then(|model| models.get(model).map(|conf| (model, conf)))
            .or_else(|| {
                models
                    .iter()
                    .next()
                    .map(|(model, conf)| (model.as_str(), conf))
            })
        {
            out.env.insert(
                "OPENCODE_PROVIDER_MODEL_ID".to_string(),
                model_id.to_string(),
            );
            if let Some(name) = model_conf.get("name").and_then(|v| v.as_str()) {
                out.env
                    .insert("OPENCODE_MODEL_DISPLAY_NAME".to_string(), name.to_string());
            }
        }
    }
}

fn env_reference_name(value: &str) -> Option<&str> {
    value.strip_prefix("{env:")?.strip_suffix('}')
}

fn json_scalar_string(value: &serde_json::Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_string());
    }
    if let Some(value) = value.as_u64() {
        return Some(value.to_string());
    }
    if let Some(value) = value.as_bool() {
        return Some(value.to_string());
    }
    None
}

fn detect_gemini(home: &Path) -> DetectedCli {
    let mut out = DetectedCli::default();
    for candidate in [
        home.join(".gemini").join("settings.json"),
        home.join(".gemini").join("config.json"),
    ] {
        if let Ok(text) = fs::read_to_string(&candidate) {
            out.source_path = Some(candidate);
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                    out.model = Some(model.to_string());
                }
                if let Some(api_key) = value.get("apiKey").and_then(|v| v.as_str()) {
                    out.env
                        .insert("GEMINI_API_KEY".to_string(), api_key.to_string());
                }
            }
            break;
        }
    }
    let oauth_path = home.join(".gemini").join("oauth_creds.json");
    if oauth_path.exists() && out.source_path.is_none() {
        out.source_path = Some(oauth_path);
    }
    out
}
