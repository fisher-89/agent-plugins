# 任务: config-agent-session

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 17 项 + 设计层新增 5 处（`load_file_face` 公共面、`infra/agent/src/lib.rs` 模块登记、`message-cards/config-apply-scope.tsx`、`components/agent/index.tsx` barrel 导出、barrel 契约散文）——逐条登记在 design「变更清单」，成因见 design「事实校准」3 / 4 / 5 与 DD5 / DD12。
>
> **本相位不改测试用例与断言，也不执行任何静态检查 / 测试回归**：新增用例（config 臂装配 / `propose_config` 矩阵 / 写面门 / P1–P4 / 卡片五态）与期望随动（第 14 项计数锚、`fs::write` 落点、`ResizablePanelGroup` 用例改造）归 test-design / test-gen / test-execution 相位——随动清单见 design 末节「交接给下游相位的随动清单」。
>
> **依赖序（十四阶段）**：
>
> - **阶段一**（`core/config` 公共面：`known_keys` / `TestFramework::ALL` / `read_document` / `validate`）是知识派生段、提案工具、写面的共同前提。
> - **阶段二**（文件面写入器 + 写面规划 + 盘面投影 `load_file_face`）依赖阶段一（`read_document` 读取单点与 `validate`）。**同阶段内序固定**：`to_file_value` → `ConfigWrite` / `plan_apply_config` → `load_file_face`——`load_file_face` 的投影语义就是 `to_file_value(assemble(盘上值).config, 盘上值)`，故 MUST 排在写入器之后（阶段一不得出现任何 `to_file_value` 的调用或符号引用）。
> - **阶段三**（知识 preamble）依赖阶段一（词表两出口）。
> - **阶段四**（`propose_config` 工具与挂载）依赖阶段一、二（`load_file_face` / `validate` / `to_file_value`）。
> - **阶段五**（per-turn 上下文与 loop 分派）依赖阶段四（`ConfigContext` 与常量）；与阶段四构成**同一编译单元**——`ToolExec::Config` 变体在阶段四落地后 `loop.rs` 的 `match mount.exec` 非穷尽，由本阶段首条闭合。
> - **阶段六**（compose 第四臂）依赖阶段三、四（知识生产者 + 工具名进挂载清单，否则成员校验 `Err`）。
> - **阶段七**（写面命令 + `write_atomic` 搬移）依赖阶段二。
> - **阶段八**（清单命令）无前置（与阶段七并列）。
> - **阶段九**（`bindings:export`）依赖阶段七、八（命令面已定）。
> - **阶段十**（`use-config-chat`）依赖阶段八 + 九（`listConfigChatSessions` 已出线）。
> - **阶段十一**（面板 / 写回动作面 / 提案卡 / barrel）依赖阶段九、十。
> - **阶段十二**（配置页分栏）依赖阶段十一。
> - **阶段十三**（架构模型资产三件）独立于代码，但须在阶段十一后核对 import 证据。
> - **阶段十四**（版本 + 边界自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ① **写面只经文件面**：任何落盘路径 MUST NOT 出现 `serde_json::to_value(&WorkspaceConfig)`；`to_file_value` 是唯一投影口（复核 `grep -n "to_value" crates/core/config/src/lib.rs` 仅命中写入器内部的 `to_value(config)`）。
> ② **config 隔离**：crate 内零 `config.json` 字面量（路径一律 `foundation::layout::config_path`）；`infra/agent` **零 `fs` 读取配置文件**（盘面读取只经 `config::load_file_face`）。
> ③ **逐字不动面**：`KNOWN_KEYS` 八项、`TestFramework` 八值枚举、`config.schema.json`、`assemble` / `assemble_suite` / `assemble_project` / `assemble_file_rule` / `opt_string` / `opt_string_list` / `threshold` 与全部诊断文案、explore / archi / change 三臂、`mint_archi_context` / `mint_db_context`、`ProtectRules` 与写保护规则集、`commands/archi` 的 `check_root` 与三条 root 文案、`session.source` 存储语义、`.dev-team/architecture/**` 的既有漂移计数复述（`repo-archi-model` / `desktop-frontend-module-entry`）、`openspec/todo.md`。
> ④ **生成物不手改**：`src/types/generated/**` 只经 `bindings:export`；`.i.ts` / `.schema.json` / `INDEX.toml` 只经 `api:export`（本变更预期零 `api:export` diff，见 design 待决问题 3）。
> ⑤ **门禁分工**：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）、`run test` 与 `cargo test --workspace` 由桌面端 static-check / test-execution 相位执行，本相位不运行；自查只收源码级判据（`grep` 零命中、读码复核、`git diff` 边界比对）。

## 阶段一：`core/config` 公共面（读 / 校验 / 词表）

- [x] `packages/desktop/src-tauri/crates/core/config/src/lib.rs`：补**模块 doc**（`//!` 段，置于 `use` 之前）——写明三件事：① config 有两副面孔（wire：`toValue` 形态的四个多词键；file：snake_case 且 `extra` 展开回顶层），**落盘只经 `to_file_value`**（该纪律句写纯文本反引号、**不用 intra-doc 链接**——被引函数在阶段二才落地，否则阶段一留下悬空链接）；② `null` 位语义（`Option` 字段线面出 `null`，组装层对 `null` 与缺键同处置 ⇒ 自洽不变量零特例）；③ `KNOWN_KEYS` 与 `extra` 的口径（顶层未知键 passthrough，元素内未知子键丢弃）。`assemble` doc 对「模块 doc null 位语义」的引用自此成立（design 事实校准 3）。
- [x] 同文件 `TestFramework`：derive 增 `PartialEq, Eq`（`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]`），并新增
  ```rust
  /// 八值枚举清单（与 [`parse_framework`] 双向互证；知识派生段与写入器 P2 锚共用）
  pub const ALL: &'static [TestFramework; 8] = &[ /* 八变体，声明序同 wire 序 */ ];

  /// 值 → 文件面字面量（与 [`parse_framework`] 的 match 逐值对位）
  pub fn as_str(&self) -> &'static str { /* 八臂，返回 "jest" … "pytest" */ }
  ```
  变体、`#[serde(rename)]` 八条、声明序零改动。
- [x] 同文件新增 `known_keys`：
  ```rust
  /// 顶层已知字段集合的只读出口（八键；知识派生段与写入器 P2 锚的唯一持有者）
  pub fn known_keys() -> &'static [&'static str] { &KNOWN_KEYS }
  ```
- [x] 同文件抽私有读取单点 `fn read_document(root: &Path) -> Result<Map<String, Value>, Vec<ConfigDiagnostic>>`（design DD5）：`NotFound` → `Err([FileMissing])`、读失败 → `Err([ReadFailed])`、语法错 → `Err([JsonInvalid])`、顶层非对象 → `Err([InvalidValue($)])`、对象 → `Ok(map)`；**诊断文案逐字搬自现 `load` 的四分支**。`load` 改为 `match read_document(root) { Ok(map) => assemble(&map), Err(diagnostics) => (WorkspaceConfig::default(), diagnostics) }` —— 输出逐字不变（`Err` 分支 MUST NOT 再调 `assemble`，否则顶层非对象会凭空多出 `schema` / `tests` 的 `defaultApplied`）。
- [x] 同文件新增 `validate`（**本阶段只落这一个公共函数**；盘面投影 `load_file_face` 依赖 `to_file_value`，落阶段二）：
  ```rust
  /// 对任意 JSON 值跑同一套校验（与 `load` 的组装语义同源）
  pub fn validate(value: &Value) -> ConfigReport
  ```
  `validate` 走 `value.as_object()`：对象 → `assemble(&map)`，否则 → `(WorkspaceConfig::default(), vec![invalid("$", …)])`（文案与 `load` 的非对象分支同字面量）。

## 阶段二：文件面写入器、写面规划与盘面投影（`core/config`）

> **阶段内依赖序**：`to_file_value` → `ConfigWrite` / `plan_apply_config` → `load_file_face`。三条按此序执行，任一时刻不引用后续条目；`load_file_face` 的投影式含 `to_file_value`，`plan_apply_config` 同时依赖阶段一的 `validate` 与本阶段的 `to_file_value`。

- [x] 同文件新增 `to_file_value`（design DD10 / DD11），五步结构变换，**顺序固定**：
  1. `let mut value = serde_json::to_value(config).map_err(...)?;`
  2. 三次递归 `rename_in_place(&mut value, "staticAnalysis", "static_analysis")` / `("writeProtection", "write_protection")` / `("dependsOn", "depends_on")`——**递归全层级**（`dependsOn` 在 `projects[i]` 内）；
  3. `integerize(&mut value)`（`f64.fract() == 0.0` 且 `abs <= 2^53` → `i64`，否则保浮点；递归；**只作用于结构体侧，即展开 `extra` 之前**）；
  4. `expand_extra(&mut value)?`（`extra: [{key,value}]` 逐条插入顶层；`Map::contains_key` 命中即 `Err(format!("extra 键与已知字段冲突：{key}"))`；`null` 值条目照插）；
  5. `Ok(trim(&value, template))`（两边对象 → 只留 `template` 出现的键、值递归；两边数组 → 按下标配对（`template[i]` 在场则递归，缺席则整值保留）；其余 → 取 normalized 侧）。
  ```rust
  pub fn to_file_value(config: &WorkspaceConfig, template: &Value) -> Result<Value, String>
  ```
- [x] 同文件新增 `ConfigWrite` 与 `plan_apply_config`：
  ```rust
  /// 写面规划产物（写面内部数据：不出线、不落库）
  #[derive(Debug, Clone)]
  pub struct ConfigWrite { pub file: PathBuf, pub content: String }

  /// 纯函数写面规划：零 `InvalidValue` 门 → file face 投影 → pretty + 尾换行（零 IO）
  pub fn plan_apply_config(root: &Path, candidate: &Value) -> Result<ConfigWrite, String>
  ```
  零 `InvalidValue` 门：`validate(candidate)` 后按 `matches!(diagnostic.kind, DiagnosticKind::InvalidValue)` 过滤，非空即 `Err`（文案逐条 `"{path}: {message}"` 再附一行）；通过 → `to_file_value(&report.config, candidate)?` → `serde_json::to_string_pretty` + `format!("{json}\n")` → `ConfigWrite { file: config_path(root), content }`。
- [x] 同文件新增 `load_file_face`（**阶段二末位**：其投影语义 = `to_file_value(assemble(盘上值).config, 盘上值)`，故 MUST 在 `to_file_value` 落地后写）：
  ```rust
  /// 盘面 file face 投影（= `to_file_value(assemble(盘上值).config, 盘上值)`）+ 同一次装配的诊断；
  /// 文件缺失 / 不可读 / 语法错 / 顶层非对象 → `(json!({}), 该条诊断)`；键冲突（不可达）→ `Err`
  pub fn load_file_face(root: &Path) -> Result<(Value, Vec<ConfigDiagnostic>), String>
  ```
  实现走阶段一的 `read_document`（**本函数不新增 `fs::read_to_string` 调用点**）：`Ok(map)` → `let (config, diagnostics) = assemble(&map);` → `to_file_value(&config, &Value::Object(map))?` → `Ok((projected, diagnostics))`；`Err(diagnostics)` → `Ok((json!({}), diagnostics))`（四条文件态诊断原样透出，不再调 `assemble`，与 `load` 同口径）。
- [x] 复核：`grep -n "to_value" packages/desktop/src-tauri/crates/core/config/src/lib.rs` 只命中写入器内部那一处；crate 内 `grep -n "config.json"` 零命中；`plan_apply_config` / `load_file_face` 均复用阶段一的 `validate` / `read_document`（本阶段零新增读盘调用点）。

## 阶段三：配置协议知识的静态 preamble

- [x] 新增 `packages/desktop/src-tauri/crates/infra/agent/src/config-knowledge.md`：静态骨架，覆盖（每项一句到两句，零盘面真值、零词表手抄、零 `<>` 占位符）：config.json 是 agent 不可写文件（写保护内置锁的事实陈述，判据归 `desktop-write-protection`）；顶层八键各自的语义与值域；阈值 0–100；`tests[].root` / `projects[].root` 非空、非绝对、无 `..`、无 glob 通配符；`projects[].language` / `depends_on` 是软规则（协议层零校验、归一只在消费侧、`depends_on` 是平级边表达安装顺序而非层级）；`context` / `rules` 当前零工程消费（仅在用户明确要求时写）；`defaultApplied` 是正常形态（「清零噪音」= 给**有默认值的**字段写真值，不是给 Option 字段写 `null`）；`propose_config` 使用指引（不给候选 = 返回当前盘面；给候选 = 校验回环；候选必须是**完整文件面**、snake_case 多词键；`is_error` 时按逐条诊断修正后重交**完整**候选）。
- [x] 新增 `packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge.rs`（与 `archi_knowledge.rs` 同构三件，design DD2）：
  ```rust
  const SKELETON: &str = include_str!("config-knowledge.md");

  pub(crate) fn knowledge() -> Option<String> { build(SKELETON).ok() }

  pub(crate) fn build(skeleton: &str) -> Result<String, String>
  ```
  `build`：骨架 `trim()` 空 → `Err("知识骨架为空")`；含 `"<>"` → `Err("知识骨架含 <> 占位符…")`；否则 `format!("{}\n\n{derived}", skeleton.trim_end())`，`derived` 两行——`config.json 顶层已知键：<config::known_keys() 八键，`" / "` 连接>`、`tests[].framework 枚举：<config::TestFramework::ALL 逐值 as_str()>`。模块 doc 写明三条纪律（词表只从代码派生 / 骨架自证 / 尽力增强不炸 run）。
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`：`mod archi_knowledge;` 之后增 `mod config_knowledge;`（字母序；**与上一条新增文件是同一条改动的两半**——不登记则不参与编译，阶段六 `crate::config_knowledge::knowledge()` 无从解析）。`#[cfg(test)]` 区的 `mod config_knowledge_test;` 留给 test-gen 相位随用例文件补，本相位不写。
- [x] 复核：文件内零 `<>`、零硬编码八键字面量、零盘面值；`knowledge()` 签名无入参、无 IO。

## 阶段四：`propose_config` 工具与挂载清单第 14 项

> **编译单元提示**：`ToolExec` 增 `Config` 变体后，`sdk/loop.rs` 的 `match mount.exec`（现为穷尽匹配，`ExploreNote` / `Archi` / `Db` / `Pure` / `Protocol` / `FinishStep` / `Ask` 七臂）立即非穷尽，故**阶段四与阶段五构成同一编译单元**：阶段四落地变体、阶段五闭合 `LoopTurn.config` 字段与分派臂，两者连续执行、不在中间交付绿构建（依赖方向仍是四 → 五，阶段五首条即补 `loop.rs`）。

- [x] 新增 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose.rs`：
  - 模块 doc：两模式 / 输出恒单条 file-face JSON / 零副作用 / 无路径参数 / 无会话状态改动。
  - `pub(crate) const PROPOSE_CONFIG_TOOL: &str = "propose_config";`
  - `pub(crate) fn definition() -> rig::completion::ToolDefinition`（design DD3）：`parameters` 为 `{"type":"object","properties":{"candidate":{"type":"object","description":"整份文件面候选（snake_case：static_analysis / write_protection / depends_on；省略即只读模式）"}},"required":[]}`；`description` 逐句写明两模式、file face、`is_error` 后按逐条诊断修正重交完整候选、无路径参数。
  - `pub(crate) struct ConfigContext { root: PathBuf }` + `pub(crate) fn new(root: PathBuf) -> Self` + `pub(crate) fn execute(&self, input: &Value) -> Result<String, String>`：
    ① `let (current, diagnostics) = config::load_file_face(&self.root)?;`
    ② `input.get("candidate")` 缺 / `Value::Null` → 只读模式（`candidate = Value::Null`、`changes = vec![]`、`summary = "只读模式：返回当前盘面（诊断 N 条：…）"` / `"…（无诊断）"`）；
    ③ 给出 → `let report = config::validate(candidate);` 过滤 `InvalidValue`：非空即 `Err`（逐条 `"{path}: {message}"` + 尾行「修正后重交完整候选（一次全量，不接受片段）」）；干净 → `let normalized = config::to_file_value(&report.config, candidate)?;`、`changes = diff(&current, &normalized)`、`summary = "候选合法，共 N 处变更"` / `"候选合法，与当前配置一致"`；
    ④ `serde_json::to_string(&serde_json::json!({ "current": current, "candidate": candidate_out, "changes": changes, "summary": summary }))` 后过 `tools::cap_result`。
  - 私有 `fn diff(current: &Value, candidate: &Value) -> Vec<Change>`（design DD6）：对象递归（键并集，`BTreeMap` 字母序）、数组与标量作叶子；`Change { path, kind, before, after }`，`kind ∈ {"added","removed","changed"}`，`path` 点分 file-face 键路径（无 `$` 前缀）。私有 `#[derive(serde::Serialize)]` 即可，**不导出**。
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`：`pub(crate) mod config;` 之后增 `pub(crate) mod config_propose;`。
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`：①`pub(crate) use crate::sdk::config_propose::PROPOSE_CONFIG_TOOL;`（与既有常量区相邻）；②`ToolExec` 增 `Config` 变体（doc：「`propose_config`：`LoopTurn.config` 上下文」）；③`build_registry` 追加第 14 项 `ToolMount { definition: crate::sdk::config_propose::definition(), tier: ToolTier::Read, exec: ToolExec::Config }`；④`Vec::with_capacity(13)` → `Vec::with_capacity(14)`；⑤`registry` 与 `build_registry` 两段 doc 的「十三项」→「十四项」、按序描述补「→ `propose_config`（Read / `Config`）」。
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs`：`input_path` 的无路径白名单加 `PROPOSE_CONFIG_TOOL`（`pub(crate) use` 列表同批带出常量），doc 由「三个架构只读工具与两只只读 DB 工具」改为「四类专用工具（archi ×3 / db ×2 / `propose_config`）」。

## 阶段五：`ConfigContext` per-turn 铸造与 loop 分派

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`：①`use crate::sdk::config_propose::ConfigContext;`；②新增（design DD7）
  ```rust
  /// `propose_config` 执行上下文 per-turn 铸造（单点；`session_pump` 唯一调用处）：
  /// 名字清单含 `propose_config` 且主仓 root 在场才铸。root MUST 取
  /// `PumpPayload.workspace_root`（主仓根 = 配置权威所在），MUST NOT 取
  /// `SessionCtx.workspace_root`（exec root / change worktree）。
  pub(crate) fn mint_config_context(
      names: &[String],
      workspace_root: Option<&Path>,
  ) -> Option<ConfigContext>
  ```
  判据 = `names.iter().any(|name| name == PROPOSE_CONFIG_TOOL)`，`root` = `workspace_root.map(|root| ConfigContext::new(root.to_path_buf()))`；③`session_pump` 在 `mint_db_context` 之后增调用：`mint_config_context(open.injections.tools.as_deref().unwrap_or_default(), payload.workspace_root.as_deref())`；④`LoopTurn` 装配增 `config`（放在 `db` 之后）。
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`：①`LoopTurn` 增 `pub config: Option<ConfigContext>`（字段 doc：per-turn 铸造、`None` = 未装配，工具调用以 `is_error` 回灌）；②`match mount.exec` 的 `ToolExec::Db` 臂之后增
  ```rust
  ToolExec::Config => match &turn.config {
      Some(ctx) => match ctx.execute(&input) {
          Ok(text) => (text, false),
          Err(error) => (error, true),
      },
      None => ("专用工具执行上下文未装配".to_owned(), true),
  },
  ```
  沙箱闸 / 写保护闸 / 成员校验三条前置零改动。

## 阶段六：`compose` 第四臂（`source = "config"`）

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：①新增常量（与 `ARCHI_SOURCE` 相邻）
  ```rust
  /// config 来源受控字符串（配置页常驻会话）——与命令层 `CONFIG_CHAT_SOURCE`、
  /// 前端 `use-config-chat` 的 `CONFIG_CHAT_SOURCE` 三处同字面量，改动需同步。
  const CONFIG_SOURCE: &str = "config";
  ```
  ②在 archi 臂之后、`if provenance.source != CHANGE_SOURCE` **之前**插入第四臂（design DD1）
  ```rust
  if provenance.source == CONFIG_SOURCE {
      const CONFIG_TOOLS: [&str; 6] = [
          "read", "grep", "glob", "ls", "bash", mount::PROPOSE_CONFIG_TOOL,
      ];
      return injected(
          &CONFIG_TOOLS,
          crate::config_knowledge::knowledge(),
          None,
          Some(agent::AskMode::Generic),
          provenance,
      );
  }
  ```
  ③`assemble_injections` 的 doc 由「三臂」改「四臂」并补 config 臂一句（六名清单：无 `write` / `edit`、无 db 工具；`preamble` = 配置协议知识；`finish = None`；`ask = Generic`；以 `source` 判定、不进 `source_ref` 相位解析）；④兜底分支注释与 doc 的「其余来源（debug 等）」补「非 change、非 config」。
- [x] 复核：explore / archi / change 三臂代码逐字不动；`assemble_injections` 仍**恰一处**；新增臂在兜底之前。

## 阶段七：写面命令与 `write_atomic` 搬移

- [x] 新增 `packages/desktop/src-tauri/src/commands/atomic.rs`：把 `commands/archi/mod.rs` 的私有 `write_atomic` **原样搬入**为 `pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String>`（design DD9：函数体、`{name}.archi-tmp` 临时名、两条失败清理与全部文案逐字不动；模块 doc 说明它是命令层唯一落盘点、`archi_set_gate` 与 `workspace_config_apply` 共用、临时后缀为历史命名）。
- [x] `packages/desktop/src-tauri/src/commands/archi/mod.rs`：删除 `write_atomic` 实现（含其 doc），改 `use crate::commands::atomic::write_atomic;`；`archi_set_gate_inner` 与 `check_root` 及三条 root 文案逐字不动。
- [x] `packages/desktop/src-tauri/src/commands/mod.rs`：`pub mod` 区增 `pub(crate) mod atomic;`（字母序首位）。
- [x] `packages/desktop/src-tauri/src/commands/config/mod.rs`：①模块 doc 改「双命令轨道：读（`workspace_config`）+ 写（`workspace_config_apply`）」，写明「写回唯一入口 = 提案卡『应用』、不经 agent 工具面、不落库、无缓存」；②私有
  ```rust
  /// root 有效性检查（config 轨道私有；不与 archi 的 check_root 合并——两条 root
  /// 文案已分叉）：空白 / 缺失 / 非目录一律 `Err`
  fn check_root(root: &Path) -> Result<(), String>
  ```
  ③命令与实现（design DD8）
  ```rust
  #[tauri::command]
  #[specta::specta]
  pub fn workspace_config_apply(
      root: String,
      candidate: serde_json::Value,
  ) -> Result<WorkspaceConfigReport, String>

  pub fn workspace_config_apply_inner(
      root: &Path,
      candidate: &serde_json::Value,
  ) -> Result<WorkspaceConfigReport, String>
  ```
  `*_inner` = `check_root` → `config::plan_apply_config(root, candidate)?` → `commands::atomic::write_atomic(&plan.file, &plan.content)?` → `config::load(root)` → `WorkspaceConfigReport` 字段平移（`config` / `diagnostics`）。doc 写明「`plan_apply_config` 失败时 `write_atomic` 不在调用链上 ⇒ 失败零写入」。
- [x] `packages/desktop/src-tauri/src/commands/mod.rs`：`all_commands!` 在 `commands::config::workspace_config` 之后增 `commands::config::workspace_config_apply`。
- [x] 复核：`grep -rn "fn write_atomic" src/commands` 恰一处（`atomic.rs`）；`grep -rn "archi-tmp" src/commands/atomic.rs` 命中（后缀保留）。

## 阶段八：config 会话清单命令

- [x] `packages/desktop/src-tauri/src/commands/exec/mod.rs`：①`ARCHI_CHAT_SOURCE` 之后新增
  ```rust
  /// config 来源受控字符串（配置页常驻会话；与 `compose::CONFIG_SOURCE`、前端
  /// `use-config-chat` 的 `CONFIG_CHAT_SOURCE` 三处同字面量，改动需同步）
  const CONFIG_CHAT_SOURCE: &str = "config";
  ```
  ②`list_archi_chat_sessions` 之后新增 `list_config_chat_sessions(stores, root, offset, limit)`（逐行同形：`is_blank_root` → 空页 `SessionPage { sessions: vec![], total: 0 }`；否则 `stores.for_root(&root)?` → `agent_runtime::session_query(store).list_sessions(Some(CONFIG_CHAT_SOURCE), None, offset, limit)`）；doc 抄 `list_archi_chat_sessions` 的口径（一 workspace 一份历史、`source_ref` 不过滤、与 change chat 两段 / 流程四段精确不相撞）。
- [x] `packages/desktop/src-tauri/src/commands/mod.rs`：`all_commands!` 在 `commands::exec::list_archi_chat_sessions` 之后增 `commands::exec::list_config_chat_sessions`。

## 阶段九：绑定类型再生成（生成物）

- [x] 运行 `pnpm -C packages/desktop run bindings:export`，确认 `src/types/generated/bindings.ts` 出现 `workspaceConfigApply: (root: string, candidate: unknown) => …` 与 `listConfigChatSessions: (root: string, offset: number, limit: number) => …`，`bindings.zod.ts` 对应 args 表项（`candidate` 落 `z.unknown()`）与 result 表项齐备。
- [x] `git diff packages/desktop/src/types/generated/` 复核：diff 只来自生成（仅两条新命令 + 必要注释），MUST NOT 出现手工编辑痕迹；`lib/ipc.ts` **零 diff**（Proxy 键遍历自动覆盖新命令）。
- [x] 复核 `api:export` 预期零 diff（本变更不新增契约条目；`INDEX.toml` / `*.schema.json` / `*.i.ts` 不动）。

## 阶段十：`use-config-chat`（会话状态面）

- [x] 新增 `packages/desktop/src/views/config/hooks/use-config-chat.ts`（照 `use-archi-chat.ts` 形态，差异逐条落地）：
  - `const CONFIG_CHAT_SOURCE = 'config'`（注释三处同字面量）、`CONFIG_CHAT_PAGE_SIZE = 20`、`DRAFT_NOTE = '发送首条消息即开启新会话'`、`WRITEBACK_NOTE = '配置写回只在提案卡上点「应用」生效'`。
  - 清单分页面：`ipc.listConfigChatSessions(root, offset, CONFIG_CHAT_PAGE_SIZE)`（**经 `ipc` 而非 `commands`**，design DD13 / 事实校准 6）；`offset === 0` 替换、否则累加；票号防旧页覆盖新页；`root === null` 零 invoke。
  - 面板本地态：`root` 变化即复位 `sessionId = null` + `clear()`（旧根会话 id / 清单不落新根）。
  - 会话面：`useAgentSession({ root, sessionId, source: CONFIG_CHAT_SOURCE, sourceRef: null, cwd: null })`（归属三件，`cwd = null` 让后端回落主仓根）。
  - 重取时机：会话 id 由 `null` 变非 null、`running` 由 true 变 false 各重取首屏（挂载不触发、零轮询）。
  - 动作面：`newSession`（`reset()` + `setSessionId(null)`）、`selectSession`、`onOpenHistory`（`reload()`）、`loadMore`、`send`（`disabledReason !== null || loading` 早退）。
  - 判据矩阵单源：`disabledReason = session.running ? '本会话运行中，请等待收口' : null`；`canSend` / `canStart` 同源；`note = disabledReason ?? (currentSessionId === null ? DRAFT_NOTE : WRITEBACK_NOTE)`。
  - 导出 `ConfigChatState extends SessionPanelState`（另含 `sessionId` / `currentSessionId` / `send` / `stop` / `canSend` / `disabledReason`）；**不持写回动作、不 import 配置域类型**（design 待决问题 2）。
- [x] 复核：本文件零 `commands.` 直调、零 `KNOWN_KEYS` / 配置语义、零手工拼 `source` 字符串过滤。

## 阶段十一：写回动作面 + 提案卡 + barrel 导出

- [x] 新增 `packages/desktop/src/components/agent/message-cards/config-apply-scope.tsx`（design DD12）：
  ```tsx
  export interface ConfigApplyActions {
    /** 主仓 root（写面命令的 store 寻址半边） */
    root: string;
    /** 写回成功后的页面刷新钩子（页面唯一取数路径） */
    onApplied: () => void;
  }
  export function ConfigApplyScope(props: { value: ConfigApplyActions; children: React.ReactNode }): React.JSX.Element
  export function useConfigApplyActions(): ConfigApplyActions | null
  ```
  内部 `createContext<ConfigApplyActions | null>(null)`；`useConfigApplyActions` 缺席返回 `null`（**不抛错**）。
- [x] `packages/desktop/src/components/agent/index.tsx`：增一条值导出 `export { ConfigApplyScope } from './message-cards/config-apply-scope';`（barrel 第 8 名；只加值导出，`ConfigApplyActions` 类型不外露）。
- [x] `packages/desktop/src/components/agent/message-cards/tool-cards.tsx`：新增 `ConfigProposeCard`（**不导出**），五态判据全落 `ToolResult` 形态（design DD14）：
  - `output === null` → 进行中（`TraceCollapsible` 收起 + `Spinner`，与既有卡同形）；
  - `output.isError` → 失败诊断原文（`whitespace-pre-wrap`，`data-testid="config-propose-error"`）；
  - `JSON.parse(output.content)` 失败 → 原文降级（`data-testid="config-propose-raw"`）；
  - `candidate === null` → 只读模式（呈 `summary` + 盘面 JSON 折叠块）；
  - 否则可应用：`changes` 逐行呈 `+ path` / `- path` / `~ path`（按 `kind` 走 `CodeBlockLine` 的 `added` / `removed` 变体，`changed` 走 removed + added 两行），`changes.length === 0` 时按钮 `disabled` + 文案「与当前配置一致」，可应用态**固定呈现**「写回会规范化文件（未知子键、键序与格式会丢失 / 变化）」（不入 tooltip、不折叠）；
  - 「应用」按钮：`useConfigApplyActions()` 为 `null` → 禁用 + 「应用只在配置页可用」；否则 `applying` 置位 → `ipc.workspaceConfigApply(action.root, candidate)` → 成功 `action.onApplied()` + 局部 `applied` 标注（按钮不禁用，幂等）；失败 `setApplyError(String(cause))` inline 呈现且按钮保持可点。
- [x] `BUILD_IN_TOOL_CARDS` 增 `propose_config: ConfigProposeCard`；文件头 doc 补第 8 张卡一句（写回唯一入口 + 五态）。
- [x] 复核：卡片零 `KNOWN_KEYS` / 零配置校验 / 零自算 diff；既有 7 卡代码零改动。

## 阶段十二：`ConfigChatPanel` 与配置页分栏

- [x] 新增 `packages/desktop/src/views/config/components/config-chat-panel.tsx`：
  ```tsx
  export interface ConfigChatPanelProps {
    /** workspace root（清单与发起的 store 寻址半边；null = 零 invoke） */
    root: string | null;
    /** 写回成功后的页面刷新钩子（透传给提案卡） */
    onApplied: () => void;
  }
  export function ConfigChatPanel(props: ConfigChatPanelProps): React.JSX.Element
  ```
  `useConfigChat({ root })` → `root === null` 时直接渲染 `SessionPanel`（无 provider，卡片呈只读态），否则以 `<ConfigApplyScope value={{ root, onApplied }}>` 包裹 `SessionPanel`（`testIdPrefix="config-chat"`、`title="配置 Agent"`）；composer 插槽内为同文件私有 `ConfigComposer`（`AgentInput`，`idPrefix="config-chat"`、placeholder 文案「问这个 workspace 的配置，让它检测项目 / 框架并给出候选…」、`disabled={!state.canSend}` + `disabledReason`）。
- [x] `packages/desktop/src/views/config/config-view.tsx`：改横向分栏（design 组件 8 / 变更清单）：外层 `flex h-full min-h-0 flex-col gap-2` → `ConfigHeader`（`config-refresh` 保留）→ `ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal"`：左 `<ConfigChatPanel root={root} onApplied={state.refresh} />`、`<ResizableHandle withHandle />`、右 `<ResizablePanel minSize="40">` 承载 `ConfigStatusFaces` 与五分区（外层 `h-full min-h-0 overflow-auto pl-1.5`）。
- [x] 同文件注释改判：`ConfigView` / `ConfigHeader` 的 doc 由「只读——无任何写回配置文件的入口」改为「呈现面零编辑控件；写回唯一入口 = 左栏提案卡的『应用』」；`data-testid="config-view"` 与五分区、错误 / 空态 / loading 三面零改动。

## 阶段十三：架构模型资产（三件）

- [x] `.dev-team/architecture/graph/model/06-frontend.toml`：①表头注释 `# 06 · 前端（30 元素 / 71 关系）` → `# 06 · 前端（30 元素 / 72 关系）`（第二行注释逐字不动）；②`App.Frontend.Views.Config` 的两条既有关系之后追加一条（design DD15）
  ```toml
  [[relation]]
  from = "App.Frontend.Views.Config"
  to = "App.Frontend.Components.Agent"
  desc = "经共享面板骨架渲染常驻 config 会话面板"
  ```
  其余元素 / 关系逐字不动（元素零新增）。
- [x] `.dev-team/architecture/decisions/0001-model-granularity.md`：§9 复算三处同批——首段「基线为 **53 元素 / 120 关系**：…前端 71 条」→「…**121 关系**：…前端 72 条」；`:71` 元素行零改动；`:72` 关系行 `06(71) = 120` → `06(72) = 121`。**既有漂移（实读 54 元素 / 122 关系、`06` 段 31 / 73）不修**，MUST NOT 顺手改。
- [x] `.dev-team/architecture/models/Frontend/Components/App.Frontend.Components.Agent.md`：职能段「以 6 条 re-export 透出 7 个名字（3 个组件 + 4 个类型）」→「以 7 条 re-export 透出 8 个名字（4 个组件 + 4 个类型）」；公共面表增一行 `ConfigApplyScope`（组件 / `message-cards/config-apply-scope.tsx` / 「提案卡的写回动作面 Provider（root + onApplied）」）；「契约面留白」段不新增 fence（7 个既有名字同样留白，成因在案）。
- [x] 复核：`grep -n "Views.Config" -A 3 graph/model/06-frontend.toml` 新边 from 行可定位；`views/config/**` 内存在 `from '@/components/agent'` 的 import 证据（`config-chat-panel.tsx` / `config-view.tsx`）。

## 阶段十四：版本、边界自查与交接

- [x] `packages/desktop/package.json`：`version` `0.7.2` → `0.7.3`；确认 `src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零改动）、`src-tauri/Cargo.toml` crate 版本行**不跟**。
- [x] 边界自查（源码级判据，不跑门禁）：
  - `grep -rn "config.json" packages/desktop/src-tauri/crates/core/config packages/desktop/src-tauri/src/commands/config packages/desktop/src-tauri/crates/infra/agent` 零命中（`foundation::layout` 例外不变）；
  - `grep -rn "serde_json::to_value(&WorkspaceConfig\|to_value(config)" packages/desktop/src-tauri` 只命中 `core/config` 写入器内部一处；
  - `grep -rn "propose_config" packages/desktop/src-tauri/crates/infra/agent/src` 只命中常量定义 / 挂载 / 白名单 / 装配 / 分派 / 执行体六处；
  - `grep -rn "commands\." packages/desktop/src/views/config` 零命中（新代码经 `ipc`）；
  - `grep -rn "PROPOSE_CONFIG_TOOL\|CONFIG_SOURCE\|CONFIG_CHAT_SOURCE\|config-chat"` 与设计的三处同字面量口径一致；
  - `git diff --stat` 未触及约束 ③ 的逐字不动面（`KNOWN_KEYS` / `TestFramework` 八值 / `config.schema.json` / 三臂 / `mint_archi_context` / `ProtectRules` / `commands/archi` 的 `check_root` / `.dev-team` 的既有计数复述 / `openspec/todo.md`）。
- [x] 契约面复核：`.dev-team/architecture/models/Backend/` 零新增（`App.Backend.Core.Config` 仍 `absent` / 0 ops）；`App.Frontend.Components.Agent.ToolCards` 契约零改动（`ConfigProposeCard` 不外露）；若 `api:check` 因 barrel 第 8 名报新 `apiUndeclared`（该元素 `gate = "off"`），按 design 待决问题 3 处置，**不得**手写 `api-reexport` fence。
- [ ] 交接（由桌面端相位执行，本相位不运行）：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）、`pnpm -C packages/desktop run test`、`cargo test --workspace`；并把 design 末节「交接给下游相位的随动清单」（第 14 项计数锚 / 结构断言 / `fs::write` 落点 / 新用例清单 / `config-view.test.tsx` 分栏改造）交给 test-gen 与 test-execution。
