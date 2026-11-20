# 架构模型规则与架构 Agent prompt 的同步面

workflow_type: requirement

## 一、结论

**结构层同步是机械闭环的，语义层同步只覆盖一份关注点清单**——词汇、目录路径、示例 TOML、门禁指路全部由代码派生且有机守；元素字段纪律（`path` / `kind` / `entry` 默认三档）、计数复述、架构域看护配置、漂移判据读数则完全没有进 prompt，也没进 `AGENT.md`，只存在于 spec 与 ADR。

本探索落三项改造与一项政策变更：

| # | 改造 | 解决的根问题 |
|---|---|---|
| 1 | 知识分层（A/B/C/D 判据） | 可脚本化的知识占用上下文、语义纪律反而缺失 |
| 2 | 收口前引擎自检 | 判据读数对会话不可达，模型可以不自查就收口 |
| 3 | `archi_drift` 只读工具 | 同上，但让模型**自己**能查到具体条目并修正 |
| 4 | 按相位锁定模型写权 | 只有 proposal / dev-design 该写模型，其余相位应只读 |

## 二、事实基线

### 2.1 `source = "archi"` 会话的 prompt 组成

```
tools    = 七内建（read/grep/glob/ls/write/edit/bash）
           + archi_module / archi_search / archi_api
preamble = archi_knowledge::knowledge()
             ├─ SKELETON = include_str!("archi-knowledge.md")   3071 B
             └─ 派生段 = ElementKind::ALL / GateTier::ALL / ApiStatus::ALL
                       / DiagnosticKind::ALL + foundation::layout 三处目录派生
System   = preamble（前）+ 仓库根 AGENT.md 逐字（后），恰一条 System 消息
           （infra/agent/src/sdk/loop.rs:309-313）
```

装配点 `infra/agent/src/compose.rs` 的 `ARCHI_SOURCE` 臂；能力要求见 `desktop-archi-agent`「架构会话的系统知识注入」。

### 2.2 已同步（有机械门）

| 规则面 | 承载 | 门 |
|---|---|---|
| 元素 / 关系字段面八项（kind/gate/api/entry/path/doc/summary/metadata） | 骨架「格式约定」 | 逐字段名断言 |
| kind 四值 / gate 三值+缺省 / api 二值+缺省 / 诊断九词 | 派生段 | `ALL` 驱动，新增变体未同步即红 |
| 模型声明目录 / 契约资产 `<分层…>/<FQN>.md` / 契约索引 | 派生段 | 派生串逐字在场 + 骨架零目录字面量 |
| 示例 TOML 可解析 | 骨架末段 | 真 `TomlParser` + `ModelGraph` 双零诊断 |
| 契约 md 三 fence 结构、四类生成物勿手改、`api:check` 链 | `AGENT.md` | 无（靠 AGENT.md 自身纪律） |
| `doc` 单值 / 仅 `api = none` / `apiUnexpected` | knowledge + AGENT.md 各写一遍 | knowledge 侧有断言，AGENT.md 侧无对拍 |

两次结构性变更（`1e0a1282` 模型声明迁 `graph/model`、`77a61b38` 契约目录镜像 FQN 分层）均同批改了 `archi_knowledge.rs` 与骨架——「目录 / 词表」这类面向来是同步的。

### 2.3 缺口（语义层，零承载）

| 规则（真源） | prompt 里有什么 | 后果 |
|---|---|---|
| `path`：叶元素必填、`./` 前缀、目录尾 `/`、文件到扩展名、容器恒空零例外（`repo-archi-model`） | 只在字段名清单里出现一个词；**示例 TOML 三个元素全无 `path`** | 抄示例写出的新元素不带 path → 漂移 `noPath` warn、不参与归属、**不进检索语料** |
| `entry` 的「约定默认」三档（单文件根=根文件自身 / 目录型根=`index.ts(x)` / crate=`src/lib.rs`），基线恰 3 行显式 entry | 只写「只在约定默认指不到时写」，默认规则本身不写 | 无法判断指不指得到；新 crate 写 entry 即破「恰三行」 |
| `kind` 四值语义（容器 domain / crate、视图 module / 组件文件 component / 包根 package） | 仅派生四词，零语义 | 新元素 kind 无依据 |
| 计数复述单源（模型表头注释 + spec 计数表 + ADR §9 三处同值：53 元素 / 120 关系） | 零提及；ADR 目录被测试硬禁字面量 | 改模型后表头/ADR 不同批更新，且 agent 不知该文件存在 |
| 架构域看护配置 `<archi_dir>/config.json`（`exclude`/`ignore`、deny_unknown_fields、禁 `$schema`） | 零提及，无派生注入 | 「什么进扫描面」这条判据会话内不可读写 |
| 漂移报告面（status 五值、`pathNotFound`/`noPath`/`ambiguousOwner`、observations、`apiFindings` 十三值） | 只指路 `archi_api`（契约轴）；`archi_drift` 是 IPC 命令，**不在 archi 工具面** | 覆盖 / 归属判据对 agent 不可达 |

### 2.4 门禁盲区

`archi_knowledge_test.rs` 守的是：派生词表在场、骨架零字面量零占位符、示例可解析、八条固定关键词与条目数冻结。**没有任何一条测试把 spec/ADR 的字段纪律与骨架散文对拍**——散文语义是唯一没有第二事实源约束的面。

隐式补救：agent 能 `read` 现网 6 个 `graph/model/*.toml` 与 `archi_module` 响应，path/kind 形态靠「邻近样本归纳」习得。后果是「纪律不显式」而非「完全不可知」，故缺口长期不显眼。

### 2.5 双真相源

`doc` / `api` 语义在 `AGENT.md`（「模块 API 契约」段）与 `archi-knowledge.md`（「格式约定」「边界」段）各写一遍，措辞几乎同句；改一处漏一处即 preamble 自相矛盾，且无一致性测试。

## 三、决策与方案

### 决策 1：知识按 A/B/C/D 分层

**判据是两轴乘积**，不是「能不能脚本化」单轴：

```
                        动作前就需要            动作后才需要
                  ┌────────────────────┬────────────────────┐
     可判定       │ B 上下文·一行指针   │ A 工具承载（读数）  │
   （机器能算）   │ 模型声明目录        │ 结构诊断 / 契约判据 │
                  ├────────────────────┼────────────────────┤
     不可判定     │ C 上下文·散文纪律   │ D 模板工具（形态）  │
   （语义/意图）  │ 写面禁令、kind 语义 │ 新元素 toml 骨架    │
                  └────────────────────┴────────────────────┘
```

三条补充规则：

1. **静默错误必须留上下文**：`metadata` 写子表、`[element.App.Auth]` 点分嵌套——解析器不报错，会读成**另一个元素**（`deny_unknown_fields` 不触发）。工具无法在动作前拦住。
2. **写入定位必须留一行**：`write` 要路径参数；`archi_module` 只对**已存在**元素给 `modelFile`，新元素落点无处可查。
3. **`kind` 语义不可脚本化**：容器=domain / crate=module / 组件文件=component 是架构判断，工具只能事后判「和邻居不像」。

**形态原则**：上下文留「何时查」，工具承载「查什么」，模板工具承载「怎么写」。骨架每句归 A/B/C/D，落 A 的必须与工具面同批移动。

机械守门先例已在案：骨架测试硬禁 `architecture` / `models/` / `decisions` 字面量——「可机械化就不许散文手抄」。推广成「A 类句子必须携带工具指路 + 工具确有该字段」即可闭环。

#### 现状错位（骨架 3071B；分节实测：格式约定 994 / 自检纪律 481 / 边界 380 / 示例 348 / 写面 328 / 模型位置 201 / 层级 156 / 续接 131）

| 骨架句 | 应属 | 现状 |
|---|---|---|
| 示例 TOML（348B） | D | 无模板工具（`archi_api bootstrap` 出的是**契约 md** 草稿） |
| 字段面 / 语法形态（约 500B） | D + 指针 | 全靠散文手抄 |
| `entry` 三条纪律 | D/A | **双向缺失**：prompt 不展开默认三档，`archi_module` 也不给解析结果 |
| `doc` 仅 `api = none` 可写 | A | 工具已有判据，可压成一行指路 |
| 祖先先声明 | A | 结构诊断 `orphanHierarchy` 可判 |
| 契约资产 / 索引两处目录（约 120B） | A | `api.contractPath` / `generatedPath` 已给；且 AGENT.md:26 手抄同一串字面量 |
| 结构诊断九词 | A | 工具 `Err` 自带 kind 词 |
| 带引号 FQN 键 / 内联 metadata | C | 必须留 |
| 写面禁令 / 三指针边界 / 续接 | C | 留（`protect` 里零 archi 规则 ⇒ 纯 prompt 约束） |

可移量 ≈ 1400B（骨架 45%），骨架目标 ≈ 1600B。

#### 移出的前置缺口（否则从「知识过期」退化成「知识缺失」）

1. **薄概览不含 `path`**（`ArchiElementBrief` = fqn/kind/gate/api/summary）⇒ 一次无参调用判不出「谁缺 path」。补 `hasPath` 才可移 path 纪律。
2. **无模型 toml 模板**：需 stdout-only 的 `archi_scaffold`（仿 `archi_api bootstrap`，零写盘）。
3. **无漂移工具**：`archi_drift` 只在 IPC（→ 决策 3）。
4. **单元素响应缺 `resolvedEntry`**：`entry` 纪律下移后仍无法验「约定默认指得到」。

#### 更大的上下文账单：`AGENT.md` 常驻

archi 会话每轮常驻 = knowledge 3071B + `AGENT.md` 逐字 6503B ≈ 9.6KB（≈5k tokens），每轮重读重拼（`loop.rs:309-313`）。按节对 archi 会话（写面被限在架构目录内）的相关性：

| `AGENT.md` 节 | 字节 | 对 archi 会话 |
|---|---|---|
| 模块 API 契约 | 2164 | 部分相关，且与骨架重复 |
| Architecture | 1397 | 相关（crate 分层＝kind/path 写入判据来源） |
| Coding Guidelines | 1128 | 无关（不改产品代码） |
| Project Rules | 1276 | 几乎无关（版本纪律 / config schema / spec corpus 全是产品侧） |
| Project Overview | 394 | 弱相关 |

「最不相关的 2.4KB 恰好最常驻」。收益高于骨架瘦身，但撞既有「`AGENT.md` 逐字注入、不包装不改写」（`desktop-agent-execution:329-339`），属独立裁决，本期不做。

### 决策 2：收口前引擎自检

模型给出**收口终答**时，引擎自己跑一次架构判据，结果作为 `SystemNotice` + 引擎日志留痕，**不回灌模型、不催轮次、不发新请求**。选它而非「提示词纪律」或「催一轮」：零额外请求、零 prompt 增量，且不依赖模型配合。

买到三件事：事实留痕（转录里有一行读数）、UI 可反应（新 subtype 可点亮画布刷新）、不可跳过性。

#### 挂点

`run` 有五个出口，只有一个是「模型说完了」：

| 出口 | 检查 | 理由 |
|---|---|---|
| 自然收敛（无 tool_calls） | ✅ 唯一挂点 | —— |
| 空 choice / 截断 / 异常终局 | ❌ | 模型没给终答，读数无意义 |
| `stop_requested`（轮间 / 轮头） | ❌ | 停止是逃逸阀，不能变慢 |
| `pause_turn` 续跑到顶 | ❌ | 非自然收敛 |

```rust
// loop.rs 自然收敛支（546-551 行）；notice 必须先于 finish_success（TurnDone）
if !is_pause_turn(finish_reason) {
    preclose::run(&sender, turn).await;
    finish_success(&sender, turn_index, started, &usage, turn).await;
    return history;
}
```

archi 臂的 `finish` 位是 `None`（`compose.rs:70-85`），无 `finish_step` 收口工具，不存在 change 相位那种「收口工具被调用即零额外请求收口」的捷径。

#### 三处新增公开面

| 需要 | 现状 | 落点 |
|---|---|---|
| 架构资产哈希 | `drift::model_hash` 私有且只覆盖 `graph/model/*.toml` | `archi::drift` 增 `pub fn assets_hash(root)`：model toml + `models/**/*.md` + `*.schema.json` + `INDEX.toml`，沿用「文件名 + 字节累加」 |
| 「这是 archi 会话」 | `turn.archi.is_some()` 会误命中 change 相位（implement / dev-design / code-review 都下发 archi 工具名） | 判 `injections.preamble.is_some()`——该位唯一生产者就是 archi 臂（`desktop-archi-agent:175`），零契约字段扩张、零新枚举 |
| 检查执行面 | `drift::check` 已公开，`infra/agent` 已依赖 `archi` | 直接调，零新 CLI / IPC |

`preamble.is_some()` 的已知边界：`knowledge()` 降级为 `None` 时漏判 ⇒ 该轮不检查。fail-open，与「知识是尽力增强项」一致。

#### 触发与载荷

```
触发 = T1(本轮写过架构域) ∪ T2(资产哈希变了)
  T1: loop 已拿到 write/edit 的 canonical path + bash 写目标（protect 在解析）
      命中 graph/ 或 models/ 前缀
  T2: run 起点 assets_hash ≠ 收口时 assets_hash
都为假 → 只在引擎日志留一行 skip，不出 notice（纯问答不产生转录噪声）
```

```jsonc
{ "subtype": "archi_preclose",
  "payload": { "status": "clean|warnings|errors|noChanges|skipped", "passed": true,
               "errors": 0, "warnings": 43, "unmatched": 302, "excluded": 3,
               "byKind": {"docMissing": 43}, "modelHash": "sha256:…",
               "artifacts": {"mismatched":0,"orphaned":0,"missing":0},
               "changedAssets": true, "durationMs": 74 } }
```

整份 `Report` 不入转录（一次几 KB 反而烧 token）。`status = errors` 时附前 N 条 `{kind, from, to, message}`。

**判据两轴成本分开**：

| 轴 | 调用 | 成本 | 建议 |
|---|---|---|---|
| 覆盖 / 归属 / 契约判据 | `drift::check(Full, changed: &[], change: None)` | 枚举 + 读 + 解析 ~358 源文件，单线程，估 30–150ms（**待实测**） | 必跑 |
| 生成物新鲜度三类差异 | `api::artifact_check` | 走一遍完整「生成预览」 | 可选，仅 T1 命中 `models/` 或 `INDEX.toml` 时跑 |

#### 执行姿势与逃逸阀

- `tokio::task::spawn_blocking`（tokio features 含 `rt`）；`Report` / `CheckError` 皆 `Send`。
- 取消：`RunHandle` 只暴露 `stop_requested() -> bool`，无移交的 `Arc<AtomicBool>` ⇒ 传 `CancelFlag::never()`，不为几十毫秒扩公开面。
- **fail-open 三面**：任何 `CheckError` → 出 `archi_preclose_skipped` notice（带 reason），照常收口；MUST NOT 阻断、MUST NOT 改 `TurnDone`。

#### 连带要改的文本

1. `archi-knowledge.md`「自检纪律」第一条从「模型义务」降为「可提前自查」（引擎已兜底）——省约 150B。
2. 「写面与可见性」第三条「画布不会自动更新」→ 前端消费 `archi_preclose` 自动刷新后删除——省约 60B。
3. spec：`desktop-archi-agent` 增 requirement；`desktop-archi-drift` 登记第三调用面；`desktop-agent-execution` 收敛事件序加「notice 先于 TurnDone」。

### 决策 3：`archi_drift` 作为 archi 臂第四个只读工具

决策 2 解决「不可跳过」，本工具解决「可自检」——两者共用 `drift::check`，互不替代。

#### 两个硬约束

**① 只能 full 模式，而这正好对**：`infra/agent` 不依赖 `vcs`，取不到 `change_edit_set` 的变更集；但 archi 臂 root = 会话 cwd = 主仓根（无 change 上下文），`HEAD` 基准对主仓恒空 diff ⇒ incremental 本就无意义。故参数面不含 `mode`，恒 `full` + `changed: []`，与 `archi_drift_inner` 的 `Full` 分支同源。将来给 change 相位（worktree root）下发才需注入 `Arc<dyn ArchiveVcsPort>`——本期不做。

**② 报告不能直出**：本仓基线（gate 全 `off`）`scan.rs:161` 把 `Off` 元素覆盖的文件推入 `unmatchedFiles`、`matched` 留空 ⇒ **302 条 unmatched（≈15KB）+ 0 matched**；`apiFindings` 43 条 `docMissing`（≈5KB）；`observations` 12 条。合计 20–25KB，逼近 `MAX_RESULT_BYTES = 50_000`。

**语义陷阱**：gate 全 `off` 的工程里 `unmatchedFiles` 没有诊断价值——它展开的是「off 元素不定级」这一常态。模型见 302 条会误判为灾难。工具描述必须点名，摘要里 302 只能出现在**计数位**。

#### 工具契约

```jsonc
{ "name": "archi_drift",
  "parameters": { "scope": "可选。'summary'（缺省）| 'entries' | 'files'" } }
```

恒 `full`、零写盘、参数里无 root（取 `ArchiContext.root`）。三值 `scope` 与 `archi_search` 的 `elements`/`doc`/`all` 同风格；值集外 → `Err` 并点名合法值。

| `scope` | 返回 |
|---|---|
| `summary`（缺省） | `status`/`passed`/`mode`、`model`（元素数、关系数、gate 分布、`structuralErrors`）、`counts`（errors / warnings / suppressed / observations / apiFindings 三档 / scan 三档）、**`errors` + `warnings` + `model.warnings` 全量明细**、`apiFindings.byKind` 计数、`durationMs` |
| `entries` | 追加 `observations` 与 `apiFindings.items` 全量 |
| `files` | 追加 `scan.unmatchedFiles` / `matched` / `excluded` / `unresolved` 全量 |

`errors`/`warnings` 放缺省位：它们通常为 0（本仓 baseline 各 0），空数组不占字节，非空时正是模型必须读的。`durationMs` 由执行体 `Instant` 计时、零改 core，顺带偿还「扫描耗时未知」的欠账。

**顺带补齐**：`CheckError::Config` 文案（如「架构域配置 ignore 第 N 条缺 `to`」）经 `is_error` 回灌——2.3 表里「config.json 会话内不可读写」由本工具面零额外实现地补上。

#### 矩阵勾选（待裁决）

| phase | role | 现状 | 建议 | 理由 |
|---|---|---|---|---|
| `dev-design` | exec / eval | `module` + `search` | **+ `drift`** | 写元素与边 ⇒ 覆盖/归属是它的直接产物 |
| `implement` | executor | `module` + `api` | **+ `drift`** | 改代码 →「未声明依赖」，即「代码变了模型没跟」 |
| `code-review` | evaluator | `module` | **+ `drift`**（需裁决） | 判越界，漂移就是越界读数 |
| `acceptance` | evaluator | `module` | **+ `drift`**（需裁决） | 同上 |
| `proposal` | executor | `search` | 暂不加（决策 4 之后需重议） | 决策 4 给了写权 ⇒ 写完同样需要自查；但 proposal 只落已确定元素，漂移面噪声可能大于收益 |
| `test-design` | executor | `search` | 不动 | 不写模型 |

要破的既有纪律：spec 现文写「`code-review` / `acceptance` 判越界**只读模型面**，不自检生成物新鲜度」，并据此排除 `archi_api`。漂移与之的区别是**时机**：门禁跑在 step 之后，evaluator 跑在 step 之内，提前拿到能少一轮 backtrack——须显式裁决。

#### 落点

| 文件 | 改动 |
|---|---|
| `sdk/mount.rs` | `ARCHI_DRIFT_TOOL` 常量；`archi_drift_definition()`（描述含「gate 全 off 时 unmatchedFiles 是常态不是错误」）；`build_registry()` 加项（13→14，注释同步）；`runner.rs:458` 的 any 判据加一名 |
| `sdk/archi_tool.rs` | `execute` 加一支 + `drift_payload()`（计时 + scope 解析 + 投影），仍过 `cap_result` |
| `compose.rs` | archi 臂 `names.push`；按裁决改 `archi_matrix` |
| **投影单点** | `fn drift_summary(&Report) -> Value`——决策 2 的 notice payload 与工具响应**共用一份**（同 `api::findings` 的装配单点纪律） |
| spec | `desktop-archi-agent` 新 requirement + 矩阵加列；`desktop-archi-drift` 调用面两处 → 三处 |
| 测试 | `mount_test`（13→14、lookup、tier=Read）、`compose_test`（矩阵逐格）、`archi_tool_test`（三 scope 形状、值集外 Err、unmatched 只在计数位） |

#### 风险

- 模型可能一轮连调多次。**不缓存**（沿 `archi_search`「零跨调用缓存」纪律），用 `durationMs` 让浪费可见。
- `scope = "files"` 在 gate 全 off 仓库一次吐 300+ 路径，`cap_result` 会截断——描述提示「只在排查归属问题时用」。

### 决策 4：按相位锁定模型写权

**政策**：模型可写 ⟺ 架构页会话 ∨ (`source = "change"` ∧ phase ∈ {proposal, dev-design} ∧ role = executor)。其余一切锁定。

两条推论必须写进 spec，否则会被读成疏漏：

- **`dev-design/evaluator` 被锁**：白名单按 `(phase, role)` 而非 phase——评审批次不写模型。
- **`explore` / `debug` 也锁**：不是「步骤」，但政策是绝对命题，不设旁路。explore 的 `edit` 只服务笔记，锁模型零损失。

#### 与现状的三处冲突（属改政策，非补工具）

| 现状原文 | 位置 |
|---|---|
| 「MUST NOT 存在专用『写模型』工具（因而也**不存在『该不该给某相位发写工具』的问题**）」 | `desktop-archi-agent:237` |
| 「内置锁面 **MUST NOT** 纳入架构域路径……纳入即等于 agent 改不了模型」（理由是内置 glob **绝对路径、相位无关**） | `desktop-write-protection:302` |
| 「MUST NOT 指示模型写模型文件」 | `desktop-phase-prompts:77` |

第二条禁令的理由是「内置规则相位无关」，没考虑「按相位条件挂载」。故正确做法不是往内置面加条目（那才是被禁的），而是**新增第三个规则来源：会话级规则**——内置面仍恰一条，字面禁令不被违反，被改写的是它的**理由**。

#### 为什么不是「造一个写模型工具」

`archi` crate **全程只读盘**，模型唯一落盘点是 `src-tauri/src/commands/archi/mod.rs:59` 的 `write_atomic`（注释：「本文件之外零 `fs::write`」）。在 infra 层给出真正写盘的 `archi_edit`，就得把落盘点下沉或复制——打穿单点纪律，且与「模型 = 源码」的隐喻相反。

**「写模型的工具」= 既有的 write / edit**（两相位本来就有七件内建）；本政策实做是**强制面**：掐掉其余相位对模型目录的射程。

#### 机制：会话级保护规则（第三来源）

```
规则序：内置(config.json) → 会话(相位锁) → 用户(自选)   首个命中即定
```

会话规则排在用户规则**之前**：两者都是 deny，只影响文案；相位锁文案要带指路，不能被用户自选 reason 盖掉。

**载体 = `SessionInjections` 第五位**（现有四位 `preamble`/`tools`/`finish`/`ask` 都是「本会话能做什么」，同性质）。它 serde-only、无 `specta::Type` ⇒ 不进 TS bindings，additive 加字段成本低；`TurnRequest.injections → SessionOpen.injections` 已自动流到 pump。另两个候选都不行：从 `tools` 反推需「标记工具」（spec 明禁）；`PumpPayload` 由 runner 自建，compose 的相位知识桥不过去。

```rust
// core/agent/src/runner.rs —— additive，serde 缺字段读 None
pub struct SessionInjections {
    pub preamble: Option<String>,
    pub tools: Option<Vec<String>>,
    pub finish: Option<FinishSpec>,
    pub ask: Option<AskMode>,
    /// 会话级写入禁区（第三来源；None = 不追加）。glob 由装配层从
    /// foundation::layout 派生，core 不解释路径语义
    pub protect: Option<Vec<ProtectRule>>,   // { glob, reason }
}
```

Cargo 零新依赖（`infra/agent` 已依赖 `foundation` 与 `archi`）。

**装配单点**（`compose.rs`，与 `archi_matrix` 并列）：

```rust
fn model_write_lock(provenance) -> Option<Vec<ProtectRule>> {
    if provenance.source == ARCHI_SOURCE { return None; }
    match change_phase_role(source_ref) {
        Some((phase, "executor")) if phase == "proposal" || phase == "dev-design" => None,
        _ => Some(vec![rule(format!("**/{}/**", layout::archi_graph_model_rel_dir()), LOCK_REASON)]),
    }
}
```

fail-closed 方向：`source_ref` 畸形 / 未知 phase / 非 change 来源 → 挂锁（与工具面「零 archi 工具」同方向）。

#### 拒绝文案必须指路

`ReasonKind` 新增 `Session(String)`，沿 `reason.rs` 的「掐断换路链」纪律：

```
该路径受相位锁保护：架构模型只允许在 proposal / dev-design 相位写入（经 %t 工具）。
若你需要新增/修改元素或关系：请写进 design.md 或 tasks.md，或在 finish_step 里
backtrack 到 dev-design 由该相位同步模型；用户也可改用架构页会话直接维护。
不要改用其他命令或工具尝试绕过（所有写入路径均被拦截）。
```

#### 锁面范围：只锁 `graph/model/**`

| 路径 | 锁 | 理由 |
|---|---|---|
| `graph/model/**` | ✅ | 就是「模型」 |
| `models/**`（契约 md） | ❌ | `implement/executor` 持有 `archi_api`；契约是手写真相，改代码相位必须能同步改契约——锁了契约判据会永远飘 |
| `graph/INDEX.toml` | ❌ | 生成物，「MUST NOT 手改」已足够 |
| `config.json` | ❌ | 看护配置，不是模型 |

#### 能力面连带（否则白名单里有权限但不会用）

| 相位 | 现状 archi 面 | 需加 | 理由 |
|---|---|---|---|
| `proposal` / executor | `search` | **+ `module`** | 要写必须先能定位（`modelFile` + `modelLine` 才能直接 `edit` 到行）；只有 search 是盲写 |
| `dev-design` / executor | `module` + `search` | **+ `drift`** | 写完自查覆盖 / 归属 |

#### 落点

| 文件 | 改动 |
|---|---|
| `core/agent/src/runner.rs` | `SessionInjections` 加 `protect` + `ProtectRule` 类型；doc「四位」改「五位」 |
| `core/agent/src/runner_test.rs` | 键面断言四键→五键（`:622`）；round-trip 与缺字段 `None` |
| `infra/agent/src/compose.rs` | `model_write_lock(provenance)` 单点；各臂置位（archi 臂与白名单 `None`，其余 `Some`） |
| `infra/agent/src/sdk/protect/rules.rs` | `ReasonKind::Session(String)`；装配接受会话规则 |
| `infra/agent/src/sdk/protect/reason.rs` | 新模板 + 一个 `deny_reason` 分支 |
| `infra/agent/src/sdk/protect/mod.rs` | `ProtectRules::with_session(...)`（保留 `load(root)` 原签名，~10 处测试调用零改动） |
| `infra/agent/src/sdk/runner.rs` | `ProtectRules::load(...).with_session(open.injections.protect)` |
| 测试 | protect（命中 / 未命中 / 文案指路 / 规则序）、compose（lock 矩阵逐格）、loop（拒绝串经 `deny` 双面出线） |

## 四、spec 改写总表

| 能力 | 改写 |
|---|---|
| `desktop-write-protection` | 内置面仍恰一条（字面不变）；新增「会话级规则（第三来源）」+「相位锁」要求；把禁令理由改写为「**相位无关的**锁被禁，相位条件的锁是形态」 |
| `desktop-archi-agent` | 《模型 = 源码》写权收窄为「通路 A 限 proposal / dev-design executor；通路 B 不限」；删「因而不存在该不该发写工具的问题」半句（问题存在，答案是「不发工具、发权限」）；矩阵加「模型写权」列；新增「收口前引擎自检」与 `archi_drift` 工具两个 requirement |
| `desktop-archi-drift` | 调用面两处（命令面 / 门禁 runner）→ 四处（+ 引擎工具面 + 引擎收口自检） |
| `desktop-phase-prompts` | 77 行按相位分流：proposal / dev-design SHALL 指示写面与自查；其余 SHALL 写「模型已锁，需要改模型请 backtrack 到 dev-design」（指路而非指示写，与不越权许诺工具面相容）；矩阵命中面补 `archi_drift` 的「何时用」一句 |
| `desktop-agent-execution` | `SessionInjections` 四位 → 五位；收敛事件序加「notice 先于 TurnDone」 |
| `repo-archi-model` | 无需改（判据本身不变，变的是谁有权改） |

## 五、开放问题

1. **矩阵是否把 `archi_drift` 给 `code-review` / `acceptance`**：需破「这两相位只读模型面」的既有条款，理由是时机差（门禁在 step 后、evaluator 在 step 内）。保守退路：只给 `dev-design/executor` + `implement/executor`。
2. **proposal 写模型的循环风险**：design 未定稿就落元素 ⇒ 名字与关系反复改。建议在 proposal 相位 prompt 加「只为已确定的元素落笔，设计未定的留到 dev-design」。
3. **批量写入可绕过单文件锁**：`git checkout -- <模型目录>` 走 `BulkGlobs`（域目录树 / changes 子树），与相位无关。要么给 `BulkGlobs` 加相位条件，要么沿「protect 本就不是安全边界」的既有口径如实登记缺口。
4. **`AGENT.md` 按来源裁剪**：收益（2.4KB 无关内容）高于骨架瘦身，但撞「逐字注入、不包装不改写」的既有 spec，属独立裁决。
5. **全量扫描耗时未实测**：`durationMs` 进 payload 与工具响应后第一次运行即可量到；量到再决定要不要加预算上限。
6. **`archi_scaffold` 零写盘模板工具**：spec 禁的是「专用**写**工具」，不写盘的模板工具不在此列，且可偿还「示例 TOML 348B 占上下文」的欠账。建议下一批。

## 六、实施顺序

1. **决策 2（收口自检）**——独立、收益立即、无政策冲突，只新增 requirement。
2. **决策 3（`archi_drift` 工具）**——与 2 共用 `drift_summary` 投影单点，宜同批；矩阵勾选先取保守方案。
3. **决策 4（相位锁）**——唯一涉及政策翻转，需改四份 spec，独立成 change。
4. **决策 1（知识分层）**——依赖 2、3 的工具面先就位；`hasPath` / `resolvedEntry` / `archi_scaffold` 是它的前置。
