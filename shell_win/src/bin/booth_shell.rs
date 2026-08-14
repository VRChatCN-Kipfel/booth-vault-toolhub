//! booth-shell：文件夹图标三件套 CLI（M3 实机验证入口，M4 并入统一 CLI）。

use std::path::PathBuf;
use std::process::ExitCode;

use shell_win::folder_icon::{
    contract_paths, has_folder_icon, make_folder_icon, reset_folder_icon,
};

fn usage() {
    eprintln!(
        "用法: booth-shell <set|reset|audit> <cover> <folder>\n  \
         set   <cover> <folder>  设置三件套\n  \
         reset <folder>           清理三件套\n  \
         audit <folder>           自检三件套"
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage();
        return ExitCode::FAILURE;
    }
    match args[1].as_str() {
        "set" => {
            if args.len() < 4 {
                usage();
                return ExitCode::FAILURE;
            }
            let cover = PathBuf::from(&args[2]);
            let folder = PathBuf::from(&args[3]);
            match make_folder_icon(&cover, &folder) {
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
        "reset" => {
            if args.len() < 3 {
                usage();
                return ExitCode::FAILURE;
            }
            let folder = PathBuf::from(&args[2]);
            match reset_folder_icon(&folder) {
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
        "audit" => {
            if args.len() < 3 {
                usage();
                return ExitCode::FAILURE;
            }
            let folder = PathBuf::from(&args[2]);
            let (ico, ini) = contract_paths(&folder);
            let ok = has_folder_icon(&folder);
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
        _ => {
            usage();
            ExitCode::FAILURE
        }
    }
}
