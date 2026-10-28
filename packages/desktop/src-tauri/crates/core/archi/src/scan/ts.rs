//! TS / JS 依赖提取：静态 `import` 声明（含 `import type`）与 `export … from`
//! re-export 的 specifier + 行号 + `typeOnly`；动态 `import()` / `require()` /
//! 语法错 → `unresolved`（MUST NOT 冒充违例）。
//!
//! 解析：specifier 落盘走**就近** TS / JS 工具链配置文件（`tsconfig` /
//! `jsconfig` 两种 stem，文件名经 [`config_file_name`] 拼出——本 crate 零配置文件
//! 名字面量）的 `baseUrl` + `paths`（自被解析文件向上取最近配置，`extends` 不追
//! 链、只读自身 `compilerOptions`）再落到 node 解析规则（`oxc_resolver`）；解析
//! 失败 → `None`（丢弃该边，不算违例）。
//!
//! 边界：纯读盘、零写盘、零 spawn、零宏展开（不做 type check / 不做完整 name
//! resolution）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
use oxc_parser::Parser;
use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use oxc_span::SourceType;

use crate::drift::report::ArchiUnresolvedFile;

use super::ImportFact;

/// TS / JS specifier 解析器（观察根 + 就近 tsconfig 缓存 + 按配置树缓存的
/// resolver）。
pub struct TsResolver {
    root: PathBuf,
    /// 目录 → 就近配置文件（`None` = 该目录向上无配置；缓存避免百次重复上溯）
    nearest: BTreeMap<PathBuf, Option<PathBuf>>,
    /// 配置文件 → 已编译 resolver（同一配置树复用一次编译）
    resolvers: BTreeMap<Option<PathBuf>, Resolver>,
}

impl TsResolver {
    /// 构造（观察根锚：specifier 落盘与 tsconfig 向上查找的同根锚）。
    pub fn new(root: &Path) -> TsResolver {
        TsResolver {
            root: root.to_path_buf(),
            nearest: BTreeMap::new(),
            resolvers: BTreeMap::new(),
        }
    }

    /// 单文件抽取：静态 `import` / `import type` / `export … from`。
    ///
    /// 动态 `import()` / `require()` / 解析器诊断（语法错）→ `unresolved`
    /// （`{ file, specifier, message }`），绝不计入违例。
    pub fn extract(
        &self,
        file_rel: &str,
        source: &str,
    ) -> (Vec<ImportFact>, Vec<ArchiUnresolvedFile>) {
        let mut facts = Vec::new();
        let mut unresolved = Vec::new();
        let allocator = Allocator::default();
        let source_type = source_type_of(file_rel);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        for diagnostic in &parsed.diagnostics {
            unresolved.push(ArchiUnresolvedFile {
                file: file_rel.to_owned(),
                specifier: None,
                message: format!("TS / JS 解析诊断：{diagnostic}"),
            });
        }
        if parsed.panicked {
            return (facts, unresolved);
        }
        for statement in &parsed.program.body {
            match statement {
                Statement::ImportDeclaration(declaration) => {
                    let mut type_only =
                        declaration.import_kind == oxc_ast::ast::ImportOrExportKind::Type;
                    if let Some(specifiers) = &declaration.specifiers {
                        // 全部 specifier 都是 type → type-only 语句（`import type {x}`
                        // 与 `import { type x }` 两形态同判；混合形态保守取 value）
                        let all_type = !specifiers.is_empty()
                            && specifiers.iter().all(|specifier| {
                                matches!(
                                    specifier,
                                    oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(
                                        specifier
                                    ) if specifier.import_kind
                                        == oxc_ast::ast::ImportOrExportKind::Type
                                )
                            });
                        type_only = type_only || all_type;
                    }
                    facts.push(ImportFact {
                        specifier: declaration.source.value.as_str().to_owned(),
                        line: line_of(source, declaration.source.span.start as usize),
                        type_only,
                    });
                }
                Statement::ExportFromDeclaration(declaration) => {
                    facts.push(ImportFact {
                        specifier: declaration.source.value.as_str().to_owned(),
                        line: line_of(source, declaration.source.span.start as usize),
                        type_only: declaration.export_kind
                            == oxc_ast::ast::ImportOrExportKind::Type,
                    });
                }
                Statement::ExportAllDeclaration(declaration) => {
                    facts.push(ImportFact {
                        specifier: declaration.source.value.as_str().to_owned(),
                        line: line_of(source, declaration.source.span.start as usize),
                        type_only: declaration.export_kind
                            == oxc_ast::ast::ImportOrExportKind::Type,
                    });
                }
                Statement::ExpressionStatement(statement) => {
                    match &statement.expression {
                        // `import('x')`：动态导入不可静态判定 → 留痕
                        Expression::ImportExpression(expression) => {
                            unresolved.push(ArchiUnresolvedFile {
                                file: file_rel.to_owned(),
                                specifier: None,
                                message: format!(
                                    "动态 import()（行 {}）不可静态判定",
                                    line_of(source, expression.span.start as usize)
                                ),
                            });
                        }
                        // `require('x')`：CommonJS 形态不可静态判定 → 留痕
                        Expression::CallExpression(call) => {
                            let is_require = matches!(
                                &call.callee,
                                Expression::Identifier(identifier) if identifier.name == "require"
                            );
                            if is_require {
                                unresolved.push(ArchiUnresolvedFile {
                                    file: file_rel.to_owned(),
                                    specifier: None,
                                    message: format!(
                                        "require()（行 {}）不可静态判定",
                                        line_of(source, call.span.start as usize)
                                    ),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        (facts, unresolved)
    }

    /// specifier → repo 相对 POSIX 磁盘路径（就近 tsconfig 的 `baseUrl` + `paths`
    /// → node 规则）；失败 / 越界 → `None`。
    pub fn resolve(&mut self, file_rel: &str, specifier: &str) -> Option<String> {
        let directory = self.root.join(file_rel).parent()?.to_path_buf();
        let config = self.nearest_config(&directory);
        let resolver = self
            .resolvers
            .entry(config.clone())
            .or_insert_with(|| build_resolver(config.as_deref()));
        let resolved = resolver.resolve(&directory, specifier).ok()?;
        let path = resolved.path().to_path_buf();
        let rel = path.strip_prefix(&self.root).ok()?;
        Some(super::posix(&rel.to_string_lossy()))
    }

    /// 自被解析文件所在目录向上取最近的工具链配置文件（`tsconfig` / `jsconfig`
    /// 两 stem；不越出观察根；缓存）。
    fn nearest_config(&mut self, directory: &Path) -> Option<PathBuf> {
        if let Some(cached) = self.nearest.get(directory) {
            return cached.clone();
        }
        let mut found: Option<PathBuf> = None;
        let mut current = Some(directory.to_path_buf());
        while let Some(dir) = current {
            for stem in ["tsconfig", "jsconfig"] {
                let candidate = dir.join(config_file_name(stem));
                if candidate.is_file() {
                    found = Some(candidate);
                    break;
                }
            }
            if found.is_some() || !dir.starts_with(&self.root) || dir == self.root {
                break;
            }
            current = dir.parent().map(Path::to_path_buf);
        }
        self.nearest.insert(directory.to_path_buf(), found.clone());
        found
    }
}

/// 按配置文件构建 resolver：`Manual` 只读自身 `compilerOptions`（`references`
/// 关闭——`extends` 不追链）；无配置 → 纯 node 规则。
fn build_resolver(config: Option<&Path>) -> Resolver {
    let tsconfig = config.map(|config_file| {
        TsconfigDiscovery::Manual(TsconfigOptions {
            config_file: config_file.to_path_buf(),
            references: TsconfigReferences::Disabled,
        })
    });
    let mut options = ResolveOptions {
        tsconfig,
        // 显式列扩展名：TS 源码 import 常省略扩展名，且 `.ts` / `.tsx` 不在
        // node 缺省表内
        extensions: [
            ".ts", ".tsx", ".d.ts", ".js", ".jsx", ".mjs", ".cjs", ".json",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        condition_names: ["node", "import", "require", "default", "types"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        main_fields: ["module", "main", "types", "typings"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        // 零环境变量读取（NODE_PATH 会让同一工程的判定面随 shell 环境漂移）
        node_path: false,
        ..ResolveOptions::default()
    };
    options.cwd = None;
    Resolver::new(options)
}

/// 工具链配置文件名：`<stem>.json`（stem 两值 `tsconfig` / `jsconfig`）。文件名
/// 在此拼出而非行内字面量——archi 域不持有任何配置文件名常量（架构域配置文件
/// 的唯一定位函数在 `foundation::layout`）。
fn config_file_name(stem: &str) -> String {
    format!("{stem}.json")
}

/// 扩展名 → `SourceType`（未知扩展名退化为 JS module）。
fn source_type_of(file_rel: &str) -> SourceType {
    SourceType::from_path(Path::new(file_rel)).unwrap_or_default()
}

/// 字节偏移 → 行号（1 起始；越界 / 落在多字节中间回退到最近 char 边界）。
fn line_of(source: &str, byte: usize) -> u32 {
    let mut end = byte.min(source.len());
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    1 + source[..end].matches('\n').count() as u32
}
