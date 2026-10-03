pub mod enc_file;
mod env_file;
pub mod file;
mod global_env;
mod inline;
mod roots;
mod source;

pub use self::{
    enc_file::{SOPS_PROGRAM, enc_file_of},
    env_file::{ENV_FILE_SUFFIX, env_file_of},
    file::{AppsFileError, load_apps_file},
    global_env::{GLOBAL_ENV_STEM, decryptor_env, load_global_env, warn_misplaced_global_env},
    inline::{InlineStart, diff_lines, encode_service_file, fold_entry, inline_entry},
    source::{SERVICE_FILE_SUFFIX, SpecSource, service_file_of},
};

fn named_file(
    cfg_dir: &std::path::Path,
    name: &str,
    suffix: &str,
) -> Result<std::path::PathBuf, usecases::SpecError> {
    usecases::validate_app_name(name)?;
    Ok(cfg_dir.join(format!("{name}.{suffix}")))
}
