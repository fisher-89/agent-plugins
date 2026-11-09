# golden 快照

由 `corpus_golden_test.rs` 生成与消费：每个 fixture 一份「判定 → 解析摘要 →
详情/列表投影」的规范化 JSON（时间戳 RFC3339），写面线面形状的漂移在本目录以
可读 diff 形式暴露。

对比按 JSON 结构语义进行，**不**按字节：

- **对象键序不是契约**——键的书写先后由 `serde_json` 的 Map 实现决定
  （`preserve_order` 特性开关时用 `IndexMap` 走插入序，否则 `BTreeMap` 走字母序；
  该特性经 `oxc_resolver` / tauri 构建期依赖间接开启，随 `cargo test` 的 crate
  组合变化）。调整 DTO 字段声明顺序不会、也不应让 golden 变红。
- **数组元素序是契约**——相位表序、attempt 升序、节点 / 边 emit 序都是业务语义，
  换位即漂移。

重写流程（有意演进 schema 时）：

```text
DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test   # 覆写
cargo test -p workflow --test corpus_golden_test                            # 复核一致
```

重写后须人工 review diff：预期范围内的键级变化才允许进入提交。纯键序重排
（结构性等价）不需要重写——测试对此已免疫。

本 README 属于 golden 目录固定成员，由语料完整性测试断言其在位。
