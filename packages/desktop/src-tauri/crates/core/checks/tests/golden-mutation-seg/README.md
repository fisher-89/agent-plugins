# golden 快照（段级 mutation 语料对拍）

每份 golden 对应 `tests/fixtures/mutation-seg/<fixture>/seg-report.json` 的
全链投影（`parse_*_seg` 宽容解析 → 单段计数归并 → 归并块结论：计数七键 +
score / threshold / pass），JSON pretty 规范化。独立于 corpus 八套 golden
（清单一致性两不相扰）。

## 重写开关

与 corpus golden 同一环境变量 `DESKTOP_GOLDEN_REWRITE=1`（两步工作流同款）：

```text
DESKTOP_GOLDEN_REWRITE=1 cargo test -p checks --test corpus_golden_test mutation_seg   # 重写
cargo test -p checks --test corpus_golden_test mutation_seg                            # 复核
```

## 维护约定

- 投影只含确定性字段（duration_ms 为工具报告值——非墙钟）；重写后必须
  review diff（漂移优先怀疑实现漂移）。
- 新增段级语料：目录 + `fixtures/README.md` 段级表行 +
  `MUTATION_SEG_FIXTURES` 常量 + `project_seg_fixture` 路由同步维护。
