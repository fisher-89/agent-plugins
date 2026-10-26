//! 错误通道定夺（PoC 回填 design「PoC 前置门」）：`ErrorHandlingMode::Throw`
//! ——`Result<T, String>` 命令的生成绑定为 `Promise<T>` reject 语义，与前端
//! 既有 `.catch → error 态` 接线逐字兼容（替换前后错误面同形）。
//! events 面不引入：应用流式全景为全 Channel、零 emit/listen。

use std::path::PathBuf;

use tauri_specta::{collect_commands, Builder, ErrorHandlingMode};

use crate::commands::all_commands;

pub(crate) mod zod;

use self::zod::ZodLanguage;

/// 全部命令的 specta builder：命令清单经 `crate::commands::all_commands!`
/// 注入（与 `main` 的原生 `generate_handler` 同源），tauri_specta 依赖仅归本模块。
fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(all_commands!(collect_commands))
        .error_handling(ErrorHandlingMode::Throw)
        // serde_json::Value 自引用递归（Value → Vec<Value> → Value），不可结构
        // 化展开；语义规则改写为 TS `unknown`（与既有 dto.ts 的 payload/usage/
        // input/key/value 口径一致，前端 renderer 自行收窄）
        .semantic_types(
            specta_typescript::semantic::Configuration::default().define::<serde_json::Value>(
                |_| specta_typescript::define("unknown").into(),
                None,
                None,
            ),
        )
        // i64/u64 时间戳与 id（毫秒时间戳、run id 等，值域远低于 2^53）按线
        // 格式（serde_json number）出线为 TS `number`，与既有 dto.ts 口径一致
        .dangerously_cast_bigints_to_number()
        // serde 相位按统一形态出线：specta-serde 0.0.13 起把 `#[serde(default)]`
        // 字段判为相位差，会把每份 DTO 裂成 `_Serialize` / `_Deserialize` 伴生
        // 类型（旧版无此分裂）。本仓纪律是「线格式单一形态」（见 core/agent 刻意
        // 避开 skip_serializing_if 的注释），两份产物也都按统一形态出线——类型面
        // 语义与升级前逐字一致，前端不会被可选性差异撕成两种读法。
        .disable_serde_phases()
}

/// 幂等导出两份产物到 `packages/desktop/src/types/generated/`：`bindings.ts`（唯一
/// 类型源）与 `bindings.zod.ts`（纯 schema，运行期校验用）。同一个 builder 连做两次
/// `.export(...)`（`Builder::export` 是 `&self`，无需重建），导出顺序固定为类型产物
/// 在前、schema 产物在后。路径以 `CARGO_MANIFEST_DIR` 定位（父目录 `create_dir_all`
/// 单点），输出确定性（无时间戳 / 无机器路径嵌入），同输入连续两次导出零 diff。
pub fn export_bindings() -> Result<(), String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "CARGO_MANIFEST_DIR 无父目录".to_owned())?
        .join("src/types/generated");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建生成目录失败: {e}"))?;
    let builder = builder();
    builder
        .export(
            specta_typescript::Typescript::default(),
            dir.join("bindings.ts"),
        )
        .map_err(|e| format!("导出 TS bindings 失败: {e}"))?;
    builder
        .export(ZodLanguage, dir.join("bindings.zod.ts"))
        .map_err(|e| format!("导出 zod schema 失败: {e}"))
}

#[cfg(test)]
mod mod_test;
