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
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_MODEL",
        ] {
            if let Some(v) = env.get(key).and_then(|v| v.as_str()) {
                out.env.insert(key.to_string(), v.to_string());
            }
        }
    }
    if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
        out.model = Some(model.to_string());
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
                out.env
                    .insert("OPENAI_MODEL".to_string(), model.to_string());
            }
            if let Some(effort) = value.get("model_reasoning_effort").and_then(|v| v.as_str()) {
                out.effort = Some(effort.to_string());
            }
            if let Some(provider) = value.get("model_provider").and_then(|v| v.as_str()) {
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
                            .insert("OPENAI_API_KEY_ENV".to_string(), env_key.to_string());
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
    out
}

fn detect_opencode(home: &Path) -> DetectedCli {
    detect_opencode_candidates([
        home.join(".opencode").join("config.json"),
        home.join(".config").join("opencode").join("config.json"),
    ])
}

fn detect_opencode_config_dir(config_dir: &Path) -> DetectedCli {
    detect_opencode_candidates([config_dir.join("config.json")])
}

fn detect_opencode_candidates<const N: usize>(candidates: [PathBuf; N]) -> DetectedCli {
    let mut out = DetectedCli::default();
    for candidate in candidates {
        if let Ok(text) = fs::read_to_string(&candidate) {
            out.source_path = Some(candidate);
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(provider) = value.get("provider").and_then(|v| v.as_object()) {
                    for (_, conf) in provider {
                        if let Some(api_key) = conf.get("apiKey").and_then(|v| v.as_str()) {
                            out.env
                                .insert("OPENCODE_API_KEY".to_string(), api_key.to_string());
                        }
                        if let Some(base_url) = conf.get("baseURL").and_then(|v| v.as_str()) {
                            out.env
                                .insert("OPENCODE_BASE_URL".to_string(), base_url.to_string());
                        }
                    }
                }
                if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                    out.model = Some(model.to_string());
                }
            }
            break;
        }
    }
    out
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
