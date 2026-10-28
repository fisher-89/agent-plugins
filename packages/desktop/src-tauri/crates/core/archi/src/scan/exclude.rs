//! 排除矩阵：默认七条约定（展开为具体 glob）+ 架构域配置 `exclude` 追加。
//!
//! **与工作区 `tests[].excludes` 零耦合**：那是测试执行 scope（可因 e2e 慢而
//! 排除、因 fixture 需要而包含），与架构扫描范围正交——混用会让调测试范围意外
//! 改变架构判定面。本文件的排除面 MUST NOT 读取任何工作区配置。
//!
//! glob 引擎为 `globset`（无花括号交替方言），故默认约定逐条展开为具体 glob；
//! `excluded[].rule` 记**命中的具体形态**（spec 场景逐字要求
//! `rule: "**/*.test.ts"`）。

use globset::{Glob, GlobSet, GlobSetBuilder};

/// 默认排除约定（七条约定 → 13 条具体 glob；逐条标注对应约定）：
///
/// 1. `**/*.test.ts` / `**/*.test.tsx` / `**/*.test.js` / `**/*.test.jsx`
///    —— `**/*.test.{ts,tsx,js,jsx}`（花括号方言，globset 不支持，展开四值）
/// 2. `**/*.spec.ts` / `**/*.spec.tsx` / `**/*.spec.js` / `**/*.spec.jsx`
///    —— `**/*.spec.{ts,tsx,js,jsx}`（同上）
/// 3. `**/__tests__/**` —— 测试目录
/// 4. `**/*_test.rs` —— Rust 单元测试文件（`src/**` 内联惯例）
/// 5. `**/tests/**` —— 集成测试目录
/// 6. `**/fixtures/**` —— 测试装置
/// 7. `**/__mocks__/**` —— mock 目录
///
/// **排除的语义理由**：测试文件天然跨元素引用被测代码，纳入扫描会让每个
/// `foo.test.ts` 对 `foo.ts` 产生一条「未声明依赖」——那不是架构问题，是测试的
/// 固有形状。
pub const DEFAULT_EXCLUDE_RULES: &[&str] = &[
    // 1. `**/*.test.{ts,tsx,js,jsx}`
    "**/*.test.ts",
    "**/*.test.tsx",
    "**/*.test.js",
    "**/*.test.jsx",
    // 2. `**/*.spec.{ts,tsx,js,jsx}`
    "**/*.spec.ts",
    "**/*.spec.tsx",
    "**/*.spec.js",
    "**/*.spec.jsx",
    // 3. 测试目录
    "**/__tests__/**",
    // 4. Rust 单元测试文件
    "**/*_test.rs",
    // 5. 集成测试目录
    "**/tests/**",
    // 6. 测试装置
    "**/fixtures/**",
    // 7. mock 目录
    "**/__mocks__/**",
];

/// 编译一次复用的排除矩阵：默认规则在前、配置追加在后（**追加不替换**）；规则
/// 原文表与 `GlobSet` 命中下标一一对齐（最小下标优先 = 默认规则优先，确定性）。
///
/// 非 `Clone`：编译产物只构建一次（`scan` 入口单点）。
pub struct ExcludeSet {
    set: GlobSet,
    /// 规则原文（下标与 `GlobSet` 编译序对齐）
    rules: Vec<String>,
}

impl ExcludeSet {
    /// 构建排除矩阵：默认约定 + 配置追加。
    ///
    /// 配置条目的合法性判据（两条显式 `Err`——配置是声明，拼错必须停给用户）：
    /// - 无通配符（`*` / `?` / `[`）→ `Err`（目录请写 `src/legacy/**`）；
    /// - `Glob::new` 失败 → `Err`（附非法条目）。
    pub fn build(extra: &[String]) -> Result<ExcludeSet, String> {
        let mut rules: Vec<String> = DEFAULT_EXCLUDE_RULES
            .iter()
            .map(|rule| (*rule).to_owned())
            .collect();
        for entry in extra {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                return Err("架构域配置 exclude 含空白条目（文件级 glob 不得为空）".to_owned());
            }
            if !trimmed.contains('*') && !trimmed.contains('?') && !trimmed.contains('[') {
                return Err(format!(
                    "架构域配置 exclude 条目不是文件级 glob：\"{trimmed}\"（无通配符；目录请写 \
                     \"{trimmed}/**\" 形态）"
                ));
            }
            rules.push(trimmed.to_owned());
        }
        let mut builder = GlobSetBuilder::new();
        for rule in &rules {
            let glob = Glob::new(rule)
                .map_err(|error| format!("架构域排除规则非法：\"{rule}\"（{error}）"))?;
            builder.add(glob);
        }
        let set = builder
            .build()
            .map_err(|error| format!("架构域排除规则编译失败：{error}"))?;
        Ok(ExcludeSet { set, rules })
    }

    /// 命中判定：返回**命中的规则原文**（最小下标优先 = 默认约定优先，确定性）。
    pub fn matched(&self, rel_path: &str) -> Option<String> {
        self.set
            .matches(rel_path)
            .into_iter()
            .min()
            .map(|index| self.rules[index].clone())
    }
}
