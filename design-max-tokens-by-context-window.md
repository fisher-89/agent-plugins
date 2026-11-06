# 设计方案：max_tokens（输出长度限制）默认基于 context_length 分段计算

> **状态**：设计稿（Draft），未实施
> **性质**：Rust 后端（`crates/infra/agent`）+ 前端提示文案 + spec 语料修订
> **背景缺陷**：dev-design 相位连续 5 次会话未能产出 `design.md` / `tasks.md`，根因为单次响应被输出上限截断（详见文末「附录 A · 事故实证」）
> **建议落点**：新建独立 openspec change（本文性质与 `agent-stream-single-message` 冲突——后者写死「Rust 后端零改动」）

---

## 1 现状（问题的根源）

### 1.1 输出上限是三档取值链，固定兜底

`crates/infra/agent/src/sdk/runner.rs:277`

```rust
pub(crate) fn resolve_output_tokens(provider_value: Option<u64>, rig_default: Option<u64>) -> u64 {
    provider_value.or(rig_default).unwrap_or(OUTPUT_TOKENS_FALLBACK)   // 32_000
}
```

`OUTPUT_TOKENS_FALLBACK = 32_000`（`runner.rs:37`）。

### 1.2 窗长与输出在装配阶段完全不相遇

`open_session`（`runner.rs:170-190`）的执行顺序：

```rust
let spec    = parse_model_spec(&model_name)?;                    // [1m] → length = 1_000_000
let model   = build_model(&self.config, &spec, self.max_tokens);  // ← 输出链只看 max_tokens，看不到窗长
let defense = ContextDefense::resolve(spec.length.or(self.context_window));  // ← 窗长只喂防线
```

`build_model` 签名（`runner.rs:225`）中没有任何窗长入参——**这条取值链结构上无法感知 `context_length`**。

### 1.3 `[1m]` 后缀只到 3 个判断点，第四点缺失

`[1m]` 经 `parse_model_spec` 解析为 `ModelSpec { name, length }`（`model_spec.rs:54`），`length` 的消费点：

| 判断点 | 是否受 `[1m]` 影响 | 代码位置 |
|---|---|---|
| ① 上下文窗长 | **是**（优先级最高） | `runner.rs:187` `ContextDefense::resolve(spec.length.or(self.context_window))` |
| ② anthropic beta 头 | **是**（仅 anthropic 协议） | `runner.rs:268` `spec.length == Some(MILLION_CONTEXT_WINDOW)` |
| ③ 请求与上报模型名 | **是**（被剥离为裸名） | `runner.rs:173`、`spec.name` |
| ④ **输出上限 max_tokens** | **否，零影响** | `runner.rs:242` 三入参中无 `spec` |

`loop.rs` 全文检索 `length` / `1m`：**零命中**。

### 1.4 实测：输入上限被抬高，输出上限不变

`engine.log` 泵启动行统计（`window=` 为 `defense.window()` 的**生效**值）：

```
deepseek-flash[1m]                 window=1000000  ×441   (10-26 → 10-30)
deepseek/deepseek-v4.1-flash[1m]   window=409600   ×438   (10-30 → 11-06)
```

409600 = `min(1_000_000 × 3/5, 400 × 1024)`，由 `MAX_DEFENSE_WINDOW`（`context.rs:17`）封顶。同期输出上限恒 32 000（`loop.rs:227` 恒 `max_tokens: None`，取 wire 默认值）。

**输入 409 600 / 输出 32 000 = 12.8 倍不对称。**

---

## 2 目标 / 非目标

### 目标

- 用户留空 `max_tokens` 时，输出上限由生效窗长分段推导，取代固定 32 000。
- 大窗模型（≥ 256K）获得与窗长匹配的输出空间。
- `[1m]` 后缀经窗长间接改善输出预算（不再「只抬输入不抬输出」）。

### 非目标（本次不做）

| 项 | 理由 |
|---|---|
| 修「截断被误判为 IO 失败并原样重发」 | 独立缺陷，见 §8.1；语义变更需独立验收 |
| `[1m]` beta 头对第三方网关的无条件发送 | 独立缺陷，见 §8.2 |
| `spec.length` 与 `context_length` 改取小 | 与既有断言冲突，见 §8.3 |
| 给 openai 协议写 `max_tokens` | spec 明确禁止，保持省略 |

---

## 3 关键决策

### D1 分段表（编译期常量，不落配置）

| 标称窗长 W（token） | 默认输出上限 | 说明 |
|---|---|---|
| `W < 32 768` | **4 096** | = `MIN_OUTPUT_TOKENS`，与表单下限同源 |
| `32 768 ≤ W < 131 072` | **16 384** | 32K–64K 档 |
| **`131 072 ≤ W < 262 144`** | **32 000** | **128K 缺省落此档 → 现行为逐字不变** |
| `262 144 ≤ W < 524 288` | **48 000** | 200K–400K 档 |
| `W ≥ 524 288` | **64 000** | 封顶（含 `[1m]` → 1M） |

区间取左闭右开 `[lo, hi)`，边界值由表驱动测试锚定。

**取值理由：**

- **128K → 32 000 是硬约束**：`DEFAULT_CONTEXT_WINDOW = 128 × 1024 = 131 072`（`context.rs:8`）。未配置任何字段的用户必须拿到与今天完全相同的 32 000，否则构成全局行为变更。
- **封顶 64 000 而非 128 000**：rig 的 anthropic 名单对 claude 5.x 给 128K、对 opus-4 / sonnet-4 给 64K，但那些模型先命中 rig 那一档（权威元数据）。**推导值只服务名单外模型**——对未知模型 64K 是安全猜测，128K 易触发 400。
- **下限 4 096 而非按比例**：`W < 16K` 时按 25% 会推导出 < 4096，与表单下限（`provider-panel.tsx:45`）与命令层 `save_agent_provider` 的拒收线自相矛盾。推导值必须落在合法区间内。
- **扁平阶梯而非区间内插**：易论证、易测边界、不引入浮点。

### D2 取值链位置：只替换最后一档

```
provider.max_tokens（显式值，最高优先）
  → rig 名单官方值（名单命中，保持权威）
    → 分段推导（取代原 32 000 固定兜底）
```

理由：spec（`desktop-agent-execution/spec.md:977`）已确立「名单命中时保留 rig 值、不被降级」。改中间档会让已上线模型的请求上限漂移。只换末档，语义收敛面最小。

### D3 推导输入 = **标称**窗长，不是防线窗长

使用 `spec.length.or(self.context_window)`（= 喂给 `ContextDefense::resolve` 的同一个值），而非 `defense.window()`（60% × 400K 折算后）。

理由：

1. 与 `[1m]` / `context_length` 的语义一致（用户填的是标称值）；
2. 标称与折算值在 `W ≥ 683K` 后落入同一档（64 000），不产生歧义；
3. 避免「输出预算随成本护栏变化」——把成本策略与能力策略分开。

### D4 缺省 `None` 与「未配置」同分支

`derive_output_tokens(None) = 32_000`，显式表达「128K 缺省档」的取值，不引入第二处字面量。

### D5 不改 `EngineConfig`，不改门面签名

`context_window` 已在 `SdkRunner`（`runner.rs:52`），推导所需输入齐备，只是没接上。零新增穿透链。

---

## 4 代码改动点位

### 4.1 新增分段常量与纯函数（`runner.rs`，紧随 `resolve_output_tokens`）

```rust
/// 输出上限分段表：(窗长下界, 默认输出上限)，升序；查表取首个下界 > W 的前一档。
/// 编译期常量而非配置面——与 MAX_ROUNDS / MAX_RETRY_TIMES / FEEDBACK_LOOP_LIMIT
/// 同类；用户级覆盖走 provider.max_tokens 列，不是第二条配置面。
const OUTPUT_TOKENS_SEGMENTS: &[(u64, u64)] = &[
    (0,            MIN_OUTPUT_TOKENS),  // 4_096
    (32 * 1024,    16_384),
    (128 * 1024,   32_000),             // 128K 缺省档：与既往兜底逐字相同
    (256 * 1024,   48_000),
    (512 * 1024,   64_000),
];

/// 窗长 → 默认输出上限（纯函数，无 IO）。`None` = 未配置窗长（128K 缺省档）。
pub(crate) fn derive_output_tokens(context_window: Option<u64>) -> u64 { /* … */ }
```

### 4.2 取值链签名扩一位

```rust
// 前
pub(crate) fn resolve_output_tokens(provider_value: Option<u64>, rig_default: Option<u64>) -> u64
// 后
pub(crate) fn resolve_output_tokens(
    provider_value: Option<u64>,
    rig_default: Option<u64>,
    context_window: Option<u64>,
) -> u64
```

### 4.3 `build_model` 增第 4 参

```rust
// 前
pub(crate) fn build_model(config: &EngineConfig, spec: &ModelSpec, max_tokens: Option<u64>) -> DynModel<Completion>
// 后
pub(crate) fn build_model(
    config: &EngineConfig,
    spec: &ModelSpec,
    max_tokens: Option<u64>,
    context_window: Option<u64>,
) -> DynModel<Completion>
```

anthropic 臂内改为三参调用；openai 臂逐字不动。

### 4.4 `open_session` 把窗长提前算出一处共两用

```rust
let spec    = parse_model_spec(&model_name)?;
let window  = spec.length.or(self.context_window);          // ← 单点
let model   = build_model(&self.config, &spec, self.max_tokens, window);
let defense = ContextDefense::resolve(window);              // ← 同源
```

顺带消除今天「语义分散于两处」的隐患。

### 4.5 泵启动日志补输出上限（排障关键）

`runner.rs:360` 现为：

```
泵启动 session=… protocol=… model=… window={window} web_search=… raw=…
```

追加 `max_tokens={output}`。缺少这一行，下次同类事故仍只能靠截断列数反推。

### 4.6 前端提示改三处（`packages/desktop/src/views/agents/components/provider-panel.tsx`）

| 位置 | 现状 | 改后 |
|---|---|---|
| `:120` placeholder | `留空 = 按协议缺省` | `留空 = 按 context_length 分档` |
| `:343-345` suffixHint 基串 | 「…**不改输出上限**；防线水位按 60% 折算且封顶 400K」 | 「…只影响上下文窗长与 anthropic beta；**输出上限随窗长分档推导**（128K→32K，512K+→64K）；防线水位按 60% 折算且封顶 400K」 |
| `:330-333` max_tokens 提示 | 「留空 = 按协议缺省；…下限 4096」 | 「留空 = 按 context_length 分档推导（未配置时 32000）；仅 anthropic 协议生效；显式填值优先且须 ≥ 4096」 |

`provider-panel.test.tsx:672 / :675` 断言这两串，须同一提交改。

---

## 5 影响面与兼容性

| 场景 | 今天 | 改后 | 是否变化 |
|---|---|---|---|
| 全部字段留空（128K 缺省） | 32 000 | 32 000 | **不变** |
| 名单内模型（如 `claude-sonnet-4-6`） | rig 官方值 | rig 官方值 | **不变** |
| 显式 `max_tokens = 8192` | 8 192 | 8 192 | **不变** |
| `context_length = 200000` | 32 000 | 32 000（128K–256K 档） | **不变** |
| `context_length = 400000` | 32 000 | 48 000 | 变（意图所在） |
| `[1m]` → 1M | 32 000 | 64 000 | 变（意图所在） |
| openai 协议 | 省略 | 省略 | **不变** |
| L3 compaction 摘要请求（`compact.rs:48` `max_tokens: None`） | wire 32 000 | 随档提升 | 变（正向：摘要更完整） |

**破坏面**：唯一真实风险是「用户声明 ≥ 256K 窗长，但网关实际输出上限 < 48K」→ 请求 400。

缓解：

1. 显式 `max_tokens` 覆盖通道原样保留；
2. 推导只在用户主动声明大窗长时才越过 32K；
3. 400 按既有 `api_error` 显式暴露（现行边界，非静默）。

---

## 6 测试清单

### 纯函数（`runner_test.rs`）

- 边界逐值：`0 / 32767 / 32768 / 131071 / 131072 / 262143 / 262144 / 524287 / 524288 / u64::MAX`
- `None` → 32 000（与 `OUTPUT_TOKENS_FALLBACK` 同值，锁死「未配置不变」）
- 单调性：0..2 000 000 抽样步进，推导值非递减
- 全域不变式：任意输入 `derived ≥ MIN_OUTPUT_TOKENS`（4 096）

### 三档链（`resolve_output_tokens`）

- provider 优先 → rig 优先 → 推导，三层两两组合各一例
- `Some(0)` / `Some(u64::MAX)` 原样透传（既有断言保持）

### 装配（`build_model`，14 处测试调用点补第 4 参）

- `window = None` → wire `default_max_tokens == Some(32_000)`（逐字回归锚）
- `window = Some(1_000_000)` → `Some(64_000)`
- 名单内模型 + 大窗长 → 仍取 rig 官方值（不被推导覆盖）
- openai 臂：任何窗长均不写 `max_tokens`

### 前端

- placeholder / 两条提示文案断言更新
- `context_length` 留空与填 400000 时提示不串

### 日志

- 泵启动行含 `max_tokens=`，与 `window=` 同源（同一测试断言两者一致）

---

## 7 文档与 spec 修订（同提交）

| 文件 | 需改内容 |
|---|---|
| `openspec/specs/desktop-agent-execution/spec.md:977` | 「输出上限取值链」三档 → 四档（末档为分段推导）；删除「`32_000` 兜底」表述 |
| 同上 `:980` | 删「输出上限与 `[1m]` 后缀**完全解耦**」，改为「后缀经窗长间接影响输出上限」 |
| 同上 `:887`（ADR 9） | 「名单外模型输出上限兜底恒 `32_000`」→「按窗长分段推导，128K 缺省档为 `32_000`」 |
| 同上 `:1007-1015` | 场景表补 (d) 大窗长档；把 (c) 期望值改为「128K 缺省档 → 32000」 |
| 同上 `:1226` | 文件职责行补「输出上限分段推导」 |
| `openspec/specs/desktop-agent-management/spec.md:13` | `max_tokens` 语义补「留空 = 按 context_length 分档推导」 |
| 同上 `:125` | 表单提示描述同步 |

---

## 8 明确不纳入本次（建议各自立 change）

### 8.1 截断误判为 IO 失败

`loop.rs:547` 是全局唯一的 `FinishReason` 消费点，只认 `pause_turn`：

```rust
fn is_pause_turn(finish_reason: Option<&FinishReason>) -> bool {
    matches!(finish_reason, Some(FinishReason::Other(reason)) if reason == PAUSE_TURN_REASON)
}
```

`Length` 无分支。截断在 rig 侧表现为 `content_block_stop` 时 tool input JSON 解析失败（`ProviderError::MalformedToolInput`，`rig-core-0.43.0/src/operation/completion.rs:544`、`providers/anthropic/streaming.rs:443`），到本地成为 `loop.rs:320` 的「API 流失败」，进入 `loop.rs:327` 的**原样重发**（`loop.rs:341-360`，请求体一字不改）。

不修此项，本次只是把撞顶概率从 32K 推到 64K，**不消除**。

### 8.2 `[1m]` beta 头无条件发送

`runner.rs:268` 只看 `spec.length`，不看 `base_url`：

```rust
if spec.length == Some(MILLION_CONTEXT_WINDOW) {
    provider.with_beta(BETA_CONTEXT_1M)   // "context-1m-2025-08-07"
}
```

第三方网关通常忽略，但若校验并拒收即 400。`[1m]` 语义只在 Anthropic 官方域有意义。

### 8.3 `spec.length` 与 `context_length` 取小

现为「后缀优先」（`spec.length.or(self.context_window)`）。若真实窗口小于 1M，防线永不触发，请求被 provider 直接 400。改为取小会推翻 `runner_test.rs:1372` 的显式断言，属语义变更。

---

## 9 交付物

- 版本：`packages/desktop/package.json` patch +1（AGENT.md 版本纪律）。
- 无 `config.schema.json` 变更（分段表是编译期常量，不是 `KNOWN_KEYS` 配置键）。
- 无 IPC / bindings 变更（`max_tokens` 列与命令面签名全部不动）。

---

## 附录 A · 事故实证（dev-design 连续 5 次未完成）

### A.1 时间线

```
proposal.md       mtime 11-06 10:50:51   35843 字节
specs/*/spec.md   mtime 11-06 10:51:00   42215 + 14160 字节
design.md / tasks.md —— 不存在（一次都没写成功）
```

`engine.log` 中 10:51 起至 15:25 停止，连续起 20+ 个 `sdk-*` 会话，最长 100 分钟。

### A.2 截断特征

```
loop 流重试 session=sdk-78-… turn_index=70 attempt=1/3
  cause=API 流失败: tool call `write` arrived with malformed JSON input:
        EOF while parsing a string at line 1 column 15894
```

- 全部 48 条 malformed 中，截断列**中位数 32 710**（min 4 219 / max 82 899）——与 32 000 兜底量级吻合。
- 单会话内重复撞击：`sdk-78` 10 次、`sdk-6` 12 次、`sdk-4` 10 次；其中一次耗尽到 `attempt=3/3`。
- 曾出现彻底失败：`10-30 18:25 loop 失败收敛 session=sdk-6 subtype=api_error turns=125 cause=…malformed JSON…49909`。

### A.3 编排侧后果

`crates/core/orchestration/src/walker.rs:3692` 收到 `AgentRunStatus::Failed` 即 run 级终止（不走相位 fail、不进决策重试）：

```rust
AgentRunStatus::Failed => {
    let reason = format!("会话失败收敛: {}", diagnose_brief(&cause));
    (StepSettle::failed(…), Some(Terminal::failed(reason)))
}
```

而 `MAX_RETRY_TIMES = 5`（`phase_table.rs:38`）配合 `phase_next.rs:193` 的 `window_fails >= MAX_RETRY_TIMES` 分叉——dev-design 的 evaluator 核对 `design.md` 是否在场，文件未写出必然 fail，故 5 次上限是**果**而非因。

### A.4 取证限制

运行时 `dev-team.exe` 对 workspace 库持有独占锁（redb），`~/.dev-team/dev/workspaces/*.redb` 读取返回 `ERROR_LOCK_VIOLATION`；`engine.log` 不记录相位 / attempt，只记 `sdk-*` 会话。上表时间线归属按「proposal 保存 → 起步时间」推断，attempt 号与终态的最终依据须在详情页 run reason 中核对。

---

## 附录 B · 待确认的开放问题

1. **分段阈值是否需要可配置？** 当前定为编译期常量（与 `MAX_ROUNDS` / `MAX_RETRY_TIMES` 同类先例）。若将来出现大量「声明大窗长但输出上限小」的网关，是否需要 `openspec/config.json` 级覆盖？
2. **`context_length = 200 000`（200K 档）该给 32 000 还是 48 000？** 本文按 `[131 072, 262 144)` → 32 000 处理（保持现行为），若希望 200K 档获益需下移分界点至 200K，但会打破「200K 配置今天拿 32 000」的兼容性。
3. **L3 compaction 摘要请求是否应保持独立上限？** 本文让其随档提升（正向），但摘要请求的语义（压到短文本）与主请求不同，可能反而需要更小的固定值。
