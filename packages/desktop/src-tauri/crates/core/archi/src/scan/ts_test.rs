//! `scan::ts` 的单元测试：静态 import / `import type` / `export … from` 的
//! specifier + 行号 + `typeOnly`；动态 `import()` / `require()` / 语法错进
//! `unresolved`；`resolve` 走就近 `tsconfig` / `jsconfig` 的 `baseUrl` + `paths`
//! 与 node 规则。
//!
//! 零 mock：真实 `oxc_parser` / `oxc_resolver`（解析语义即被测对象）；`resolve`
//! 的落盘判定必须真盘——故用 std 自建临时目录（本 crate 无 dev-dependency）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::ts::TsResolver;

// ---------------------------------------------------------------------------
// 真实临时目录（本 crate 无 dev-dependency，故用 std 自建）
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "archi-ts-test-{}-{}-{}",
            std::process::id(),
            unique,
            tag
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录失败");
        TempDir(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    /// 落一个 repo 相对文件（自动建父目录）。
    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建父目录失败");
        }
        fs::write(&path, content).expect("写文件失败");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 纯文本抽取（零盘：`extract` 只吃 `&str`）。
fn extract(file_rel: &str, source: &str) -> (Vec<(String, u32, bool)>, Vec<String>) {
    let temp = TempDir::new("extract");
    let resolver = TsResolver::new(temp.root());
    let (facts, unresolved) = resolver.extract(file_rel, source);
    (
        facts
            .into_iter()
            .map(|fact| (fact.specifier, fact.line, fact.type_only))
            .collect(),
        unresolved.into_iter().map(|entry| entry.message).collect(),
    )
}

// ---------------------------------------------------------------------------
// 正向：静态 import 三形态
// ---------------------------------------------------------------------------

/// 正向：默认导入 / 具名导入 / 命名空间导入三形态各产一条 `ImportFact`
/// （specifier 原文、行号、`type_only = false`）。
#[test]
fn 静态import三形态() {
    let source = "import a from './x';\nimport { b } from './y';\nimport * as c from './z';\n";
    let (facts, unresolved) = extract("src/a.ts", source);
    assert_eq!(
        facts,
        vec![
            ("./x".to_owned(), 1, false),
            ("./y".to_owned(), 2, false),
            ("./z".to_owned(), 3, false),
        ],
        "三条静态 import 各一条 fact，行号逐条正确"
    );
    assert!(unresolved.is_empty(), "静态 import 不产 unresolved");
}

/// 正向（AC-4）：`import type { T } from './types'` → 一条且 `type_only = true`
/// （同权产出，只标注，不豁免）。
#[test]
fn import_type标注type_only() {
    let (facts, unresolved) = extract("src/a.ts", "import type { T } from './types';\n");
    assert_eq!(
        facts,
        vec![("./types".to_owned(), 1, true)],
        "type-only 标注"
    );
    assert!(unresolved.is_empty(), "type-only 不产 unresolved");
}

/// 正向：`export { a } from` / `export * from` / `export type { T } from` 三形态各
/// 一条，末条 `type_only = true`。
#[test]
fn export_from三形态() {
    let source = "export { a } from './a';\nexport * from './b';\nexport type { T } from './c';\n";
    let (facts, unresolved) = extract("src/index.ts", source);
    assert_eq!(
        facts,
        vec![
            ("./a".to_owned(), 1, false),
            ("./b".to_owned(), 2, false),
            ("./c".to_owned(), 3, true),
        ],
        "re-export 三形态行号与 typeOnly 逐条正确"
    );
    assert!(unresolved.is_empty(), "re-export 不产 unresolved");
}

// ---------------------------------------------------------------------------
// 边界：行号精度
// ---------------------------------------------------------------------------

/// 边界：行号取 `from '<specifier>'` 所在行——单行 import 恒等于 `import` 关键字
/// 行；跨行 import 取 specifier 字面量所在行；第 1000 行 import 逐值正确。
#[test]
fn 行号精度三类位置() {
    let single = "import { widget } from './widget';\n";
    let (facts, _) = extract("src/a.ts", single);
    assert_eq!(facts[0].1, 1, "单行 import：行号 = import 关键字行 = 1");

    let multiline = "import {\n  a,\n  b,\n} from './multi';\n";
    let (facts, _) = extract("src/a.ts", multiline);
    assert_eq!(
        facts[0].1, 4,
        "跨行 import：行号取 specifier 字面量所在行（第 4 行）"
    );

    let mut deep = String::new();
    for index in 1..1000 {
        deep.push_str(&format!("// 第 {index} 行填充\n"));
    }
    deep.push_str("import { z } from './deep';\n");
    let (facts, _) = extract("src/a.ts", &deep);
    assert_eq!(facts.len(), 1, "深位置仍抽出一条");
    assert_eq!(facts[0].1, 1000, "第 1000 行的 import 行号逐值正确");
}

// ---------------------------------------------------------------------------
// 异常：动态导入 / require / 语法错
// ---------------------------------------------------------------------------

/// 异常（AC-3 长尾）：动态 `import()` / `require()` / 模板拼串三形态 → 各一条
/// `unresolved`（file / message 非空，specifier 无归因故为 `None`），零 fact。
#[test]
fn 动态导入与require进unresolved() {
    let temp = TempDir::new("dynamic");
    let resolver = TsResolver::new(temp.root());
    let cases = [
        "import('./dyn');\n",
        "const namespace = 'x';\nrequire('./req');\n",
        "const name = 'a';\nimport(`./${name}`);\n",
    ];
    for source in cases {
        let (facts, unresolved) = resolver.extract("src/a.ts", source);
        assert!(facts.is_empty(), "动态形态不产 ImportFact：{source:?}");
        assert_eq!(unresolved.len(), 1, "动态形态各一条 unresolved：{source:?}");
        let entry = &unresolved[0];
        assert_eq!(entry.file, "src/a.ts", "留痕文件为被解析文件");
        assert!(entry.specifier.is_none(), "动态形态无 specifier 可归因");
        assert!(!entry.message.is_empty(), "留痕文案非空");
    }
}

/// 异常：语法错三形态 → `unresolved` 非空且不 panic（已解析出的前缀 fact 允许
/// 保留）。
#[test]
fn 语法错进unresolved且不panic() {
    let cases = [
        "import a from './a';\nfunction broken(\n",
        "import { b } from './b';\nconst x = ((((\n",
        "<<<>>>\n",
    ];
    for source in cases {
        let (_, unresolved) = extract("src/a.ts", source);
        assert!(
            !unresolved.is_empty(),
            "语法错必须有留痕（不 panic、不冒充违例）：{source:?}"
        );
        assert!(
            unresolved.iter().all(|message| !message.is_empty()),
            "留痕文案非空"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：零 fact 面与 specifier 文本边界
// ---------------------------------------------------------------------------

/// 边界：空源 / 仅注释 / 仅类型声明（无 import）→ 零 fact 零 `unresolved`。
#[test]
fn 零fact面() {
    for source in [
        "",
        "// 只有注释\n/* 块注释 */\n",
        "type A = { x: number };\n",
    ] {
        let (facts, unresolved) = extract("src/a.ts", source);
        assert!(facts.is_empty(), "空面不产 fact：{source:?}");
        assert!(unresolved.is_empty(), "空面不冒充留痕：{source:?}");
    }
}

/// 边界：specifier 文本原样保留（`@scope` 包 / `#internal` 别名 / `?raw` 查询串 /
/// emoji / >1000 chars）——不归一、不截断。
#[test]
fn specifier文本原样保留() {
    let long = "a".repeat(1200);
    let cases = [
        "@scope/pkg",
        "#internal",
        "./a?raw",
        "./图标/🎯",
        long.as_str(),
    ];
    for specifier in cases {
        let source = format!("import x from '{specifier}';\n");
        let (facts, unresolved) = extract("src/a.ts", &source);
        assert_eq!(facts.len(), 1, "specifier {specifier:?} 应产一条 fact");
        assert_eq!(facts[0].0, specifier, "specifier 原文逐字保留（不归一）");
        assert!(unresolved.is_empty(), "静态 import 不产留痕");
    }
}

/// 正向：混合文件（2 静态 + 1 动态 + 1 `import type`）→ fact 3 条 + `unresolved`
/// 1 条，行号互不串位。
#[test]
fn 混合文件行号不串位() {
    let temp = TempDir::new("mixed");
    let resolver = TsResolver::new(temp.root());
    let source = "import a from './a';\nimport type { T } from './t';\nimport('./dyn');\nimport b from './b';\n";
    let (facts, unresolved) = resolver.extract("src/mixed.ts", source);
    let lines: Vec<u32> = facts.iter().map(|fact| fact.line).collect();
    assert_eq!(lines, vec![1, 2, 4], "三条静态事实的行号互不串位");
    assert_eq!(
        facts
            .iter()
            .map(|fact| fact.type_only)
            .collect::<Vec<bool>>(),
        vec![false, true, false],
        "type-only 只标注第二条"
    );
    assert_eq!(unresolved.len(), 1, "一行动态导入一条留痕");
}

// ---------------------------------------------------------------------------
// 正向：resolve 相对路径与配置映射
// ---------------------------------------------------------------------------

/// 正向：node 规则三布局（`src/x.ts` / `src/x/index.ts` / `src/x.tsx`）各落对文件，
/// 结果为仓相对 `/` 分隔。
#[test]
fn resolve相对路径三布局() {
    let flat = TempDir::new("flat");
    flat.write("src/x.ts", "export const x = 1;\n");
    flat.write("src/a.ts", "import x from './x';\n");
    let mut resolver = TsResolver::new(flat.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "./x"),
        Some("src/x.ts".to_owned()),
        "同名 .ts 文件"
    );

    let directory = TempDir::new("dir-form");
    directory.write("src/x/index.ts", "export const x = 1;\n");
    directory.write("src/a.ts", "import x from './x';\n");
    let mut resolver = TsResolver::new(directory.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "./x"),
        Some("src/x/index.ts".to_owned()),
        "目录形态 index.ts"
    );

    let tsx = TempDir::new("tsx");
    tsx.write("src/x.tsx", "export const X = () => null;\n");
    tsx.write("src/a.ts", "import { X } from './x';\n");
    let mut resolver = TsResolver::new(tsx.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "./x"),
        Some("src/x.tsx".to_owned()),
        ".tsx 扩展名在解析表内"
    );
}

/// 正向（AC-3）：tsconfig `paths` 别名解析 → `src/auth/token.ts`。
#[test]
fn resolve_tsconfig别名() {
    let temp = TempDir::new("alias");
    temp.write(
        "tsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@/*\": [\"src/*\"] }\n  }\n}\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write("src/a.ts", "import { token } from '@/auth/token';\n");
    let mut resolver = TsResolver::new(temp.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "@/auth/token"),
        Some("src/auth/token.ts".to_owned()),
        "别名经 tsconfig paths 解析"
    );
}

/// 正向：只有 `jsconfig.json` 时与 tsconfig 同结果（两 stem 同权）。
#[test]
fn resolve_jsconfig同权() {
    let temp = TempDir::new("jsconfig");
    temp.write(
        "jsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@/*\": [\"src/*\"] }\n  }\n}\n",
    );
    temp.write("src/auth/token.js", "export const token = 1;\n");
    temp.write("src/a.js", "import { token } from '@/auth/token';\n");
    let mut resolver = TsResolver::new(temp.root());
    assert_eq!(
        resolver.resolve("src/a.js", "@/auth/token"),
        Some("src/auth/token.js".to_owned()),
        "jsconfig 与 tsconfig 同权"
    );
}

/// 边界：就近配置按子树分缓存——根配置与子树配置各映射自家同名别名，互不串味。
#[test]
fn resolve就近配置分树() {
    let temp = TempDir::new("nearest");
    temp.write(
        "tsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@/*\": [\"src/*\"] }\n  }\n}\n",
    );
    temp.write(
        "packages/web/tsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@/*\": [\"src/*\"] }\n  }\n}\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write(
        "packages/web/src/auth/token.ts",
        "export const token = 2;\n",
    );
    temp.write("src/a.ts", "import { token } from '@/auth/token';\n");
    temp.write(
        "packages/web/src/a.ts",
        "import { token } from '@/auth/token';\n",
    );
    let mut resolver = TsResolver::new(temp.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "@/auth/token"),
        Some("src/auth/token.ts".to_owned()),
        "根子树映射到根 src"
    );
    assert_eq!(
        resolver.resolve("packages/web/src/a.ts", "@/auth/token"),
        Some("packages/web/src/auth/token.ts".to_owned()),
        "子树按自己的配置映射（缓存不串味）"
    );
}

/// 边界：无配置 / 仅 `baseUrl` / 仅 `paths` 三形态——node 规则与 `baseUrl` 相对
/// 解析仍可用，`paths` 无 `baseUrl` 时不 panic。
#[test]
fn resolve配置缺失与单键形态() {
    let none = TempDir::new("no-config");
    none.write("src/auth/token.ts", "export const token = 1;\n");
    none.write("src/a.ts", "import { token } from './auth/token';\n");
    let mut resolver = TsResolver::new(none.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "./auth/token"),
        Some("src/auth/token.ts".to_owned()),
        "无配置文件时 node 规则可用"
    );

    let base_only = TempDir::new("base-url");
    base_only.write(
        "tsconfig.json",
        "{\n  \"compilerOptions\": { \"baseUrl\": \".\" }\n}\n",
    );
    base_only.write("src/auth/token.ts", "export const token = 1;\n");
    base_only.write("src/a.ts", "import { token } from 'src/auth/token';\n");
    let mut resolver = TsResolver::new(base_only.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "src/auth/token"),
        Some("src/auth/token.ts".to_owned()),
        "只有 baseUrl 时按基址解析"
    );

    let paths_only = TempDir::new("paths-only");
    paths_only.write(
        "tsconfig.json",
        "{\n  \"compilerOptions\": { \"paths\": { \"@/*\": [\"src/*\"] } }\n}\n",
    );
    paths_only.write("src/auth/token.ts", "export const token = 1;\n");
    paths_only.write("src/a.ts", "import { token } from '@/auth/token';\n");
    let mut resolver = TsResolver::new(paths_only.root());
    let outcome = resolver.resolve("src/a.ts", "@/auth/token");
    assert!(
        outcome.as_deref().is_none_or(|path| !path.is_empty()),
        "paths 无 baseUrl 时结果可判（不 panic），落盘路径非空"
    );
}

/// 异常（AC-3）：外部包 / `node:` 协议 / 包内子路径 → `None`（静默丢弃，不当
/// unresolved）。
#[test]
fn resolve外部包静默丢弃() {
    let temp = TempDir::new("external");
    temp.write("src/a.ts", "import x from 'react';\n");
    let mut resolver = TsResolver::new(temp.root());
    for specifier in ["react", "node:fs", "lodash/get"] {
        assert_eq!(
            resolver.resolve("src/a.ts", specifier),
            None,
            "{specifier} 外部包"
        );
    }
}

/// 异常：不可落盘（`./missing` / 含 `\0` 的畸形 specifier / >1000 chars）→ `None`
/// 且不 panic。
#[test]
fn resolve不可落盘() {
    let temp = TempDir::new("unresolvable");
    temp.write("src/a.ts", "import x from './missing';\n");
    let mut resolver = TsResolver::new(temp.root());
    assert_eq!(
        resolver.resolve("src/a.ts", "./missing"),
        None,
        "目标不存在"
    );

    let malformed = resolver.resolve("src/a.ts", "\0");
    assert_eq!(malformed, None, "畸形 specifier 返回 None 不 panic");

    let long = format!("./{}.ts", "a".repeat(1100));
    assert_eq!(resolver.resolve("src/a.ts", &long), None, "超长 specifier");
}

/// 边界：同一 `(file_rel, specifier)` 连续三次解析结果相等（缓存不改变结论）。
#[test]
fn resolve确定性() {
    let temp = TempDir::new("determinism");
    temp.write(
        "tsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@/*\": [\"src/*\"] }\n  }\n}\n",
    );
    temp.write("src/auth/token.ts", "export const token = 1;\n");
    temp.write("src/a.ts", "import { token } from '@/auth/token';\n");
    let mut resolver = TsResolver::new(temp.root());
    let first = resolver.resolve("src/a.ts", "@/auth/token");
    let second = resolver.resolve("src/a.ts", "@/auth/token");
    let third = resolver.resolve("src/a.ts", "@/auth/token");
    assert_eq!(first, second, "两次解析结果相等");
    assert_eq!(second, third, "三次解析结果相等");
}

// ---------------------------------------------------------------------------
// 边界：new 根形态
// ---------------------------------------------------------------------------

/// 边界：不存在的 root / root 是普通文件 / root 含空格与非 ASCII——构造零 panic，
/// 后续 `extract` / `resolve` 返回型可判。
#[test]
fn new根边界零panic() {
    let temp = TempDir::new("root-boundary");
    let missing = temp.root().join("missing-root");
    let file_root = temp.root().join("plain.txt");
    fs::write(&file_root, "not a directory\n").expect("写占位文件失败");
    let spaced = temp.root().join("含 空格 与中文");
    fs::create_dir_all(&spaced).expect("建目录失败");

    for root in [missing, file_root, spaced] {
        let mut resolver = TsResolver::new(&root);
        let (facts, unresolved) = resolver.extract("src/a.ts", "import a from './a';\n");
        assert_eq!(facts.len(), 1, "抽取面与 root 形态无关");
        assert!(unresolved.is_empty(), "静态 import 不产留痕");
        let resolved = resolver.resolve("src/a.ts", "./a");
        assert!(resolved.is_none(), "目标不在盘上 → None（可判、不 panic）");
    }
}
