// ---------------------------------------------------------------------------
// 写入保护（desktop-write-protection）：规则装配 + 判定入口
//
// **落在 loop 守卫链第四段**：成员校验 → policy 档位门 → 路径沙箱 →
// **写入保护闸** → 执行。保护闸位于沙箱改写之后（此时 `path` 已是 canonical
// 绝对路径，判定无需再做 IO 级规范化），且位于执行体之前；拒绝形态复用既有
// `deny()`（拒绝通知 + 同 id is_error ToolResult，`permission_denied` 子类），
// 零新事件词汇、零新退出码。
//
// **装配 / 判定分工**：
// - [`ProtectRules::load`] 是唯一 IO 点（读工作区配置 + 内置规则合并 + 相对
//   glob 归一），每 run 一次（泵起播时）；[`ProtectRules::with_session`] 随后把
//   会话级规则（相位锁）并入同一结构——两者同一次装配，随 `LoopTurn.protect`
//   下发；
// - [`ProtectRules::evaluate`] 是零 IO 零副作用纯函数（无文件系统访问 / 无路径
//   二次规范化 / 无事件通道 / 不 panic），每 tool_call 一次。
//   故 run 内改配置不改变本轮行为（下一 run 重读）。
//
// **射程边界**（由 `evaluate` 的工具名分派表表达，loop 侧不做工具名特判）：
// 只覆盖 `write` / `edit`（按 `path`）与 `bash`（按 `command` + `cwd`）；只读
// 四工具（`read` / `grep` / `glob` / `ls`）与专用工具 `create_explore_note`
// 恒 `None`。专用工具直通是既定知情边界：用户若把探索笔记目录写进保护，
// `edit` 被拒而专用工具直通——缺口记录在案，不在本模块补偿。
//
// **写模型工具经 [`ProtectRules::evaluate_target`] 判定、不经 `evaluate` 分派**：
// `archi_element` / `archi_relation` 的目标文件只有 `plan_*` 之后才解析出来，故执行体在
// 落点解析后以**同一**规则集判定（同一规则序、同一首命中即拒、同一拒串族）。MUST NOT
// 因为「工具无 `path` 入参」把它判在保护射程之外。
//
// **失败面矩阵**（fail-open 于防护、fail-closed 于内置面）：
// | 面 | 方向 | 语义 |
// |---|---|---|
// | 配置读取（缺失 / 坏 JSON / 字段违例） | fail-open（配置面） | 只用内置规则 |
// | 用户 / 会话 glob 非法 / 展开超限 | fail-open（防护面） | 该条不命中（方言层收敛） |
// | `path` / `command` 缺失或非字符串 | fail-open | 放行，错误归属执行体 |
// | 提取出无法解析的 token（含 `$` / 空串） | fail-open | 跳过该 op，其余照判 |
// | 内置规则 | fail-closed | 恒在，不受任何配置状态影响 |
//
// **知情边界**：`xargs` / `sh -c` 递归拆解、生成脚本再执行、`-m` 模块、变量
// 中转路径、base64 编码脚本、`printf` 拼串喂 stdin 均不可静态判别（见
// `shell` / `inline` 的模块 doc）；命令级任意执行与未受保护文件改动仍无护栏。
// ---------------------------------------------------------------------------

use std::path::Path;

use serde_json::Value;

use agent::SessionProtectRule;

use crate::sdk::protect::rules::Rule;

mod inline;
mod reason;
mod rules;
mod shell;

pub(crate) use reason::{contract_lock_template, session_lock_template};

/// 保护规则集（装配产出的纯数据；位次即优先级——内置在前、首个命中即定）。
/// 字段私有：只由 [`ProtectRules::load`] 构造。
#[derive(Debug, Clone)]
pub(crate) struct ProtectRules {
    rules: Vec<Rule>,
}

impl ProtectRules {
    /// 规则装配（**唯一 IO 点**，每 run 一次）：内置恒在，`Some(root)` 时追加
    /// 该 root 的用户规则；`None`（未注入 workspace root）只回内置——不产出
    /// 「保护关闭」的空规则集。
    pub(crate) fn load(workspace_root: Option<&Path>) -> ProtectRules {
        ProtectRules {
            rules: rules::load(workspace_root),
        }
    }

    /// 会话规则追加（**纯数据、零 IO**）：会话规则插在内置之后、用户之前——位次即
    /// 优先级、首个命中即定，故相位锁串不会被用户自选 `reason` 盖掉。
    ///
    /// 与内置规则同一次装配、同一 `ProtectRules` 结构参与判定——**不是第五段闸**。
    /// 会话规则值非法时与用户规则同向收敛（不命中、不 panic）。
    pub(crate) fn with_session(self, session: Option<Vec<SessionProtectRule>>) -> ProtectRules {
        ProtectRules {
            rules: rules::with_session_rules(self.rules, session.as_deref()),
        }
    }

    /// 判定入口（零 IO 纯函数）：命中返回完整拒绝串，未命中返回 `None`。
    ///
    /// 射程由本分派表表达：`write` / `edit` 按路径、`bash` 按命令文本，其余
    /// 工具（含只读四工具与 `create_explore_note`）恒放行。
    pub(crate) fn evaluate(&self, tool: &str, input: &Value, cwd: &Path) -> Option<String> {
        match tool {
            "write" | "edit" => self.evaluate_path(tool, input),
            "bash" => self.evaluate_shell(tool, input, cwd),
            _ => None,
        }
    }

    /// write / edit 路径判定：`path` 缺失或非字符串 → 放行（不抢执行体的错误
    /// 归属）；归一后直接匹配（`path` 已是沙箱解析后的绝对路径，MUST NOT 再做
    /// 一次路径解析——判定零 IO）。
    fn evaluate_path(&self, tool: &str, input: &Value) -> Option<String> {
        let raw = input.get("path").and_then(Value::as_str)?;
        self.deny_normalized(tool, raw)
    }

    /// 写模型工具的**落点**判定入口（`archi_element` / `archi_relation` 的目标文件只有
    /// `plan_*` 之后才知道，故不经 [`ProtectRules::evaluate`] 的工具名分派）。
    ///
    /// 与 `write` / `edit` 同一 `normalize` → `match_path` → `deny_for` 链、同一规则序、
    /// 同一拒串族——零第二份规则集。
    pub(crate) fn evaluate_target(&self, tool: &str, path: &Path) -> Option<String> {
        self.deny_normalized(tool, &path.to_string_lossy())
    }

    /// 归一 + 匹配 + 拒串合成（两条消费面共用的一条链）。
    fn deny_normalized(&self, tool: &str, raw: &str) -> Option<String> {
        let path = rules::normalize(raw);
        let rule = self.match_path(&path)?;
        Some(deny_for(rule, &path, tool))
    }

    /// bash 命令判定：三路按序——① 三态提取（`Path` 走路径 match、`Tree` 走
    /// 子树相交）② 内联脚本关联式 ③ 批量还原。首个命中即返（不遍历剩余操作，
    /// 不拼接多条 reason）。
    fn evaluate_shell(&self, tool: &str, input: &Value, cwd: &Path) -> Option<String> {
        let command = input.get("command").and_then(Value::as_str)?;
        if command.trim().is_empty() {
            return None;
        }
        // 路径分隔符归一：命令文本里的反斜杠形态（Windows 路径）在提取前先
        // 转正斜杠，缩窄两方言差异（内联关联式另用原文，见 `inline` 的转义
        // 纪律——`\"` 归一成 `/"` 会破坏嵌套引号面）
        let normalized = command.replace('\\', "/");

        for op in shell::extract_file_ops(&normalized) {
            if op.kind == shell::OpKind::Revert {
                continue; // 撤销动作不入保护判定
            }
            match op.target {
                shell::OpTarget::Path(path) => {
                    let Some(resolved) = rules::resolve(cwd, &path) else {
                        continue; // `$` / 空串：该 op 放行，其余照判
                    };
                    if let Some(rule) = self.match_path(&resolved) {
                        return Some(deny_for(rule, &resolved, tool));
                    }
                }
                shell::OpTarget::Tree(dir) => {
                    let Some(resolved) = rules::resolve(cwd, &dir) else {
                        continue;
                    };
                    if let Some(rule) = self.dir_rule_hit(&resolved, cwd) {
                        return Some(deny_for(rule, &resolved, tool));
                    }
                }
            }
        }

        for target in inline::write_targets(command) {
            let Some(resolved) = rules::resolve(cwd, &target) else {
                continue;
            };
            if let Some(rule) = self.match_path(&resolved) {
                return Some(deny_for(rule, &resolved, tool));
            }
        }

        if let Some(detail) = shell::bulk_revert_detail(&normalized, &rules::bulk_globs(), cwd) {
            return Some(reason::bulk_revert_reason(&detail, tool));
        }

        None
    }

    /// 逐条匹配（内置在前 → 首个命中即定）：匹配一律经
    /// `checks::globmatch::match_glob`（glob 方言单点，禁第二份实现）。
    fn match_path(&self, path: &str) -> Option<&Rule> {
        self.rules
            .iter()
            .find(|rule| checks::globmatch::match_glob(path, &rule.glob))
    }

    /// 子树相交判定（`find` 搜索根 / `git clean` 目录参数）：按本规则前缀复算，
    /// 与 [`rules::glob_dir_hit`] 共用 [`rules::dir_hits_prefix`] 比较面。
    fn dir_rule_hit(&self, dir: &str, anchor: &Path) -> Option<&Rule> {
        self.rules
            .iter()
            .find(|rule| rules::dir_hits_prefix(dir, &rule.concrete_prefix(anchor)))
    }
}

/// 拒绝串合成（按规则来源三态选串；单串返回，事件与 ToolResult 由 loop 侧
/// 用同一变量合成）。
fn deny_for(rule: &Rule, path: &str, tool: &str) -> String {
    reason::deny_reason(&rule.reason, path, tool)
}

/// 命令文本的写入落点（**与保护闸同一写面提取面**）：分隔符归一 → `shell` 的三态
/// 提取 → 只留 `Write` + `Path` → 经 [`rules::resolve`] 归一。
///
/// 消费方只有收口自检的 T1 判据（MUST NOT 第二份写面解析）。`Revert` / `Delete` 不入
/// ——删除与撤销对架构资产的影响由 `archi::drift::assets_hash` 兜底；inline /
/// 重定向之外的写面同样由哈希兜底。
pub(crate) fn written_paths(command: &str, cwd: &Path) -> Vec<String> {
    if command.trim().is_empty() {
        return Vec::new();
    }
    let normalized = command.replace('\\', "/");
    shell::extract_file_ops(&normalized)
        .into_iter()
        .filter(|op| op.kind == shell::OpKind::Write)
        .filter_map(|op| match op.target {
            shell::OpTarget::Path(raw) => rules::resolve(cwd, &raw),
            shell::OpTarget::Tree(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod inline_test;
#[cfg(test)]
// `evaluate` 分派面与跨模块组合锚的落点（test-design 的显式补列：含逻辑的
// `mod.rs` 配同级 `mod_test.rs`，沿 `compose.rs` / `testexec/mod.rs` 先例）
mod mod_test;
#[cfg(test)]
mod reason_test;
#[cfg(test)]
mod rules_test;
#[cfg(test)]
mod shell_test;
