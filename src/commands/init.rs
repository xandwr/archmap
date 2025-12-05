use crate::cli::InitArgs;
use crate::config::generate_config_template;
use crate::fs::{FileSystem, default_fs};
use crate::style;

pub fn cmd_init(args: InitArgs) -> i32 {
    cmd_init_with_fs(args, default_fs())
}

pub fn cmd_init_with_fs(args: InitArgs, fs: &dyn FileSystem) -> i32 {
    let config_path = args.path.join(".archmap.toml");
    if fs.exists(&config_path) {
        style::error(&format!(
            ".archmap.toml already exists at {}",
            style::path(&config_path)
        ));
        return 1;
    }

    let template = generate_config_template();
    if let Err(e) = fs.write(&config_path, &template) {
        style::error(&format!("Failed to write config file: {}", e));
        return 1;
    }

    style::success(&format!(
        "Created .archmap.toml at {}",
        style::path(&config_path)
    ));
    0
}
