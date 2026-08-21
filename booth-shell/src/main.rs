//! booth-shell：文件夹图标 CLI。
//! 跨平台通用入口，行为对齐 `booth shell` 子命令：
//! - Windows → shell_win 三件套（ico + desktop.ini + 属性位）
//! - macOS   → shell_mac Finder 自定义图标
//! - 其他     → set/reset 报错退出码 2；audit 判 FAIL（退出码 1）

use std::path::PathBuf;
use std::process::ExitCode;

fn usage() {
    eprintln!(
        "用法: booth-shell <set|reset|audit> <cover> <folder>\n  \
         set   <cover> <folder>  设置文件夹图标\n  \
         reset <folder>           清理文件夹图标\n  \
         audit <folder>           自检文件夹图标"
    );
}

#[cfg(windows)]
mod impl_win {
    use std::path::Path;
    use std::process::ExitCode;

    use shell_win::folder_icon::{
        contract_paths, has_folder_icon, make_folder_icon, reset_folder_icon,
    };

    pub fn set(cover: &Path, folder: &Path) -> ExitCode {
        match make_folder_icon(cover, folder) {
            Ok(()) => {
                println!("ok: 三件套已写入 {}", folder.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    }

    pub fn reset(folder: &Path) -> ExitCode {
        match reset_folder_icon(folder) {
            Ok(()) => {
                println!("ok: 已清理 {}", folder.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    }

    pub fn audit(folder: &Path) -> ExitCode {
        let (ico, ini) = contract_paths(folder);
        let ok = has_folder_icon(folder);
        println!(
            "audit {}: {} {}\n  ico 存在: {}\n  ini 存在: {}",
            folder.display(),
            if ok { "PASS" } else { "FAIL" },
            if ok {
                "三件套完整"
            } else {
                "三件套不完整"
            },
            ico.exists(),
            ini.exists()
        );
        if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "macos")]
mod impl_mac {
    use std::path::Path;
    use std::process::ExitCode;

    use shell_mac::folder_icon::{has_folder_icon, make_folder_icon, reset_folder_icon};

    pub fn set(cover: &Path, folder: &Path) -> ExitCode {
        match make_folder_icon(cover, folder) {
            Ok(()) => {
                println!("ok: 图标已写入 {}", folder.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    }

    pub fn reset(folder: &Path) -> ExitCode {
        match reset_folder_icon(folder) {
            Ok(()) => {
                println!("ok: 已清理 {}", folder.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    }

    pub fn audit(folder: &Path) -> ExitCode {
        let ok = has_folder_icon(folder);
        println!(
            "audit {}: {} {}",
            folder.display(),
            if ok { "PASS" } else { "FAIL" },
            if ok {
                "Finder 图标完整"
            } else {
                "Finder 图标不完整"
            }
        );
        if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage();
        return ExitCode::FAILURE;
    }
    let code: ExitCode = match args[1].as_str() {
        "set" => {
            if args.len() < 4 {
                usage();
                ExitCode::FAILURE
            } else {
                #[cfg(windows)]
                {
                    impl_win::set(&PathBuf::from(&args[2]), &PathBuf::from(&args[3]))
                }
                #[cfg(target_os = "macos")]
                {
                    impl_mac::set(&PathBuf::from(&args[2]), &PathBuf::from(&args[3]))
                }
                #[cfg(all(not(windows), not(target_os = "macos")))]
                {
                    eprintln!("error: booth-shell set 仅支持 Windows / macOS");
                    ExitCode::from(2)
                }
            }
        }
        "reset" => {
            if args.len() < 3 {
                usage();
                ExitCode::FAILURE
            } else {
                #[cfg(windows)]
                {
                    impl_win::reset(&PathBuf::from(&args[2]))
                }
                #[cfg(target_os = "macos")]
                {
                    impl_mac::reset(&PathBuf::from(&args[2]))
                }
                #[cfg(all(not(windows), not(target_os = "macos")))]
                {
                    eprintln!("error: booth-shell reset 仅支持 Windows / macOS");
                    ExitCode::from(2)
                }
            }
        }
        "audit" => {
            if args.len() < 3 {
                usage();
                ExitCode::FAILURE
            } else {
                #[cfg(windows)]
                {
                    impl_win::audit(&PathBuf::from(&args[2]))
                }
                #[cfg(target_os = "macos")]
                {
                    impl_mac::audit(&PathBuf::from(&args[2]))
                }
                #[cfg(all(not(windows), not(target_os = "macos")))]
                {
                    println!(
                        "audit {}: FAIL 文件夹图标仅支持 Windows / macOS",
                        PathBuf::from(&args[2]).display()
                    );
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            usage();
            ExitCode::FAILURE
        }
    };
    code
}
