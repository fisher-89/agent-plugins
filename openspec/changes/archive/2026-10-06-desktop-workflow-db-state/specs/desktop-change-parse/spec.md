# desktop-change-parse Specification (Delta)

## REMOVED Requirements

### Requirement: 三代结构代际探测

**Reason**: 代际探测（v2 / v1 / v0）的探测对象是 workflow.json 的文件与字段存在性；双向墙裁定下 desktop 不再读 workflow.json，inventory 代际概念整体退役——change 的状态面单源自 workspace 库（db 有记录 = 完整状态面），磁盘目录存在性只承载产物发现（db 缺记录 = 文档形态），不再是代际信号。

**Migration**: 存量 CLI change 目录零迁移零触碰（workflow.json 原样保留在磁盘，desktop 以文档形态展示其产物）；desktop 侧的清单与详情读面改走 db ∪ 磁盘发现语义（desktop-change-queries delta）；`crates/core/workflow/src/parse/`（`detect.rs` / `workflow_file.rs` 及测试）整体删除，`detect_inventory` 消费点随 queries 重写消失。

### Requirement: serde 宽松解析与损坏降级

**Reason**: 宽松解析策略的解析对象是 workflow.json（未知字段忽略、单条损坏降级、核心字段损坏整体降级）；该文件退出 desktop 读面后，解析降级语义无对象。「不可解析（unparsable）」状态随之退役——db 行的 native_model 解码失败另有 `StoreError` 显式错误面，不再是「降级展示 + 警示」的解析层概念。字段形状唯一真理源（TS 侧 `workflow.schema.ts`）与 Rust 侧的对齐约束随双向墙消失。

**Migration**: 前端「workflow.json 无法解析」警示与清单 unparsable 标注一并退役（desktop-change-flow-view delta）；`change_flow_start` 前置校验由「workflow.json 可解析」改为「db 已建档」（desktop-change-orchestration delta）；插件侧 schema 与解析零改动（CLI 工作流照旧）。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/parse/` | **（删除）** | `detect.rs` / `workflow_file.rs` 及测试整体退役；`detect_inventory` / `load_workflow` 导出面随模块删除 |
| `plugins/dev-team/bin/src/schemas/workflow.schema.ts` | 不再被 desktop 参照 | 双向墙下 Rust 侧与其零对齐关系；该文件随插件照旧服务 CLI 工作流，MUST NOT 由本变更修改 |
| 存量 workflow.json | 磁盘惰性残留 | 原样保留零触碰；desktop 不解析、不展示其状态字段 |
