use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};

use super::{InstallOptions, application_source, default_install_root};

pub const DESKTOP_FILE: &str = "codex-plus-plus.desktop";

// Exec 同时经过 Desktop Entry 字符串转义和参数转义；百分号必须变成 %%。
pub fn desktop_entry(executable: &Path) -> anyhow::Result<String> {
    ensure!(
        executable.is_absolute(),
        "Codex++ 可执行文件必须使用绝对路径"
    );
    let path = executable.to_str().context("可执行文件路径不是 UTF-8")?;
    ensure!(
        !path.chars().any(char::is_control),
        "可执行文件路径含有控制字符"
    );
    let mut argument = String::new();
    for ch in path.chars() {
        match ch {
            '%' => argument.push_str("%%"),
            '\\' | '"' | '`' | '$' => {
                argument.push('\\');
                argument.push(ch);
            }
            _ => argument.push(ch),
        }
    }
    let argument = argument.replace('\\', "\\\\");
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Codex++\n\
         Comment=Configure providers and launch Codex desktop\n\
         Exec=\"{argument}\" %u\nIcon=codex-plus-plus\nTerminal=false\n\
         Categories=Development;\nStartupNotify=true\n\
         MimeType=x-scheme-handler/codexplusplus;x-scheme-handler/dreamskin;\n"
    ))
}

fn entrypoint_path(options: &InstallOptions) -> anyhow::Result<PathBuf> {
    let root = options
        .install_root
        .clone()
        .or_else(default_install_root)
        .context("无法定位 XDG 应用目录")?;
    Ok(root.join(DESKTOP_FILE))
}

pub fn install_entrypoint(options: &InstallOptions) -> anyhow::Result<()> {
    let executable = application_source(options);
    ensure!(
        executable.is_file(),
        "找不到 Codex++ 可执行文件：{}",
        executable.display()
    );
    let contents = desktop_entry(&executable)?;
    let path = entrypoint_path(options)?;
    std::fs::create_dir_all(path.parent().context("应用入口缺少父目录")?)?;
    std::fs::write(path, contents)?;
    Ok(())
}

pub fn uninstall_entrypoint(options: &InstallOptions) -> anyhow::Result<()> {
    match std::fs::remove_file(entrypoint_path(options)?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn desktop_exec_quotes_paths_and_escapes_field_codes() {
        let entry = desktop_entry(Path::new("/tmp/My App/100%/$codex\"\\`")).unwrap();
        assert!(
            entry.contains(r#"Exec="/tmp/My App/100%%/\\$codex\\"\\\\\\`" %u"#),
            "{entry}"
        );
        assert!(desktop_entry(Path::new("relative/bin")).is_err());
        assert!(desktop_entry(Path::new("/tmp/bin\nExec=other")).is_err());
    }

    #[test]
    fn repair_writes_user_entrypoint_without_copying_or_modifying_executable() {
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("Codex++");
        std::fs::write(&executable, "binary").unwrap();
        let options = InstallOptions {
            install_root: Some(temp.path().join("applications")),
            launcher_path: Some(executable.clone()),
            ..Default::default()
        };
        install_entrypoint(&options).unwrap();
        let path = entrypoint_path(&options).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("Name=Codex++")
        );
        std::fs::write(&path, "broken").unwrap();
        install_entrypoint(&options).unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            desktop_entry(&executable).unwrap()
        );
        assert_eq!(std::fs::read_to_string(executable).unwrap(), "binary");
    }
}
