//! TS bindings 导出工具：`cargo run -p export-bindings` 触发
//! [`dev_team::bindings::export_bindings`]（重导出是构建步骤不是测试，检查链
//! 以此显式驱动，保持零测试执行纪律）。独立成员 crate 而非 dev-team 包
//! `src/bin` 目标：app 包须保持单 bin，多 bin 会被 tauri build 拖带进 release
//! 产物并干扰 bundler 主二进制选择。

fn main() {
    if let Err(err) = dev_team::bindings::export_bindings() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
