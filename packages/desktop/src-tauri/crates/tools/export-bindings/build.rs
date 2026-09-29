// 与根包 build.rs 同源的清单注入（差异：无 tauri_build，纯工具 bin 只需
// RT_MANIFEST）。lib 依赖不传递 cargo:rustc-link-arg，本 crate 的 bin（及
// 潜在测试目标）链接 dev_team → tauri → tao/muda 后同样静态导入
// TaskDialogIndirect，无清单加载即 0xC0000139，故须自注入同一份清单。

fn main() {
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

/// 与根包 build.rs 的 `APP_MANIFEST` 同源（tauri-build 默认 windows-app-
/// manifest.xml：Common-Controls v6 SxS 依赖），改一处须同步另一处。
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
