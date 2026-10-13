# desktop-workspace-store Specification (Delta)

## MODIFIED Requirements

### Requirement: 记录信封 API

store SHALL 暴露只读的通用记录信封 API 穿透模型层类型壁垒：

- `list_models() -> Vec<ModelInfo{ name, count }>`：已注册模型清单与记录计数
- `scan(model, offset, limit) -> RecordsPage{ records: Vec<RecordEnvelope{ key: Value, value: Value }>, total: u64 }`：按模型分页扫描并随页返回分页元数据——`records` 为区间记录信封（`key` / `value` 以 JSON Value 呈现），`total` 为该模型全量计数（与 `records` 同一读事务快照）

native_db 类型 MUST NOT 越信封（信封值经 serde_json 编解码）；API SHALL 只读，MUST NOT 提供写操作。新注册的模型 SHALL 无需改动信封 API 即可被清单与扫描覆盖（查看器零模型特定代码）。

#### Scenario: 新模型零代码可浏览

- **WHEN** store 新增一个模型注册后调用 `list_models` 与 `scan`
- **THEN** 新模型出现在清单（含计数）且可分页扫描，信封 API 与查看器代码均无改动

#### Scenario: 分页扫描

- **WHEN** 对任一模型以 `scan(model, offset, limit)` 分页请求
- **THEN** 返回分页信封：对应区间的记录信封（key/value 为 JSON Value）+ `total` 全量计数（同读事务快照），翻页拼接覆盖全量且不重不漏

#### Scenario: 信封 API 只读

- **WHEN** 审查 store 公共 API 与 db 轨道命令
- **THEN** 信封面仅有 list_models / scan 读操作，无任何写入口

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/envelope.rs` | 记录信封类型与静态注册表 | 新增 `RecordsPage{ records, total }` 分页信封 DTO（与 `ModelInfo` / `RecordEnvelope` 同出口）；`MODEL_ENTRIES`「登记一行接入」机制不变，`RecordsPage` 由通用 `scan_model` 组装、无模型特定字段 |
| `Store::list_models` / `Store::scan`（`crates/infra/store/src/store.rs`） | 通用读面收口 | `scan` 返回 `RecordsPage`（records + total 同一读事务）；`list_models` 契约不变；MAX_SCAN_LIMIT 截断与主键自然序 skip/take 语义不变 |
| `crates/infra/store/src/lib.rs` | 公共导出 | `RecordsPage` 随 `ModelInfo` / `RecordEnvelope` re-export；native_db / native_model 类型不越公共签名（类型壁垒不变） |
