# migrate-archi-decide-to-mcp — explore notes

## 设计约束（2026-08-12）

**决定：`archi_decide` 的 Zod input/output 使用判别联合（`z.discriminatedUnion`），判别字段为 `action`。**

- 关闭 proposal 开放项：「Zod 判别联合 vs handler 内按 action 校验」→ **采用判别联合**，不在 handler 里做松散可选字段 + 手写分发校验作为主路径。
- 预期形状（示意）：

```ts
z.discriminatedUnion('action', [
  z.object({ action: z.literal('create'), /* create 必填/可选 */ }),
  z.object({ action: z.literal('list'), /* list 可选 filter */ }),
  z.object({ action: z.literal('update'), /* update 必填 */ }),
])
```

- `project_root`：与其它 MCP 工具一致（可在各分支共有，或外层 intersection；design 定稿）。
- output 同样按 `action` 判别（delta spec 已写 “Output schema | Discriminated by action”）。
- 不要求回溯 proposal：正式需求仍是单工具 + `action`；本条是实现层 schema 选型，进 `design.md` 即可。
