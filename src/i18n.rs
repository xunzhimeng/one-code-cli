use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Zh,
}

pub fn language() -> Language {
    for key in ["OCC_LANG", "LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = env::var(key) {
            let normalized = value.to_ascii_lowercase();
            if normalized.starts_with("zh") || normalized.contains("chinese") {
                return Language::Zh;
            }
            if normalized.starts_with("en") {
                return Language::En;
            }
        }
    }
    Language::En
}

pub fn is_zh() -> bool {
    language() == Language::Zh
}

/// Returns the localized message for the given key.
/// Falls back to the English version if no translation exists.
pub fn t(key: &str) -> &'static str {
    if is_zh() {
        match key {
            // ── doctor ──
            "doctor.title" => "One Code CLI 环境检查",
            "doctor.current_dir" => "当前目录",
            "doctor.config_search" => "配置搜索顺序",
            "doctor.loaded_config" => "已加载配置",
            "doctor.storage" => "存储与会话",
            "doctor.backends" => "CLI",
            "doctor.profiles" => "Agent",
            "doctor.using_defaults" => "使用内置默认配置",
            "doctor.cwd_readable" => "工作目录可读",
            "doctor.doc_root_writable" => "运行目录可写",
            "doctor.executable" => "可执行文件",
            "doctor.source" => "来源",
            "doctor.resume" => "恢复",
            "doctor.unknown_backend" => "引用了未知 CLI",

            // ── config show ──
            "config.summary" => "配置概览",
            "config.version" => "版本",
            "config.default_profile" => "默认 agent",
            "config.default_profile_unset" => "(未设置)",
            "config.doc_root" => "运行记录目录 doc_root",
            "config.proxy" => "代理转发",
            "config.proxy_enabled" => "启用",
            "config.proxy_disabled" => "关闭",
            "config.default_timeout" => "默认超时",
            "config.timeout_none" => "无",
            "config.loaded_files" => "配置来源",
            "config.using_defaults" => "使用内置默认配置",
            "config.search_order" => "配置搜索顺序",
            "config.backend_defaults" => "CLI 默认 agent",
            "config.available_profiles" => "可用 agent",
            "config.notes" => "说明",
            "config.note_doc_root" => {
                "doc_root 是 run artifact 目录，result.md、stdout.log、stderr.log 会写到这里。"
            }
            "config.note_profile" => {
                "agent 决定调用哪个 CLI、默认参数、prompt 传递方式，以及 model / effort 设置。"
            }
            "config.note_backend_defaults" => {
                "cli_type_defaults 决定 --cli <name> 默认解析到哪个 agent。"
            }
            "config.note_raw" => "使用 `occ config show --raw` 查看完整 TOML。",

            // ── vibe ──
            "vibe.title" => "One Code CLI 对话模式",
            "vibe.hint" => "输入 /help 查看命令，/exit 退出",
            "vibe.transcript_off" => "关闭",
            "vibe.transcript_resume" => "后端原生恢复",
            "vibe.transcript_managed" => "occ 管理",
            "vibe.bye" => "再见 👋",
            "vibe.help_title" => "可用命令：",
            "vibe.help_help" => "显示命令列表",
            "vibe.help_status" => "显示当前 CLI、模型、会话和上下文状态",
            "vibe.help_profile" => "切换到指定 agent，清除 CLI 选择",
            "vibe.help_backend" => "切换 CLI，自动解析默认 agent",
            "vibe.help_model_set" => "设置后续消息的模型",
            "vibe.help_model_clear" => "清除模型覆盖",
            "vibe.help_effort_set" => "设置后续消息的 effort",
            "vibe.help_effort_clear" => "清除 effort 覆盖",
            "vibe.help_session" => "显示当前会话 ID",
            "vibe.help_clear" => "清除 occ 管理的上下文记录",
            "vibe.help_exit" => "退出",
            "vibe.status_title" => "当前状态：",
            "vibe.transcript_cleared" => "上下文记录已清除",
            "vibe.backend_cleared" => "(已清除)",

            // ── dry-run ──
            "dry.title" => "命令计划（dry-run）",
            "dry.profile" => "Agent alias",
            "dry.backend" => "CLI type",
            "dry.model" => "模型",
            "dry.model_source" => "模型来源",
            "dry.effort" => "Effort",
            "dry.effort_source" => "Effort 来源",
            "dry.cwd" => "工作目录",
            "dry.command" => "命令",
            "dry.env_keys" => "环境变量",
            "dry.env_removed" => "移除变量",
            "dry.prompt_transport" => "Prompt 传递",
            "dry.prompt_file" => "Prompt 文件",
            "dry.timeout" => "超时",
            "dry.timeout_none" => "无",
            "dry.stream" => "实时流",
            "dry.stream_on" => "开启",
            "dry.stream_off" => "关闭",

            // ── common ──
            "common.none" => "无",
            "common.builtin" => "内置",
            "common.config" => "配置",

            _ => t_en(key),
        }
    } else {
        t_en(key)
    }
}

fn t_en(key: &str) -> &'static str {
    match key {
        // ── doctor ──
        "doctor.title" => "One Code CLI doctor",
        "doctor.current_dir" => "Current directory",
        "doctor.config_search" => "Config search order",
        "doctor.loaded_config" => "Loaded config files",
        "doctor.storage" => "Storage and sessions",
        "doctor.backends" => "CLIs",
        "doctor.profiles" => "Agents",
        "doctor.using_defaults" => "using built-in defaults",
        "doctor.cwd_readable" => "cwd readable",
        "doctor.doc_root_writable" => "doc_root writable",
        "doctor.executable" => "executable",
        "doctor.source" => "source",
        "doctor.resume" => "resume",
        "doctor.unknown_backend" => "references unknown CLI",

        // ── config show ──
        "config.summary" => "Configuration summary",
        "config.version" => "version",
        "config.default_profile" => "default agent",
        "config.default_profile_unset" => "(unset)",
        "config.doc_root" => "doc_root run artifact directory",
        "config.proxy" => "proxy forwarding",
        "config.proxy_enabled" => "enabled",
        "config.proxy_disabled" => "disabled",
        "config.default_timeout" => "default timeout",
        "config.timeout_none" => "none",
        "config.loaded_files" => "Loaded config files",
        "config.using_defaults" => "using built-in defaults",
        "config.search_order" => "Config search order",
        "config.backend_defaults" => "CLI default agents",
        "config.available_profiles" => "Agents",
        "config.notes" => "Notes",
        "config.note_doc_root" => {
            "doc_root is the run artifact directory for result.md, stdout.log, and stderr.log."
        }
        "config.note_profile" => {
            "agent selects the CLI, default args, prompt transport, and model / effort settings."
        }
        "config.note_backend_defaults" => {
            "cli_type_defaults controls how --cli <name> resolves to an agent."
        }
        "config.note_raw" => "Use `occ config show --raw` to print the full TOML.",

        // ── vibe ──
        "vibe.title" => "One Code CLI vibe",
        "vibe.hint" => "type /help for commands, /exit to quit",
        "vibe.transcript_off" => "off",
        "vibe.transcript_resume" => "native resume mode",
        "vibe.transcript_managed" => "occ-managed",
        "vibe.bye" => "bye 👋",
        "vibe.help_title" => "Commands:",
        "vibe.help_help" => "show commands",
        "vibe.help_status" => "show selected CLI, model, session, and transcript state",
        "vibe.help_profile" => "switch to an exact occ agent and clear CLI",
        "vibe.help_backend" => "switch CLI and let occ resolve the default agent",
        "vibe.help_model_set" => "set model for later messages",
        "vibe.help_model_clear" => "clear model override",
        "vibe.help_effort_set" => "set effort for later messages",
        "vibe.help_effort_clear" => "clear effort override",
        "vibe.help_session" => "show current session id",
        "vibe.help_clear" => "clear occ-managed transcript context",
        "vibe.help_exit" => "quit",
        "vibe.status_title" => "Status:",
        "vibe.transcript_cleared" => "transcript cleared",
        "vibe.backend_cleared" => "(cleared)",

        // ── dry-run ──
        "dry.title" => "Command plan (dry-run)",
        "dry.profile" => "Agent",
        "dry.backend" => "CLI",
        "dry.model" => "Model",
        "dry.model_source" => "Model source",
        "dry.effort" => "Effort",
        "dry.effort_source" => "Effort source",
        "dry.cwd" => "Working dir",
        "dry.command" => "Command",
        "dry.env_keys" => "Env vars",
        "dry.env_removed" => "Env removed",
        "dry.prompt_transport" => "Prompt via",
        "dry.prompt_file" => "Prompt file",
        "dry.timeout" => "Timeout",
        "dry.timeout_none" => "none",
        "dry.stream" => "Stream",
        "dry.stream_on" => "on",
        "dry.stream_off" => "off",

        // ── common ──
        "common.none" => "none",
        "common.builtin" => "builtin",
        "common.config" => "config",

        _ => "???",
    }
}
