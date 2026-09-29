# Desktop:Rust API 生成 TS 类型(前后端接口类型同步)

## 主题与背景

desktop 是 Tauri 2 应用。Rust 侧命令(core/infra/store 三层 + app 命令层)经 serde
camelCase 序列化过 IPC;前端目前用**手工维护的 TS 镜像文件**对齐线格式。本探索调研:
镜像现状与漂移风险、代码生成方案空间(ts-rs / tauri-specta / 只校验不生成)、以及
本项目特有的工程约束(knip、vp 工具链、core 禁 Tauri)。

---

## 一、现状地图(2026-09-28 调研)

### 类型源头(Rust 侧,三层 + app 层)

```
crates/core/agent    AgentBlock / AgentEventKind / AgentEvent
                     (serde tag="kind" 内部标签枚举, rename_all + rename_all_fields camelCase)
crates/core/workflow ChangeList / ChangeDetail / ArtifactEnvelope /
                     ExploreDoc / ExploreScanEntry
crates/infra/store   WorkspaceRecord / AgentRunRecord / ExploreRecord /
       model.rs      ModelInfo / RecordEnvelope
                     (AgentRunRecord 带 native_model 版本化, V1 形态不出线)
src-tauri/commands   AgentRunMessage(ipc 信封, app 层 IPC 类型, tag="ipc")
       exec/agent.rs FileWatchEvent(watch/)
```

### 前端镜像(手工,分裂在两处)

- `src/types/dto.ts`(~250 行):注释到处是「对齐 Rust serde camelCase 序列化」
- `src/lib/agent-transport.ts`:`AgentRunMessage` + `AgentStartChainParams`
  两个镜像**不在 dto.ts**,单独写在 transport 文件里 → 第三处易漂点
- 调用面:~27 个 ts/tsx 文件直接 `invoke<T>('command_name', { args })`,
  23 条命令注册于 `main.rs` `generate_handler!`——命令名字符串、参数 key 的
  camelCase 全靠手写记忆,**invoke 泛型只是断言,不校验命令名与参数形状**

### 同步链(现状)

```
Rust struct ──serde camelCase──▶ IPC 线格式 ──人肉对齐──▶ dto.ts 手写镜像
     │                                                          ▲
     └── 字段增删/改名时,镜像靠 reviewer 记得改 ──────────────────┘
         (无任何机械校验;漂移只能在运行时暴露或被 review 漏掉)
```

近期 git 史已有相关震荡:`4c81c9e drop env/bare tier from agent runs`、
`a41fde3 cascade-delete explore runs/events`——每次 store 字段演进都是一次
镜像人肉跟随。

## 二、关键发现

### 1. TS 镜像比 Rust 类型「更强」——codegen 会造成类型降级

Rust 侧 `status` / `env` / `permission_mode` 均为 `String`(「受控字符串」仅是
doc 注释约定);TS 镜像却是字面量 union(`'running' | 'completed' | …`)。
**真正的单一事实源在 Rust 侧并不存在**——TS 镜像在补 Rust 的位。直接上代码生成
会把 union 降级回 `string`,是 type safety 倒退。若走生成路线,前置工作应是
Rust 侧 String → 真枚举(serde 能表达,且让 Rust 侧也拿到穷尽性检查)。

### 2. AgentEvent 的 serde 形态是硬骨头(但可行)

`tag = "kind"` + `rename_all_fields = "camelCase"` 的内部标签枚举。
出线类型**没有 flatten**(flatten 只在 store 内部的 AgentEventRecord,不出线)。
ts-rs changelog 确认支持 `rename_all_fields`(#287 修过含 tuple/unit 变体的场景);
tauri-specta/specta 对 internally tagged 支持良好。→ 需 PoC 验证而非臆断。

### 3. 约束清单

| 约束 | 出处 | 影响 |
|---|---|---|
| core/infra 禁 Tauri | Cargo.toml 注释 | ts-rs / specta 是普通 derive crate,**不破约束**;但需过 workspace.dependencies 收敛 |
| knip 扫 src/** | knip.json | 生成文件放 `src/types/generated/` 会进 project 集,未被引用的导出报 dead code(需 ignore 或保证全被用) |
| vp 工具链 | package.json | `server:test = cargo test --workspace`——ts-rs 的 export-to-file 恰好搭 cargo test 顺风车,零新脚本 |
| 构建产物易 stale | 项目记忆 | 倾向生成物入库 + CI 一致性校验,而非纯构建时生成 |
| 注释风格 | dto.ts | 手写镜像的中文 JSDoc 很密;ts-rs 可带 doc comment 但风格会变 |

## 三、方案空间

### A. ts-rs(生成类型;stable)

`#[derive(TS)]` + `#[ts(export)]`,`cargo test` 时导出 `.ts` 到约定目录,入库。
- ✅ stable(非 RC)、不依赖 tauri、搭现有 cargo test 顺风车
- ✅ serde attr 兼容面覆盖本项目所有出线形态(待 PoC)
- ❌ 只生成类型,**不生成命令绑定**——invoke 裸字符串问题原样保留
- ❌ String→union 降级问题依旧(除非先 Rust enum 化)

### B. tauri-specta(类型 + 命令绑定一步到位;长期 RC)

类型 + 全部 23 条命令的 typed invoke 包装 + events。
- ✅ 收益最大:命令名、参数 key camelCase、返回类型全编译期校验
- ⚠️ 截至 2026-05 停在 `2.0.0-rc.25`,多年 RC;下游普遍 pin 精确版本
  (RC 间无 semver 兼容)——供应链风险需掂量
- ⚠️ specta derive 要进 core/infra 类型层

### C. 只校验不生成(drift check)

保持手写镜像,加机械校验:ts-rs export 到临时目录与 `dto.ts` diff
(或 serde round-trip JSON snapshot 比对)。最保守,守住「漂移能被 CI 抓住」,
不解决 invoke 裸字符串。

### 待决策问题(下一步线索)

1. 生成范围:只 DTO,还是连命令绑定?(决定 A vs B vs C)
2. Rust String → enum 前置重构做不做?(不做则 codegen 是降级)
3. 生成物入库 vs 构建时生成?(结合 build-artifacts-stale 前科)
4. ipc 信封 AgentRunMessage 的镜像收编进 dto.ts / 生成范围?
5. PoC:用 AgentEvent(内部标签枚举)跑 ts-rs 与 specta 各一轮,验证出线形态。

## 四、决策记录(2026-09-28)

| 待决问题 | 决策 |
|---|---|
| 1. 生成范围 | **B:tauri-specta**(类型 + 命令绑定一步到位) |
| 2. Rust String → enum 前置重构 | **做** |
| 3. 入库 vs 构建时生成 | **入库**,并在静态检查、构建时更新(重导出) |

## 五、决策后调研(2026-09-28 第二轮)

### 1. enum 化比预想近得多——core 枚举已存在,缺的是「出线」

`crates/core/agent/src/runner.rs:21,42` 已有 `AgentEnvMode { Default, Bare }`
与 `AgentPermissionMode { Default, AcceptEdits, BypassPermissions }`,
derive Serialize/Deserialize(camelCase)齐全。但:

- store 层 record 字段仍是 `pub env: String` / `permission_mode: String`
  (infra/store/model.rs:72,116),app 层写入靠 `as_str().to_owned()` 手动降级
- `as_str()` 与 serde 线格式是**双轨人肉保持一致**(口径注释自述「与 serde 线格式一致」)
- **status 连枚举都没有**:app 层 4 个裸常量(exec/agent.rs:42-48
  `STATUS_RUNNING..STOPPED`),store String,TS union 全靠人肉
- 纠偏:此前怀疑 `AgentEnvMode = 'default'|'bare'` 是 4c81c9e 的漂移残留——不成立,
  Rust 侧值域就是 default/bare,两边一致

→ 真实重构清单:**新增 `AgentRunStatus` 枚举** + store 三字段 String → 枚举 +
删 `as_str()` 双轨。serde unit variant + camelCase rename 的线格式与现状逐字一致
(`AcceptEdits` → `"acceptEdits"`),**线格式零变化**。风险:旧库若存过非法枚举值,
反序列化由「透传 String」变「报错」——严格度提升,需评估存量数据。

### 2. 流式全景:全 Channel、零 emit/listen——specta 覆盖面收敛

```
命令(22 条,main.rs generate_handler!)
├─ 普通查询/变更 ──▶ invoke<T>() 返回值           ← specta 直接覆盖
├─ agent_start ────▶ on_event: Channel<AgentRunMessage>   ┐
├─ watch_subscribe ─▶ on_event: Channel<FileWatchEvent>   ┘ 两处流式
└─ (无任何 app.emit / listen 事件)                ← specta events 部分用不上
```

tauri-specta v2 文档确认支持 `tauri::ipc::Channel` 作命令参数(上游有专门 issue
管理该支持面),`agent_start` 的核心通道可被 typed。前端迁移后
`agent-transport.ts` 手写的 `AgentRunMessage`/`AgentStartChainParams` 镜像自然收编
(原待决问题 4 随 B 方案消解);transport 内运行时校验(readChainParams 等)是
ai-sdk body 穿透所需,与静态类型互补,保留。

### 3. 供应链现状(2026-09):仍停在 rc.25

tauri-specta / specta 均为 `2.0.0-rc.25`(约 2026-06 发布),stable 2.0 未出;
下游以 `=` 精确 pin + specta/specta-typescript 三件套 lockstep。备选 taurpc
(0.8.2,底层同为 specta)支持 Channel,若 rc 风险否决可在提案期对比。

### 4. 入库 + 检查/构建时更新的挂点

- 生成物:`src/lib/bindings.ts`(或 `src/types/generated/`)入库
- 导出触发:tauri-specta `builder.export(Typescript, path)` 需 Rust 侧入口——
  挂 `#[test]`(搭 `server:test = cargo test --workspace` 顺风车)或专用 bin
- scripts 串接:`check` 与 `build` 前置重导出(npm `pre` hook 或显式串),
  保证本地检查/构建永远拿到新鲜生成物
- 一致性守卫:重导出后 `git diff --exit-code` 思路(类 fmt --check),
  防「入库物悄悄过期」——对应 build-artifacts-stale 前科
- knip:`project` 扫 `src/**`,生成目录需在 knip.json ignore(生成物非手写代码,
  未引用的命令包装不应报 dead code)

### 剩余待决(下一轮线索)

1. 迁移策略:22 命令 × ~27 调用文件,一次性切 vs 按命令分批
2. dto.ts 退役形态:直接删 vs 保留 re-export shim 过渡
3. 存量库校验:enum 化前扫一遍 redb 存量 status/env 值域,确认无野值
4. PoC(实现期):rc.25 对 `tag="kind"` + `rename_all_fields` 枚举、
   `Channel<AgentRunMessage>` 信封的实际出线形态

## 六、决策记录(2026-09-28 第三轮)

### 1. 剩余两问拍板

| 待决问题 | 决策 |
|---|---|
| 迁移策略(一次性 vs 按命令分批) | **一次性切换**:22 命令 × ~27 调用文件,单个 change 内全部裸 invoke → typed bindings,不分批 |
| dto.ts 退役形态 | **保留纯 re-export shim 过渡**:迁移后仅 `export type * from generated`;退役判据 = knip 报零引用 → 删除,不按时间过渡 |

### 2. 本轮查证结论

- **workflow 域无需 enum 化前置**:`Verdict`/`FileLogOp`
  (core/workflow/src/model/workflow.rs:51,110)、`Inventory`(model/inventory.rs:10)、
  `ChangeSource`(queries/list.rs:16)Rust 侧已是真枚举,生成路线无降级;降级面收敛于
  agent 域三字段(env/permission_mode/status),enum 化前置清单不扩
- **dto.ts 全量是 Rust 镜像**:250 行无任何前端独有类型 → shim 无「长住居民」,终局必删;
  过渡期唯一价值 = 一次性大切换的容错网(漏网旧 import 不炸编译/运行时,knip unused
  报告兜出漏网名单);次要用途:生成名与现名有出入时逐名 re-export 做名称映射,
  切换期 import 语句可不动
- **`source: String` 不收紧**(store model.rs:93、envelope.rs:26):两侧均为 `string`,
  不构成降级;收紧需再一轮线格式审视,留受控字符串注释,真要收紧另开 change

### 3. 落地编排(A/B 分段,「一次性」指前端切换)

```
change A(Rust 前置,纯内政)
  存量 redb 值域扫描(agent 三字段)→ 新增 AgentRunStatus
  → store 三字段 String→enum → 删 as_str() 双轨
  验收面:线格式逐字不变,cargo test --workspace 全绿;先行独立合入
change B(主体,一次性切换)
  PoC 最先(rc.25 × AgentEvent tag="kind" + rename_all_fields × Channel)
  → specta 接入 + 生成物入库 + git diff --exit-code 守卫
  → 22 命令 × 27 文件一次切完 → dto.ts shim 化
收尾(B 内任务或下一 change)
  knip 报 dto.ts 零引用 → 删 shim
```

A/B 不捆原因:A 有独立验收面(线格式零变化),先落地后 B 纯粹「加东西」,diff
不混行为变更,revert/bisect 粒度干净;B 内不拆——bindings 不入库前端无法切,
中间态无存在意义。

### 4. 一次性 vs 分批依据(留档)

- 双轨共存期 = 0:不存在「一半命令 typed、一半裸字符串」混态;分批则每批间两套
  心智/异常路径
- invoke→bindings 为机械替换,量不在风险里;真实风险是生成物与手写镜像的字段级
  差异(null vs optional 等,tsc 拦截),分批同样存在只是摊薄 N 次
- desktop 本地应用无灰度,分批「每步小」兑现不了价值
- revert 干净(单 change 整体回滚);review 属模式校验而非逐行推理

### 5. 待决状态

| 待决 | 状态 |
|---|---|
| 1. 迁移策略 | ✅ 本轮:一次性 |
| 2. dto.ts 退役 | ✅ 本轮:shim 过渡,knip 零引用退役 |
| 3. 存量库值域扫描 | 挂 change A 前置任务 |
| 4. PoC(rc.25 出线形态) | change B 实现期第一步 |

→ 待决清零,够格起 change;按编排先开 change A(enum 化前置)。

## 七、补充决策(2026-09-28,起 change 时)

**A+B 合为一个 change**:用户拍板不拆 A/B——enum 化前置、tauri-specta 接入、
生成物入库 + 一致性守卫、22 命令前端一次切换、dto.ts shim 化,全部纳入本
change(desktop-rust-ts-type-sync,workflow_type=requirement)。第六节「A/B
分段编排」的拆分建议作废;A 原独立验收面(线格式逐字不变、cargo test 全绿)
转为 change 内部 enum 化任务的验收点,仍应排在 specta 接入之前完成。

## 参考出处

- ts-rs changelog(rename_all_fields 支持):
  https://github.com/Aleph-Alpha/ts-rs/blob/main/CHANGELOG.md
- tauri-specta v2 仓库:https://github.com/specta-rs/tauri-specta
- tauri-specta 版本史(2.0.0-rc.25, 2026-05):https://crates.io/crates/tauri-specta
