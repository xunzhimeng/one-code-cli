use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::{OccError, OccResult};
use crate::output::{self, Table};
use crate::skills;

pub fn skills_list() -> OccResult<()> {
    let mut table = Table::new(&["NAME", "DESCRIPTION"]);
    for skill in skills::all() {
        table.add_row(vec![
            skill.name.to_string(),
            fit_description(skill.description, 36),
        ]);
    }
    table.print();
    Ok(())
}

fn fit_description(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let head = max_chars.saturating_sub(3);
    let mut text = value.chars().take(head).collect::<String>();
    text.push_str("...");
    text
}

pub fn skills_show(name: &str) -> OccResult<()> {
    let skill = skills::require(name)?;
    println!("{}", skill.skill_md);
    Ok(())
}

pub fn skills_export(name: &str, target: &Path) -> OccResult<()> {
    let path = skills::export(name, target)?;
    println!("exported: {}", output::display_path(&path));
    Ok(())
}

fn default_skills_target() -> OccResult<PathBuf> {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        return Ok(PathBuf::from(home).join(".agents").join("skills"));
    }
    directories::BaseDirs::new()
        .map(|base_dirs| base_dirs.home_dir().join(".agents").join("skills"))
        .ok_or_else(|| {
            OccError::new(
                "home_not_found",
                "Unable to locate the user home directory.",
            )
        })
}

pub fn skills_install(target: Option<PathBuf>) -> OccResult<()> {
    let target = match target {
        Some(path) => path,
        None => default_skills_target()?,
    };
    for path in skills::install(&target)? {
        println!("installed: {}", output::display_path(&path));
    }
    Ok(())
}

pub fn skills_doctor(target: Option<PathBuf>) -> OccResult<()> {
    let target = match target {
        Some(path) => path,
        None => default_skills_target()?,
    };
    let mut output = String::new();
    for line in skills::doctor(&target)? {
        output.push_str(&line);
        output.push('\n');
    }
    output.push('\n');
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(output.as_bytes());
    let _ = stdout.flush();
    Ok(())
}
