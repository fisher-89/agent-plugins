//! TS bindings 导出 bin：`cargo run --bin export-bindings` 触发
//! [`dev_team::bindings::export_bindings`]（重导出是构建步骤不是测试，检查链
//! 以此显式驱动，保持零测试执行纪律）。

fn main() {
    if let Err(err) = dev_team::bindings::export_bindings() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
