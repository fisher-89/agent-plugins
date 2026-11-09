# 测试文件命名与语法

## 命名规范

测试文件与源文件**同目录**（colocated），按扩展名决定名字：

| 源文件 | 测试文件 |
|---|---|
| `foo.ts` | `foo.test.ts` |
| `foo.tsx` | `foo.test.tsx` |
| `foo.js` | `foo.test.js` |
| `foo.jsx` | `foo.test.jsx` |
| `foo.mts` | `foo.test.mts` |
| `foo.cts` | `foo.test.cts` |

若源文件是 `index.ts` / `index.tsx`，测试文件命名沿用目录语义：放在同目录的 `index.test.ts`。

**不要**把测试文件放到 `__tests__/` 或 `tests/` 目录——除非项目已有测试全部这么组织（此时跟随既有约定，并在阶段 7 简报中说明）。

## 框架语法

### vite-plus / vitest

```ts
import { describe, it, expect, vi } from 'vitest';

describe('模块名', () => {
  it('应在输入为空时返回默认值', () => {
    expect(fn('')).toBe('default');
  });
});
```

- vite-plus 用与 vitest 相同的语法（`vp test` 即 vitest）
- 导入统一从 `'vitest'` 取
- 环境为 jsdom 时可直接操作 DOM

### jest

```ts
import { describe, it, expect, jest } from '@jest/globals';

describe('模块名', () => {
  it('应在输入为空时返回默认值', () => {
    expect(fn('')).toBe('default');
  });
});
```

项目若未配置 `injectGlobals: false`，全局变量可用；但**优先显式导入**，避免与 ESM 配置冲突。

### bun

```ts
import { describe, test, expect } from 'bun:test';
```

bun 用 `test()` 而非 `it()`。

## React 组件

```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Button } from './button';

describe('Button', () => {
  it('应在被点击时触发回调', () => {
    let clicked = false;
    render(<Button onClick={() => { clicked = true; }}>提交</Button>);
    screen.getByText('提交').click();
    expect(clicked).toBe(true);
  });
});
```

- 文件用 `.test.tsx`
- 优先 `@testing-library/react`，避免直接操作内部实现
- 查询用 `screen.getByRole` / `getByText`，避免 `container.querySelector`

## 代码要求

| 要求 | 原因 |
|---|---|
| 静态 `import`，不用 `import()` | 动态导入破坏静态分析与 mock 提升 |
| 不加 `skip` / `only` / TODO 占位 | 生成的测试应立即可执行 |
| 描述用中文 | 与项目既有测试风格一致，人工可读 |
| 标识符、路径、命令保持英文 | — |
| 用项目已有的断言风格 | 不混用 `assert` 与 `expect` |
| mock 用完即还原 | 见 `mock-rules.md` |

## 断言选择

| 场景 | 推荐 |
|---|---|
| 值相等 | `toBe`（原始值）/ `toEqual`（对象数组） |
| 误差 | `toBeCloseTo` |
| 异常 | `toThrow` / `rejects.toThrow` |
| DOM 存在 | `toBeInTheDocument`（配合 jest-dom） |
| 调用次数 | `toHaveBeenCalledTimes` |

避免对实现细节断言（私有变量、内部调用顺序），优先断言**可观察的行为与返回**。
