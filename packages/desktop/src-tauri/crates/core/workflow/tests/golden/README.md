# golden 快照

由 `corpus_golden_test.rs` 生成与消费：每个 fixture 一份"判定 → 解析摘要 →
详情/列表投影"的规范化 JSON（键排序、时间戳 RFC3339），与 TS 侧 zod schema
的漂移在本目录以可读 diff 形式暴露。

重写流程（有意演进 schema 时）：

```text
DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test   # 覆写
cargo test -p workflow --test corpus_golden_test                            # 复核一致
```

本 README 属于 golden 目录固定成员，由语料完整性测试断言其在位。
