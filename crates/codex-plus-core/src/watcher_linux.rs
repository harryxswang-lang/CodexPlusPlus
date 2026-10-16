//! Linux 桌面进程识别：只认当前用户的 Electron 主进程，排除 CLI 和 renderer。
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct DesktopProcess {
    pid: u32,
    executable: PathBuf,
    birth_id: String,
    debug_port: Option<u16>,
}

fn read_process(root: &Path, pid: u32) -> Option<DesktopProcess> {
    let dir = root.join(pid.to_string());
    let current_uid = std::fs::metadata(root.join("self")).ok()?.uid();
    if std::fs::metadata(&dir).ok()?.uid() != current_uid {
        return None;
    }
    let executable = std::fs::read_link(dir.join("exe")).ok()?;
    let name = executable.file_name()?.to_str()?;
    if !matches!(name, "ChatGPT" | "chatgpt" | "Codex" | "codex")
        || !executable.parent()?.join("resources/app.asar").is_file()
    {
        return None;
    }
    let cmdline = std::fs::read(dir.join("cmdline")).ok()?;
    let args: Vec<_> = cmdline
        .split(|byte| *byte == 0)
        .filter_map(|arg| std::str::from_utf8(arg).ok())
        .collect();
    if args
        .iter()
        .any(|arg| *arg == "--type" || arg.starts_with("--type="))
    {
        return None;
    }
    let debug_port = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--remote-debugging-port=")?.parse().ok())
        .or_else(|| {
            args.windows(2).find_map(|pair| {
                (pair[0] == "--remote-debugging-port")
                    .then(|| pair[1].parse().ok())
                    .flatten()
            })
        });
    // comm 可以含空格和括号；最后一个右括号之后才是从字段 3 开始的 stat。
    let stat = std::fs::read_to_string(dir.join("stat")).ok()?;
    let birth_id = stat
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .to_string();
    Some(DesktopProcess {
        pid,
        executable,
        birth_id,
        debug_port,
    })
}

fn desktop_processes(root: &Path) -> Vec<DesktopProcess> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(|pid| read_process(root, pid))
        .collect()
}

pub(crate) fn process_ids(port: Option<u16>) -> Vec<u32> {
    matching_processes(Path::new("/proc"), port)
        .into_iter()
        .map(|process| process.pid)
        .collect()
}

fn matching_processes(root: &Path, port: Option<u16>) -> Vec<DesktopProcess> {
    desktop_processes(root)
        .into_iter()
        .filter(|process| port.is_none() || process.debug_port == port)
        .collect()
}

pub(crate) fn app_is_running(app_dir: &Path) -> bool {
    let executable = crate::app_paths::build_codex_executable(app_dir);
    let executable = executable.canonicalize().unwrap_or(executable);
    desktop_processes(Path::new("/proc"))
        .iter()
        .any(|p| p.executable == executable)
}

pub(crate) fn stop_and_wait(port: Option<u16>) {
    let root = Path::new("/proc");
    let targets = matching_processes(root, port);
    for target in &targets {
        // 发信号前复查身份；不使用进程名匹配，也不回退为终止全部实例。
        if same_process(root, target) {
            let _ = std::process::Command::new("kill")
                .args(["-TERM", "--", &target.pid.to_string()])
                .status();
        }
    }
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline && targets.iter().any(|target| same_process(root, target)) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn same_process(root: &Path, target: &DesktopProcess) -> bool {
    read_process(root, target.pid).is_some_and(|current| {
        current.executable == target.executable && current.birth_id == target.birth_id
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn proc_scan_excludes_cli_and_renderers_and_preserves_port_identity() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("proc");
        std::fs::create_dir(&root).unwrap();
        symlink(".", root.join("self")).unwrap();
        let app = temp.path().join("chatgpt");
        std::fs::create_dir_all(app.join("resources")).unwrap();
        std::fs::write(app.join("resources/app.asar"), "").unwrap();
        let main = app.join("ChatGPT");
        std::fs::write(&main, "").unwrap();
        let cli = app.join("resources/codex");
        std::fs::write(&cli, "").unwrap();
        for (pid, exe, args) in [
            (10, &main, "ChatGPT\0--remote-debugging-port=9229\0"),
            (11, &main, "ChatGPT\0--type=renderer\0"),
            (12, &cli, "codex\0"),
            (13, &main, "ChatGPT\0--remote-debugging-port\09231\0"),
        ] {
            let dir = root.join(pid.to_string());
            std::fs::create_dir(&dir).unwrap();
            symlink(exe, dir.join("exe")).unwrap();
            std::fs::write(dir.join("cmdline"), args).unwrap();
            let stat = format!("{pid} (Chat GPT) S {} 12345 0", "0 ".repeat(18));
            std::fs::write(dir.join("stat"), stat).unwrap();
        }
        let mut processes = desktop_processes(&root);
        processes.sort_by_key(|p| p.pid);
        assert_eq!(
            processes
                .iter()
                .map(|p| (p.pid, p.debug_port))
                .collect::<Vec<_>>(),
            vec![(10, Some(9229)), (13, Some(9231))]
        );
        assert_eq!(processes[0].birth_id, "12345");
        assert_eq!(
            matching_processes(&root, Some(9229))
                .iter()
                .map(|p| p.pid)
                .collect::<Vec<_>>(),
            vec![10]
        );
        assert_eq!(
            matching_processes(&root, Some(9231))
                .iter()
                .map(|p| p.pid)
                .collect::<Vec<_>>(),
            vec![13]
        );
        assert!(matching_processes(&root, Some(9999)).is_empty());
        let target = processes[0].clone();
        assert!(same_process(&root, &target));
        std::fs::write(
            root.join("10/stat"),
            format!("10 (ChatGPT) S {} 99999 0", "0 ".repeat(18)),
        )
        .unwrap();
        assert!(!same_process(&root, &target));
    }
}
