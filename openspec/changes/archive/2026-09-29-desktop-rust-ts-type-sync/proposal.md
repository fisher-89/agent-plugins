# 提案: desktop-rust-ts-type-sync

> **变更**: desktop-rust-ts-type-sync
> **日期**: 2026-09-28
> **状态**: 草案（proposal 阶段）

---

## 问题

desktop 是 Tauri 2 应用，Rust 侧命令（core/infra/store 三层 + app 命令层）经 serde camelCase 序列化过 IPC，前端靠**手工维护的 TS 镜像**对齐线格式。同步链完全人肉：

- 镜像分裂两处：`src/types/dto.ts`（约 250 行，注释到处是「对齐 Rust serde camelCase 序列化」）+ `src/lib/agent-transport.ts`（`AgentRunMessage` / `AgentStartChainParams` 两个镜像独立于 dto.ts）。字段增删/改名全靠 reviewer 记得跟随，无任何机械校验，漂移只能在运行时暴露或被 review 漏掉。
- invoke 裸字符串：约 27 个 ts/tsx 文件直接 `invoke<T>('command_name', { args })`，22 条命令注册于 `main.rs` `generate_handler!`。命令名字符串、参数 key 的 camelCase 全靠手写记忆，invoke 泛型只是断言，不校验命令名与参数形状。
- 震荡有前科：`4c81c9e`（drop env/bare tier）、`a41fde3`（cascade-delete explore runs）每次 store 字段演进都是一次镜像人肉跟随。
- 深层缺陷：**TS 镜像比 Rust 类型「更强」**——Rust 侧 `status` / `env` / `permission_mode` 均为 String（受控值域仅是 doc 注释约定），TS 侧却是字面量 union。真正的单一事实源在 Rust 侧并不完整，TS 镜像在补 Rust 的位。

---

## 提案

以 tauri-specta 一揽子根治：**Rust 类型为单一事实源，生成 TS 类型 + 全部命令的 typed invoke 绑定，生成物入库 + 一致性守卫，前端一次性切换**。四步串联：

1. **Rust 前置 enum 化**（纯内政，先于 specta 接入）：新增 `AgentRunStatus` 枚举（core/agent，与既有 `AgentEnvMode` / `AgentPermissionMode` 同列）；store `AgentRunRecord` 三字段（`env` / `permission_mode` / `status`）String → 枚举；`as_str()` 落库双轨（与 serde 线格式人肉对齐）退役。serde camelCase 线格式逐字不变（unit variant + rename_all camelCase 与现值域一致）；落库编码形态变化经 native_model 版本机制原地演进，存量值域扫描先行。不做此步，codegen 会把 union 降级回 `string`，是类型安全倒退。
2. **tauri-specta 接入**：22 条命令从 `generate_handler!` 迁 specta builder 注册，生成 TS 类型 + typed invoke 绑定（命令名、参数 key camelCase、返回类型全编译期校验）。流式全景为全 Channel、零 emit/listen，`agent_start` / `watch_subscribe` 的 `Channel<...>` 参数一并 typed（specta v2 支持）； specta events 部分用不上。PoC 先行：rc.25 对 `AgentEvent`（`tag="kind"` + `rename_all_fields` 内部标签枚举）与 `Channel<AgentRunMessage>` 的实际出线形态是实现第一步。
3. **生成物入库 + 一致性守卫**：bindings 文件入库版本控制；导出经 Rust 侧入口（搭 `server:test = cargo test --workspace` 顺风车或专用 bin，design 定夺）；check / build 前置重导出，重导出后以 `git diff --exit-code` 类守卫防「入库物悄悄过期」（对应 build-artifacts-stale 前科）；knip ignore 生成目录。
4. **前端一次性切换 + dto.ts shim 过渡**：约 27 个调用文件全部裸 invoke → typed bindings，不分批（双轨共存期 = 0，机械替换，单 change 整体 revert 干净）；`agent-transport.ts` 手写镜像收编进生成物（运行时校验保留，与静态类型互补）；`dto.ts` 收敛为纯 re-export shim（漏网旧 import 不炸，knip 兜漏网名单），退役判据 = knip 报零引用即删，不按时间过渡。

`source` / `source_ref` 保持 String 不收紧（两侧均 string，不构成降级；真要收紧另开 change）。workflow 域类型（`Verdict` / `Inventory` / `ChangeSource` / `FileLogOp`）Rust 侧已是真枚举，生成无降级，仅加导出 derive。

---

## 能力

### 新增能力

- **desktop-ipc-type-bindings** — IPC 类型与命令绑定的代码生成契约：tauri-specta builder 注册全部命令并生成 typed bindings、生成物入库 + 重导出一致性守卫、前端裸 invoke 清零的一次性切换、dto.ts shim 过渡与判据驱动退役、依赖落位（tauri-specta 仅 desktop-app；specta derive 准入 core/infra）与 knip 边界。

### 修改的能力

- **desktop-workspace-store** — 以 ADDED requirement 形式新增「AgentRunRecord 状态字段类型化」：`env` / `permission_mode` / `status` 三字段 String → core/agent 枚举（新增 `AgentRunStatus`），受控字符串约定与 `as_str()` 双轨退役，演进经 native_model 版本机制，线格式逐字不变。既有 requirement（含 `source` 缺省 `debug`、v1→v2 演进、级联删除等）不受影响。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/Cargo.toml` — workspace.dependencies 收敛 `specta` / `specta-typescript` / `tauri-specta`（`=` 精确 pin，三件套 lockstep）
- `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` — 新增 `AgentRunStatus` 枚举；三个枚举加 `specta::Type` derive；`as_str` 落库双轨退役
- `packages/desktop/src-tauri/crates/core/agent/src/event.rs` — `AgentEvent` / `AgentEventKind` / `AgentBlock` 等出线类型加 `specta::Type`
- `packages/desktop/src-tauri/crates/core/workflow/src/` — 出线类型（`ChangeList` / `ChangeDetail` / `ArtifactEnvelope` / `ExploreDoc` / `ExploreScanEntry` 等）加 `specta::Type`，语义不动
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — `AgentRunRecord` 三字段 String → 枚举 + native_model 版本演进；store 模型加 `specta::Type`
- `packages/desktop/src-tauri/src/commands/`（exec / watch / queries / workspaces / explores / db）— 命令签名 specta 化；`AgentRunMessage` / `FileWatchEvent` derive；`STATUS_*` 裸常量退役
- `packages/desktop/src-tauri/src/main.rs` — `generate_handler!` → tauri-specta builder 注册
- `packages/desktop/src-tauri/src/`（新文件，形态 design 定夺）— specta builder 组装 + `export(Typescript, path)` 导出入口（`#[test]` 搭 cargo test 或专用 bin）
- `packages/desktop/src/types/generated/bindings.ts`（新）— 生成物入库；落位（`src/lib/bindings.ts` vs `src/types/generated/`）design 可调，knip ignore 同步
- `packages/desktop/src/types/dto.ts` — 收敛为纯 re-export shim（`export type * from` 生成物；个别名称出入经逐名 re-export 映射）
- `packages/desktop/src/lib/agent-transport.ts` — 删手写 `AgentRunMessage` / `AgentStartChainParams` 镜像，接生成绑定；`readChainParams` 等运行时校验保留
- `packages/desktop/src/hooks/`、`packages/desktop/src/views/`、`packages/desktop/src/app.tsx` — 全部 invoke 调用点（约 27 文件）切 typed bindings，机械替换
- `packages/desktop/knip.json` — 生成目录 ignore
- `packages/desktop/package.json` — check / build 前置重导出 + `git diff` 一致性守卫脚本串接

### 测试文件

- `packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs` — `as_str` 双轨断言改为 serde JSON 线格式逐字断言
- `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` 及 model 相关测试 — 旧形态（String 字段）记录 fixture 自动升级、存量值域扫描断言
- `packages/desktop/src-tauri/src/commands/exec/mod_test.rs` 等命令层测试 — 状态断言枚举化跟改
- `packages/desktop/src/lib/agent-transport.test.ts` 及其余前端既有 vp 测试 — 类型迁移跟改（镜像断言改对生成物）
- 导出入口幂等性（重导出无 diff）即一致性守卫的测试面

### 删除文件

- `packages/desktop/src/types/dto.ts`（判据驱动：shim 化后 knip 报零引用即删；允许落本 change 收尾任务或紧随 change，不设时间过渡期）

### 不要修改

- `source` / `source_ref` 字段类型（保持 String；两侧均 string 不构成降级，收紧另开 change）
- IPC serde camelCase 线格式（逐字不变，含 `AgentEvent` 内部标签形态）
- `AgentRunner` trait 面与 `agent-cli` 租户实现语义（flag 组装行为不变；flag 所需字符串值的取值来源 design 定夺，语义不变）
- workflow 域类型语义（`Verdict` / `Inventory` 等已是真枚举，仅加导出 derive）
- store 演进纪律（native_model 版本机制；MUST NOT 重新引入 legacy redb 迁移层）
- 前端运行时校验逻辑（`readChainParams` 等 ai-sdk body 穿透所需，与静态类型互补，保留）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 新增 `AgentRunStatus`；store 三字段 String → 枚举；`as_str()` 双轨退役 | serde JSON 序列化断言与枚举化前值域逐字一致（`"default"` / `"acceptEdits"` / `"running"` …）；`cargo test --workspace` 全绿 |
| AC-2 | 存量值域扫描 + native_model 版本演进 | 含枚举化前记录的 fixture 库打开与重放成功，无手工迁移步骤；存量三字段值域扫描确认无野值 |
| AC-3 | tauri-specta 接入 + 生成物入库 | 生成 bindings 文件入库，覆盖全部 22 条命令与全部出线 DTO；`agent_start` / `watch_subscribe` 的 Channel 参数 typed；PoC（`tag="kind"` + `rename_all_fields` 出线形态）留档 |
| AC-4 | 重导出挂点 + 一致性守卫 | check / build 前自动重导出；人为改 Rust 类型后不重导出提交，守卫以工作区 diff 报非干净 |
| AC-5 | 前端一次性切换 | `src/` 源码无裸 `invoke('command_name')` 字符串调用（生成物内部除外）；`pnpm check`（tsc + knip + cargo）全绿 |
| AC-6 | dto.ts shim 化 | shim 文件仅 re-export；漏网旧 import 不炸编译；knip 报告 shim 引用计数（零引用即触发删除） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| tauri-specta 长期停留 RC（2.0.0-rc.25），RC 间无 semver 兼容 | 供应链：升级需三件套 lockstep 跟进 | 中 | `=` 精确 pin；升级事件驱动不追新；备选 taurpc（底层同为 specta）与 ts-rs 降级路径已调研留痕 |
| rc.25 对 `tag="kind"` + `rename_all_fields` 枚举与 `Channel<AgentRunMessage>` 出线形态不符预期 | 生成类型不可用或劣化，接入返工 | 低-中 | PoC 作为实现第一步最先行；不合则回退「ts-rs 只生成类型 + 手写薄 invoke 层」备选方案 |
| enum 化改变 bincode 落库编码形态（String 长度前缀 ≠ 变体索引），即使 serde 线格式不变 | 存量 v2 记录直接解码失败 | 高（必然发生） | native_model 版本机制原地演进（v2 → v3，from 旧形态自动升级）；存量值域扫描先行；野值处理策略 design 定夺 |
| 一次性切换约 27 文件，生成物与手写镜像字段级差异（null vs optional 等）集中暴露 | 编译错误批量出现、回归面大 | 中 | tsc 全量类型检查拦截；vp test 回归；替换机械故 diff 可模式化 review；单 change 整体 revert 干净 |
| 生成物入库后悄悄过期，前端类型与 Rust 真源脱钩 | 漂移问题换个形态回归 | 中 | check / build 前置重导出 + `git diff --exit-code` 类守卫（对应 build-artifacts-stale 前科） |
| specta derive 进 core/infra 被误判破「禁 Tauri」分层约束 | 分层纪律争议、评审反复 | 低 | specta 非 tauri 系（普通 derive crate，同 serde 先例）；依赖落位写进 spec 留痕，依赖方向测试机械可验 |
| knip 把生成物未引用导出报 dead code | `client:check` 红灯 | 高（必然发生） | `knip.json` ignore 生成目录（生成物非手写代码） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 生成范围：只 DTO 还是连命令绑定 | tauri-specta（类型 + 命令绑定一步到位） | 命令名 / 参数 key camelCase / 返回类型全编译期校验，invoke 裸字符串根治；全 Channel 零 emit 的流式全景下覆盖面收敛且 Channel 受支持 | ts-rs（只类型，裸字符串原样保留）；只校验不生成（drift check，治标） |
| Rust String → enum 前置重构做不做 | 做（agent 域 `env` / `permission_mode` / `status` 三字段） | 不做则 codegen 把 union 降级回 string，类型安全倒退；Rust 侧同时得穷尽性检查；线格式零变化 | 不做（接受降级） |
| 生成物入库 vs 构建时生成 | 入库 + 检查/构建时重导出 + diff 守卫 | build-artifacts-stale 前科；desktop 本地应用无 CI 生成时机保证 | 纯构建时生成（产物易 stale） |
| 前端迁移策略 | 一次性切换（22 命令 × 约 27 文件） | 双轨共存期 = 0；替换机械、量不在风险里；desktop 无灰度，分批兑现不了「每步小」的价值；revert/bisect 粒度干净 | 按命令分批（每批间两套心智/异常路径） |
| dto.ts 退役形态 | 纯 re-export shim 过渡，knip 报零引用即删 | dto.ts 全量是 Rust 镜像、无前端独有类型，终局必删；过渡期唯一价值 = 一次性大切换的容错网 + 名称映射 | 直接删（漏网 import 炸编译/运行时） |
| enum 化前置与主体是否拆两个 change | 合为一个 change | 用户拍板；enum 化的独立验收面（线格式逐字不变、cargo test 全绿）转为 change 内部任务排序约束（排在 specta 接入之前） | 拆 A/B 两 change 分段合入 |
| `source` 是否随 enum 化收紧 | 不收紧，保持 String | 两侧均为 string 不构成降级；收紧需再一轮线格式审视 | 收紧为枚举（另开 change） |

### 待决问题

- PoC 实际出线形态（rc.25 × `AgentEvent` 内部标签枚举 × `Channel<AgentRunMessage>`）——实现期第一步，结果回填 design
- 生成物落位：`src/lib/bindings.ts` vs `src/types/generated/`——design 定夺（knip ignore 同步）
- v2 → v3 演进转换遇野值（非法枚举字符串）的处理：报错 vs 兜底变体——design 定夺，存量值域扫描结果为准
- 导出入口形态：`#[test]` 搭 `server:test` vs 专用 bin——design 定夺
- `flags.rs` 所需 permission-mode 字符串值的单一来源：serde 序列化派生 vs 保留 CLI 值域 helper——design 定夺（行为不变）
