# 任务: retire-dev-team-plugin

> 覆盖 proposal「变更范围」：**删除 361 tracked**（`plugins/**` 237 + `dist/**` 124）+ 两 marketplace 文件 + **50 份插件期语料目录**；**新增 1**（`crates/core/config/config.schema.json`）；**实现面修改 13**（`.gitignore` / 三份根文档 / `explore-stance.ts` / `core/config` 两文件 / `core/checks` 三文件 / 两处主 spec Purpose / `packages/desktop/package.json`）+ **生成物随动 1**（`src/types/generated/bindings.ts`，必须入索引）+ **配置指针 1 文件 2 处值**（`openspec/config.json` 的 `$schema` 与 `write_protection.files[0].glob`，已落盘）。13 份主 spec 的正文改动由已落盘的 delta 承载（本相位零改）。
>
> **配置指针通道（v3 落地）**：`openspec/config.json` 位于写保护内置锁定面（`rules::builtin_rules` 派生 `**/openspec/config.json`，恒在首位、fail-closed、无 override），但其两处指针**已按值级替换落盘**（阶段二 2B，改动恰 2 行、CRLF 保持、`JSON.parse` 通过、旧路径命中归零），`protect/**` 零代码改动。复核命令见阶段七自查 B1 / C，可随时复跑。
>
> **依赖序**：**阶段一**（schema 镜像落位）必须先于**阶段三**（删除 `plugins/`）——逐字副本的来源即将消失，删后无法取回；**阶段二 / 三 / 四 / 五 / 六 / 七**全部随同一变更面收口（D2），`openspec/config.json` 的两行指针改动同在该变更面内（阶段七自查 B1 / C）。
>
> **单提交收口（D2）**：本变更全程不做中间 push —— 任一中间提交都是坏状态（marketplace `source` 指 404、`$schema` 指 404、主 spec 描述不存在代码、`.gitignore` 死条目先于资产消失）。tasks 的阶段划分只描述本地作业顺序与自查点。`openspec/config.json` 的两行指针与其余改动**同属该变更面**。
>
> **本相位不写测试用例**：`crates/core/config/src/lib_test.rs` 的三条一致性断言为测试用例，归 test-gen 相位（设计已定契约：`include_str!` 取镜像 + `super::{KNOWN_KEYS, parse_framework}` + 测试内 `wire_name` 穷尽 match，零新增导出）；本相位只落 schema 镜像与注释面。`corpus_golden_test.rs` / 两份 README 的改动是**既有用例的措辞随动**（断言值、语料、行为零改），不新增用例。
>
> **静态脚本由桌面端执行**：`pnpm -C packages/desktop run check` / `run test` / `cargo test --workspace` 由 test-execution 相位跑，本相位不重复执行，只做下文列出的机械自查（`git grep` / `diff` / `ls` / `wc` / `git check-ignore`）。
>
> **不改面（D14，逐条硬约束）**：`openspec/config.json`（**仅两处指针值例外**，其余字段与字节零改；MUST NOT 为改指针动内置锁定语义）、`openspec/changes/archive/**`、`openspec/explores/**`、`openspec/todo.md`、`crates/core/checks/tests/{fixtures/**,golden/*.json}` 的样本字面量、`crates/infra/agent/src/sdk/protect/{rules,reason}_test.rs` 样本、`packages/desktop/src-tauri/**` 的产品逻辑、`core/orchestration` 的「插件同源 / 同形」注释、`stryker.config.json` / `tsconfig.json` / `vite.config.ts`（不进 `write_protection`）、`.claude/settings.local.json`。
>
> 自查命令的 cwd 除特别标注外均为仓库根。

## 阶段一：schema 镜像落位（必须先于删除）

- [x] 预检迁移源：确认 `plugins/dev-team/bin/dev-team-config.schema.json` 在案且 173 行，记录顶层键集与 `framework.enum` 现值作为「迁入前基线」
- [x] 逐字复制到 crate 内：`cp plugins/dev-team/bin/dev-team-config.schema.json packages/desktop/src-tauri/crates/core/config/config.schema.json`（**MUST NOT** 重新生成、重排、改缩进、改键序）
- [x] 自查 A（逐字一致）：`diff plugins/dev-team/bin/dev-team-config.schema.json packages/desktop/src-tauri/crates/core/config/config.schema.json` 零输出（删除后等价判据：镜像在案 173 行，`properties` 7 键、`framework.enum` 8 值序同 `TestFramework` 声明序、根 `additionalProperties: {}`）
- [x] 自查 B（隔离禁令）：`grep -c "config\.json" packages/desktop/src-tauri/crates/core/config/config.schema.json` 为 0（镜像不含 `config.json` 子串，不破 `desktop-crate-layout` 的两条字面量禁令）
- [x] 自查 C（断言锚点在位）：镜像顶层 `properties` 恰 7 键、`tests.items.properties.framework.enum` 恰 8 值，与 `crates/core/config/src/lib.rs` 的 `KNOWN_KEYS[7]`（`:27-35`）与 `TestFramework` 八变体逐字对齐
- [x] 核对断言读取路径可用：`src/lib_test.rs` 内 `include_str!("../config.schema.json")` 相对 `src/` 解析到 crate 根（本相位只核对路径正确，不写断言；用例归 test-gen 相位）
- [x] 登记 git 记录形态：内容全同被 git 记为 `R plugins/dev-team/bin/dev-team-config.schema.json -> packages/desktop/src-tauri/crates/core/config/config.schema.json`，AC-4 判「内容逐字一致」，无需为消 R 记录而改字节（D3 / 事实校准 7）

## 阶段二：`.gitignore` 死条目 + 配置指针（可执行面 / 值级替换面）

### 2A 可执行面（agent 可写）

- [x] `.gitignore`：删 `# 构建相关` 标题与其下 `plugins/*/.pack-staging/`、`*/dev-team/bin/*.cjs.map` 三行整块；**保留** `.*/` 与 `!.claude-plugin` / `!.cursor-plugin` / `!.github` / `!.gitignore` / `!.claude/` 等否定项与其余条目（`git diff .gitignore` 应只有 3 行删除）
- [x] 自查：`git check-ignore -v packages/desktop/knip.json` 无命中（exit=1，新 glob 走保护侧配置而非 `.gitignore`）；全仓无 `*.cjs.map` / `*.pack-staging*` 路径（`git ls-files | grep -c 'cjs\.map\|pack-staging'` = 0）

### 2B 配置指针（`openspec/config.json`，值级替换，已落盘）

- [x] **指针 1**：`openspec/config.json:2` 的 `"$schema"` 值整体替换为
      `https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json`
      （前缀形态沿用现状，只换尾路径，D6；与 `README.md:94` 项目配置段**逐字同值**）
- [x] **指针 2**：`openspec/config.json:23` 的 `write_protection.files[0].glob` 替换为 `packages/desktop/knip.json`
      （root 相对，与旧值同形态；`reason` 与该对象其余字段**未动**，D7）。语义已代码核对：`rules::user_rules` / `normalize_user_glob` 对相对形态加 `**/` → 实际模式 `**/packages/desktop/knip.json`，`**/` 零段可命中，仓库根文件必命中；内置 `**/openspec/config.json` fail-closed 恒在兜底
- [x] 改法自查（值级替换，非整文件重写）：改动面**恰 2 行**，其余字段与行尾（CRLF）字节零改 —— `git diff --numstat -- openspec/config.json` = `2	2`；`git diff -- openspec/config.json | cat -A` 显示两行均以 `^M$` 结尾
- [x] 落盘自查（AC-11 第二段，现行通过）：`git grep -c "plugins/dev-team" -- openspec/config.json` **零输出**（exit=1）；`node -e 'JSON.parse(…)` 通过；两值与下表「改为（现行实况）」列逐字相等
- [x] 保护面自查（`repo-surface-scope` 明文禁令）：`git status --porcelain -- packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect` 零输出 —— 未给内置规则加 override、未改任何保护语义
- [x] 基线自查（AC-11 第一段，对改写前副本仍有效）：改写前 `git grep -c "plugins/dev-team" -- openspec/config.json` = **2**，两行逐值与下表「现值」列字符串相等
- [x] 目标可达自查（AC-4）：镜像 `packages/desktop/src-tauri/crates/core/config/config.schema.json` 在案 173 行；目标 URL 尾路径 `packages/desktop/src-tauri/crates/core/config/config.schema.json` 与仓库内现存路径逐字同形；`packages/desktop/knip.json` 在案（346B）且 `git check-ignore -v packages/desktop/knip.json` 无命中（exit=1）

**值级替换（本相位已执行；下面为可复跑脚本与改法口径，供任一副本重放）**

1. `openspec/config.json` 第 2 行 `"$schema"` 的值整体替换为
   `https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json`
   （与 `README.md` 项目配置段**逐字同值**）；
2. 第 23 行 `write_protection.files[0].glob` 的值替换为 `packages/desktop/knip.json`（`reason` 及该对象其余字段不动）。

改法 SHALL 为**值级替换**：该文件是 **CRLF 行尾**，纯 LF 的 `git apply` 补丁会判不匹配。改动面**恰 2 行**，其余字段与行尾字节零改（可先对副本模拟验证：恰 2 行变动、CRLF 保持、`JSON.parse` 通过、改后 `plugins/dev-team` 命中数 0）：

```bash
node -e '
const fs=require("fs");const f="openspec/config.json";let s=fs.readFileSync(f,"utf8");
s=s.replace("https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/plugins/dev-team/bin/dev-team-config.schema.json",
            "https://raw.githubusercontent.com/fisher-89/agent-plugins/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json");
s=s.replace("\"glob\": \"plugins/dev-team/bin/knip.json\"","\"glob\": \"packages/desktop/knip.json\"");
fs.writeFileSync(f,s,"utf8");'
```

逐字对照（便于手工编辑）：

| 位置 | 现值（改写前） | 改为（**现行实况**） |
|------|------|------|
| `:2` `"$schema"` | `…/refs/heads/master/plugins/dev-team/bin/dev-team-config.schema.json` | `…/refs/heads/master/packages/desktop/src-tauri/crates/core/config/config.schema.json` |
| `:23` `write_protection.files[0].glob` | `plugins/dev-team/bin/knip.json` | `packages/desktop/knip.json` |

- **生效时机**：两值随同一变更面提交；新 glob 一经落地即为用户规则 `**/packages/desktop/knip.json`，与实现面写入不再相撞（写入已完成、随同一次提交落盘）。
- **为何用值级替换**：该文件是写保护内置锁定面的对象，改动面越大风险越高；值级替换把 diff 收在恰 2 行，`protect/**` 零代码改动、未给内置规则加 override（`repo-surface-scope` 明文禁止项）。
- **复核时机**：归档前与发版前各复跑一次阶段七自查 C（`grep` 零输出、`git diff` 恰 2 行、两值等于上表）。

## 阶段三：四类资产删除 + 50 份语料（同批，单条批量）

- [x] 删除前反证一：`ls -d openspec/specs/*/ | grep -vc 'openspec/specs/desktop-'` = 51
- [x] 删除前反证二：`ls -d openspec/specs/*/ | grep -v 'openspec/specs/desktop-' | grep -vc 'workspace-layout-resolution'` = 50（差一则必须停下核对，防误删活 spec）
- [x] 删除前反证三：`ls -d openspec/specs/desktop-*/ | wc -l` = 34（保留面）；`git ls-files plugins | wc -l` = 237、`git ls-files dist | wc -l` = 124
- [x] 删除插件源码与三类产物：`git rm -r --quiet plugins dist/claude-plugins dist/cursor-plugins dist/cursor-home-image`
- [x] 删除对外市场：`git rm --quiet .claude-plugin/marketplace.json .cursor-plugin/marketplace.json`
- [x] 删除 50 份插件期语料（**单条批量**：先落目录清单文件，再从清单执行；清单 = 阶段三前两条反证的口径，MUST NOT 从 `ls` 管道直接喂 `rm`）：
      `architecture-agent` / `architecture-commit-gate` / `architecture-decisions` / `architecture-model` / `architecture-validation` / `c4-cli-integration` / `c4-dsl-syntax` / `change-create` / `change-files` / `change-list` / `cli-unit-test-execute` / `config-get` / `config-schema` / `cursor-home-image` / `dual-platform-plugin-build` / `embedded-cli` / `eval-check-cli` / `eval-json-protection` / `file-type-isolation` / `glob-matching` / `hook-output-import` / `include-fragments` / `intent-router` / `json-design-schemas` / `marketplace-config` / `mcp-project-root` / `mcp-tool-namespace` / `module-boundary-contracts` / `multi-plugin-layout` / `mutation-testing` / `pge-workflow-engine` / `phase-agents` / `phase-lifecycle` / `phase-skills` / `pipeline-backtrack` / `protect-files-hook` / `spec-list` / `static-check-hook` / `test-detect-frameworks` / `test-exclude-filter` / `test-execution-diagnostics` / `test-get-framework-config` / `test-only-workflow` / `test-path-resolver` / `unit-test-executor` / `update-architecture-skill` / `user-prompt-hook` / `workflow-file-inventory` / `workflow-orchestration` / `write-protection-config`
      → `git rm -r --quiet $(cat /tmp/retired-capabilities.txt)`（50 项，逐行 `openspec/specs/<name>` 形态）
- [x] 物理兜底清空未 tracked 残留：`rm -rf plugins dist .claude-plugin .cursor-plugin`（本工作树实测无残留，此步为「`.pack-staging` / `node_modules` / `reports` 若在则同步清空」的兜底）
- [x] 自查 A（AC-1 目录面）：`ls plugins dist .claude-plugin .cursor-plugin` 四条均 `No such file or directory`（**空目录也不保留**）
- [x] 自查 B（AC-2 语料面）：`ls openspec/specs | wc -l` = 35；`find openspec/specs -type d -empty` 与 `find openspec/specs -name spec.md -size 0` 均零输出
- [x] 自查 C（保留面）：`ls -d openspec/specs/desktop-*/ | wc -l` = 34 且 `workspace-layout-resolution/spec.md` 在案

## 阶段四：desktop 侧引用清面（零行为改）

- [x] `packages/desktop/src/lib/explore-stance.ts`：改写文件头注释块为单源自持（删「与插件 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链 / 改动需双侧同步」全部表述，改述「desktop 包自持，两形与笔记约定为本文件单源」）；`modeAndStance` / `commonNoteRules` 等**文本与行为零改**（`git diff` 只应命中注释块）
- [x] `packages/desktop/src-tauri/crates/core/config/src/lib.rs`：12 处 doc 注释参照物措辞直述化 —— `:8`（`literal('spec-driven')` 对齐）、`:11`（`prefault('.')` 对齐）、`:14/17/20/23`（`CLI defaults.ts 对齐`）、`:37`（`CLI zod enum 同序同串 / 显式 rename 对齐 CLI 串`）、`:72/93/189`（三处 `Default` doc）、`:344`（`zod number().min(0).max(100).optional().prefault(N) 移植`）、`:378`（`CLI zod 枚举串 → 框架枚举`）；**代码、常量值、类型、签名零改**
- [x] 生成物随动（D18）：`TestFramework` 的 doc 注释会流入生成物，故同一批次跑 `pnpm -C packages/desktop run bindings:export` 同步 `src/types/generated/bindings.ts`（1 行注释对位，复跑幂等零 diff）；**生成物须 `git add` 入索引**——`bindings:check` 的 `git diff --exit-code` 比的是工作区与索引，未入索引即判漂移
- [x] `packages/desktop/src-tauri/crates/core/checks/tests/corpus_golden_test.rs`：`:20-22` 头注释、`:291` doc、`:311` 断言文案改「已固化 `core/checks` 实现 + 入仓 golden 金样」口径；**断言值 / 语料清单 / 行为零改**
- [x] `packages/desktop/src-tauri/crates/core/checks/tests/fixtures/README.md`：`:3-4` 引用块去「以 CLI zod 权威形态录制」溯源措辞，改「golden 期望基准为已固化 `core/checks` 实现 + 入仓金样；重写开关与两步工作流见 `tests/golden/README.md`」；语料清单表与样本目录**零改**
- [x] `packages/desktop/src-tauri/crates/core/checks/tests/golden/README.md`（盘查补正 #1）：`:4`「TS zod schema 漂移由此表现为可读 diff」→「实现漂移由此表现为可读 diff」、`:26`「golden 期望值基准为 CLI zod 权威产出形态」→「golden 期望值基准为 `core/checks` 实现落定的产出形态」；重写开关、文件集与语料一致性约定**零改**
- [x] 自查 A：`git grep -nE "zod|defaults\.ts" -- packages/desktop/src-tauri/crates/core/config packages/desktop/src-tauri/crates/core/checks` —— `core/config` 侧零命中；`core/checks` 的 `tests/` 面本轮清面后零命中（`tests/golden/*.json` 与 `tests/fixtures/corpus/**` 是已豁免的数据样本）；残留仅 `core/checks/src/{aggregate,model}.rs` 与 `src/model_test.rs` 的注释（不在设计「变更清单」，按 D14 不改面，零行为影响）
- [x] 自查 B：`git grep -n "CLI 权威\|CLI zod" -- packages/desktop/src-tauri/crates/core` —— `core/config` 与 `core/checks/tests/**` 均零命中；残留同自查 A（D14 不改面）
- [x] 自查 C（零行为改）：`git diff --numstat` 中 `crates/core/config/src/lib.rs` 的增量剔除 `///` 行后零变动（零代码 / 零常量）；`corpus_golden_test.rs` 三处仅头注释 / doc 与断言 message 字符串（`assert_eq!` 期望值零改）、两份 README 仅文本行

## 阶段五：两处主 spec Purpose 直改 + delta 核对

- [x] `openspec/specs/desktop-phase-prompts/spec.md` 的 `## Purpose`：`14 个全静态 md（6 executor + 8 evaluator）` → `16 个全静态 md（7 executor + 9 evaluator，含 code-analyze 两份）`；「迁移是对插件 `plugins/dev-team/agents/*.md` 的改写而非搬运」→ 直述自持表述；requirement 段零改
- [x] `openspec/specs/desktop-change-create/spec.md` 的 `## Purpose`：删「插件面零改动」分句（保留「MUST NOT 产出 workflow.json（双向墙），禁止 CLI 子进程」）；requirement 段零改
- [x] 核对 14 份 delta 在案：`ls openspec/changes/retire-dev-team-plugin/specs | wc -l` = 14（13 个 desktop capability + `repo-surface-scope`）
- [x] 核对 MODIFIED header 与主 spec 逐字一致：对每份 delta 取 `^### Requirement:` 与 `openspec/specs/<capability>/spec.md` 的同名 header 对拍（14 份中 13 份 desktop delta 全覆盖；`repo-surface-scope` 为纯 ADDED，主 spec 目录归档时才出现）
- [x] 核对 RENAMED 形态：`desktop-phase-prompts` 的 `FROM: ### Requirement: 14 个静态角色知识文件清单` / `TO: … 16 个 …` 与主 spec 现 header 逐字一致
- [x] 核对 `desktop-write-protection` delta 的三条 MODIFIED 保住 D15 三个锚点：六条规则语义（内置面 / 用户面 / 序 / 方言 / 时机 / 根缺席）逐条直述、档 B 五族判据表与「已知粗粒度代价」列未丢、inline 的「MUST NOT 采用 `^(python\|node)\s` 整体豁免」否定式护栏与「已知漏面」四条在位；并确认 delta 已把「改指针走值级替换并复核、MUST NOT 加 override」写进内置锁定 bullet 与 Module Contract；同时确认 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` **零代码改动**
- [x] 核对 `repo-surface-scope` delta 承载两条 v3 条款：指针改写限值级替换（保护面零改动）、双段判据（旧值副本命中数基线 / 落盘后零命中 + 可达）
- [x] 自查：`git grep -n "plugins/dev-team" -- openspec/specs` 的命中**必须恰为** delta 覆盖的 16 行 8 位点（`desktop-change-archive:2` / `desktop-change-create:4` / `desktop-change-orchestration:2` / `desktop-change-worktree:2` / `desktop-explore-page:2` / `desktop-release-pipeline:2` / `desktop-test-execution:1` / `desktop-workspace-config:1`）；两处 Purpose 已零命中。出现清单外命中，说明主 spec 存在 proposal 未登记的字面量位点 → 回到设计登记，**MUST NOT** 就地改写（D12 分层判据）

## 阶段六：仓库根文档重写

- [x] `README.md` 整篇重写为桌面仓说明：**安装**（GitHub Releases：Windows `*_x64-setup.exe` / macOS `*_aarch64.dmg` + mac 首次右键「打开」放行，细节指向 `packages/desktop/RELEASE.md`）、**开发**（前置 Node / pnpm / Rust；`pnpm -C packages/desktop install`；`run dev` / `run client:dev`；常用门禁）、**构建**（`run build`，须设 `TAURI_SIGNING_PRIVATE_KEY` 与空 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`）、**发版**（bump `packages/desktop/package.json` patch → push master → `desktop-release.yml` detect/build/finalize → Releases；不 bump 不触发；绝不 `git push --tags`）
- [x] 同文件补**仓库结构**（唯一包 `packages/desktop`：`src`（React 前端）/ `src-tauri/crates/{core,infra,desktop-app}` / 仓库根 `openspec/{specs,changes,explores}`）与**项目配置**（`openspec/config.json` 的 `$schema` 现值 + `tests` / `static_analysis` / `write_protection` 三字段小表）——与阶段二 2B 的指针「改为」列逐字同值（D9 / D16）
- [x] 同文件补**历史一句**（「插件层已退役，历史见 `openspec/changes/archive/**`」），该句 MUST NOT 含 `plugins/dev-team` 字面量、安装命令或产物名
- [x] `AGENT.md` 整篇重写：去掉 `DEPRECATED` 前言与「Slim Plugin」主叙事；四段 = Project Overview / Project Rules / Architecture / Coding Guidelines（现有五条原样保留，含 no-test-only-exports）
- [x] `CLAUDE.md` 整篇重写：正文与 `AGENT.md` 一致，仅标题与面向对象句不同
- [x] 自查 A（AC-6）：`git grep -n "plugins/dev-team" -- README.md AGENT.md CLAUDE.md` 零命中；三文件均含 dev / build / 发版入口；无插件市场安装命令（`claude plugin` / `/add-plugin` / `install.mjs` 零命中）
- [x] 自查 B：`diff <(tail -n +3 AGENT.md) <(tail -n +3 CLAUDE.md)` 仅标题 / 前言差异可见

## 阶段七：版本交付与单提交收口

- [x] `packages/desktop/package.json`：`"version": "0.5.30"` → `"0.5.31"`（单行）
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 的 `version` 仍为 `"../package.json"`（零改）、`packages/desktop/src-tauri/Cargo.toml` 的 `[package] version` 零改
- [x] 单提交收口（D2）：把阶段一～七的实现面改动作为一个提交落盘，提交信息点明「退役 plugins/dev-team 插件层与插件期语料，仓库自此纯桌面仓」；**MUST NOT** 拆成「先删后改」两提交、MUST NOT 中间 push。本轮改动以单一工作区变更面收口（deletions 已 `git rm` 入索引），落地提交由归档 Commit 步承接；生成物 `bindings.ts` 与新增件 `config.schema.json` 均已 `git add` 入索引（避免归档 Commit 用 `-a` 时漏掉未 tracked 新增件）。`openspec/config.json` 的两行指针改动**同在该变更面内**（阶段二 2B）
- [x] 自查 D（AC-10 版本）：`grep '"version"' packages/desktop/package.json` = `0.5.31`；`tauri.conf.json` 仍为 `../package.json` 引用；`Cargo.toml` 零 diff
- [x] 自查 E（AC-2 归档前复核）：`ls openspec/specs | wc -l` = 35、零空目录、零 0 字节 `spec.md`（归档合入 `repo-surface-scope` 后应为 36，由归档相位复核）
- [x] 自查 A（AC-1 同提交）：变更面（索引 + 工作区）同时含 `plugins/`、`dist/`、`.claude-plugin/marketplace.json`、`.cursor-plugin/marketplace.json` 与 `openspec/specs/<50 目录>` 的删除记录 —— `git status --porcelain | grep -c '^D '` = **412**（`plugins` 236 项——237 tracked 中 1 项作为 rename 源计入 `R` 记录、`dist` 124、两 marketplace 2、50 份语料 50；另 1 条 `R plugins/dev-team/bin/dev-team-config.schema.json -> …/crates/core/config/config.schema.json` 与 1 条 `A` 新增镜像，逐类反证见阶段三三条自查）。本相位零提交（改动留在单一工作区变更面，deletions 已入索引），同提交断言由归档 Commit 落地后复核 `git log --stat -1`
- [x] **自查 B1（AC-3 活跃面零命中 + AC-11 指针复核）**——两段均已通过：
      ① **活跃面零命中**（D12 ①，`openspec/config.json` **在检索面内**）：
      `git grep -n "plugins/dev-team" -- ':!openspec/changes/archive' ':!openspec/explores' ':!openspec/changes/retire-dev-team-plugin' ':!openspec/specs' ':!packages/desktop/src-tauri/crates/core/checks/tests/fixtures' ':!packages/desktop/src-tauri/crates/core/checks/tests/golden' ':!packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules_test.rs' ':!packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason_test.rs'` → **零输出**（源码 / `.gitignore` / 根文档 / `.github/**` / `openspec/config.json` 全在检索面；仅 4 处数据样本豁免）；
      ② **指针复核**（AC-11）：`git grep -c "plugins/dev-team" -- openspec/config.json` **零输出**（exit=1），`git diff --numstat -- openspec/config.json` = `2	2`，两值等于阶段二 2B 表「改为」列，且 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` 零 diff。
      **MUST NOT** 为过本条而改产品保护语义、预改样本或把该文件移出核对面（`repo-surface-scope` 明文禁止）
- [x] 自查 B2（AC-3 实现相位 · 主 spec 层）：`git grep -n "plugins/dev-team" -- openspec/specs` 的命中恰为 delta 覆盖的 16 行 8 位点（清单见阶段五），两处 Purpose 已归零；全量零命中（含主 spec）属**归档相位**判据，随 spec 同步段合入 delta 后复跑
- [x] **自查 C（AC-4 + AC-11 第二段）**——两段均已通过：
      ① 镜像面：镜像在案 173 行且与迁出前逐字一致；目标 URL 尾路径与仓库内现存路径同形；`packages/desktop/knip.json` 在案且 `git check-ignore` 无命中（exit=1）；镜像不含 `config.json` 子串；
      ② 指针面：`grep -n "\$schema\|knip" openspec/config.json` 出现两新值、`git diff --numstat -- openspec/config.json` = `2	2`、`git grep -c "plugins/dev-team" -- openspec/config.json` 零输出、`JSON.parse` 通过
- [x] 交接 test-gen 相位：`crates/core/config/src/lib_test.rs` 三条一致性断言（键集 / 枚举 / 往返 + 测试内 `wire_name` 穷尽 match）**尚未落盘**（实测该文件无 `include_str!` / `KNOWN_KEYS` / `parse_framework` / `wire_name` 命中）—— 设计已定契约、用例归 test-gen 相位；契约见 `design.md`「数据模型 - 一致性不变量」。`core/checks/tests/{corpus_golden_test.rs,fixtures/README.md,golden/README.md}` 三处措辞随动已落盘（仅注释 / 文档文本），不再列入交接面
