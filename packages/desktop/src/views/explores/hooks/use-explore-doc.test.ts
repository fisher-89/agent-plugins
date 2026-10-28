import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, driftReports, resetDrift } from '../../../lib/ipc-drift';
import type { ExploreDoc } from '../../../types/dto';
import { useExploreDoc } from './use-explore-doc';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（read_explore / explore_doc_path /
// watch_subscribe / watch_unsubscribe）；Channel mock 为可编程 class（捕获
// onmessage，测试内手动注入 FileWatchEvent 信号模拟后端推送）。防抖窗口用
// vi.useFakeTimers 精确推进 500ms。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  // 与真实 `@tauri-apps/api/core` 的 `Channel` 同构（原型 accessor + 私有字段，
  // 投递读私有字段）：替身把 `onmessage` 放成实例字段时，投递会绕过装配层包裹面
  class ChannelMock {
    #onmessage: (event: unknown) => void = () => {};
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
    get onmessage(): (event: unknown) => void {
      return this.#onmessage;
    }
    set onmessage(handler: (event: unknown) => void) {
      this.#onmessage = handler;
    }
    deliver(event: unknown): void {
      this.#onmessage(event);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture 与 mock 行为
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const NAME = 'api-retry';

function doc(content: string): ExploreDoc {
  return { name: NAME, content };
}

/** Channel mock 实例形状（vi.hoisted 内 class 不外泄类型，取其结构）。 */
interface ChannelLike {
  onmessage: (event: unknown) => void;
  /** 生产同构投递口（读私有字段，不经 `get`） */
  deliver: (event: unknown) => void;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('未创建 watch Channel');
  return instance;
}

let docResult: ExploreDoc | null | string;
/** 布局派生路径：字符串 / `null` 按 fixture resolve；`{ reject }` 形态驱动失败臂 */
let derivedPath: string | null | { reject: string };
let subscribeResult: number | string;

function mockIpc() {
  ChannelMock.instances.length = 0;
  docResult = doc('# 探索笔记\n初稿');
  derivedPath = `${ROOT}\\openspec\\explores\\${NAME}.md`;
  subscribeResult = 7;
  invokeMock.mockImplementation((command: string) => {
    if (command === 'read_explore') {
      return typeof docResult === 'string' ? Promise.reject(docResult) : Promise.resolve(docResult);
    }
    if (command === 'explore_doc_path') {
      // `{ reject }` 形态走失败臂（与 docResult / subscribeResult 的「字符串即 reject」
      // 同手法，因字符串本身是合法派生路径）——供 `resolvePath` 的 Promise reject 臂驱动
      return typeof derivedPath === 'object' && derivedPath !== null
        ? Promise.reject(derivedPath.reject)
        : Promise.resolve(derivedPath);
    }
    if (command === 'watch_subscribe') {
      return typeof subscribeResult === 'string'
        ? Promise.reject(subscribeResult)
        : Promise.resolve(subscribeResult);
    }
    if (command === 'watch_unsubscribe') {
      return Promise.resolve(true);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
});

afterEach(() => {
  vi.useRealTimers();
});

/** 挂载并冲刷微任务：首轮 read_explore 与订阅链（path → subscribe）均就绪。 */
async function mounted(root: string | null = ROOT, name: string | null = NAME) {
  return renderHook((args: [string | null, string | null]) => useExploreDoc(args[0], args[1]), {
    initialProps: [root, name] as [string | null, string | null],
  });
}

/** 渲染完成后冲刷一次微任务（fake timers 下 waitFor 不可用的替代）。 */
async function flush() {
  await act(async () => {});
}

function readCallCount(): number {
  return invokeMock.mock.calls.filter(([name]) => name === 'read_explore').length;
}

function subscribeCall(): { onEvent: unknown; path: unknown } | null {
  const call = invokeMock.mock.calls.find(([name]) => name === 'watch_subscribe');
  return call ? (call[1] as { onEvent: unknown; path: unknown }) : null;
}

describe('useExploreDoc：文档取数与 watch 订阅（AC-7）', () => {
  it('有效 root+name 挂载：read_explore 拉取一次、doc 呈现内容；explore_doc_path 后恰发起一次 watch_subscribe', async () => {
    const { result } = await mounted();
    await flush();

    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
    expect(result.current.error).toBeNull();
    expect(readCallCount()).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith('explore_doc_path', { root: ROOT, name: NAME });
    const subscription = subscribeCall();
    expect(subscription).not.toBeNull();
    expect(subscription?.path).toBe(derivedPath);
    expect(subscription?.onEvent).toBeInstanceOf(ChannelMock);
    expect(invokeMock.mock.calls.filter(([name]) => name === 'watch_subscribe')).toHaveLength(1);
  });

  it('root/name 为 null：不取数、不订阅，loading 复位为 false（AC-8 新话题未选中的空态半）', async () => {
    const { result } = await mounted(ROOT, null);
    await flush();

    expect(readCallCount()).toBe(0);
    expect(subscribeCall()).toBeNull();
    expect(result.current.doc).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });
});

describe('useExploreDoc：信号 500ms trailing 防抖重拉（AC-7）', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('Channel 推入 1 个信号：推进 500ms 后恰重拉一次 read_explore', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();
    expect(readCallCount()).toBe(1);

    act(() => {
      lastChannel().deliver({ path: derivedPath });
    });
    expect(readCallCount()).toBe(1);

    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();
    expect(readCallCount()).toBe(2);
    expect(invokeMock).toHaveBeenLastCalledWith('read_explore', { root: ROOT, name: NAME });
  });

  it('500ms 窗口内推入 3 个信号：合并为恰一次重拉', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    act(() => {
      lastChannel().deliver({ path: derivedPath });
      vi.advanceTimersByTime(200);
      lastChannel().deliver({ path: derivedPath });
      vi.advanceTimersByTime(200);
      lastChannel().deliver({ path: derivedPath });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(2);
  });

  it('信号载荷仅 { path }（无内容字段）仍正常触发重拉；契约破坏注入内容字段也只按信号处理', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    act(() => {
      lastChannel().deliver({ path: derivedPath, content: '不应被读取的字节' });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(2);
    // 通道无内容纪律：预览只经显式拉取更新
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
  });

  it('卸载：以订阅时返回的 subscription_id 发起 watch_unsubscribe，卸载后信号不再引发任何取数', async () => {
    const { result, unmount } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    unmount();

    expect(invokeMock).toHaveBeenCalledWith('watch_unsubscribe', { subscriptionId: 7 });
    expect(invokeMock.mock.calls.filter(([name]) => name === 'watch_unsubscribe')).toHaveLength(1);
    const docBefore = result.current.doc;
    act(() => {
      lastChannel().deliver({ path: derivedPath });
      vi.advanceTimersByTime(1000);
    });
    await flush();
    expect(result.current.doc).toBe(docBefore);
  });
});

describe('useExploreDoc：缺失与失败语义（AC-7/AC-8）', () => {
  it('explore_doc_path 返回 null（穿越名/blank root）：不发起订阅、doc 空态不崩', async () => {
    derivedPath = null;

    const { result } = await mounted();
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(subscribeCall()).toBeNull();
    expect(readCallCount()).toBe(1);
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
    expect(result.current.error).toBeNull();
  });

  it('read_explore reject：error 置位、不崩；watch 订阅仍建立（文档缺失与订阅生命周期解耦，D5）', async () => {
    docResult = 'IPC 断开';

    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toContain('IPC 断开'));

    expect(result.current.doc).toBeNull();
    expect(subscribeCall()).not.toBeNull();
  });

  it('watch_subscribe reject（IPC 失败）：页面不崩、refresh 显式拉取仍可用', async () => {
    subscribeResult = '订阅失败';
    const { result } = await mounted();
    await waitFor(() => expect(result.current.doc).not.toBeNull());

    expect(result.current.error).toBeNull();

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(readCallCount()).toBe(2));
    expect(result.current.doc).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 强化断言（变异补杀）：loading 生命周期、参数变更竞态（取消抑制）、防抖
// 取消、订阅退订时序。
// ---------------------------------------------------------------------------

describe('useExploreDoc：loading 生命周期与订阅时序', () => {
  it('有效 root+name 挂载：同步进入 loading 态，取数完成后复位', async () => {
    const { result } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, NAME] as [string | null, string | null] },
    );

    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.doc).not.toBeNull();
    expect(result.current.error).toBeNull();
  });

  it('订阅在挂载期间保持就绪：不提前退订', async () => {
    const { result } = await mounted();
    await flush();
    expect(result.current.doc).not.toBeNull();

    expect(invokeMock.mock.calls.filter(([name]) => name === 'watch_unsubscribe')).toHaveLength(0);
  });

  it('卸载先于订阅建立：订阅就绪后仍以订阅 id 补发退订', async () => {
    let resolveSubscribe: (id: number) => void = () => {};
    invokeMock.mockImplementation((command: string) => {
      if (command === 'read_explore') return Promise.resolve(doc('# 慢订阅'));
      if (command === 'explore_doc_path') return Promise.resolve(derivedPath);
      if (command === 'watch_subscribe') {
        return new Promise<number>((resolve) => {
          resolveSubscribe = resolve;
        });
      }
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { unmount } = await mounted();
    // 前置：订阅请求已在飞行中（未落定）——扫描链（path → subscribe）经 ipc 后
    // 多跳微任务，须等 `watch_subscribe` 真正发出再卸载，场景才是「卸载先于订阅建立」
    await waitFor(() => expect(subscribeCall()).not.toBeNull());
    unmount();

    await act(async () => {
      resolveSubscribe(21);
    });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('watch_unsubscribe', { subscriptionId: 21 }),
    );
  });
});

describe('useExploreDoc：信号防抖与卸载取消', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it('信号推入后在防抖窗口内卸载：挂起的重拉被取消丢弃（不触发取数）', async () => {
    const { result, unmount } = await mounted();
    await flush();
    expect(readCallCount()).toBe(1);

    act(() => {
      lastChannel().deliver({ path: derivedPath });
    });
    unmount();
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    await flush();

    expect(readCallCount()).toBe(1);
    expect(result.current.doc).not.toBeNull();
  });
});

describe('useExploreDoc：参数变更竞态（取消抑制）', () => {
  it('旧文档迟到的 resolve 不覆盖新文档（cancelled 抑制迟到写入）', async () => {
    let resolveOld: (value: ExploreDoc | null) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'read_explore') {
        if (params?.name === 'old-doc') {
          return new Promise<ExploreDoc | null>((resolve) => {
            resolveOld = resolve;
          });
        }
        return Promise.resolve({ name: 'new-doc', content: '新文档内容' });
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(`${ROOT}\\openspec\\explores\\${String(params?.name)}.md`);
      }
      if (command === 'watch_subscribe') return Promise.resolve(11);
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, 'old-doc'] as [string | null, string | null] },
    );
    await flush();

    rerender([ROOT, 'new-doc'] as [string | null, string | null]);
    await flush();
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '新文档内容' });

    await act(async () => {
      resolveOld({ name: 'old-doc', content: '迟到的旧文档' });
    });
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '新文档内容' });
    expect(result.current.error).toBeNull();
  });

  it('旧文档迟到的 reject 不置错误（cancelled 抑制迟到失败）', async () => {
    let rejectOld: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'read_explore') {
        if (params?.name === 'old-doc') {
          return new Promise<ExploreDoc | null>((_resolve, reject) => {
            rejectOld = reject;
          });
        }
        return Promise.resolve({ name: 'new-doc', content: '新文档内容' });
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(`${ROOT}\\openspec\\explores\\${String(params?.name)}.md`);
      }
      if (command === 'watch_subscribe') return Promise.resolve(11);
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, 'old-doc'] as [string | null, string | null] },
    );
    await flush();

    rerender([ROOT, 'new-doc'] as [string | null, string | null]);
    await flush();

    await act(async () => {
      rejectOld(new Error('迟到的 IPC 失败'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '新文档内容' });
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-5 回归锁定）：read_explore / explore_doc_path /
// watch_subscribe / watch_unsubscribe 裸 invoke → typed bindings 机械替换后，
// 命令名与参数逐字不变；Channel<FileWatchEvent> 底层构造保留（类型来自生成
// 物），信号仅按 { path } 处理（mock 机制切换后依旧生效）。
// ---------------------------------------------------------------------------

describe('useExploreDoc：生成绑定调用面', () => {
  it('四命令经生成绑定入口后命令名与参数逐字不变；watch 信号仅按 { path } 处理（载荷注入仍只按信号处理）', async () => {
    vi.useFakeTimers();
    const { result } = await mounted();
    await flush();

    expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: ROOT, name: NAME });
    expect(invokeMock).toHaveBeenCalledWith('explore_doc_path', { root: ROOT, name: NAME });
    expect(subscribeCall()?.path).toBe(derivedPath);
    expect(subscribeCall()?.onEvent).toBeInstanceOf(ChannelMock);
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));

    // FileWatchEvent 生成类型形态：信号仅 { path }；注入内容字段也只按信号处理
    act(() => {
      lastChannel().deliver({ path: derivedPath, content: '不应被读取的字节' });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();
    expect(readCallCount()).toBe(2);
    expect(invokeMock).toHaveBeenLastCalledWith('read_explore', { root: ROOT, name: NAME });
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
  });

  it('explore_doc_path 返回 null 不订阅、500ms 窗口内多信号合并为恰一次重拉（语义在绑定切换后不变）', async () => {
    vi.useFakeTimers();
    derivedPath = null;
    const noSubscribe = await mounted();
    await flush();

    expect(subscribeCall()).toBeNull();
    expect(readCallCount()).toBe(1);
    expect(noSubscribe.result.current.doc).not.toBeNull();

    // 正常订阅下防抖合并：窗口内两信号合并为一次重拉（第二次信号重置计时）
    derivedPath = `${ROOT}\\openspec\\explores\\${NAME}.md`;
    const { result } = await mounted();
    await flush();
    const readsBefore = readCallCount();
    act(() => {
      lastChannel().deliver({ path: derivedPath });
      vi.advanceTimersByTime(200);
      lastChannel().deliver({ path: derivedPath });
      vi.advanceTimersByTime(200);
      lastChannel().deliver({ path: derivedPath });
      vi.advanceTimersByTime(500);
    });
    await flush();
    expect(readCallCount()).toBe(readsBefore + 1);
    expect(result.current.doc).not.toBeNull();
  });

  it('read_explore / watch_subscribe reject → 错误态与订阅生命周期解耦行为不变', async () => {
    docResult = 'IPC 断开';
    const { result } = await mounted();
    await waitFor(() => expect(result.current.error).toContain('IPC 断开'));

    expect(result.current.doc).toBeNull();
    expect(subscribeCall()).not.toBeNull();

    // 订阅失败不影响文档呈现（错误态解耦）
    docResult = doc('# 探索笔记\n订阅失败对照');
    subscribeResult = '订阅失败';
    const subscribeFailed = await mounted();
    await waitFor(() => expect(subscribeFailed.result.current.doc).not.toBeNull());
    expect(subscribeFailed.result.current.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：四命令参数面放行
// （`string | null` 路径面）、失效信号信封逐条 soft-report 与既有退订幂等。
// ---------------------------------------------------------------------------

describe('useExploreDoc：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('readExplore(root, name) 放行：文档结果干净零漂移、既有重拉链不变', async () => {
    const { result } = await mounted();
    await flush();

    expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: ROOT, name: NAME });
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
    expect(driftCount()).toBe(0);
  });

  it.each([
    ['返回 null（无文档路径）', null],
    ['返回字符串', `${ROOT}openspecexplores\${NAME}.md`],
  ])('exploreDocPath %s → 订阅分支与既有短路语义不变、零漂移', async (_label, path) => {
    derivedPath = path;
    await mounted();
    await flush();

    expect(subscribeCall() !== null).toBe(path !== null);
    expect(driftCount()).toBe(0);
  });

  it('合法失效信号信封投递 → handler 收到原消息（同一引用）、零漂移', async () => {
    vi.useFakeTimers();
    await mounted();
    await flush();

    const signal = { path: derivedPath };
    act(() => {
      lastChannel().deliver(signal);
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(2);
    expect(driftCount()).toBe(0);
    vi.useRealTimers();
  });

  it('脏信封（缺 path）→ 漂移恰 1 条 + 既有 500ms 防抖重拉仍触发（不丢流、不断流）', async () => {
    vi.useFakeTimers();
    await mounted();
    await flush();

    act(() => {
      lastChannel().deliver({ kind: 'changed' });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('watchSubscribe');
    // strictObject 的必填键缺失 → issue 路径是该字段（`#/path`），非根
    expect(driftReports()[0].path).toBe('#/path');
    vi.useRealTimers();
  });

  it('reject 串（readExplore 失败）→ 既有 error 文案逐字不变、零漂移、订阅分支不阻断页面', async () => {
    docResult = '读取失败：EACCES';
    const { result } = await mounted();
    await flush();

    expect(result.current.error).toBe('读取失败：EACCES');
    expect(driftCount()).toBe(0);
  });

  it('卸载 → watchUnsubscribe(subscriptionId) 1 次、零漂移；exploreDocPath 为 null 时零退订调用', async () => {
    const { unmount } = await mounted();
    await flush();
    unmount();
    await flush();

    const unsubscribes = invokeMock.mock.calls.filter(([name]) => name === 'watch_unsubscribe');
    expect(unsubscribes).toHaveLength(1);
    expect(driftCount()).toBe(0);

    invokeMock.mockClear();
    resetDrift();
    derivedPath = null as unknown as string;
    const second = await mounted();
    await flush();
    second.unmount();
    await flush();

    expect(invokeMock.mock.calls.filter(([name]) => name === 'watch_unsubscribe')).toHaveLength(0);
    expect(driftCount()).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// change file-watch-signal-hook：共享文件 watch 信号消费件
// `src/hooks/use-file-watch-signal.ts` 不新增独立测试文件（`no test-only exports`：
// 六个私有臂 armKey:null / resolvePath:null / 订阅 reject / 迟到退订 / 防抖重置 /
// 清理 全部经 `useExploreDoc` 公开面可达）。以下 describe 块只**追加**断言，上方
// 既有 `describe` / `it` / `it.each` 的断言行零改动（纯重构判据）。
// ---------------------------------------------------------------------------

/** 某命令的调用次数（按命令名筛 invoke 面） */
function callCountOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

/** 某命令的调用载荷（按调用序；只取第二实参 payload） */
function payloadsOf(command: string): unknown[] {
  return invokeMock.mock.calls.filter(([name]) => name === command).map(([, payload]) => payload);
}

/** watch 两条命令的调用序（退订先于重订的顺序判据） */
function watchSequence(): string[] {
  return invokeMock.mock.calls
    .map(([name]) => name as string)
    .filter((name) => name === 'watch_subscribe' || name === 'watch_unsubscribe');
}

describe('useFileWatchSignal：经消费方公开面（C1–C5）', () => {
  describe('挂载与接线（C1/C2）', () => {
    it('有效臂键挂载：路径解析恰一次、订阅恰一次且首参为本件构造的 Channel 实例，订阅链不额外取数', async () => {
      await mounted();
      await flush();

      expect(callCountOf('explore_doc_path')).toBe(1);
      expect(invokeMock).toHaveBeenCalledWith('explore_doc_path', { root: ROOT, name: NAME });

      const channels = ChannelMock.instances;
      expect(channels).toHaveLength(1);
      const subscription = subscribeCall();
      expect(subscription).not.toBeNull();
      // C1：订阅首参必须就是本件构造的那个 Channel 实例（不是别处新建的）
      expect(subscription?.onEvent).toBe(channels[0]);
      expect(subscription?.path).toBe(derivedPath);
      expect(callCountOf('watch_subscribe')).toBe(1);
      // 本件零取数：订阅链完成不新增 read_explore
      expect(readCallCount()).toBe(1);
    });

    it('初始化顺序：先 explore_doc_path 后 watch_subscribe，两条命令各一次', async () => {
      await mounted();
      await flush();

      expect(
        invokeMock.mock.calls
          .map(([name]) => name as string)
          .filter((name) => name === 'explore_doc_path' || name === 'watch_subscribe'),
      ).toEqual(['explore_doc_path', 'watch_subscribe']);
    });
  });

  describe('信号载荷形态无关（D5）', () => {
    beforeEach(() => {
      // 脏载荷会命中 `ipc` 的 Channel 信封 soft-report（DEV 汇总告警）：静音只影响本块输出
      vi.spyOn(console, 'warn').mockImplementation(() => {});
    });

    afterEach(() => {
      vi.restoreAllMocks();
    });

    it.each([
      ['undefined（空投递）', undefined],
      ['空对象 {}', {}],
      ['契约破坏形态 { path, content }', { path: '不应被读取的路径', content: '不应被读取的字节' }],
    ])('载荷形态 %s：窗口内均合并为一次重拉，doc 不因载荷变化', async (_label, event) => {
      vi.useFakeTimers();
      const { result } = await mounted();
      await flush();
      expect(readCallCount()).toBe(1);

      act(() => {
        lastChannel().deliver(event);
      });
      act(() => {
        vi.advanceTimersByTime(500);
      });
      await flush();

      expect(readCallCount()).toBe(2);
      expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
    });
  });

  describe('防抖窗口（C3）', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    it('订阅就绪后投递 1 条信号：窗口内零重拉，推进 500ms 恰重拉一次', async () => {
      await mounted();
      await flush();
      expect(readCallCount()).toBe(1);

      act(() => {
        lastChannel().deliver({ path: derivedPath });
      });
      expect(readCallCount()).toBe(1);

      act(() => {
        vi.advanceTimersByTime(500);
      });
      await flush();

      expect(readCallCount()).toBe(2);
      expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: ROOT, name: NAME });
    });

    it('窗口阈值：推进 499ms 零触发、恰好 500ms 触发恰一次', async () => {
      await mounted();
      await flush();

      act(() => {
        lastChannel().deliver({ path: derivedPath });
      });
      act(() => {
        vi.advanceTimersByTime(499);
      });
      await flush();
      expect(readCallCount()).toBe(1);

      act(() => {
        vi.advanceTimersByTime(1);
      });
      await flush();
      expect(readCallCount()).toBe(2);
    });

    it('重置语义边界：第 400ms 再投一条 → 899ms 零触发、900ms 触发恰一次', async () => {
      await mounted();
      await flush();

      act(() => {
        lastChannel().deliver({ path: derivedPath });
      });
      act(() => {
        vi.advanceTimersByTime(400);
      });
      act(() => {
        lastChannel().deliver({ path: derivedPath });
      });
      act(() => {
        vi.advanceTimersByTime(499);
      });
      await flush();
      expect(readCallCount()).toBe(1);

      act(() => {
        vi.advanceTimersByTime(1);
      });
      await flush();
      expect(readCallCount()).toBe(2);
    });
  });

  describe('失败静默与退订外壳（C4/C5）', () => {
    it('watch_subscribe reject：无信号投递时推进 1000ms 不产生重拉（失败链自身不触发信号）', async () => {
      vi.useFakeTimers();
      subscribeResult = '订阅失败';
      const { result } = await mounted();
      await flush();
      expect(readCallCount()).toBe(1);

      act(() => {
        vi.advanceTimersByTime(1000);
      });
      await flush();

      expect(readCallCount()).toBe(1);
      expect(result.current.error).toBeNull();
      expect(result.current.doc).not.toBeNull();
    });

    it('路径解析 reject：零订阅、零退订、静默（doc 正常呈现、error 为 null、loading 复位）', async () => {
      derivedPath = { reject: '布局派生失败' };
      const { result, unmount } = await mounted();
      await flush();

      // 臂键非 null ⇒ Channel 仍建立（失败只掐断「路径 → 订阅」这一段）
      expect(ChannelMock.instances).toHaveLength(1);
      expect(callCountOf('watch_subscribe')).toBe(0);
      expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
      expect(result.current.error).toBeNull();
      expect(result.current.loading).toBe(false);

      unmount();
      await flush();
      // subscriptionId 恒 null ⇒ 卸载零退订
      expect(callCountOf('watch_unsubscribe')).toBe(0);
    });

    it('订阅 reject 后卸载：subscription_id 从未写入 ⇒ 零退订', async () => {
      subscribeResult = '订阅失败';
      const { unmount } = await mounted();
      await flush();

      unmount();
      await flush();

      expect(callCountOf('watch_unsubscribe')).toBe(0);
    });

    it('对照：订阅成功 1 次后卸载 ⇒ 恰一次退订（同一卸载驱动两臂对照）', async () => {
      const { unmount } = await mounted();
      await flush();
      expect(callCountOf('watch_subscribe')).toBe(1);

      unmount();
      await flush();

      expect(payloadsOf('watch_unsubscribe')).toEqual([{ subscriptionId: 7 }]);
    });
  });

  describe('臂键与大小边界', () => {
    it.each([
      ['name 为 null', ROOT, null],
      ['root 为 null', null, NAME],
      ['root 为空串', '', NAME],
      ['name 为空串', ROOT, ''],
    ])(
      '臂键为 null（%s）：零 Channel 构造、零 watch 命令、零取数（含卸载后）',
      async (_label, root, name) => {
        const { unmount } = await mounted(root, name);
        await flush();

        expect(ChannelMock.instances).toHaveLength(0);
        expect(callCountOf('watch_subscribe')).toBe(0);
        expect(callCountOf('watch_unsubscribe')).toBe(0);
        expect(readCallCount()).toBe(0);

        unmount();
        await flush();
        expect(callCountOf('watch_unsubscribe')).toBe(0);
      },
    );

    it('超长臂键（root + name 合计 > 1000 字符）：仍单次订阅且入参逐字不截断', async () => {
      const longRoot = `C:\\demo\\${'很长目录名'.repeat(120)}`;
      const longName = `note-${'x'.repeat(600)}`;
      derivedPath = `${longRoot}\\openspec\\explores\\${longName}.md`;
      expect(longRoot.length + longName.length).toBeGreaterThan(1000);

      await mounted(longRoot, longName);
      await flush();

      expect(invokeMock).toHaveBeenCalledWith('explore_doc_path', {
        root: longRoot,
        name: longName,
      });
      expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: longRoot, name: longName });
      expect(callCountOf('watch_subscribe')).toBe(1);
      expect(subscribeCall()?.path).toBe(derivedPath);
      expect(callCountOf('watch_unsubscribe')).toBe(0);
    });

    it('同臂键连续重渲染（回调身份每次都是新函数）：零重订，信号仍由最新引用触发一次重拉', async () => {
      vi.useFakeTimers();
      const { rerender } = await mounted();
      await flush();

      rerender([ROOT, NAME]);
      rerender([ROOT, NAME]);
      await flush();

      expect(ChannelMock.instances).toHaveLength(1);
      expect(callCountOf('watch_subscribe')).toBe(1);
      expect(callCountOf('watch_unsubscribe')).toBe(0);

      act(() => {
        lastChannel().deliver({ path: derivedPath });
      });
      act(() => {
        vi.advanceTimersByTime(500);
      });
      await flush();

      expect(readCallCount()).toBe(2);
    });

    it('零副作用面：订阅 → 信号 → 卸载全序列中 console.warn / console.error 零调用', async () => {
      vi.useFakeTimers();
      const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
      const error = vi.spyOn(console, 'error').mockImplementation(() => {});
      resetDrift();

      const { unmount } = await mounted();
      await flush();
      act(() => {
        lastChannel().deliver({ path: derivedPath });
      });
      act(() => {
        vi.advanceTimersByTime(500);
      });
      await flush();
      unmount();
      await flush();

      expect(warn).not.toHaveBeenCalled();
      expect(error).not.toHaveBeenCalled();
      expect(readCallCount()).toBe(2);
      vi.restoreAllMocks();
    });
  });
});

describe('useExploreDoc：臂键与重订边界（M1）', () => {
  it('显式 refresh 只重取数不重订订阅：watch_subscribe 恒 1、watch_unsubscribe 恒 0', async () => {
    const { result } = await mounted();
    await flush();
    expect(readCallCount()).toBe(1);

    act(() => {
      result.current.refresh();
    });
    await flush();

    expect(readCallCount()).toBe(2);
    expect(ChannelMock.instances).toHaveLength(1);
    expect(callCountOf('watch_subscribe')).toBe(1);
    expect(callCountOf('watch_unsubscribe')).toBe(0);
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
  });

  it('refresh 之后投递信号：500ms 后仍自动重拉（刷新未杀掉订阅）', async () => {
    vi.useFakeTimers();
    const { result } = await mounted();
    await flush();
    expect(readCallCount()).toBe(1);

    act(() => {
      result.current.refresh();
    });
    await flush();
    expect(readCallCount()).toBe(2);

    act(() => {
      lastChannel().deliver({ path: derivedPath });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(3);
    expect(callCountOf('watch_subscribe')).toBe(1);
    expect(callCountOf('watch_unsubscribe')).toBe(0);
  });
});

describe('useExploreDoc：信号重拉与状态面（AC-4）', () => {
  it('信号触发的重拉全程不置 loading（外部保存不引起预览 loading 闪动）', async () => {
    vi.useFakeTimers();
    const { result } = await mounted();
    await flush();
    expect(result.current.loading).toBe(false);

    act(() => {
      lastChannel().deliver({ path: derivedPath });
    });
    expect(result.current.loading).toBe(false);

    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(2);
    expect(result.current.loading).toBe(false);
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
  });

  it('信号重拉不清 error：先失败的文档经信号重拉成功后 error 保持原串', async () => {
    vi.useFakeTimers();
    docResult = '读取失败：EACCES';
    const { result } = await mounted();
    await flush();
    expect(result.current.error).toBe('读取失败：EACCES');
    expect(result.current.doc).toBeNull();

    docResult = doc('# 修复后的内容');
    act(() => {
      lastChannel().deliver({ path: derivedPath });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(2);
    expect(result.current.doc).toEqual(doc('# 修复后的内容'));
    expect(result.current.error).toBe('读取失败：EACCES');
    expect(result.current.loading).toBe(false);
  });

  it('M2：窗口内点刷新与待发信号各拉一次（挂载 1 + 刷新 1 + 信号 1），结果幂等、loading / error 不变', async () => {
    vi.useFakeTimers();
    const { result } = await mounted();
    await flush();
    expect(readCallCount()).toBe(1);

    act(() => {
      lastChannel().deliver({ path: derivedPath });
    });
    act(() => {
      result.current.refresh();
    });
    await flush();
    expect(readCallCount()).toBe(2);

    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    expect(readCallCount()).toBe(3);
    expect(result.current.doc).toEqual(doc('# 探索笔记\n初稿'));
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });
});

describe('useExploreDoc：订阅身份与有参 ↔ 空参平移', () => {
  it('订阅身份含 name：name 变更 ⇒ 旧 id 退订一次、新增一次订阅、路径以新 name 派生、doc 为新文档', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'read_explore') {
        return Promise.resolve({
          name: String(params?.name),
          content: `内容-${String(params?.name)}`,
        });
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(`${ROOT}\\openspec\\explores\\${String(params?.name)}.md`);
      }
      if (command === 'watch_subscribe') return Promise.resolve(11);
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, 'old-doc'] as [string | null, string | null] },
    );
    await flush();
    expect(result.current.doc).toEqual({ name: 'old-doc', content: '内容-old-doc' });

    rerender([ROOT, 'new-doc'] as [string | null, string | null]);
    await flush();

    expect(payloadsOf('explore_doc_path')).toEqual([
      { root: ROOT, name: 'old-doc' },
      { root: ROOT, name: 'new-doc' },
    ]);
    expect(callCountOf('watch_subscribe')).toBe(2);
    expect(payloadsOf('watch_unsubscribe')).toEqual([{ subscriptionId: 11 }]);
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '内容-new-doc' });
  });

  it('订阅身份含 root：切根 ⇒ 旧 id 退订一次 + 新根重订；随后卸载只以新 id 退订一次', async () => {
    const newRoot = 'C:\\demo\\beta';
    let nextId = 11;
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'read_explore') {
        return Promise.resolve({
          name: String(params?.name),
          content: `根-${String(params?.root)}`,
        });
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(
          `${String(params?.root)}\\openspec\\explores\\${String(params?.name)}.md`,
        );
      }
      if (command === 'watch_subscribe') {
        const id = nextId;
        nextId += 1;
        return Promise.resolve(id);
      }
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { rerender, unmount } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, NAME] as [string | null, string | null] },
    );
    await flush();
    expect(payloadsOf('explore_doc_path')).toEqual([{ root: ROOT, name: NAME }]);

    rerender([newRoot, NAME] as [string | null, string | null]);
    await flush();

    expect(payloadsOf('explore_doc_path')).toEqual([
      { root: ROOT, name: NAME },
      { root: newRoot, name: NAME },
    ]);
    // 旧臂先退订、新臂再订阅（React cleanup 先于新 effect）
    expect(watchSequence()).toEqual(['watch_subscribe', 'watch_unsubscribe', 'watch_subscribe']);
    expect(payloadsOf('watch_unsubscribe')).toEqual([{ subscriptionId: 11 }]);

    unmount();
    await flush();
    expect(payloadsOf('watch_unsubscribe')).toEqual([
      { subscriptionId: 11 },
      { subscriptionId: 12 },
    ]);
  });

  it('臂键变更与待发计时交叉：信号在飞时切 name ⇒ 旧臂计时随旧臂键清理，重拉只来自新臂挂载取数', async () => {
    vi.useFakeTimers();
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command === 'read_explore') {
        return Promise.resolve({
          name: String(params?.name),
          content: `内容-${String(params?.name)}`,
        });
      }
      if (command === 'explore_doc_path') {
        return Promise.resolve(`${ROOT}\\openspec\\explores\\${String(params?.name)}.md`);
      }
      if (command === 'watch_subscribe') return Promise.resolve(11);
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, 'old-doc'] as [string | null, string | null] },
    );
    await flush();
    expect(readCallCount()).toBe(1);

    act(() => {
      lastChannel().deliver({ path: `${ROOT}\\openspec\\explores\\old-doc.md` });
    });
    act(() => {
      vi.advanceTimersByTime(200);
    });

    rerender([ROOT, 'new-doc'] as [string | null, string | null]);
    await flush();
    expect(readCallCount()).toBe(2);

    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();

    // 旧臂的待发信号已随旧臂键清理，不产生第三次取数
    expect(readCallCount()).toBe(2);
    expect(result.current.doc).toEqual({ name: 'new-doc', content: '内容-new-doc' });
  });

  it('有参 → 空参（root 变 null）：旧订阅退订一次、零新订阅，doc / error 复位为 null、loading 为 false', async () => {
    const { result, rerender } = renderHook(
      (args: [string | null, string | null]) => useExploreDoc(args[0], args[1]),
      { initialProps: [ROOT, NAME] as [string | null, string | null] },
    );
    await flush();
    expect(result.current.doc).not.toBeNull();

    rerender([null, NAME] as [string | null, string | null]);
    await flush();

    expect(payloadsOf('watch_unsubscribe')).toEqual([{ subscriptionId: 7 }]);
    expect(callCountOf('watch_subscribe')).toBe(1);
    expect(ChannelMock.instances).toHaveLength(1);
    expect(result.current.doc).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it.each([
    ['root 为空串', '', NAME],
    ['name 为空串', ROOT, ''],
  ])(
    '空串参数（%s）：臂键为 null ⇒ 零 Channel、零 watch 命令、零取数',
    async (_label, root, name) => {
      await mounted(root, name);
      await flush();

      expect(ChannelMock.instances).toHaveLength(0);
      expect(callCountOf('watch_subscribe')).toBe(0);
      expect(callCountOf('watch_unsubscribe')).toBe(0);
      expect(readCallCount()).toBe(0);
    },
  );

  it('刷新后旧取数实例被作废：迟到信号只由当前实例发起一次重拉，旧实例结果不覆盖 doc', async () => {
    vi.useFakeTimers();
    let resolveOld: (value: ExploreDoc | null) => void = () => {};
    let reads = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'read_explore') {
        reads += 1;
        if (reads === 1) {
          return new Promise<ExploreDoc | null>((resolve) => {
            resolveOld = resolve;
          });
        }
        return Promise.resolve({ name: NAME, content: `第 ${reads} 次读取` });
      }
      if (command === 'explore_doc_path') return Promise.resolve(derivedPath);
      if (command === 'watch_subscribe') return Promise.resolve(7);
      if (command === 'watch_unsubscribe') return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result } = await mounted();
    await flush();
    expect(result.current.doc).toBeNull();

    act(() => {
      result.current.refresh();
    });
    await flush();
    expect(result.current.doc).toEqual({ name: NAME, content: '第 2 次读取' });

    act(() => {
      lastChannel().deliver({ path: derivedPath });
    });
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await flush();
    expect(readCallCount()).toBe(3);
    expect(result.current.doc).toEqual({ name: NAME, content: '第 3 次读取' });

    await act(async () => {
      resolveOld({ name: NAME, content: '迟到的旧结果' });
    });

    expect(result.current.doc).toEqual({ name: NAME, content: '第 3 次读取' });
    expect(result.current.error).toBeNull();
  });
});
