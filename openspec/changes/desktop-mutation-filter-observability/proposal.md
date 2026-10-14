# 提案: desktop-mutation-filter-observability

> **变更**: desktop-mutation-filter-observability
> **日期**: 2026-10-14
> **状态**: draft

---

## 问题

桌面端变更流程跑突变测试报「突变执行中断：Stryker 段执行失败（退出码 1，未产出可解析段报告）：…st/src/stryker.js:49:48) at async Stryker.runMutationTest」。四层成因：

1. **根因：mutate 清单混入不可插桩文件**。`build_config` 把 `change_edit_set` 原始编辑集**不加过滤**直喂 Stryker `mutate`；Stryker 插桩器只认 `.js/.jsx/.mjs/.cjs/.ts/.mts/.cts/.tsx/.vue/.html/.htm/.svelte`，其余抛 `Unable to parse … No parser registered for <ext>!`（栈帧 `stryker.js:49` = 流水第 2 步插桩期，json 报告只在全部跑完后落盘——故「未产出可解析段报告」）。`packages/desktop` js-suite 根下 `package.json` / `*.md` / `*.css` / `*.yaml` / `*.png` / `*.toml` / `*.py` 被 change 碰过即引爆；suite 串行 + 中断 break，js 桶先崩则 rust（cargo-mutants）轮不到跑。CLI 时代无此问题（`deriveSourceFiles` 测试文件反推源文件 + `isFileExcluded` 消费 excludes），直喂口径移植时两道过滤均未带。
2. **诊断被遮蔽**：`output_tail` 末 500 **字符**截断，JS 错误「消息在前、栈帧在后」，根因行被 `…` 截掉只剩栈帧尾巴。testexec runner 与 mutation 各有一份同口径私有实现（口径分裂：另 bootstrap 300 字无省略号、static-check 无截断）。
3. **失败现场被清理**：中断后 `cleanup_stryker_dirs` 照跑，失败段临时 config 与 `_stryker-tmp` 被删，无法单段复现。
4. **合入面漏洞**：`mutation-stryker-seg-*.conf.json` 未被任何 ignore 规则覆盖（`_stryker-tmp` 已在插件仓根 `.gitignore`，但临时 config 即便在本仓 worktree 也会被归档 `commit_all` 的 `add -A` 收走；用户 workspace 两面皆裸）。

## 提案

只改桌面端（`packages/desktop/src-tauri`），CLI 零改动，三面：

1. **mutate 清单过滤（根因修复）**：落点 `bucket_suites` 归桶时刻——`suite.excludes` 全桶消费（复用 `core/checks::aggregate::is_file_excluded`，提升 pub）；Stryker 桶专属两规则（tool-scoped，node-test 等 NoExecutor 桶不受约束）：可插桩扩展名 allowlist（镜像 instrumenter `getFormat`）+ 测试文件剔除（`__tests__/`、`.test.` / `.spec.`——CLI `deriveSourceFiles` 反向口径）。空桶语义走既有管线：全桶滤空 → `EmptyDiff` skip；单桶滤空 → 掉出 `runnable`。连带堵死「mutate 清单非空但 Stryker 匹配 0 文件 → dry-run 0 mutant → score 100.0 静默假 Pass」路径。
2. **exec crate 增加 `ProcessOutput`（脚本执行 step 统一抽象层输出结构）**：`exit_code` + 合并原始字节；`error_excerpt()` 失败主因提取（前 10 个错误行组，每组带至多 3 行续行、遇空行止）；`tail()` 末尾 50 行（行口径替代字符口径）；`failure_message(head)` 统一装配（空输出逐字等于 head，保「空面无冒号缀」语义）。三消费方迁移：stryker / cargo-mutants 段失败消息、testexec `non_zero_exit_error`；删两份 `output_tail`。static-check（全文工件）/ bootstrap（成功摘要）/ agent bash 不动。
3. **失败段产物保留 + 合入排除**：执行期——失败（Interrupted / Fatal）的 Stryker 桶临时 config + `_stryker-tmp` 不清理（保留粒度 = 失败桶全临时面：段 index 不上抛、sandbox 本跨段共享、清单有界），反馈面附路径供单段复现；成功照清。合入期——`change_edit_set` 取数链路内幂等确保两条 ignore 规则写入 workspace `.git/info/exclude`（经 `git rev-parse --git-path` 解析，linked worktree 共享主仓 common-dir 一份），一次覆盖 `commit_all` 的 `add -A`、`dirty` 的 `status --porcelain`、清单的 `ls-files --others --exclude-standard` 三面；配套撤出索引面残留（`git ls-files --cached` 探测——intent-to-add 不出 `diff --cached` 但 `add -A` / commit 会真正收录；`git reset -q --` 只撤索引文件留盘）。不动用户 `.gitignore`、零 diff。

## 影响

- **core/orchestration 零改动**：`ToolStepOutput` / `MutationTestOutcome` / `MutationBlock` 字段集不动（golden wire contract 冻结面不触碰）；`feedback_text` 本就是自由 String。
- **反馈面文案变化（用户可见）**：段中断 / 非零退出失败文本从「单尾部」变为「错误信息 + 输出末尾（50 行）」双段 + 保留产物路径段；desktop 版本 bump 0.5.5 → 0.5.6。
- **规格随动**：`desktop-mutation-testing` 四条 Requirement（清单封装 / 框架矩阵 / sandbox 清理时点 / 双时点反馈边）MODIFIED delta 见 `specs/`；`desktop-change-archive` 无需改（`commit_all` 尊重 ignore 语义已覆盖）。
