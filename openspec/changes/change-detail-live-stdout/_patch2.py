import io

p = 'openspec/changes/change-detail-live-stdout/tasks.md'
s = io.open(p, encoding='utf-8').read()

# 1) 本轮新增的三项实现任务尚未落地 ⇒ 保持未勾选（其余既有任务由上一轮 implement 勾选）
pairs = [
    ('- [x] 新增 `packages/desktop/src/components/ui/code-block-copy-button.tsx`（D19）',
     '- [ ] 新增 `packages/desktop/src/components/ui/code-block-copy-button.tsx`（D19）'),
    ('- [x] 修改 `packages/desktop/src/components/ui/code-block.tsx`（D19 / D19-a）',
     '- [ ] 修改 `packages/desktop/src/components/ui/code-block.tsx`（D19 / D19-a）'),
    ('- [x] **覆盖率复算自查（D19 的机械判据；实际执行由桌面端 test-execution 相位承接）**',
     '- [ ] **覆盖率复算自查（D19 的机械判据；实际执行由桌面端 test-execution 相位承接）**'),
]
for old, new in pairs:
    assert old in s, old[:40]
    s = s.replace(old, new)

# 2) D19-a 的 import 源切换单列一条未勾选任务（既有「结构三段」条目已勾选，不再回改）
anchor = '- [ ] 新增 `packages/desktop/src/views/changes/flow/stdout-text.ts`'
assert anchor in s
added = ("- [ ] 切换 `packages/desktop/src/views/changes/flow/stdout-panel.tsx` 的复制按钮 import 源（D19-a）："
         "`import { CodeBlockCopyButton } from '@/components/ui/code-block';` → "
         "`import { CodeBlockCopyButton } from '@/components/ui/code-block-copy-button';`。"
         "**MUST NOT** 从 `@/components/ui/code-block` barrel import（该 barrel 顶部的 `highlightCode` / "
         "`useHighlightedCode` 两条静态再导出会把 shiki 高亮器树带进 import 图与覆盖率分母）；其余 import 与组件体不动\n")
s = s.replace(anchor, added + anchor, 1)

io.open(p, 'w', encoding='utf-8').write(s)
print('ok')
