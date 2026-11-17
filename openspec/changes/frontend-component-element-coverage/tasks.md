# 任务: frontend-component-element-coverage

> 覆盖 proposal「变更范围 - 实现文件」**4 行全量**（`models/06-frontend.toml`、`decisions/0001-model-granularity.md`、`package.json`、`tauri.conf.json` 的零内容改动确认）+ 设计 D5 的 **`api/INDEX.toml` 重生成**（AC-6 修正）+ `reports/archi/` 的**实现面留痕一份**（`model-sync-check.md`）+ 报告槽位卫生（增量报告改名 `incremental-<UTC>.json`）。**全量报告 `reports/archi/full-<UTC>.json` 与全量槽位读数不在实现面**（design D11：门禁 runner 恒增量且是唯一自动落盘点、`archi_drift` 命令零写盘）——列在末尾「移交实测清单」，不勾选、不产出，MUST NOT 用增量报告顶替。两份 spec delta **已在案且零改动**（存活 spec 由归档链 spec-sync 段合并），`packages/desktop/src/**` 与 `src-tauri/**` **不在实现面**。
>
> 依赖序：**阶段一**（`06` 的两元素 / 七边 / 表头）解锁阶段三（`INDEX.toml` 的条目面由元素面派生）与阶段五（静态面核对有输入）；**阶段二**（ADR 同步）与阶段一共享同一组数值，须在阶段一落地后逐字对表；**阶段四**（版本行）与前三阶段互不依赖；**阶段五**收官（静态面核对 + 报告槽位卫生）。
>
> 四条不可动摇的机械锚点：① `06` 落数据后 `grep -c '^\[element\.'` 恰 **30**、`grep -c '^\[\[relation\]\]'` 恰 **71**，其余五份模型逐字不动（`entry` 合计仍 3、`api` 仍 2 且均 `"none"`）；② `.dev-team/architecture/api/**` 的唯一允许变化是 `INDEX.toml` 的 **+2 条全空 `[[entry]]`**（由 `pnpm run api:export` 产出），`.schema.json` / `.i.rs` / `.i.ts` 三类**零 diff**；③ `packages/desktop/src/**` 与 `packages/desktop/src-tauri/**`（除 `tauri.conf.json` 的形态核对）零 diff——出现任何 diff 即误动，回退而不是改判据；④ `reports/archi/` 内只允许 `model-sync-check.md` 与 `incremental-<UTC>.json`（可省），**零**裸名 `<UTC>.json`；`full-<UTC>.json` 缺失是合法态（移交未完成），一旦出现则其 `scan.mode` MUST 为 `"full"` 且 `scannedFiles > 0`。
>
> 自查 cwd：**仓库根**（模型 / ADR / 生成物 / `git` 面）；`packages/desktop`（`package.json` / `tauri.conf.json` / `pnpm` 脚本）。静态门禁（`api:check` / `server:check` / `client:check`）由桌面端 static-check / test-execution 承接，**不在本相位执行**；**全量漂移实测**（`archi_graph` 规模与 `diagnostics`、`archi_file_tree` 两处 `owners`、`unmatchedFiles` / `observations` / `errors` / `warnings` / `status`，以及 `full-<UTC>.json` 的落盘）按 design D11 **整包移交**同一相位；本相位只核与扫描模式无关的静态面（计数同值 / `apiFindings` 分桶 / `INDEX` 条目 / `import` 证据行 / 报告槽位卫生），阶段五不持全量 checkbox。

## 阶段一：模型数据（`06` 两元素 / 七边 / 表头）

- [x] 修改 `.dev-team/architecture/models/06-frontend.toml` 表头第 1 行：`# 06 · 前端（28 元素 / 64 关系）` → `# 06 · 前端（30 元素 / 71 关系）`
- [x] 修改同文件表头第 2 行：把「共享组件文件 10」改为「共享组件文件 12」，同句「元素 = 页壳 1 + 视图 9 + 视图内子域 3 + 共享组件文件 12 + 原语层 1，另有 4 个容器。」其余逐字不动
- [x] 新增元素 `[element."App.Frontend.Components.Agent.SessionPanel"]`：插在 `[element."App.Frontend.Components.Agent.AgentSessionView"]` 段之后、`[element."App.Frontend.Components.Agent.ToolCards"]` 段之前（该组既有顺序为 FQN 字典序）；四个字段逐字为 `kind = "component"` / `gate = "off"` / `path = ["./packages/desktop/src/components/agent/session-panel.tsx"]` / `summary = "会话面板骨架（头区动作 / 状态行 / 历史清单 / 折叠窄条，两处会话面共用）"`；**不写** `entry` / `api` / `docs` / `metadata`
- [x] 新增元素 `[element."App.Frontend.Components.Standard.Tooltip"]`：插在 `[element."App.Frontend.Components.Standard.Select"]` 段之后、`[element."App.Frontend.Components.Ui"]` 段之前；四字段逐字为 `kind = "component"` / `gate = "off"` / `path = ["./packages/desktop/src/components/standard/tooltip.tsx"]` / `summary = "文本气泡（原语层 Tooltip 的业务封装）"`；同样**不写** `entry` / `api` / `docs` / `metadata`
- [x] 新增关系 ④：在 `[[relation]]` 段内 `from = "App.Frontend.Components.Agent"` 组的末尾（`→ …AgentSessionView` 之后）追加 `from = "App.Frontend.Components.Agent"` / `to = "App.Frontend.Components.Agent.SessionPanel"` / `desc = "barrel 重导出面板组件与其状态类型"`
- [x] 新增关系 ① ② ③：组成 `from = "App.Frontend.Components.Agent.SessionPanel"` 的新组，插在 `from = "App.Frontend.Components.Agent.AgentSessionView"` 组之后、`from = "App.Frontend.Components.Agent.ToolCards"` 组之前；组内按 `to` 字典序三条：`→ App.Backend.Shell`（`desc = "经生成的命令面类型取会话清单"`）、`→ App.Frontend.Components.Agent.AgentSessionView`（`desc = "面板正文复用整段会话视图"`）、`→ App.Frontend.Components.Ui`（`desc = "头区按钮与可拖拽分栏使用原语层"`）
- [x] 新增关系 ⑤：在 `from = "App.Frontend.Components.Standard.Select"` 组之后追加 `from = "App.Frontend.Components.Standard.Tooltip"` / `to = "App.Frontend.Components.Ui"` / `desc = "以原语层气泡组合文本提示"`
- [x] 新增关系 ⑥：在 `from = "App.Frontend.Views.Changes"` 组内插在 `→ App.Frontend.Components.Standard.Dailog` 与 `→ App.Frontend.Components.Ui` 之间：`to = "App.Frontend.Components.Standard.Tooltip"` / `desc = "列表工具栏的图标按钮以气泡给出提示"`
- [x] 新增关系 ⑦：在 `from = "App.Frontend.Views.Explores"` 组内插在 `→ App.Frontend.Components.Agent` 与 `→ App.Frontend.Components.Ui` 之间：`to = "App.Frontend.Components.Standard.Tooltip"` / `desc` 同 ⑥（既有先例：同用途边共用一句）
- [x] 自查 阶段一（计数与留白）：`grep -c '^\[element\.' .dev-team/architecture/models/*.toml` ⇒ `3 / 8 / 8 / 1 / 3 / 30`；`grep -c '^\[\[relation\]\]'` ⇒ `0 / 9 / 24 / 13 / 3 / 71`（合计 53 / 120）；`grep -c '^entry = '` ⇒ `04` 1、`05` 2、其余四份 0（合计仍 3）；`grep -c '^api = '` ⇒ `05` 恰 2 且均为 `"none"`
- [x] 自查 阶段一（结构与顺序）：`git diff` 内 `06` 只新增 2 个 5 行元素段与 7 个 3 行关系段 + 表头 2 行改数，其余 28 段元素与 64 条边逐字未动；两新元素无 `entry` / `api` / `docs` / `metadata` 行；七条新边的 `from` / `to` 与「数据模型 - 关系」表逐字相等；端点边（任一端为 `App.Frontend.Components.Agent`）合计 7 条、三个无 `path` 容器零端点、自环零条
- [x] 自查 阶段一（判据仍忠于代码）：七条边的证据行逐行复核 `session-panel.tsx:4–5,7,9`、`components/agent/index.tsx:5–6`、`standard/tooltip.tsx:1`、`views/changes/change-list-view.tsx:5`、`views/explores/explore-list-view.tsx:5`；`session-panel.tsx` 的 `@/hooks/use-agent-session` 与 `tooltip.tsx` 之外无遗漏 import 目标（新增边集合 == 两文件 import 目标集合按元素去重后的结果）

## 阶段二：裁定记录同步（`§1` / `§3` / `§6` / `§7` / `§8` / `§9`）

- [x] 修改 `.dev-team/architecture/decisions/0001-model-granularity.md` `§1`：`基线为 **51 元素 / 113 关系**` → `基线为 **53 元素 / 120 关系**`，同句 `前端 64 条（全在 \`06\`）` → `前端 71 条（全在 \`06\`）`；后端 49 条的四项拆分逐字不动
- [x] 修改 `§3`：`端点边恰 **6 条**：2 条出边（barrel 重导出 \`AgentInput\` / \`AgentSessionView\`）` → `端点边恰 **7 条**：3 条出边（barrel 重导出 \`AgentInput\` / \`AgentSessionView\` / \`SessionPanel\`）`；4 条入边的清单与两条反例逐字不动
- [x] 修改 `§6`：`基线 51 个元素 \`gate\` 全部显式写 \`off\`` → `基线 53 个元素 …`；其余（冷启动 ratchet 的理由、「本轮只负责让模型第一次存在」）逐字不动
- [x] 修改 `§7`（两处）：① `否则「指向 \`App.Backend.Shell\` 的 11 条命令面边」无法全部成立` → `12 条命令面边`；② 在「这是前端指向后端的唯一合法边来源」之后补一句声明式约定：「该映射是**声明式约定**：`src/types/**` 无元素归属，扫描解析出目标文件后按 `Owner::None` 丢弃该边，故命令面边在报告里落 `observations[].unfulfilledDeclaration`（`gate` 全 `off` ⇒ 永不进 `errors` / `warnings`）——MUST NOT 用「观测项为空」验收前端关系面」
- [x] 修改 `§8`：保留既有条目（`src/hooks` 无载体一条 + 漂移纪律 / 修改纪律两条）逐字不动，在「已知缺口」处**新增四条**（AC-7 修正：三条 → 四条）：⓪ `crates/tools/specta-zod-patched`——根 `Cargo.toml` 的 `[workspace] members` 明列该目录，命中 `§2` crate 档「每个成员 crate 一元素」却无元素（vendored 上游 exporter，`[package] name = "specta-zod"`）；标注「先于本变更存在」，补齐须新增 crate 元素并连带换段全部计数，或整目录删除（届时判据自然归零）——两条路都属独立变更；① `components/file/file-viewer.tsx`——`§2` 三层表无 `components/<其它目录>` 档，**补它须改本能力：新增 `components/<其它目录>` 粒度档**；② `views/architecture/agent/`——递归非测试源文件 3 个（`archi-chat-composer.tsx` / `archi-chat-panel.tsx` / `hooks/use-archi-chat.ts`）命中原判据但未建元素，补齐属独立变更；③ 11 条已建元素间未声明的跨元素 `import`（逐条 FQN：`Views.Architecture → Components.Agent` / `→ Standard.Dailog`、`Views.Changes → Components.Agent` / `→ Standard.Drawer` / `→ Changes.Renderers`、`Views.Explores → Standard.Dailog`、`AgentInput → Standard.Select` / `→ Ui`、`ToolCards → Ui`、`barrel → AgentMessages`、`Changes.Flow → Changes`）——最后一条是「子 → 父」形态（声明即 `dependsOnAncestor` 停摆），补齐须先改代码分层；同条括号内附反向缺口一句：`Views.Changes.Agent → Components.Ui` 已声明而扫描面无证据
- [x] 修改 `§9` 复算块：`06(28)` → `06(30)`、`06(64)` → `06(71)`、合计 `51` → `53`、`113` → `120`、`（容器 10 + 叶 41）` → `（容器 10 + 叶 43）`、`前端 64（import 证据）` → `前端 71`、`前端 64 = 58（…）+ 6（… barrel 端点边）` → `前端 71 = 64（…）+ 7（… barrel 端点边）`；末段的 `58` → `64`、`6 条 barrel 端点边` → `7 条`（该段的判据句「要回到 64 只能撤销第 3 节的 barrel 例外或改第 7 节的映射规则」随之换值）
- [x] 自查 阶段二（三处同值）：`grep -n '53 元素 / 120 关系\|06(30)\|06(71)\|71 = 64' .dev-team/architecture/decisions/0001-model-granularity.md` 与 `06-frontend.toml` 表头、以及 `openspec/changes/frontend-component-element-coverage/specs/repo-archi-model/spec.md` 计数表三处逐字同值；ADR 内 `51` / `113` / `6 条`（barrel）/ `11 条命令面边` 的**现值命中为零**（只允许作为「已退场 / 旧式」叙述出现）
- [x] 自查 阶段二（判据未动）：`git diff` 内 `§2`（三档表）、`§4`（dev 边排除）、`§5`（反向边）逐字未变；新增文本内 MUST NOT 出现任何新的 MUST / SHALL 条款（只有数字换值与既有纪律的转述）；`grep -n '§2\|§4\|§5'` 定位的段落零改动
- [x] 自查 阶段二（AC-7 反向判据）：`grep -rn 'file-viewer\|architecture/agent\|specta-zod' .dev-team/architecture/models/` 零命中；`06` 元素数仍 30（`grep -c '^\[element\.'`）、`05` 元素数仍 3（`specta-zod-patched` 未入模型）；ADR `§8` 的**四条**登记各自写明「为什么现在不补 / 补齐须做什么」，且 MUST NOT 被读成本变更的差集

## 阶段三：生成物重生成（`INDEX.toml` 的 +2 条全空槽位）

- [x] 在 `packages/desktop` 下运行 `pnpm run api:export`（`cargo run -q -p archi-api-gen -- generate`），**只**允许 `.dev-team/architecture/api/INDEX.toml` 变化；`*.schema.json` / `*.i.rs` / `*.i.ts` 三类**零 diff**（两个新元素无契约 ⇒ `present = false` ⇒ 不产 `.i.*`、不产 schema）
- [x] 核对 `INDEX.toml` 的差分形态：`[[entry]]` 由 42 → **44** 条；新增 `App.Frontend.Components.Agent.SessionPanel`（位于 `…AgentSessionView` 与 `…Agent.ToolCards` 之间）与 `App.Frontend.Components.Standard.Tooltip`（位于 `…Standard.Select` 与 `…Components.Ui` 之间），两条六字段逐字为 `status = "absent"` / `types = 0` / `ops = 0` / `degraded = 0` / `schema = ""` / `generated = ""`；表头四行（含 `映射规则摘要 sha256:…`）逐字不变
- [x] 自查 阶段三（零手改与幂等）：`git diff .dev-team/architecture/api/INDEX.toml` 的内容全部来自生成器（无手工编辑痕迹）；再跑一次 `pnpm run api:export` 输出「0 份写入」形态、`git diff` 零变化（幂等）
- [x] 自查 阶段三（判据面同源）：`.dev-team/architecture/api/` 下无新增 / 删除文件（`git status --porcelain .dev-team/architecture/api` 只有 `M INDEX.toml` 一条）；`INDEX.toml` 条目数 == `path` 非空元素数 == **44**（与 AC-3 的分母同值，两处互为交叉校验）

## 阶段四：版本纪律

- [x] 修改 `packages/desktop/package.json`：`"version"` 由 `0.6.47` 改为 `0.6.48`（只 +1 一次、不追溯）；其余键与 `scripts` 段零改动
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json`：仍是 `"version": "../package.json"` 引用形态、未被写成字面版本号（**零内容改动**，`git diff` 为空）
- [x] 核对 `packages/desktop/src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变（MUST NOT 随 `package.json`）
- [x] 自查 阶段四：`grep -n '"version"' packages/desktop/package.json` ⇒ `0.6.48`；`git diff --stat packages/desktop/src-tauri/Cargo.toml packages/desktop/src-tauri/tauri.conf.json` **零输出**

## 阶段五：核对与移交

- [x] 核对变更清单覆盖面（AC-6 修正后白名单）：`git status --porcelain` 只含 `.dev-team/architecture/models/06-frontend.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md`、`.dev-team/architecture/api/INDEX.toml`、`packages/desktop/package.json`、本 change 目录（`design.md` / `tasks.md` / `reports/archi/**`）；**零** `packages/desktop/src/**` 与 `packages/desktop/src-tauri/**` 源码改动、**零** 测试文件改动、**零** `openspec/specs/**` 改动、**零** 两份 delta 改动
- [x] 核对「不要修改」面零 diff：`git diff --stat .dev-team/architecture/models/{01-app,02-backend-core,03-backend-infra,04-backend-shell,05-backend-tools}.toml` 为空；`git diff --stat packages/desktop/src packages/desktop/src-tauri/src packages/desktop/src-tauri/crates packages/desktop/src/types/generated` 为空；`git diff --stat openspec/specs/` 为空；`AGENT.md` / `CLAUDE.md` / `.dev-team/architecture/config.json` / `.dev-team/config.json` 为空
- [x] 报告槽位卫生（design D11）：把已落的增量报告 `reports/archi/20261116T145803082118Z.json`（实测 `mode = "incremental"` / `scan.scannedFiles 0` / `unmatchedFiles []` / `observations 120` / `status = noChanges`）改名为 `reports/archi/incremental-20261116T145803082118Z.json`，并在 `model-sync-check.md` §0 声明「本份为增量旁证、非基线」；改后 `ls reports/archi` 只允许 `model-sync-check.md` 与 `incremental-*.json`（**零**裸名 `<UTC>.json`）。MUST NOT 把它（或任何增量报告）改名成 `full-*.json`、MUST NOT 让任何 AC 以它为基线——增量模式 `scannedFiles = 0` ⇒ `unmatchedFiles` 恒空、`status = noChanges`、`observations` 为全部声明未兑现
- [x] 修订留痕 `openspec/changes/frontend-component-element-coverage/reports/archi/model-sync-check.md`（按 design D11 重排，不再声称实现面已取全量）：§0 模式声明改为「全量读数不在实现面——门禁 runner 恒增量且是唯一自动落盘点、`archi_drift` 命令零写盘 ⇒ 全量槽位由移交相位取，本文件只留静态面实测与移交指向」；① 两文件归属的**静态判据**（`path` 唯一最长前缀、与既有文件型根零冲突、祖先链齐全）＋ 全量 `unmatchedFiles` / `observations` / `owners` 的移交指向与失败特征（+1 条即成立、+3 条即归属失败）；② `apiFindings` 按 `kind` 分桶的元素 FQN 清单（`apiEntryMissing` 12 逐条点名、`apiMissing` 29 逐条点名且含 `SessionPanel` / `Tooltip`、其余七条各 0、总 41、分母 44），并标注「与 `mode` 无关 ⇒ 实现面可读」；③ 七条边逐条证据（文件:行，第 ③ 条标注「声明式命令面边」）＋ 模型内指向 `App.Backend.Shell` 的前端边 12 条 ＋ `INDEX.toml` 的 +2 条读数 ＋ `archi_module` 邻接读数
- [x] 静态面取值核对（实现面可核部分，全部与扫描模式无关）：`06` 表头 / ADR `§9` / `…/specs/repo-archi-model/spec.md` 计数表三处同值（53 / 120、`06` 段 30 / 71、`71 = 64 + 7`、源 24）；`apiFindings` **41** = `apiEntryMissing` 12 + `apiMissing` 29、分母 44；`INDEX.toml` 44 条且 `api:export` 二跑「0 份写入」（幂等 ⇒ 等价于 `artifacts` 三列空）；`grep -c 'to = "App.Backend.Shell"'` 前端边 **12**；`archi_module("App")` 结构化诊断为空；`packages/desktop/src/types/generated` 零 diff
- [x] 登记「邻接现值未同步」到 `reports/archi/model-sync-check.md` 末尾（AC-8 / AC-9 的边界，不改 spec）：`openspec/specs/desktop-archi-api-check/spec.md:79` 的「本仓 42 个」与 `openspec/specs/desktop-archi-api/spec.md:313` 的「42 个 / `Resolved` 30 / `Absent` 12」在本变更落地后应为 **44 / 32 / 12**，本变更按裁决**不改**（前者所在 requirement 不属两份 delta 的 8 个 MODIFIED；后者所在能力被 AC-10 钉为零 diff），承接方见 design「待决问题」1

**移交实测清单（不勾为任务；承接方＝桌面端 static-check / test-execution 或人，见 design D11）**

> 实现面不产出、不勾选：缺项是「移交未完成」，不是「实现未完成」。四项共同的红线——MUST NOT 用增量报告、MUST NOT 手写伪 JSON、MUST NOT 单改模型数字去凑预估。

- **T1 全量报告落盘**：取 `archi_drift(root, "full")` 出参**原样**写 `openspec/changes/frontend-component-element-coverage/reports/archi/full-<UTC 时间戳>.json`（不裁剪、不改键）；双条件——文件名前缀 `full-` ＋ 文件内 `scan.mode = "full"` 且 `scannedFiles > 0`、`changedFiles = 0`；缺任一条即不合格，增量报告 MUST NOT 占此槽位
- **T2 图正常态（AC-1）**：`archi_graph(root)` = **53 元素 / 120 关系**、`diagnostics` 空、`warnings` 空；报告 `status = clean`、门禁 `passed = true`（增量报告恒 `noChanges`，不可替代）；反向判据：出现 `unknownField` / `unknownReference` 即停手查宿主与文件形态，MUST NOT 改模型去迁就
- **T3 归属（AC-2）**：`archi_file_tree` 中 `packages/desktop/src/components/agent/session-panel.tsx` 与 `…/standard/tooltip.tsx` 各为 `mark = "elementRoot"` 且 `owners` 恰 1 个 FQN（`…Components.Agent.SessionPanel` / `…Components.Standard.Tooltip`）；反向 `mark = "ambiguous"` 或 `owners.len() ≥ 2` 即不合格（同时会让 T2 的 `warnings` 非空）
- **T4 扫描面（AC-4 / AC-5）**：全量 `unmatchedFiles` 与加元素前的全量基线**集合相等**（不增不减；余项仍只余 `components/file/file-viewer.tsx` 与 `components/standard/hooks/use-open.ts`）；全量 `observations` 相对基线**恰 +1 条**且新增项逐字为 `App.Frontend.Components.Agent.SessionPanel → App.Backend.Shell`（出现 3 条 ⇒ ① ② 未兑现 ⇒ 归属失败）；`errors` / `warnings` 前后皆空、`scan.scannedFiles` 前后同值
- **不符时的处置**：MUST 先查证据面（是否有别的元素 / 边进出扫描面、`entry` 是否仍 3 处、`INDEX` 是否已重生成），MUST NOT 单改模型数字去凑预估（proposal R-1）
- **若想以「回退 `INDEX.toml`」换 AC-6 的字面白名单**：先读 design D5 / 待决问题 2——那会让 `api:check` 与仓库 `check` 链同时红，正确处置是改 AC 而非改数据
