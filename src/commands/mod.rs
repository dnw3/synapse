pub mod models_cmd;
pub mod plugin_cmd;
pub mod skill_cmd;

pub use self::models_cmd::run_models_command;
pub use self::plugin_cmd::run_plugin_command;
pub use self::skill_cmd::run_skill_command;

use std::path::Path;

/// Recursively copy a directory.
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}
