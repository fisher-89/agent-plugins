# Mock 最小原则

只 mock 两类东西。其余一律真实组合。

## 允许 mock 的两类

### 1. 进程边界依赖

跨越当前进程的依赖：

| 类别 | 例子 |
|---|---|
| 文件系统 | `node:fs` 的读写 |
| 网络 | `fetch`、`axios`、`http` |
| 数据库 | 连接、查询 |
| 子进程 | `child_process`、`execa` |
| 时间 | `Date.now()`、`new Date()`、计时器 |
| 随机 | `Math.random()`、`crypto.randomUUID()` |
| 环境 | `process.env`、`import.meta.env` |
| 全局状态 | `window.location`、`localStorage`、`navigator` |

### 2. 被测 API 的显式入参

当依赖通过参数或注入传入时，可传入假实现：

```ts
// 允许：fetcher 是显式参数
runSync(db: Database, fetcher: Fetcher)

// 测试时
runSync(fakeDb, fakeFetcher)
```

## 禁止 mock 的内部模块

同一仓库内的模块之间互相调用**必须真实组合**：

```ts
// 禁止
vi.mock('./utils/parse');

// 允许：真实调用 parse，只对 parse 内部的 IO 做 mock
```

理由：mock 内部模块会把测试变成"验证 mock 是否被调用"，与真实行为脱节。真实组合还能顺带覆盖上游模块。

**跨模块的链路用例**挂靠到链路入口模块的那一节，`describe` 标题写清链路方向（如 `CLI参数 → 配置持久化`）。

## 常见做法

### 文件系统

```ts
import { vi } from 'vitest';

vi.mock('node:fs', () => ({
  readFileSync: vi.fn(() => '{"key":"value"}'),
  existsSync: vi.fn(() => true),
}));
```

优先用真实临时目录（`node:fs` + `os.tmpdir()`）而非 mock——真实文件系统更可信。仅当行为难以用真实文件复现时用 mock。

### 时间

```ts
import { vi, beforeEach, afterEach } from 'vitest';

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date('2026-01-01T00:00:00Z'));
});

afterEach(() => {
  vi.useRealTimers();
});
```

### 网络

```ts
global.fetch = vi.fn(() =>
  Promise.resolve({ ok: true, json: () => Promise.resolve({ data: 1 }) } as Response),
);
```

或用 `msw` 拦截——若项目已依赖 `msw`，跟随既有做法。

### 环境变量

```ts
beforeEach(() => {
  vi.stubEnv('NODE_ENV', 'test');
});

afterEach(() => {
  vi.unstubAllEnvs();
});
```

## 清理

**每个 mock 必须在测试结束后还原**，否则污染后续用例：

| 手段 | 作用 |
|---|---|
| `vi.restoreAllMocks()` | 还原 `vi.spyOn` 创建的 mock |
| `vi.clearAllMocks()` | 清空调用记录（不还原实现） |
| `vi.unstubAllEnvs()` | 还原 `vi.stubEnv` |
| `vi.useRealTimers()` | 还原假计时器 |
| `afterEach` / `afterAll` | 放置上述调用的地方 |

模块级 `vi.mock()` 是提升的、跨文件生效，无需在 `afterEach` 里还原——但它的 `fn` 实现若是共享状态，需在 `beforeEach` 重置。

## React 组件中的 mock

| 对象 | 处置 |
|---|---|
| 子组件 | **不 mock**，真实渲染 |
| 路由 | 用项目已用的测试路由（如 MemoryRouter）；无则可 mock 路由 hook |
| 网络请求 | 按上述网络规则 |
| 计时器 / 动画 | 通常关闭动画或推进假计时器 |

断言优先用可见文本与角色，而非组件内部状态。
