mod enc_file;
mod env_file;
mod file;
mod global_env;
mod inline;
mod roots;
mod source;

pub use self::{
    enc_file::{ENC_FILE_SUFFIX, EncFileError, SOPS_PROGRAM, enc_file_of, load_enc_file},
    env_file::{ENV_FILE_SUFFIX, EnvFileError, env_file_of, load_env_file, parse_env_file},
    file::{
        AppEntry, AppsFile, AppsFileError, ReadyProbeEntry, SandboxEntry, SpecDefaults, SpecRoots,
        load_apps_file, load_service_file, parse_apps_file, parse_service_file, resolve_checked,
    },
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
