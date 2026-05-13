use std::path::{Path, PathBuf};

use crate::error::{OccError, OccResult};
use crate::output::{self, Table};
use crate::skills;

pub fn skills_list() -> OccResult<()> {
    let mut table = Table::new(&["NAME", "DESCRIPTION"]);
    for skill in skills::all() {
        table.add_row(vec![skill.name.to_string(), skill.description.to_string()]);
    }
    table.print();
    Ok(())
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

pub fn skills_install(target: &Path) -> OccResult<()> {
    for path in skills::install(target)? {
        println!("installed: {}", output::display_path(&path));
    }
    Ok(())
}

pub fn skills_doctor(target: Option<PathBuf>) -> OccResult<()> {
    let target = match target {
        Some(path) => path,
        None => directories::BaseDirs::new()
            .map(|base_dirs| base_dirs.home_dir().join(".agents").join("skills"))
            .ok_or_else(|| {
                OccError::new(
                    "home_not_found",
                    "Unable to locate the user home directory.",
                )
            })?,
    };
    for line in skills::doctor(&target)? {
        println!("{}", line);
    }
    Ok(())
}
