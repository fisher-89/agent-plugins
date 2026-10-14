# worktree 清理与 active 上限（2026-10-14 探索）

主题：桌面端归档完成后自动移除 worktree；单工作区最多 5 个运行中 change 并存，避免 worktree 磁盘占用失控。

## 现状地图

```
  create                              archive 相位 (10 步)                    磁盘
  ────                                ────────────────────                    ────
    │                                 PhaseStart
    ├─ 建档 (worktree/base_commit)    Preflight
    ├─ git worktree add               Commit      ── worktree 内 commit_all
    │   └─ branch change/<name>       Rebase      ── worktree 内重放到主仓 tip
    ├─ 目录树 + explore.md            ResolveConflicts ── agent 解冲突
    ├─ bootstrap (pnpm install …)     Merge       ── 主仓 ff-only 快进
    │   └─ ⚠ node_modules / target    SpecSync    ── agent 同步 delta specs
    └─ ⚠ 脏仓警告                     Seal        ── 目录改名 + db 翻转
                                        Finalize  ── 脏探测（归档两路径）
                                        PhaseLog
                                                              ▲
                                                              │
                                              ┌───────────────┴──────────────┐
                                              │  worktree 与 branch 留在原地  │
                                              │  ← 显式 V1 手动边界（3 处 spec）│
                                              └──────────────────────────────┘
```

「归档后不清理」是被三处 spec 显式留痕的 V1 裁定，不是遗漏：

| 出处 | 原话 |
|------|------|
| `desktop-change-worktree` V1 边界 1 | `git worktree remove` + `git branch -d` 由用户手动执行；桌面 cleanup 按钮留后续 |
| `desktop-change-archive` V1 边界 1 | 归档相位成功后不执行清理（同裁定维持）；cleanup 按钮留后续变更 |
| `desktop-change-state-store` | 双写操作零 `git worktree` / `git branch` 调用（清理仍为手动边界） |

且 `write/archive.rs`（D13）明写「归档不触碰 worktree / branch」——裸 `archive` 写面是纯 fs + db 双写、**零 git 触点**的写面纪律。本轮「归档后自动移除 worktree」即偿还该边界，须同时处置这条红线的归属（git 触点不进 write 面 → 走 orchestration 步）。

## 定夺（2026-10-14）

1. **上限口径 = active 建档数**：db `status=active` 的 change 数 ≤ **5**（硬编码定值）。另加 **worktree 磁盘实占数保底检查**，门槛 = 变更门槛 **+2 = 7**（冗余防线，防 db 与磁盘漂移时失控）。两道检查都在 create 前置。
2. **清理落点 = archive 相位新增 Cleanup 步**（序列插位 design 定）；**清理完成后 Seal（状态翻转）收口**（二轮定夺 B：Seal 整体挪到 Cleanup 之后）。清理动作 = `git worktree remove`；失败仅 step detail 警告（二轮定夺 A）。
3. **branch 不删**：`change/<name>` 分支保留（历史可溯；`worktree remove` 本就不动分支）。
4. **弃置（abandon）本轮不做**：上限生效后「active 占额度但无退路」的洞已知，另立变更偿还。本轮错误文案至少要引导「归档或手动清理」。
5. **上限硬编码**：不进 `config.json`（对齐 `FEEDBACK_LOOP_LIMIT` 先例）。

## 归档链时序（定夺 2 + 二轮 B 定稿）

```
  … SpecSync → Cleanup(新) → Seal → …
                │
                └─ worktree remove 成功 → Seal（改名 + db 翻转）收口
                   失败 → step detail 警告，Seal 照常收口（残留手动清）
```

Cleanup 成为双写收口前置；Seal（目录改名 + db status 翻转）**整体后移**，「清理完成后修改 change 状态」即此序。清理失败不阻断 Seal（归档必收口，残留是警告不是回滚），与既有半完成重试哲学同源（改名成功翻转失败 → 重试续半边）。十一步序列与 Finalize 相对座位在 design 定稿。

## 开放点处置（2026-10-14 二轮定夺）

- **A. 清理失败语义** → **警告落 step detail 即可**：不扩 `ArchiveOutcome`（两字段不动，零撞 golden wire 冻结）。清理失败 = step detail 警告 + 链照常收口；worktree 残留由用户手动清理（与 V1 手动边界同哲学，降级为兜底而非主路径）。
- **B. 状态翻转位次** → **Seal 整体挪到 Cleanup 之后**：Seal 不拆半边（改名 + db 翻转整体后移），Cleanup 成为双写收口前置。时序：

  ```
  … SpecSync → [Cleanup(新)] → Seal → …
                │
                └─ worktree remove（尽力；失败仅 step detail 警告，不阻断）
  ```

  「完成后修改 change 状态」由此成立：清理先走，Seal（状态翻转）收口在后。清理失败仍不阻断 Seal——归档必收口，残留 worktree 是警告不是回滚。Finalize 步的相对座位（留在 Seal 前或后）design 定稿，语义不变（归档两路径脏探测）。
- **C. `ChangeRecord.worktree` 死路径** → **保留不改**：清理后 change 关闭（archived，无后续操作），worktree 字段作历史锚原样留，前端信息行照旧呈现路径（或可注明已清理，非必须）。零 golden 重写。

## 剩余开放点（留给 design / proposal）

- **D. 裸 `archive_change` 命令路径**：D11 逃生口（无前端入口）不经 archive 相位 → 无清理 step。是否接受漏清、或裸命令补 best-effort 清理，design 定。
- **E. 清理 step 幂等**：worktree 已不存在时（用户已手动清 / 重试链）应幂等成功而非失败。
- **F. 上限执法点细节**：两道检查（active≤5 / worktree 目录数≤7）均为 create 前置第八/九道（全 IO 前置、拒绝面零 worktree/零建档/零目录，沿前置七道风格）；worktree 目录数扫描锚 = `worktrees/{身份段}/` 子树。错误文案需含当前计数与上限。
- **G. `run_install` 残留**：bootstrap 的 node_modules / target 是磁盘大头，`worktree remove --force` 可带走；无需单独机制（Cleanup 用 `--force` 待 design 确认——手动边界场景用户可能想保留残留排查）。

## 后续

- 要立 change 时跑 `dev-team:phase-proposal`（会把本文件 promote 进 change 目录）。
- 弃置（abandon）另立变更偿还（定夺 4 的欠账）。
