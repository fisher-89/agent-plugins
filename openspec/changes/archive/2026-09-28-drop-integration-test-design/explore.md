# 探索笔记: 移除集成测试设计，单层 sociable 单元测试模型

> 日期: 2026-09-28 · 主题: plugins/dev-team test-design 阶段移除集成测试层
> 拟议 change 名: `drop-integration-test-design`

## 动机

现行 test-design 是双层模型: solitary 单元测试(默认 mock 依赖)+ 独立集成测试层(组合真实模块)。
收成单层 **sociable 单元测试**: 用例默认组合进程内真实模块，只 mock 进程边界 —— 即把集成层的
「仅跨进程边界才 mock」原则吸收进单元测试层，然后删掉那一层。

佐证: 现有单元测试的 Mock主体 定义已写明「系统源代码之外的依赖」，最小 mock 原则本就在纸面，
只是无检查强制，且跨模块用例另走了集成通道。

## 现状地图(「集成测试」存量)

```
设计层(test-design phase)
├─ templates/artifacts/test-design.md.template
│   ├─ 验收范围表「测试类型」列: 单元测试/集成测试
│   └─ ## 集成测试 整章(关系标题→测试文件/涉及模块表/关联AC/关系描述/场景/用例/Mock策略)
├─ agents/test-design-planner.md
│   ├─ Step 8「集成测试框架识别」(命名; test_detect_frameworks 调用本身仍需要)
│   ├─ Step 9 识别跨模块交互→生成集成测试章节
│   ├─ Step 13 写入顺序含集成测试
│   └─ 「集成测试关系标题命名指南」整节
├─ agents/test-design-evaluator.md
│   ├─ T2 验收范围↔章节对应(含集成方向)
│   ├─ T5 章节清单含集成测试(可选)
│   └─ T8-I1..I8 集成测试格式检查(整块 8 条)
├─ agents/test-gen-generator.md   Step 2 解析「单元/集成」两类表格
└─ agents/test-gen-evaluator.md   G4(集成用例骨架)、G5、Process step 4

外围残留
├─ templates/test-pytest.py   # === Integration Tests === 骨架
├─ cli.ts:19 / workflow.ts ×3  "unit + integration" 文案
└─ (既有笔误) evaluator T8-G1 写「保留 5 列」却只列 4 个列名(改表时自然消解)

已清干净(历史 change，无需再动)
├─ integration-test 阶段已从 phase 表移除(有测试守卫)
└─ test_resolve_paths 已只返回 unit_tests(2026-07-16-remove-integration-test-path-resolution)
```

## 决策记录

### D1. 组合用例挂靠: 全部 colocate 到链路入口模块的测试文件(方案 A)

- 多模块组合用例 = 链路入口模块 per-file 章节里 `#### 用例` 表的若干行；describe 标题可写链路
  (如 `CLI参数 → workflow.json持久化`)。不设独立章节、不设 `__tests__/` 组合测试区。
- `test_resolve_paths` 的 source→test_file colocated 推导天然覆盖，工具零改动。
- G4(集成用例骨架检查)整条删除后，G3 单表核对自然覆盖组合用例 —— test-gen 全流程不需要
  「组合」这个概念。
- 被测入口模块约定: 链路的发起方/最上层调用方。

### D2. 验收范围表简化

- 删「测试类型」列(不再分类)。
- 「被测文件或模块」列只填承载用例的测试文件(单值)。
- AC 追溯保持在验收范围表的文件级映射(单元用例表本就没有 AC 列，集成章节的 `关联AC` 删除后
  不补替代机制)。

### D3. 最小 mock 原则升格为硬检查

规则:

- **允许 mock**: 进程边界依赖(数据文件/配置/DB/接口/网络/子进程/全局变量/运行环境)。
- **入参例外**: 内部模块本身作为被测 API 的显式入参/注入依赖传入时，允许 mock/替身。理由:
  被测 API 的契约是「如何消费这个输入」，输入方模块自身的正确性由它自己的单元测试保证；
  测试里真实调用输入方模块反而会把两个模块的失败混在一起。
- **禁止 mock**: 其余一切内部模块间调用 —— 被测模块内部 import 并调用的协作模块必须真实组合。

落点:

- test-design-evaluator 新增检查: `Mock策略` 表中 Mock主体 必须是进程边界依赖或「被测 API 显式
  入参的内部模块」;出现模块内部 wiring 的 mock(如 `vi.mock(仓库内模块路径)`)→ fail,应改为
  组合真实模块。
- test-gen-evaluator G5 同步收紧: 生成代码中的模块 mock 声明必须能对应到 Mock策略 表中合法条目。

### D4. 文案清理

- `cli.ts:19` 与 `workflow.ts` ×3 的 "unit + integration" → "all tests"(workflow.ts prompt 文案
  如有测试断言牵连，跟着改)。

### D5. 死模板删除

- `templates/test-jest.js` / `test-pytest.py` / `test-rust.rs` 全仓库零引用 → 直接删除
  (test-gen-generator 只引用 test-design.md.template，框架语法表内嵌在 agent 正文)。

### D6. 存量不动

- `bin/__tests__/**`、`build/__tests__/**` 自称「集成测试」的存量测试文件本次不迁移、不改注释;
  未来新增组合用例按新规则 colocate，两种风格并存，可接受。

## 修改面清单

| 文件 | 动作 |
|---|---|
| `plugins/dev-team/templates/artifacts/test-design.md.template` | 删「测试类型」列与 `## 集成测试` 整章;单元测试章节注释写入最小 mock 原则(含入参例外)与组合用例挂靠规则 |
| `plugins/dev-team/agents/test-design-planner.md` | 删 Step 9 与命名指南;Step 8 改名「测试框架识别」;Step 11 增加组合用例挂靠指引;Step 13 写入顺序去掉集成段;验收范围映射规则同步 D2 |
| `plugins/dev-team/agents/test-design-evaluator.md` | T2/T5 去集成方向;删 T8-I 整块;T8-G1 随表格改写(笔误自然消解);新增最小 mock 检查(D3) |
| `plugins/dev-team/agents/test-gen-generator.md` | Step 2 表格解析去掉「集成测试」引用 |
| `plugins/dev-team/agents/test-gen-evaluator.md` | 删 G4;G5 收紧(D3);Process step 4 去集成对照;后续条目重编号 |
| `plugins/dev-team/templates/test-jest.js` / `test-pytest.py` / `test-rust.rs` | 删除 |
| `plugins/dev-team/bin/src/cli.ts` / `lib/workflow.ts` | "unit + integration" 文案改写;相关测试断言跟随 |
| `plugins/dev-team/package.json` | 版本 bump + `pnpm -C plugins/dev-team run build` 重建产物(项目规则) |

## 遗留问题

无 —— 三项设计点(挂靠/验收范围/mock 检查)与范围边界均已定。唯 D6 的风格并存若日后造成困扰，
可另起迁移 change。
