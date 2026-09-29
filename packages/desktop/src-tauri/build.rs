// Tauri 构建脚本（dev-team 包随根 Cargo.toml 落位在 src-tauri，构建脚本
// cwd 即包根）。tauri.conf.json / capabilities / icons 均在本目录，
// tauri-build 的相对路径解析直接可用。
//
// Windows 清单注入归本脚本统一收口：tauri-build 默认经 tauri-winres →
// embed_resource::compile 只把应用清单注入 bin 目标（cargo:rustc-link-arg-bins
// 不覆盖 --test），单元测试二进制无清单时 comctl32 绑定到 system32 的 v5 版本
// （无 TaskDialogIndirect 导出），进程加载即 0xC0000139 STATUS_ENTRYPOINT_NOT_
// FOUND——cargo test 被环境性阻断。故关掉 tauri-build 的 bins 专用清单
// （new_without_app_manifest），改由本脚本以 cargo:rustc-link-arg 对**全部**
// 目标（bin / lib 测试 / 集成测试）注入同一份清单，二进制各得且仅得一份
// RT_MANIFEST。
fn main() {
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attributes).expect("tauri_build::try_build 失败");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out_dir = std::env::var("OUT_DIR").expect("构建脚本 OUT_DIR 未设置");
        let manifest_path = std::path::Path::new(&out_dir).join("app.manifest");
        let rc_path = std::path::Path::new(&out_dir).join("app-manifest.rc");
        std::fs::write(&manifest_path, APP_MANIFEST).expect("写应用清单失败");
        std::fs::write(&rc_path, "1 24 \"app.manifest\"\n").expect("写清单 rc 失败");
        embed_resource::compile_for_everything(rc_path, embed_resource::NONE)
            .manifest_required()
            .expect("编译 Windows 应用清单失败");
    }
}

/// 与 tauri-build 默认 `windows-app-manifest.xml` 同源的应用清单：Common-
/// Controls v6 依赖（tauri/tao/muda 的对话框 API 静态导入 `TaskDialogIndirect`，
/// 仅 comctl32 v6 SxS 程序集导出）。
const APP_MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#;
