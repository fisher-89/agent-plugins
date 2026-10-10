import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { RunNotice } from '../../../types/generated/bindings';
import { useChangeFlowRun } from './use-change-flow-run';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（四命令 start / stop / confirm / answer +
// watch 补订，可切换 resolve / reject 并记录入参）；Channel mock 为可编程
// class（捕获 onmessage，测试直接投递 RunNotice kind-only 通知驱动 onNotice
// 分流断言）。组件态无状态归并（查询缓存 + 失效重取归 use-change-detail 承
// 接——两钩并一后的缩位契约）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式，绑定类型为事实源）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\flow';
const CHANGE_ID = 'add-feature';

type NoticeKind = RunNotice['ipc'];

// ---------------------------------------------------------------------------
// 可编程 IPC：start 可切 reject（字符串 = reject 文本）
// ---------------------------------------------------------------------------

let startResult: string | Error | { code: number } | null;

function mockIpc() {
  ChannelMock.instances.length = 0;
  startResult = null;
  invokeMock.mockImplementation((command: string) => {
    if (command === 'change_flow_start') {
      return startResult === null
        ? Promise.resolve({ runId: 'run-1727', status: 'running' })
        : Promise.reject(startResult);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
});

interface Params {
  root: string | null;
  id: string | null;
  activeRunPresent: boolean;
}

/** Channel mock 实例形状（vi.hoisted 内 class 不外泄类型，取其结构）。 */
interface ChannelLike {
  onmessage: ((event: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('订阅未建立：无 Channel 实例');
  return instance;
}

/** 投递 kind-only 通知（经最新 Channel 实例）。 */
function deliver(notice: RunNotice): void {
  act(() => {
    lastChannel().onmessage?.(notice);
  });
}

function calls(command: string): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === command);
}

function startCallArgs(): Record<string, unknown> {
  const call = calls('change_flow_start').at(-1);
  if (!call) throw new Error('change_flow_start 未被调用');
  return call[1] as Record<string, unknown>;
}

interface HookParams extends Params {
  onNotice: (kind: NoticeKind) => void;
}

function noopNotice(): (kind: NoticeKind) => void {
  return () => {};
}

async function mounted(params: Params = { root: ROOT, id: CHANGE_ID, activeRunPresent: false }) {
  const rendered = renderHook((input: HookParams) => useChangeFlowRun(input), {
    initialProps: { ...params, onNotice: noopNotice() },
  });
  await act(async () => {});
  return rendered;
}

describe('useChangeFlowRun：通知订阅生命周期（activeRun 在场才补订）', () => {
  it('activeRunPresent=false（无运行 run 的常态路径）：零 watch 零 Channel（惰性构造）', async () => {
    await mounted();

    expect(calls('change_flow_watch')).toHaveLength(0);
    expect(ChannelMock.instances).toHaveLength(0);
  });

  it('activeRunPresent=true：change_flow_watch 补订（Channel 实例随行；onmessage 分流回调接 onNotice）', async () => {
    const onNotice = vi.fn();
    const { result } = renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      { initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: true, onNotice } },
    );
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));

    const watchArgs = calls('change_flow_watch')[0][1] as Record<string, unknown>;
    expect(watchArgs.root).toBe(ROOT);
    expect(watchArgs.id).toBe(CHANGE_ID);
    expect(watchArgs.onEvent).toBeInstanceOf(ChannelMock);
    expect(typeof result.current.start).toBe('function');

    // kind-only 通知到达 → onNotice 分流回调（sessionEvent / 其余分流面）
    deliver({ ipc: 'sessionEvent' });
    deliver({ ipc: 'step' });
    expect(onNotice).toHaveBeenCalledTimes(2);
    expect(onNotice).toHaveBeenNthCalledWith(1, 'sessionEvent');
    expect(onNotice).toHaveBeenNthCalledWith(2, 'step');
  });

  it('activeRunPresent true→false rerender（终态除名）：cleanup 弃投递、不重复补订', async () => {
    const onNotice = vi.fn();
    const rendered = renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      { initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: true, onNotice } },
    );
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));
    const channel = ChannelMock.instances[0];

    rendered.rerender({ root: ROOT, id: CHANGE_ID, activeRunPresent: false, onNotice });
    await act(async () => {});
    expect(calls('change_flow_watch')).toHaveLength(1);

    // 通知到达即落空（onmessage 置空后投递不再回流）
    channel.onmessage = null;
    expect(onNotice).not.toHaveBeenCalled();
  });

  it('root / id 为 null：零 invoke、零 Channel（未选定 workspace 空闲态）', async () => {
    const { result } = await mounted({ root: null, id: null, activeRunPresent: true });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(ChannelMock.instances).toHaveLength(0);
    expect(result.current.error).toBeNull();
  });

  it('卸载后重挂：订阅引用释放（全新 Channel 实例）、change_flow_watch 重新补订', async () => {
    const onNotice = vi.fn();
    const first = renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      { initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: true, onNotice } },
    );
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));
    const firstChannel = ChannelMock.instances[0];
    first.unmount();

    renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      {
        initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: true, onNotice },
      },
    );
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(2));

    expect(ChannelMock.instances).toHaveLength(2);
    expect(ChannelMock.instances[1]).not.toBe(firstChannel);
  });
});

describe('useChangeFlowRun：start 发起与订阅（发起先行订阅再 invoke）', () => {
  it('start(false) → invoke change_flow_start 携 root/id 与新 Channel 实例（提前 resolve 后通知即刻有落点）', async () => {
    const { result } = await mounted();
    expect(ChannelMock.instances).toHaveLength(0);

    await act(async () => {
      await result.current.start(false);
    });

    expect(calls('change_flow_start')).toHaveLength(1);
    const args = startCallArgs();
    expect(args.root).toBe(ROOT);
    expect(args.id).toBe(CHANGE_ID);
    expect(args.onEvent).toBeInstanceOf(ChannelMock);
  });

  it('activeRun 在场已建订阅时 start 复用既有 Channel（不重挂、双通道不叠加）', async () => {
    const onNotice = vi.fn();
    const rendered = renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      { initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: true, onNotice } },
    );
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));
    expect(ChannelMock.instances).toHaveLength(1);

    await act(async () => {
      await rendered.result.current.start(false);
    });

    expect(ChannelMock.instances).toHaveLength(1);
    expect(startCallArgs().onEvent).toBe(ChannelMock.instances[0]);
  });

  it('发起侧所建 Channel 通知回流：onNotice 分流真实可达（非恢复侧代打）', async () => {
    const onNotice = vi.fn();
    const { result } = renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      { initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: false, onNotice } },
    );
    await act(async () => {
      await result.current.start(false);
    });
    expect(ChannelMock.instances).toHaveLength(1);

    deliver({ ipc: 'confirmWait' });
    expect(onNotice).toHaveBeenCalledWith('confirmWait');
  });
});

describe('useChangeFlowRun：start autoNextPhase 传参（停等节奏发起定格，D4）', () => {
  it('start(true) → invoke 携 autoNextPhase: true（经真实 bindings 生成物出线，commands 模块不 mock）', async () => {
    const { result } = await mounted();

    await act(async () => {
      await result.current.start(true);
    });

    expect(calls('change_flow_start')).toHaveLength(1);
    expect(startCallArgs().autoNextPhase).toBe(true);
    expect(startCallArgs().root).toBe(ROOT);
    expect(startCallArgs().id).toBe(CHANGE_ID);
  });

  it('start(false) → autoNextPhase: false 显式出线（默认档亦为显式实参——签名必填无缺省，载荷键在场）', async () => {
    const { result } = await mounted();

    await act(async () => {
      await result.current.start(false);
    });

    expect(calls('change_flow_start')).toHaveLength(1);
    expect(startCallArgs().autoNextPhase).toBe(false);
    expect(Object.keys(startCallArgs())).toContain('autoNextPhase');
  });

  it('root / id 为 null 时 start(true / false) 均 no-op 零 invoke（守卫先行于传参）', async () => {
    const { result } = await mounted({ root: null, id: null, activeRunPresent: false });

    await act(async () => {
      await result.current.start(true);
    });
    await act(async () => {
      await result.current.start(false);
    });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
  });
});

describe('useChangeFlowRun：stop / confirm / answer 透传', () => {
  it('stop() → invoke change_flow_stop 携 { root, id } 原样透传', async () => {
    const { result } = await mounted();
    await act(async () => {
      await result.current.stop();
    });

    expect(calls('change_flow_stop')).toHaveLength(1);
    expect(calls('change_flow_stop')[0]).toEqual([
      'change_flow_stop',
      { root: ROOT, id: CHANGE_ID },
    ]);
  });

  it('confirm(true / false) → invoke change_flow_confirm 携 proceed 原样透传（两次调用各自成形）', async () => {
    const { result } = await mounted();
    await act(async () => {
      await result.current.confirm(true);
    });
    await act(async () => {
      await result.current.confirm(false);
    });

    const confirmCalls = calls('change_flow_confirm');
    expect(confirmCalls).toHaveLength(2);
    expect(confirmCalls[0]).toEqual([
      'change_flow_confirm',
      { root: ROOT, id: CHANGE_ID, proceed: true },
    ]);
    expect(confirmCalls[1]).toEqual([
      'change_flow_confirm',
      { root: ROOT, id: CHANGE_ID, proceed: false },
    ]);
  });

  it('answer(text) → invoke change_flow_answer 携 answer 原样透传（ask 应答回流）', async () => {
    const { result } = await mounted();
    await act(async () => {
      await result.current.answer('继续推进 implement');
    });

    expect(calls('change_flow_answer')).toHaveLength(1);
    expect(calls('change_flow_answer')[0]).toEqual([
      'change_flow_answer',
      { root: ROOT, id: CHANGE_ID, answer: '继续推进 implement' },
    ]);
  });

  it('root / id 为 null（双 null 与混合 null 半边）时四操作均 no-op（零 invoke 零 Channel）', async () => {
    for (const params of [
      { root: null, id: null, activeRunPresent: false },
      { root: ROOT, id: null, activeRunPresent: false },
      { root: null, id: CHANGE_ID, activeRunPresent: false },
    ] as Params[]) {
      invokeMock.mockClear();
      ChannelMock.instances.length = 0;
      const { result } = await mounted(params);
      await act(async () => {
        await result.current.start(false);
      });
      await act(async () => {
        await result.current.stop();
      });
      await act(async () => {
        await result.current.confirm(true);
      });
      await act(async () => {
        await result.current.answer('文本');
      });

      expect(invokeMock).not.toHaveBeenCalled();
      expect(ChannelMock.instances).toHaveLength(0);
      expect(result.current.error).toBeNull();
    }
  });
});

describe('useChangeFlowRun：参数变更 rerender（useCallback 依赖数组真实生效）', () => {
  it('换根 rerender 后四操作以新 root 出线（旧闭包不滞留）', async () => {
    const ROOT2 = 'C:\\demo\\flow-2';
    const rendered = await mounted();
    rendered.rerender({
      root: ROOT2,
      id: CHANGE_ID,
      activeRunPresent: false,
      onNotice: noopNotice(),
    });
    await act(async () => {});

    await act(async () => {
      await rendered.result.current.start(false);
    });
    await act(async () => {
      await rendered.result.current.stop();
    });
    await act(async () => {
      await rendered.result.current.confirm(true);
    });
    await act(async () => {
      await rendered.result.current.answer('换根后应答');
    });

    expect(startCallArgs().root).toBe(ROOT2);
    expect(calls('change_flow_stop').at(-1)).toEqual([
      'change_flow_stop',
      { root: ROOT2, id: CHANGE_ID },
    ]);
    expect(calls('change_flow_confirm').at(-1)).toEqual([
      'change_flow_confirm',
      { root: ROOT2, id: CHANGE_ID, proceed: true },
    ]);
    expect(calls('change_flow_answer').at(-1)).toEqual([
      'change_flow_answer',
      { root: ROOT2, id: CHANGE_ID, answer: '换根后应答' },
    ]);
  });
});

describe('useChangeFlowRun：invoke reject（异常面）', () => {
  it('start reject → error 呈现可重试：重试成功后 error 复位、订阅照常建立（Channel 复用不叠加）', async () => {
    const { result } = await mounted();

    startResult = '同 change 已有并行 run';
    await act(async () => {
      await result.current.start(false);
    });
    expect(result.current.error).toBe('同 change 已有并行 run');
    // Channel 惰性构造先于 invoke：reject 时订阅已建立，重试复用同一实例
    expect(ChannelMock.instances).toHaveLength(1);

    startResult = null;
    await act(async () => {
      await result.current.start(false);
    });
    expect(result.current.error).toBeNull();
    expect(calls('change_flow_start')).toHaveLength(2);
    expect(ChannelMock.instances).toHaveLength(1);
    expect(startCallArgs().onEvent).toBe(ChannelMock.instances[0]);
  });

  it('start reject 携非 string（Error 对象 / 普通对象）：error 归一为 String(cause) 文本（readError 非字符串分支真实参与）', async () => {
    const { result } = await mounted();

    startResult = new Error('db: 并行 run 冲突');
    await act(async () => {
      await result.current.start(false);
    });
    expect(result.current.error).toBe('Error: db: 并行 run 冲突');

    startResult = { code: 42 };
    await act(async () => {
      await result.current.start(false);
    });
    expect(result.current.error).toBe('[object Object]');
  });
});

describe('useChangeFlowRun：返回形状消费面', () => {
  it('UseChangeFlowRunResult 键集恰为五键且形状不回退（start/stop/confirm/answer/error——state 随两钩并一退役）', async () => {
    const { result } = await mounted();

    expect(Object.keys(result.current).sort()).toEqual(
      ['answer', 'confirm', 'error', 'start', 'stop'].sort(),
    );
    expect(typeof result.current.start).toBe('function');
    expect(typeof result.current.stop).toBe('function');
    expect(typeof result.current.confirm).toBe('function');
    expect(typeof result.current.answer).toBe('function');
    expect(result.current.error === null || typeof result.current.error === 'string').toBe(true);
  });
});

describe('useChangeFlowRun：onNotice 分流五 kind（RunNotice kind-only 全集）', () => {
  it('投递 step / sessionEvent / ask / confirmWait / finished 五种通知各自回调一拍（AC-8 kind 位）', async () => {
    const onNotice = vi.fn();
    renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      { initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: true, onNotice } },
    );
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));

    deliver({ ipc: 'step' });
    deliver({ ipc: 'sessionEvent' });
    deliver({ ipc: 'ask' });
    deliver({ ipc: 'confirmWait' });
    deliver({ ipc: 'finished' });

    expect(onNotice).toHaveBeenCalledTimes(5);
    expect(onNotice).toHaveBeenNthCalledWith(1, 'step');
    expect(onNotice).toHaveBeenNthCalledWith(2, 'sessionEvent');
    expect(onNotice).toHaveBeenNthCalledWith(3, 'ask');
    expect(onNotice).toHaveBeenNthCalledWith(4, 'confirmWait');
    expect(onNotice).toHaveBeenNthCalledWith(5, 'finished');
  });

  it('activeRunPresent false 后投递零回调（终态除名的订阅释放面——finished 后通道自然断开的前端半边）', async () => {
    const onNotice = vi.fn();
    const rendered = renderHook(
      (input: Params & { onNotice: (kind: NoticeKind) => void }) => useChangeFlowRun(input),
      { initialProps: { root: ROOT, id: CHANGE_ID, activeRunPresent: true, onNotice } },
    );
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));

    deliver({ ipc: 'finished' });
    expect(onNotice).toHaveBeenCalledWith('finished');

    // 终态除名（activeRunPresent false）→ cleanup 弃投递：订阅引用释放、通道
    // 不可达（ref 弃持的释放面），迟滞通知零回调
    rendered.rerender({ root: ROOT, id: CHANGE_ID, activeRunPresent: false, onNotice });
    const channel = ChannelMock.instances[0];
    channel.onmessage = null;
    deliver({ ipc: 'step' });
    expect(onNotice).toHaveBeenCalledTimes(1);
  });
});
