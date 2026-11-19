# 架构文档：公共面（模块 doc）与 api 契约合一

workflow_type: requirement

目标方向（用户裁决）：**③ 文档内容面** —— 手写「公共面」表改由契约/代码生成或校验，散文与 api 段不再各写一份签名。

## 1 · 先纠正一个前提：文件层早已合一

```
.dev-team/architecture/models/<分层目录…>/<FQN>.md      ← 唯一手写真相
├── 散文段：职能 / 边界 / 公共面 或 操作 / 不变量 / 承载理由 / 契约面留白
└── fenced json：api-op / api-types / api-reexport
        │ api:export（archi-api-gen）
        ▼
<FQN>.schema.json（同目录）+ graph/INDEX.toml + <入口主干>.i.rs / <入口主干>.i.ts
```

- `api::paths::carrier_path`：`declared` → 派生 `<models>/…/<FQN>.md`；`none` → 模型 `doc` 字段。
- `paths.rs:278` 注释：「契约 md 与入口文档不是两份资产，只是同一落点的两个用途」。
- 模型侧只有 2 个 `api = "none"`（`Tools.ExportBindings` / `Tools.ArchiApiGen`），且都没写 `doc`。

所以「模块 doc 与 api-doc 是两份文件」在今天不成立；分裂发生在**同一份文件内部**。

## 2 · 真正的分裂：两种书写形态，判据覆盖度不同

```
后端形态（Foundation 9 op / ArchiRuntime 1 op）
  ### <op-id>            ← 名字：有判据（heading 集合 ≡ api-op fence 归属集合，parse+verify 硬 gate）
      散文一句语义        ← 手写
      ```json api-op```  ← 签名：唯一，且与代码归一签名对拍（apiSignatureMismatch, error 档）
  ## 类型  ```json api-types```

前端形态（Components.Agent + 7 个子元素，共 8 份）
  ## 公共面
  | 名字 | 形态 | 来源文件 | 语义 |   ← 名字/形态/语义全部手写，零判据
  ## 类型  ```json api-types
  {}
```
  零 `api-op`；`$defs` 恒 `{}`（契约面留白）。

实测（本仓当前）：8 份前端文档的公共面表名字与 `components/agent/index.tsx` 的 barrel 导出**逐条一致**（7 条）——目前靠人肉纪律还绿，但没有任何机械判据会把它拉回绿。

## 3 · 「公共面」的三个潜在持有者

```
       ① 散文「公共面」表          ② 契约 fence（api-op / $defs）      ③ 代码导出面（extract）
       手写 · 零判据               手写 · 与 ③ 锁死（error 档）          机器事实 · 判据输入
          │                            │                                    │
          │  ✗ 未锁                    │  ✓ 已锁（apiSignatureMismatch）     │
          └────────────────────────────┴────────────────────────────────────┘
                      ✗ 未锁（apiUndeclared 当前 Off 档 ⇒ 静默）
```

- ②↔③：`normalize_signature`（代码侧）对 `signature_of`（契约侧），`apiSignatureMismatch` = **error**。
- ①↔③：`apiUndeclared`（「代码有导出未在契约声明」）判据在，但 `severity() = Off`，且调用处 `if severity != Off` ⇒ **今天永不执行**（`check.rs:199`）。
- ①↔②：不存在。唯一先例是 `api-reexport` 的散文↔fence 双向 gate（`verify` 第三条：`api-reexport` 键集 ≡ 散文列表首标识符集）。

⇒ 「合一」的实质是**选定唯一持有者**，另两个降级为派生或校验对象。

## 4 · 三条路，同撞一堵墙

| 路 | 做法 | 依赖 |
|---|---|---|
| A · 散文↔代码 gate | `## 公共面` 表行首名字集合 ≡ `extract_element` 的导出集合（新 gate，装载期或判据期） | 只需一个表格扫行器；**绕开 fence 与生成链** |
| B · 散文↔fence gate | 表名字集合 ≡ `ops ∪ $defs ∪ reexports`（沿用 reexport 范式） | 前端 8 份 `$defs` 为 `{}` ⇒ 立刻红 ⇒ 必须先写前端契约 |
| C · 表退役，改渲染 | 删「公共面」表，架构页/文档视图从契约渲染人读 API 参考 | 同 B，且需前端有契约内容否则渲染为空表 |
| D · 统一「每符号一节」 | 前端也改成后端形态（`### <name>` + fence + 散文语义） | 同 B；且 `### <op-id>` 无 fence 是结构性错，不能只写heading |

B/C/D 都卡在**前端 TS 契约面留白**，三个已知缺口（AgentError.md「契约面留白」段已记录）：

1. **`$ref` 归一不对称**：`schema_type` 里 `$ref` → `ApiType::Unknown(ref_name)` → 短形 `?AgentEvent`；代码侧具名类型 → `ApiType::Named` → 短形 `AgentEvent` ⇒ 恒不等。（一行改动：`$ref` → `Named`。现存 10 份契约里**零 `$ref` 实例**，改动零现存影响。）
2. **组件形态无词汇**：`ApiSymbolKind` 只有 Function/Type/Trait/Enum，组件即 Function；契约侧无「组件 / JSX 元素」位（`Element` 一类返回位无法表达）。
3. **`.i.ts` 生成物与格式化器冲突**：`emit::ts` 末尾多一个空行，`vp check` 剥掉后 `api:check` 零 diff 门判 `mismatched`；`client:check` 与 `api:check` 只能二选一（出路：生成器去末尾空行，或 `src/**/*.i.ts` 纳入豁免）。

## 5 · 裁决记录（已定）

| # | 问题 | 裁决 |
|---|---|---|
| 1 | 合一落在哪一层 | **③ 文档内容面**（手写「公共面」表 vs 契约/代码） |
| 2 | 「公共面」名字清单的唯一持有者 | **契约 fence**。散文不持有名字清单 |
| 3 | 前端 8 个 React 组件元素的契约 | **本变更允许临时留白**；组件契约另起探索 |
| 4 | 留白的寄存形态 | **不加 fence**，散文「契约面留白」段保留；留白是**临时债，未来要修复** |
| 5 | `apiUndeclared` 是否抬档 | **不抬**（保持 `Off`）——留白期会误伤临时债 |
| 6 | 缺口 1（`$ref`→`Named`）、缺口 3（`.i.ts` 末尾空行） | **非阻塞缺口都不修**，随组件契约探索一并处理 |
| 7 | 前端 8 份「公共面」表本变更处置 | **原样不动**，等组件契约变更一并退役 |
| 8 | 机械化的形状 | **散文表格抽取 + 第四条 verify gate**（`契约非留白` 时才生效），同批写 spec 纪律 |

## 6 · 本变更的形状（收敛）

### 代码面（唯一改动点在 `contract.rs`）

```
新增 collect_surface_table(sections) -> Vec<String>
  扫 `##` 段 body 的表格行：| `名字` | 形态 | … | ⇒ 取首列反引号内词，非 ASCII 标识符即跳过
  形态受限：只认表格行（列表项已有 collect_reexport_prose，正文名字绝不判）

verify(contract, schema) 追加第四条 gate（与 api-reexport 双向 gate 同族、同错误面）
  前置：契约非留白（contract.ops 非空 ∨ contract.types 非空）
  ⊢ 散文表格名字集 ≡ ops ∪ $defs ∪ reexports 的名字集，不等即 ApiError::Contract 点名差集
```

- 错误面走 `ApiError::Contract` ⇒ 装载期中止，**零新增 finding kind**（十三值 / `golden_test` 的「恰 13 份 sidecar」不动）。
- 与 `schema_type` / 归一签名无关 ⇒ 缺口 1 确实可以继续不修。

### 零命中实测（本仓当前语料）

| 文件 | 表格行 | 契约非留白？ | gate 是否生效 |
|---|---|---|---|
| `Backend/Core/Foundation.md` | 0 | 是（9 op） | 生效但无表 ⇒ 不误伤 |
| `Backend/Infra/ArchiRuntime.md` | 0 | 是（1 op / 1 type） | 同上 |
| 前端 8 份 | 3–8 | 否（`$defs = {}`、零 op） | **不生效** ⇒ 表格原样不动合法 |

### 待 design 期定的实现细节

- gate 作用域：只扫 `## 公共面` 段（零误伤，但段名成为机器约定），还是扫任意段的表格行（更泛，但散文里将来出现参数表 / 对比表会误伤）。倾向后者 + 双向 gate（表格名字集本就应该精确等于公共面），若误伤再收敛为段名白名单。
- 「契约非留白」的判据取 `contract.ops` / `contract.types`（手写真假，语义清晰）而非 `schema["$defs"]`（生成物）。

### spec 面

- `desktop-archi-api`：契约是公共面名字清单的**唯一持有者**；散文只承载语义（职能 / 边界 / 不变量 / 承载理由 / 逐 op 语义），MUST NOT 以表格清单复述契约名字；前端组件元素契约留白为**临时状态**，其「公共面」表随组件契约到场一并退役（退役由第四条 gate 机械强制）。
- `desktop-archi-api-check`：本条走装载期 `ApiError::Contract`，不进 §判据面；留白面零判据的现状与理由登记在案。

## 7 · 仍开放的问题

- **组件契约探索**（另起）：缺口 2（组件 / JSX 元素无契约词汇）是主干；顺带缺口 1（`$ref`→`Named`，一行）与缺口 3（`emit::ts` 收口去末尾空行，`render_scope` 每块 `\n\n` ⇒ 文件末尾恒多一空行）。该探索的终点 = 前端 8 份「公共面」表退役、名字进 `$defs` / `api-op`。
- **留白的登记形态**：本变更不加 fence、不抬 `apiUndeclared`。留白债何时可被机器计数（INDEX 债务列？）留待留白清账时再议。
- **gate 作用域**：见 §6「待 design 期定的实现细节」。
