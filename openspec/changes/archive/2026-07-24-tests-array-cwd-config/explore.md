# tests 数组化 + cwd / config

探索时间：2026-07-22

## 背景

todo：`config中指定test运行的cwd和配置文件相对路径`

现状问题：

- `test.framework` + `test.overrides` 双层模型；`file`/glob 同时承担「匹配范围」和「推导 cwd」（`deriveWorkingDirectory`）
- 支持 glob 的 `file` **无法保证唯一根目录**（集合 ≠ 单点）
- 框架配置文件路径无法声明；命令模板无 `--config`，靠进 cwd 后框架自动发现
- 全局 `coverage` / `mutation` / `exclude` 与 override 级联，增加合成逻辑
- 实际场景需要：执行目录在上级、覆盖率/突变范围、临时文件落点——三者今日绑在同一 `directory` 上

今日三场景取值（耦合）：

| 场景 | 当前取值 |
|------|----------|
| (1) 执行 cwd | `derive(file)` → `plan.directory` → script `cd`；stryker `rootPath` 同此 |
| (2) 覆盖率/突变范围 | 归属：`file`∩¬`exclude`；突变列表：测试输出 `sourceFiles` 再滤 exclude |
| (3) 临时文件 | 相对 `directory`：`coverage/`、`.stryker-tmp/`、`stryker.config.*`、`reports/mutation/` |

## 外层结构（已拍板）

去掉最外层 `test` 对象，改为顶层 **`tests: []`**。  
键名用复数 `tests`（与旧 `test` 对象断开，breaking 干净）。  
**每个数组元素 = 一个可执行 suite。**

## 已拍板（横切）

### 1. 阈值与配置默认值在 schema 层解决

- 在 **schema 目录**专门放置常量（覆盖率 lines/branches/functions、mutation score 等）
- **不再**在 config 顶层提供全局 `coverage` / `mutation` 默认块
- 所有配置默认值统一在 schema 层（`prefault` / 常量）解决
- 消费者读 schema parse 后的结果，不手写级联

现状参考：`config.schema.ts` 内已有 `TEST_COVERAGE_*` / `TEST_MUTATION_SCORE_DEFAULT`，可抽到 schema 目录专用常量文件。

### 2. 锚点必填（由「file 不可省」演进）

- 早期：`file` 不可省略（多套配置需可校验）
- **现行理解**：锚点职责交给必填、**无通配符**的 `root`；`includes` 可选（见方案 G）
- 不做「省略匹配范围 → 回落框架 default_glob 且无锚点」的短写

## 方案对比

### 方案一览

```
              锚点/匹配           执行目录              范围(2)                临时(3)         心智负担
─────────────────────────────────────────────────────────────────────────────────────────────────
A 现状         file 一身兼         derive(file)          file∩¬exclude          directory      低配置/高隐式
B file+cwd     file=集合           cwd(相对项目根)       file∩¬exclude          cwd            中
C B+artifacts  同 B                cwd                   同 B                   artifacts      偏高
D B+scope      file+scope          cwd                   scope??file            cwd            偏高
E 统一 tmp     同 B                cwd                   同 B                   全局/suite     中高
F 收紧 file    file=目录           cwd??file             file 树                cwd            低（表达力差）
G 语义化 ★     root+includes       root/(cwd??.)         root∩inc∩¬exc         absCwd         中（语义最稳）
```

A–F 条目曾设想为「当前 override + cwd/config」；**G 为当前倾向的条目形状**。  
外层对所有方案均为 `tests: []`（G 不例外）。

### 方案 G（倾向）— 条目字段

```json
{
  "tests": [
    {
      "root": "plugins/dev-team/bin",
      "framework": "vite-plus",
      "cwd": ".",
      "config": "vite.config.ts",
      "includes": ["src/**/*.{ts,tsx}"],
      "excludes": ["./*", "src/schemas/**/*"],
      "coverage": { "lines": 80, "branches": 70, "functions": 75 },
      "mutation": { "score": 70 }
    }
  ]
}
```

| 字段 | 必填 | 约束 / 相对谁 | 语义 |
|------|------|---------------|------|
| `root` | 是 | 无通配符，相对 projectRoot | suite 唯一锚点 |
| `framework` | 是 | enum | 测试框架 |
| `cwd` | 否 | 相对 **root**；缺省 `"."` | 执行目录；可用 `".."` 表示上级执行 |
| `config` | 否 | 相对 **root** | 框架配置文件 |
| `includes` | 否 | glob[]，相对 **root**（待最终确认） | 匹配/范围集合 |
| `excludes` | 否 | glob[]，相对 **root** | 从范围中排除 |
| `coverage` | 否 | 对象 | 覆盖率阈值；缺省 schema 常量 |
| `mutation` | 否 | 对象 | 变异阈值；缺省 schema 常量 |

路径约定：

```
absRoot   = projectRoot / root
absCwd    = absRoot / (cwd ?? ".")
absConfig = absRoot / config
scope     = under(root) ∩ includes ∩ ¬excludes
artifacts = absCwd 下（默认；碰撞再考虑 C/E）
```

最小配置示例：

```json
{ "tests": [{ "root": "plugins/dev-team/bin", "framework": "vite-plus" }] }
```

上级执行示例：

```json
{
  "tests": [
    {
      "root": "plugins/dev-team/bin/src",
      "cwd": "..",
      "framework": "vite-plus"
    }
  ]
}
```

### G 下三场景取值

| 场景 | 取值 |
|------|------|
| (1) 执行目录 | `root/(cwd??".")` |
| (2) 覆盖率/突变范围 | `root` ∩ `includes`（缺省见开放问题）∩ ¬`excludes`；突变可再 ∩ 测试输出 sourceFiles |
| (3) 临时文件 | 默认 **absCwd** 下 |

### 为何倾向 G（相对 B）

- `root` 禁通配符 → 「一组配置无唯一根」消失；多 suite = 多条 `root`
- `cwd` / `config` / glob 都以 `root` 为原点，相对性一致
- 上级执行用 `cwd: ".."`，不必心算两条 projectRoot 相对路径
- 命名：`includes`/`excludes` 比 overloaded 的 `file` 清晰
- 旧约束「file 必填」在 G 下改读为 **「root 必填；includes 可选」**

代价：多 suite 共享同一 absCwd 时 (3) 可能互相覆盖 → 串行执行或后续加 artifacts/统一 tmp，不作为第一版字段。

## 仍开放（提案已拍板项已移入 proposal；此处仅留未决）

- artifacts 第一版跟 absCwd；碰撞后再考虑方案 C/E（提案明确不做第一版字段）

## 补充探索（2026-07-22）：suite.`config` 能否适配所有脚本？

### 结论

**不能**用「整段 `test_execution` 末尾 append `config_flag path`」适配所有框架，尤其是链式脚本。

| 框架 | 命令形态 | 稳定 config CLI | 末尾 append |
|------|----------|-----------------|-------------|
| jest / vitest / vite-plus | 单命令 | `--config` | 勉强可用，flag 易落在 `{files}` 后 |
| bun / node-test / go | 单命令 | 无统一文件型 flag | 不应注入 |
| **pytest** | **两段链式** | `-c`（非 `--config`） | 只贴最后一段 / flag 错误 |
| **rust** | **两段链式** | 无 vitest 式 config 文件语义（靠 Cargo.toml 位置等） | 无效或误伤 |

pytest 今日：`pytest -v {files}; pytest --cov=...` — 整串后追加参数无法正确作用到两段。  
rust：`cargo test; ...; cargo llvm-cov ...` — cargo `--config` 是 key=value，与 suite.`config`「配置文件路径」语义不同。

### 拍板倾向（已写入 proposal）

1. **注入方式**：模板占位 `{config_args}`，生成时展开为 `` `${config_flag} ${relPath}` `` 或空串；**禁止**对整段 script 盲 append。
2. **第一版 `config_flag`**：仅 `jest` / `vitest` / `vite-plus` → `"--config"`；`pytest` / `rust` / `go` / `bun` / `node-test` → `null`。
3. **静默忽略不可接受**：suite 声明了 `config` 但框架 `config_flag == null` 时，plan/校验阶段 **失败并提示**（或 schema 级拒绝），不静默丢弃。
4. **pytest 后续**：若支持，用 `config_flag: "-c"` 且链式**每一段**模板都含 `{config_args}`；不纳入本 change 必达。
5. **rust**：不适合同一个 `config` 字段语义；靠 `root`/`cwd` 发现；若将来需要另议 `manifest` 等字段。

## 影响面（实现时）

- `config.schema.ts` / `dev-team-config.schema.json`；schema 目录常量文件
- `normalizeFrameworks`、`isFileExcluded`、`readCoverageThresholds`、mutation 合成、`test-report` overrides
- plan：`directory` = absCwd 相对项目根；script `cd`；`{config_args}` 占位注入（非 append）
- 无 `config_flag` 的框架 + suite.`config` → 显式错误
- stryker `rootPath` / mutate 路径相对 absCwd 的重写
- agent / prompt / specs 中 `test.framework` · `test.overrides` 表述
- 本仓库 `openspec/config.json`：迁入 `tests[]`（G 字段）

## 与历史的关系

```
test.frameworks: []  →  test.framework + overrides  →  tests: [] + 方案 G 条目
```

扁平数组回归；条目用 `root` 锚定，用 `includes`/`excludes` 表达集合，用 schema 默认替代全局默认层。
