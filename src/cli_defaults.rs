use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use directories::BaseDirs;
use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct DetectedCli {
    pub source_path: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub model: Option<String>,
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
        claude: home.as_ref().map(detect_claude).unwrap_or_default(),
        codex: home.as_ref().map(detect_codex).unwrap_or_default(),
        opencode: home.as_ref().map(detect_opencode).unwrap_or_default(),
        gemini: home.as_ref().map(detect_gemini).unwrap_or_default(),
    }
}

fn detect_claude(home: &PathBuf) -> DetectedCli {
    let path = home.join(".claude").join("settings.json");
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

fn detect_codex(home: &PathBuf) -> DetectedCli {
    let mut out = DetectedCli::default();
    let path = home.join(".codex").join("config.toml");
    if let Ok(text) = fs::read_to_string(&path) {
        out.source_path = Some(path);
        if let Ok(value) = text.parse::<toml::Value>() {
            if let Some(model) = value.get("model").and_then(|v| v.as_str()) {
                out.model = Some(model.to_string());
                out.env
                    .insert("OPENAI_MODEL".to_string(), model.to_string());
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
    let auth_path = home.join(".codex").join("auth.json");
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

fn detect_opencode(home: &PathBuf) -> DetectedCli {
    let mut out = DetectedCli::default();
    for candidate in [
        home.join(".opencode").join("config.json"),
        home.join(".config").join("opencode").join("config.json"),
    ] {
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

fn detect_gemini(home: &PathBuf) -> DetectedCli {
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
