# golden 快照（checks corpus 全链黄金对拍）

每份 golden 对应 `tests/fixtures/corpus/<fixture>` 的全链投影（宽容解析 →
完整性 checklist → 诊断树 → 覆盖聚合），JSON pretty 规范化；TS zod schema
漂移由此表现为可读 diff。损坏样本的投影记录 unparsable 标记，使「宽松解析
悄悄吞数据」在 golden 上可见。

## 重写开关

环境变量 `DESKTOP_GOLDEN_REWRITE=1` 时，投影不再对比而是覆写入仓 golden
（workflow corpus_golden_test 先例同款，env 开关 + 复核两步工作流）：

```text
DESKTOP_GOLDEN_REWRITE=1 cargo test -p checks --test corpus_golden_test   # 重写
cargo test -p checks --test corpus_golden_test                            # 复核：与现 golden 等价
```

## 文件集

`<fixture>.json` × 语料清单八套（见 `tests/fixtures/README.md` corpus 表）+
本 README。文件集与语料集合的一致性由 `语料完整性_golden目录与语料集合一致`
执法。

## 维护约定

- golden 期望值基准为 CLI zod 权威产出形态；重写仅用于有意的 schema 演进，
  重写后必须 review diff（漂移优先怀疑实现漂移）。
- 投影只含确定性字段：时间戳 / 耗时等墙钟字段不入投影。
