use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{OccError, OccResult};
use crate::output;

#[derive(Debug, Clone, Serialize)]
pub struct SkillFile {
    pub path: &'static str,
    pub body: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillTemplate {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub skill_md: &'static str,
    pub skill_toml: &'static str,
    pub files: &'static [SkillFile],
}

pub fn all() -> &'static [SkillTemplate] {
    &SKILLS
}

pub fn get(name: &str) -> Option<&'static SkillTemplate> {
    all().iter().find(|skill| skill.name == name)
}

pub fn require(name: &str) -> OccResult<&'static SkillTemplate> {
    get(name).ok_or_else(|| {
        OccError::new(
            "skill_not_found",
            format!("Skill '{}' was not found.", name),
        )
    })
}

pub fn export(name: &str, target: &Path) -> OccResult<PathBuf> {
    let skill = require(name)?;
    export_skill(skill, target)
}

pub fn install(target: &Path) -> OccResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for skill in all() {
        paths.push(export_skill(skill, target)?);
    }
    Ok(paths)
}

pub fn doctor(target: &Path) -> OccResult<Vec<String>> {
    let mut lines = Vec::new();
    if target.exists() {
        lines.push(format!(
            "ok target exists: {}",
            output::display_path(target)
        ));
    } else {
        lines.push(format!("missing target: {}", output::display_path(target)));
    }
    match which::which("occ") {
        Ok(path) => lines.push(format!(
            "ok occ executable: {}",
            output::display_path(&path)
        )),
        Err(_) => lines.push("missing occ executable in PATH".to_string()),
    }
    for skill in all() {
        let dir = target.join(skill.name);
        let ok = dir.join("SKILL.md").exists() && dir.join("skill.toml").exists();
        lines.push(format!(
            "{} {}",
            if ok { "ok" } else { "missing" },
            skill.name
        ));
    }
    Ok(lines)
}

fn export_skill(skill: &SkillTemplate, target: &Path) -> OccResult<PathBuf> {
    let dir = target.join(skill.name);
    remove_obsolete_files(skill, &dir)?;
    write_file(&dir.join("SKILL.md"), skill.skill_md)?;
    write_file(&dir.join("skill.toml"), skill.skill_toml)?;
    for file in skill.files {
        write_file(&dir.join(file.path), file.body)?;
    }
    Ok(dir)
}

fn remove_obsolete_files(skill: &SkillTemplate, dir: &Path) -> OccResult<()> {
    for file in obsolete_files(skill.name) {
        let path = dir.join(file);
        if path.exists() {
            fs::remove_file(&path).map_err(|error| {
                OccError::io(
                    "doc_root_not_writable",
                    format!("Failed to remove '{}'", output::display_path(&path)),
                    error,
                )
            })?;
        }
    }
    Ok(())
}

fn obsolete_files(skill_name: &str) -> &'static [&'static str] {
    match skill_name {
        "using-one-code-cli" => &[
            "examples/run-with-profile.md",
            "examples/run-with-backend.md",
        ],
        _ => &[],
    }
}

fn write_file(path: &Path, body: &str) -> OccResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to create '{}'", output::display_path(parent)),
                error,
            )
        })?;
    }
    fs::write(path, body).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to write '{}'", output::display_path(path)),
            error,
        )
    })
}

static USING_ONE_CODE_CLI_FILES: &[SkillFile] = &[
    SkillFile {
        path: "examples/run-with-agent.md",
        body: include_str!("../assets/skills/using-one-code-cli/examples/run-with-agent.md"),
    },
    SkillFile {
        path: "examples/run-with-cli.md",
        body: include_str!("../assets/skills/using-one-code-cli/examples/run-with-cli.md"),
    },
    SkillFile {
        path: "examples/resume-session.md",
        body: include_str!("../assets/skills/using-one-code-cli/examples/resume-session.md"),
    },
    SkillFile {
        path: "examples/read-result.md",
        body: include_str!("../assets/skills/using-one-code-cli/examples/read-result.md"),
    },
];

static SKILLS: [SkillTemplate; 1] = [SkillTemplate {
    name: "using-one-code-cli",
    title: "Use One Code CLI",
    description:
        "Protocol for agents to delegate coding tasks through occ and read result documents.",
    skill_md: include_str!("../assets/skills/using-one-code-cli/SKILL.md"),
    skill_toml: include_str!("../assets/skills/using-one-code-cli/skill.toml"),
    files: USING_ONE_CODE_CLI_FILES,
}];
