# desktop-agent-execution Specification（delta）

## MODIFIED Requirements

### Requirement: Claude CLI 租户（MVP）

`agent-cli` SHALL 以本机 Claude Code CLI（`-p` 无头 + `--output-format stream-json --verbose`）实现 `AgentRunner`：

- stream-json SHALL 是唯一线上格式（解析路径唯一）；text / json 格式 MUST NOT 进入实现
- 环境档位 SHALL 双档：`default`（完整环境，页面默认）/ `bare`（`--bare` 显式开关，跳过 hooks / skills / custom commands / subagents / plugins / MCP / 自动记忆发现）。bare 档 SHALL 提示认证前提（bare 不读 OAuth 凭据与系统 keychain，须 `ANTHROPIC_API_KEY` 或 `--settings` 配 `apiKeyHelper`）
- permission-mode SHALL 参数面三档（`default` / `acceptEdits` / `bypassPermissions`），默认 `bypassPermissions`（`--dangerously-skip-permissions`）——`-p` 默认 `default` 档下需审批工具直接被拒、看不到真实 loop，本机自有 repo 的调试页场景裁决以完整循环为默认
- cwd SHALL 为当前 workspace root（隐含，不设参数）；model MUST NOT 进 MVP 参数面（继承用户 CLI 默认）
- CLI 发现 SHALL 处理 Windows `.cmd` shim（`cmd /C` 包装或解析真实入口）；CLI 不可发现时 SHALL 显式报错（错误事件 / Err），MUST NOT 静默空转
- **resume 续会话 SHALL 实现**：`AgentRunParams` 增可选 `resume_session_id: Option<String>`，非空时 flag 组装 `--resume <id>`（与 `-p` 组合即「以该 session 续发新一轮」）；`None` 时行为与既有 one-shot 完全一致。`--continue`（隐式续最近会话）MUST NOT 实现——续话一律显式 `session_id`，语义明确可追溯
- MVP MUST NOT 实现：用户 kill 运行、`--continue` 隐式续会话、`--include-partial-messages` 增量流

#### Scenario: flag 组装

- **WHEN** 以 default + bypassPermissions 参数组装命令行
- **THEN** 含 `-p --output-format stream-json --verbose --dangerously-skip-permissions`，不含 `--bare` 与 `--resume`
- **AND** bare 档时命令行含 `--bare`，stream-json 与 `--verbose` 不变

#### Scenario: resume flag 组装

- **WHEN** 以 `resume_session_id = Some("sess-1")` 组装命令行
- **THEN** 参数含 `--resume sess-1`，其余 flag（`-p` / stream-json / `--verbose` / env 与 permission 档位）不变；`None` 时不出现 `--resume`

#### Scenario: JSONL 逐行泵

- **WHEN** 以多行 stream-json 输出 fixture 驱动 runner 解析（不经真实进程）
- **THEN** 逐行归一化为 AgentEvent 序列且行间无串扰，进程退出后 run 收敛

#### Scenario: Windows CLI 发现

- **WHEN** PATH 上的 `claude` 为 `.cmd` shim
- **THEN** spawn 成功（经 `cmd /C` 包装或解析后的真实入口）；CLI 不存在时得到显式错误而非静默空转

#### Scenario: bare 认证失败如实呈现

- **WHEN** 无 `ANTHROPIC_API_KEY` 时以 bare 档发起运行
- **THEN** 失败以错误事件流入时间线呈现，不静默、不崩 UI

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令：

- `agent_start`：执行命令。body SHALL 维持三件事纪律（参数转换 → 调用 → 错误映射），编排 SHALL 收在 `run_agent()` 编排函数（组装 runner → 事件流 tee 双 sink → 状态收敛）；该函数与 `*_inner` 同列 app 层微形态（详见 desktop-app-shell）。命令 SHALL 增可选参数：`resume_session_id`（续会话）、`source`（来源受控字符串，缺省 `debug`）、`source_ref`（来源内定位）、`parent_run_id`（链上游 run）——编排 SHALL 将其写入 run 记录字段面；调试页 invoke（不传新参数）行为 SHALL 与既有 one-shot 完全一致（向后兼容）
- `agent_runs` / `agent_run_events`：查询薄包装（无状态，参数 → store 查询 → DTO）

错误约定沿用既有模板 `Result<T, String>`：CLI 不可发现、spawn 失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: agent_start 编排收口

- **WHEN** 审查 `agent_start` 实现
- **THEN** 命令体为参数转换 + 调用 `run_agent()` + 错误映射三段，runner 组装与 tee 在编排函数内，不膨胀命令体

#### Scenario: 来源与 resume 参数透传

- **WHEN** explore 页以 `source="explore"`、`source_ref=<记录键>`、`resume_session_id=<sid>` invoke `agent_start`
- **THEN** 生成 run 记录携带对应字段且进程以 `--resume` 续话；调试页 invoke（不传这些参数）生成的记录 `source="debug"`、链字段为 `None`，行为与演进前一致

#### Scenario: 查询薄包装

- **WHEN** 前端 invoke `agent_runs` / `agent_run_events`
- **THEN** 命令无状态、直查 store 返回 DTO，无领域解释

#### Scenario: 错误 reject 传达

- **WHEN** CLI 不可发现时发起 `agent_start`
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

### Requirement: MVP 边界与已知限制

以下边界 SHALL 作为 MVP 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **无 kill**：MUST NOT 提供终止运行入口；应用中途关闭时 claude 子进程可能残留（Windows 尤甚）——孤儿进程为已知限制
2. **续会话经 resume 落地**：`--resume <session_id>` 随 explore 会话链实现（见「Claude CLI 租户（MVP）」）；`--continue` 隐式续会话 MUST NOT 实现；调试页自身仍不提供 resume 入口（续话仅 explore 会话链消费）
3. **留存无清理**：事件转录无上限增长
4. **交互工具限制**：`-p` 下 AskUserQuestion 类交互工具的行为面不变（拒绝或受限如实呈现）；explore 页将其 ToolUse 渲染为静态可读卡片（问题/选项原样呈现），答案经 composer 文本输入并 resume 续话；questionnaire 组件接线为二期
5. **bypassPermissions 默认**：deny 规则仍生效；agent 运行可在 workspace 内无审批改动文件——本裁决限于本机自有 repo 的调试场景

#### Scenario: 无终止入口

- **WHEN** 审查调试页、explore 页与命令面
- **THEN** 无 kill / cancel 命令或按钮，运行中页面仅呈现流式状态

#### Scenario: 限制留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情
