// Tauri 构建脚本（dev-team 包随根 Cargo.toml 落位在 src-tauri，构建脚本
// cwd 即包根）。tauri.conf.json / capabilities / icons 均在本目录，
// tauri-build 的相对路径解析直接可用。
fn main() {
    tauri_build::build()
}
