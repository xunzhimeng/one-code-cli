mod backends;
mod config_cmd;
mod doctor;
mod profiles;
mod runs;
mod sessions;
mod skills_cmd;

use std::env;
use std::path::PathBuf;

use crate::config;
use crate::error::{OccError, OccResult};

pub use backends::{backends_list, backends_show};
pub use config_cmd::{
    config_export_html, config_html, config_init, config_path, config_settings, config_show,
    config_ui, config_validate,
};
pub use doctor::doctor;
pub use profiles::{profiles_add, profiles_list, profiles_show, profiles_test};
pub use runs::{runs_list, runs_open, runs_show};
pub use sessions::{sessions_latest, sessions_list, sessions_migrate, sessions_show};
pub use skills_cmd::{skills_doctor, skills_export, skills_install, skills_list, skills_show};

pub(crate) fn current_cwd() -> OccResult<PathBuf> {
    env::current_dir()
        .map_err(|error| OccError::io("cwd_not_found", "Failed to read current directory", error))
}

pub(crate) fn load_current(config_arg: Option<&PathBuf>) -> OccResult<config::EffectiveConfig> {
    let cwd = current_cwd()?;
    config::load(config_arg, &cwd)
}

pub(crate) fn current_doc_root(config_arg: Option<&PathBuf>) -> OccResult<PathBuf> {
    let cwd = current_cwd()?;
    let config = config::load(config_arg, &cwd)?;
    Ok(config.resolved_doc_root(&cwd, None))
}
