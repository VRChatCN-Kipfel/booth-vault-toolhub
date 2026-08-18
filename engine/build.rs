//! 编译期注入构建来源标记（CI 注入的 `BUILD_SOURCE` 环境变量 → `BOOTH_BUILD_SOURCE`）。
//!
//! 仅当 CI 在编译前显式设置 `BUILD_SOURCE=branch` 时写 `cargo:rustc-env`；
//! 未设置（本地开发）或设为 `tag` 时不写变量，`update::cmp_version` 走真实版本比较，
//! 避免分支构建被误判为「已是最新」（分支版本串里的数字段会与 release tag 撞档）。

fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_SOURCE");
    if std::env::var("BUILD_SOURCE").as_deref() == Ok("branch") {
        println!("cargo:rustc-env=BOOTH_BUILD_SOURCE=branch");
    }
}
