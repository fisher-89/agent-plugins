# desktop-change-queries Delta

## ADDED Requirements

### Requirement: 详情暴露 attempt 会话槽位

change 详情聚合 SHALL 在 attempt 记录（eval 条目的线面投影）中暴露记录在案的会话槽位：executor / evaluator / decision 三槽位的会话 id，自 workflow.json eval 条目的会话槽位字段直读透出。无槽位字段的条目（旧代际 workflow.json 或缺省落账）SHALL 以 null 呈现三值，MUST NOT 报错、MUST NOT 以 sourceRef 反查等派生手段虚构槽位值。DTO SHALL 沿既定口径（纯 derive、零字段属性），经 bindings 再生成同步前端类型。查询层 SHALL 保持纯读：槽位为直读透出，MUST NOT 在查询层写入或回填 workflow.json。

#### Scenario: 含槽位条目透出会话 id

- **WHEN** 查询某 eval 条目携带 executor / evaluator / decision 会话槽位的 change 详情
- **THEN** 对应 attempt 记录的三个会话 id 与 workflow.json 记录值逐字一致，无派生改写

#### Scenario: 旧条目降级 null

- **WHEN** 查询不含会话槽位字段的既有 change（v2 / v1 历史形状）详情
- **THEN** 全部 attempt 记录的三槽位均为 null，聚合不报错、详情其余区块照常

#### Scenario: bindings 同步

- **WHEN** DTO 增槽位字段后执行 bindings 重导出
- **THEN** 前端生成物含槽位类型，一致性守卫（git diff --exit-code）绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/detail.rs` `AttemptRecord` | 槽位线面投影 | 三槽位会话 id 直读透出；无槽位 → null；纯 derive 零字段属性口径不变 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | 经 export-bindings 再生成；一致性守卫拦截漂移 |
| `workflow::queries` | 纯读边界 | 槽位直读不派生、不回填；纯读语义不变 |
