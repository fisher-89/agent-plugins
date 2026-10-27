//! 第二产物导出：`src/types/generated/bindings.zod.ts`（纯 zod schema）。
//!
//! 职责切分（见 `desktop-ipc-type-bindings` 规格）：`bindings.ts` 仍是唯一
//! 类型源（typed invoke 包装 + 全部 DTO 类型 + JSDoc），本产物只出 schema，零
//! `export type`、零 `z.infer`、零类型注解。前端装配层（`src/lib/ipc.ts`）拿两张产物
//! 做「参数面 strict / 结果面与信封面 soft-report」的分级校验。
//!
//! 管线（`ZodLanguage::export`）：格式映射（`specta_serde::Format` + 大整数 / `char` /
//! `serde_json::Value` 改写，并隐藏 `Value` 与 `Result` 的具名 schema）→ 逐命令渲染
//! `argsSchemas` / `resultSchemas` / `channelArgs` 三张表 → 组装 `Zod`（banner /
//! `degraded()` prelude / 三表 raw）取完整字符串 → 生成期硬约束自检 → 降级清单打
//! stderr → 落盘。
//!
//! 相位口径：`builder()` 显式 `disable_serde_phases()`（见 `bindings/mod`），故格式层是
//! `specta_serde::Format`（统一形态），与 `bindings.ts` 逐字同源——两份产物因此语义
//! 等价、业务代码也拿不到 `_Serialize` / `_Deserialize` 双读法。三表仍按方向标注
//! `Phase::Deserialize`（参数面 / Channel 信封）与 `Phase::Serialize`（结果面）并经
//! `select_phase_datatype` 取值：当前是恒等变换，日后重开相位时各自自动对齐。

use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::Path;

use specta::datatype::{DataType, Fields, NamedDataType, NamedReferenceType, Primitive, Reference};
use specta::{Format, FormatError, Types};
use specta_serde::{select_phase_datatype, Phase};
use specta_zod::Zod;
use tauri_specta::{BuilderConfiguration, Casing, LanguageExt};

/// 产物 banner：生成器标识 + 「类型不在此文件」的提醒。
const FILE_BANNER: &str = "\
// 本文件由 `cargo run -p export-bindings` 生成（specta-zod 0.0.4 + PATCH(1..3)），勿手改。
// 纯 schema 产物：只出 schema，不出类型别名、不做类型推断（类型唯一源是 `./bindings.ts`）。
// 装饰器（default / transform / catch / pipe / refine）一律禁用：`z.intersection` 的合并
// 语义在两侧装饰冲突时会 throw，soft-report 链路 MUST NOT 因此在自身内部崩掉。
// IPC 类型唯一源是 `./bindings.ts`；本文件只供 `src/lib/ipc.ts` 做运行期校验。";

/// prelude：zod 导入 + 降级哨兵。哨兵不导出（产品零多余导出），但 `degraded(` 文本可
/// grep、可 `.meta()` 回读原因——合法 `unknown` 仍出 `z.unknown()`，两者形态可区分。
const FILE_PRELUDE: &str = "\
import { z } from \"zod\";

/** 降级哨兵：运行时等价 `z.unknown()`，`meta.degraded` 记原因（可 grep / 可 `.meta()` 回读） */
const degraded = (reason: string) => z.unknown().meta({ degraded: reason });";

/// `Channel<T>` 形参位的原因文案（非 JSON 可表达 → 跳过值校验）。
const CHANNEL_REASON: &str = "channel: 非 JSON 可表达，跳过值校验";

/// 生成期禁止出现的修饰与类型别名（`z.intersection` 的 `mergeValues` 在 `.default()` +
/// `.transform()` 并存时是 throw，soft-report 链路 MUST NOT 因此在自身内部崩掉）。
///
/// `z.object(` 同样入列：strict 写死是本产物的一条硬约束（PATCH(1)），loose 对象既
/// 让「多余键拒绝」失效、又无法被类型层自检发现，只能在生成期 fail-closed。
const FORBIDDEN_SNIPPETS: &[&str] = &[
    ".default(",
    ".transform(",
    ".catch(",
    ".pipe(",
    ".refine(",
    "export type ",
    "z.infer",
    "z.ZodType<",
    "z.object(",
];

/// zod 侧 exporter 的语言入口：单 builder 的第二次 `.export(...)` 走这里。
pub(crate) struct ZodLanguage;

impl LanguageExt for ZodLanguage {
    type Error = specta_zod::Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), specta_zod::Error> {
        let format = ZodFormat::new(cfg);
        let mapped = format
            .map_types(&cfg.types)
            .map_err(|err| specta_zod::Error::framework("zod 格式映射失败", err))?;
        let types = mapped.as_ref();

        let tables = render_command_tables(&format, cfg, types)?;
        let exporter = Zod::new()
            .header(FILE_BANNER)
            .framework_prelude(FILE_PRELUDE)
            .with_raw(tables);
        let text = exporter.export(types, AlreadyMapped)?;

        guard_forbidden(&text)?;
        report_degraded(cfg, types);
        std::fs::write(path, text)?;
        Ok(())
    }
}

/// 格式层：`specta_serde::Format`（统一 serde 形态）+ 本仓线格式口径的三处改写。
///
/// 复用 `specta_serde::Format`（serde 语义的公开层；上游 `SpectaFormat` 是 private
/// 不可复用），再叠加大整数、`char`、`serde_json::Value` 三条改写：
///
/// - 大整数（`u64`/`i64`/`u128`/`i128`/`usize`/`isize`）→ `z.number()`，受
///   `dangerously_cast_bigints_to_number` 支配，与 TS 侧逐字同口径（关闭时上游会以
///   `bigint_forbidden` 显式失败，不静默放宽）；
/// - `char` → `z.string()`（挡掉上游的 `.refine(...)`，生成期硬约束不许 `refine`）；
/// - `serde_json::Value` 的**一切**引用（`NamedReferenceType::Reference` 与
///   `NamedReferenceType::Inline` 两形态）→ `z.unknown()`，并把 `Value` 的具名 schema
///   置空（不渲染）。判据按 `NamedReference` 的 id 解析（`Types::get`）：两种引用形态
///   的 `inner` 不同 ⇒ 值不等，`hidden_references` 的等值匹配只能覆盖 `Reference`
///   形态，`#[specta(inline)]` 形态必须另经 id 解析才能命中（否则 `Value` 会漏成上游
///   的 enum union）。
///
/// 另把 `Result` 的具名 schema 置空：46 个命令的 `Result` 全在结果面被解包 Ok 臂，
/// 无保留类型引用它。两条失败面不同（上游 `reference_dt` 不检查目标 `ty`）：
///
/// - 引用 `types` 中**不存在**的具名类型 → `primitives::inline` 报 dangling / missing
///   类错误，导出失败且不写盘；
/// - 被隐藏声明（`ty: None`）仍被引用 → 导出**成功**、产物留一个未声明标识符，只能由
///   生成期 tsc 自检桩（`bindings.zod.check.ts` → `TS2304`）拦截。
///
/// 前者 fail-closed 在导出链，后者 fail-closed 在自检链——MUST NOT 把「隐藏声明」当作
/// 可自证安全。
pub(crate) struct ZodFormat {
    cast_bigints: bool,
}

impl ZodFormat {
    fn new(cfg: &BuilderConfiguration) -> Self {
        Self {
            cast_bigints: cfg.dangerously_cast_bigints_to_number,
        }
    }

    /// 递归改写单个 datatype（供 `Format::map_type` 与命令三表渲染共用）。
    ///
    /// `types` 是隐藏面判据的第二条来源：`hidden` 只按 `Reference` **等值**匹配（能覆盖
    /// `NamedReferenceType::Reference` 形态），`NamedReferenceType::Inline`（`#[specta(inline)]`）
    /// 的 `inner` 不同 ⇒ 值不等，故一律再经 `types.get`（按 id 解析、与形态无关）判
    /// `Value`。两条判据并存：等值匹配保持既有映射结果逐字不变，id 匹配补上内联形态。
    fn remap_dt(&self, dt: DataType, types: &Types, hidden: &[Reference]) -> DataType {
        match dt {
            DataType::Primitive(primitive) => match primitive {
                Primitive::char => DataType::Reference(specta_zod::define("z.string()")),
                Primitive::usize
                | Primitive::isize
                | Primitive::u64
                | Primitive::i64
                | Primitive::u128
                | Primitive::i128
                    if self.cast_bigints =>
                {
                    DataType::Reference(specta_zod::define("z.number()"))
                }
                other => DataType::Primitive(other),
            },
            DataType::List(mut list) => {
                let ty = self.remap_dt((*list.ty).clone(), types, hidden);
                list.ty = Box::new(ty);
                DataType::List(list)
            }
            DataType::Map(mut map) => {
                let key = self.remap_dt(map.key_ty().clone(), types, hidden);
                let value = self.remap_dt(map.value_ty().clone(), types, hidden);
                map.set_key_ty(key);
                map.set_value_ty(value);
                DataType::Map(map)
            }
            DataType::Nullable(inner) => {
                DataType::Nullable(Box::new(self.remap_dt(*inner, types, hidden)))
            }
            DataType::Struct(mut strct) => {
                strct.fields = self.remap_fields(strct.fields, types, hidden);
                DataType::Struct(strct)
            }
            DataType::Enum(mut enm) => {
                for (_, variant) in &mut enm.variants {
                    variant.fields = self.remap_fields(variant.fields.clone(), types, hidden);
                }
                DataType::Enum(enm)
            }
            DataType::Tuple(mut tuple) => {
                tuple.elements = tuple
                    .elements
                    .into_iter()
                    .map(|element| self.remap_dt(element, types, hidden))
                    .collect();
                DataType::Tuple(tuple)
            }
            DataType::Intersection(parts) => DataType::Intersection(
                parts
                    .into_iter()
                    .map(|ty| self.remap_dt(ty, types, hidden))
                    .collect(),
            ),
            DataType::Generic(generic) => DataType::Generic(generic),
            DataType::Reference(mut reference) => {
                if hidden.contains(&reference) {
                    return DataType::Reference(specta_zod::define("z.unknown()"));
                }
                if let Reference::Named(named) = &mut reference {
                    if types.get(named).is_some_and(is_value_ndt) {
                        return DataType::Reference(specta_zod::define("z.unknown()"));
                    }
                    match &mut named.inner {
                        NamedReferenceType::Inline { dt, .. } => {
                            let inner = self.remap_dt((**dt).clone(), types, hidden);
                            **dt = inner;
                        }
                        NamedReferenceType::Reference { generics, .. } => {
                            for (_, ty) in generics.iter_mut() {
                                let mapped = self.remap_dt(ty.clone(), types, hidden);
                                *ty = mapped;
                            }
                        }
                        NamedReferenceType::Recursive(_) => {}
                    }
                }
                DataType::Reference(reference)
            }
        }
    }

    fn remap_fields(&self, fields: Fields, types: &Types, hidden: &[Reference]) -> Fields {
        match fields {
            Fields::Unit => Fields::Unit,
            Fields::Unnamed(mut unnamed) => {
                for field in &mut unnamed.fields {
                    if let Some(ty) = field.ty.take() {
                        field.ty = Some(self.remap_dt(ty, types, hidden));
                    }
                }
                Fields::Unnamed(unnamed)
            }
            Fields::Named(mut named) => {
                for (_, field) in &mut named.fields {
                    if let Some(ty) = field.ty.take() {
                        field.ty = Some(self.remap_dt(ty, types, hidden));
                    }
                }
                Fields::Named(named)
            }
        }
    }
}

impl Format for ZodFormat {
    fn map_types(&'_ self, types: &Types) -> Result<Cow<'_, Types>, FormatError> {
        let mut mapped = specta_serde::Format.map_types(types)?.into_owned();
        let hidden = hidden_references(&mapped);
        mapped.iter_mut(|ndt| {
            if is_hidden_ndt(ndt) {
                ndt.ty = None;
                return;
            }
            if let Some(ty) = ndt.ty.take() {
                ndt.ty = Some(self.remap_dt(ty, types, &hidden));
            }
        });
        Ok(Cow::Owned(mapped))
    }

    fn map_type(&'_ self, types: &Types, dt: &DataType) -> Result<Cow<'_, DataType>, FormatError> {
        let mapped = specta_serde::Format.map_type(types, dt)?.into_owned();
        let hidden = hidden_references(types);
        Ok(Cow::Owned(self.remap_dt(mapped, types, &hidden)))
    }
}

/// 直通格式：三表在渲染时已各自做过相位选择与本层改写，`Zod::export` 内部的
/// `map_types` 不能再施加一次 serde 格式（会二次改写已选相位的图）。
struct AlreadyMapped;

impl Format for AlreadyMapped {
    fn map_types(&'_ self, types: &Types) -> Result<Cow<'_, Types>, FormatError> {
        Ok(Cow::Owned(types.clone()))
    }

    fn map_type(&'_ self, _types: &Types, dt: &DataType) -> Result<Cow<'_, DataType>, FormatError> {
        Ok(Cow::Owned(dt.clone()))
    }
}

/// `serde_json::Value` 与 `std`/`core::Result` 的具名 schema 不渲染（前者改用
/// `z.unknown()` 内联表达，后者恒在结果面被解包）。
fn is_hidden_ndt(ndt: &NamedDataType) -> bool {
    is_value_ndt(ndt) || is_result_ndt(ndt)
}

fn is_value_ndt(ndt: &NamedDataType) -> bool {
    ndt.name == "Value" && ndt.module_path.contains("serde_json")
}

fn is_result_ndt(ndt: &NamedDataType) -> bool {
    ndt.name == "Result" && matches!(&*ndt.module_path, "std::result" | "core::result")
}

/// 被隐藏具名类型的全部引用（`Value` 的一切引用改写为 `z.unknown()`）。
fn hidden_references(types: &Types) -> Vec<Reference> {
    types
        .into_unsorted_iter()
        .filter(|ndt| is_value_ndt(ndt))
        .map(|ndt| ndt.reference(vec![]))
        .collect()
}

/// 逐命令渲染 `argsSchemas` / `resultSchemas` / `channelArgs` 三张表（raw 段）。
fn render_command_tables(
    format: &ZodFormat,
    cfg: &BuilderConfiguration,
    types: &Types,
) -> Result<String, specta_zod::Error> {
    let exporter = Zod::new();
    let hidden = hidden_references(types);

    let mut args = String::from(
        "/** 逐命令参数 schema（键序 = Rust 形参序；键名 = invoke payload 键名 lowerCamelCase） */\n\
         export const argsSchemas = {\n",
    );
    let mut results = String::from(
        "/** 逐命令结果 schema（`Result` 取 Ok 臂；`Option` 出 nullable） */\n\
         export const resultSchemas = {\n",
    );
    let mut channels = String::from(
        "/** 逐命令 `Channel<T>` 形参的信封 schema（键 = 形参名；无 Channel 的命令为空对象） */\n\
         export const channelArgs = {\n",
    );

    for command in &cfg.commands {
        let accessor = cfg
            .function_casing
            .apply(cfg.command_accessor_name(command))
            .into_owned();

        let mut arg_entries = String::new();
        let mut channel_entries = String::new();
        for (name, dt) in command.args() {
            let key = Casing::CamelCase.apply(name);
            match channel_envelope_dt(dt, types) {
                Some(envelope) => {
                    write!(
                        arg_entries,
                        "\n\t\t\t\"{key}\": degraded(\"{CHANNEL_REASON}\"),"
                    )?;
                    let schema = render_phase_dt(
                        format,
                        &exporter,
                        types,
                        envelope,
                        Phase::Deserialize,
                        &hidden,
                    )?;
                    write!(
                        channel_entries,
                        "\n\t\t\tget \"{key}\"() {{ return {schema}; }},"
                    )?;
                }
                None => {
                    let schema =
                        render_phase_dt(format, &exporter, types, dt, Phase::Deserialize, &hidden)?;
                    write!(arg_entries, "\n\t\t\t\"{key}\": {schema},")?;
                }
            }
        }

        writeln!(
            args,
            "\tget \"{accessor}\"() {{ return z.strictObject({{{arg_entries}\n\t\t}}); }},"
        )?;

        let result = match result_dt(command.result(), types) {
            Some(dt) => render_phase_dt(format, &exporter, types, dt, Phase::Serialize, &hidden)?,
            None => "z.unknown()".to_owned(),
        };
        writeln!(results, "\tget \"{accessor}\"() {{ return {result}; }},")?;

        writeln!(
            channels,
            "\tget \"{accessor}\"() {{ return {{{channel_entries}\n\t\t}}; }},"
        )?;
    }

    args.push_str("} as const;");
    results.push_str("} as const;");
    channels.push_str("} as const;");

    Ok(format!("{args}\n\n{results}\n\n{channels}"))
}

/// 渲染「相位选择 + 本层改写」后的 schema 表达式（`primitives::inline` 是唯一的
/// 具名 schema 命名路径，与具名 schema 段逐字一致）。
fn render_phase_dt(
    format: &ZodFormat,
    exporter: &Zod,
    types: &Types,
    dt: &DataType,
    phase: Phase,
    hidden: &[Reference],
) -> Result<String, specta_zod::Error> {
    let phased = select_phase_datatype(dt, types, phase);
    let remapped = format.remap_dt(phased, types, hidden);
    specta_zod::primitives::inline(exporter, types, &remapped)
}

/// `Result` 的 Ok 臂（命令结果面与 `bindings.ts` 的 `Promise<T>` 同口径）。
fn result_dt<'a>(result: Option<&'a DataType>, types: &Types) -> Option<&'a DataType> {
    let result = result?;
    let DataType::Reference(Reference::Named(reference)) = result else {
        return Some(result);
    };
    let Some(ndt) = types.get(reference) else {
        return Some(result);
    };
    if !is_result_ndt(ndt) {
        return Some(result);
    }
    match &reference.inner {
        NamedReferenceType::Reference { generics, .. } => generics.first().map(|(_, ok)| ok),
        NamedReferenceType::Inline { .. } | NamedReferenceType::Recursive(_) => None,
    }
}

/// `Channel<T>`（Tauri 给 `tauri::ipc::Channel` 派生出的 `TAURI_CHANNEL` 占位类型）的
/// 消息载荷类型。判据照 `tauri-specta`（`name == "TAURI_CHANNEL"` 且 module 在 `tauri::`）。
fn channel_envelope_dt<'a>(dt: &'a DataType, types: &Types) -> Option<&'a DataType> {
    let DataType::Reference(Reference::Named(reference)) = dt else {
        return None;
    };
    let ndt = types.get(reference)?;
    if !(ndt.name == "TAURI_CHANNEL" && ndt.module_path.starts_with("tauri::")) {
        return None;
    }
    match &reference.inner {
        NamedReferenceType::Reference { generics, .. } => {
            generics.first().map(|(_, payload)| payload)
        }
        NamedReferenceType::Inline { .. } | NamedReferenceType::Recursive(_) => None,
    }
}

/// 生成期硬约束自检：命中即失败（fail-closed，不落半成品）。
fn guard_forbidden(text: &str) -> Result<(), specta_zod::Error> {
    for snippet in FORBIDDEN_SNIPPETS {
        if text.contains(snippet) {
            return Err(specta_zod::Error::framework(
                format!("zod 产物含禁用片段 `{snippet}`（生成期硬约束，见 spec「schema 只表达接受面」）"),
                "",
            ));
        }
    }
    Ok(())
}

/// 降级清单打 stderr：降级 MUST NOT 静默发生（`degraded(...)` 只出现在 `Channel`
/// 形参位；出现新降级面时这里必须同步多一行）。
fn report_degraded(cfg: &BuilderConfiguration, types: &Types) {
    for command in &cfg.commands {
        let accessor = cfg
            .function_casing
            .apply(cfg.command_accessor_name(command))
            .into_owned();
        for (name, dt) in command.args() {
            if channel_envelope_dt(dt, types).is_some() {
                let key = Casing::CamelCase.apply(name);
                eprintln!("[zod-bindings] degraded: {accessor}.{key} — {CHANNEL_REASON}");
            }
        }
    }
}

#[cfg(test)]
#[path = "zod_test.rs"]
mod zod_test;
