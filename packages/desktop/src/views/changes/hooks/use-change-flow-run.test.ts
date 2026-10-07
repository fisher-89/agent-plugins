import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeRunSnapshot, ChangeStepState, RunUpdate } from '../../../types/dto';
import { useChangeFlowRun, type UseChangeFlowRunResult } from './use-change-flow-run';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（六命令 change_flow_state / start / stop /
// confirm / answer / watch，可切换 resolve / reject 并记录入参）；Channel mock 为
// 可编程 class（捕获 onmessage，测试直接投递 RunUpdate 信封驱动归并）。
// run-state 纯函数不 mock：真实实现参与归并（内部模块不 mock）。
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
const CHANGE = 'add-feature';
const TS = 1727000000000;

function snapshot(overrides: Partial<ChangeRunSnapshot> = {}): ChangeRunSnapshot {
  return {
    runId: 'run-1727',
    status: 'running',
    phase: 'implement',
    attempt: 2,
    ask: null,
    ...overrides,
  };
}

function stepRow(overrides: Partial<ChangeStepState> = {}): ChangeStepState {
  return {
    phase: 'implement',
    attempt: 1,
    step: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    ...overrides,
  };
}

function textMessage(seq: number, text: string): RunUpdate {
  return {
    ipc: 'sessionEvent',
    sessionId: 'ses-exec-1',
    event: {
      seq,
      timestampMs: TS,
      kind: 'message',
      role: 'assistant',
      blocks: [{ kind: 'text', text }],
      parentToolUseId: null,
    },
  };
}

// ---------------------------------------------------------------------------
// 可编程 IPC：state / start 可切 reject（字符串 = reject 文本）
// ---------------------------------------------------------------------------

let stateResult: ChangeRunSnapshot | string | null | undefined | Promise<ChangeRunSnapshot | null>;
let startResult: string | Error | { code: number } | null;

function mockIpc() {
  ChannelMock.instances.length = 0;
  stateResult = null;
  startResult = null;
  invokeMock.mockImplementation((command: string) => {
    if (command === 'change_flow_state') {
      return typeof stateResult === 'string'
        ? Promise.reject(stateResult)
        : Promise.resolve(stateResult);
    }
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
  change: string | null;
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

/** 投递 RunUpdate 信封（经最新 Channel 实例）。 */
function deliver(update: RunUpdate) {
  act(() => {
    lastChannel().onmessage?.(update);
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

async function mounted(params: Params = { root: ROOT, change: CHANGE }) {
  const rendered = renderHook((input: Params) => useChangeFlowRun(input), { initialProps: params });
  await act(async () => {});
  return rendered;
}

describe('useChangeFlowRun：挂载快照恢复（D9）', () => {
  it('挂载 invoke change_flow_state：运行中快照 → state 按 initialRunState 逐字段恢复 + change_flow_watch 补订', async () => {
    stateResult = snapshot({ status: 'waitingConfirm', phase: 'dev-design', attempt: 1 });
    const { result } = await mounted();
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));

    expect(invokeMock).toHaveBeenCalledWith('change_flow_state', { root: ROOT, change: CHANGE });
    expect(result.current.state).toEqual({
      runId: 'run-1727',
      status: 'waitingConfirm',
      phase: 'dev-design',
      attempt: 1,
      ask: null,
      confirmPhase: 'dev-design',
      steps: [],
      finishedReason: null,
      liveEvents: {},
    });
    // 补订走 Channel：watch 携 root/change 与订阅实例
    const watchArgs = calls('change_flow_watch')[0][1] as Record<string, unknown>;
    expect(watchArgs.root).toBe(ROOT);
    expect(watchArgs.change).toBe(CHANGE);
    expect(watchArgs.onEvent).toBeInstanceOf(ChannelMock);
  });

  it('快照为 null（无运行 run）：state 保持 null、change_flow_watch 不发起、零 Channel 实例（惰性构造）', async () => {
    const { result } = await mounted();

    expect(result.current.state).toBeNull();
    expect(calls('change_flow_state')).toHaveLength(1);
    expect(calls('change_flow_watch')).toHaveLength(0);
    expect(ChannelMock.instances).toHaveLength(0);
  });

  it('终态快照（completed）：state 恢复但不补订（终局由图派生规则承载，零订阅）', async () => {
    stateResult = snapshot({ status: 'completed', phase: 'code-analyze', attempt: 1 });
    const { result } = await mounted();
    await waitFor(() => expect(result.current.state?.status).toBe('completed'));

    expect(calls('change_flow_watch')).toHaveLength(0);
    expect(ChannelMock.instances).toHaveLength(0);
  });

  it('终态快照（stopped / failed）：state 恢复但不补订（isTerminalStatus 三终局字面量逐档对齐——completed 之外两档同归零订阅零 Channel）', async () => {
    for (const status of ['stopped', 'failed'] as const) {
      invokeMock.mockClear();
      ChannelMock.instances.length = 0;
      stateResult = snapshot({ status, phase: 'implement', attempt: 3 });
      const rendered = renderHook((input: Params) => useChangeFlowRun(input), {
        initialProps: { root: ROOT, change: CHANGE },
      });
      await act(async () => {});
      await waitFor(() => expect(rendered.result.current.state?.status).toBe(status));

      expect(calls('change_flow_state')).toHaveLength(1);
      expect(calls('change_flow_watch')).toHaveLength(0);
      expect(ChannelMock.instances).toHaveLength(0);
      rendered.unmount();
    }
  });

  it('快照结果为 undefined（线面缺省退化）：nullish 归并双字面量守卫生效——state 降级 null、零订阅零 Channel', async () => {
    stateResult = undefined;
    const { result } = await mounted();

    expect(result.current.state).toBeNull();
    expect(calls('change_flow_state')).toHaveLength(1);
    expect(calls('change_flow_watch')).toHaveLength(0);
    expect(ChannelMock.instances).toHaveLength(0);
  });

  it('root / change 为 null：零 invoke、state null（未选定 workspace 空闲态）', async () => {
    const { result } = await mounted({ root: null, change: null });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.state).toBeNull();
  });

  it('混合 null 半边（root 有值 change null / root null change 有值）：恢复分支零 invoke 零 Channel、state null（守卫「||」两操作数各自独立可辨）；null 半边补齐 rerender 后恢复分支重新查快照（effect 依赖数组携带 root / change）', async () => {
    const rootOnly = await mounted({ root: ROOT, change: null });
    expect(invokeMock).not.toHaveBeenCalled();
    expect(ChannelMock.instances).toHaveLength(0);
    expect(rootOnly.result.current.state).toBeNull();
    rootOnly.unmount();

    const changeOnly = await mounted({ root: null, change: CHANGE });
    expect(invokeMock).not.toHaveBeenCalled();
    expect(ChannelMock.instances).toHaveLength(0);
    expect(changeOnly.result.current.state).toBeNull();
    changeOnly.unmount();

    // 参数从 null 半边补齐（rerender 触发依赖比较）：恢复分支重新查快照——参数真正就绪才 invoke
    const filled = renderHook<UseChangeFlowRunResult, Params>(
      (input: Params) => useChangeFlowRun(input),
      {
        initialProps: { root: null, change: CHANGE },
      },
    );
    await act(async () => {});
    expect(calls('change_flow_state')).toHaveLength(0);
    filled.rerender({ root: ROOT, change: CHANGE });
    await waitFor(() => expect(calls('change_flow_state')).toHaveLength(1));
    expect(filled.result.current.state).toBeNull();
    expect(calls('change_flow_watch')).toHaveLength(0);
    expect(ChannelMock.instances).toHaveLength(0);
    filled.unmount();
  });
});

describe('useChangeFlowRun：start 发起与订阅（运行期订阅）', () => {
  it('start(false) → invoke change_flow_start 携 root/change 与新 Channel 实例，成功即以返回摘要置状态种子（权威面非臆造）', async () => {
    const { result } = await mounted();
    expect(ChannelMock.instances).toHaveLength(0);

    await act(async () => {
      await result.current.start(false);
    });

    expect(calls('change_flow_start')).toHaveLength(1);
    const args = startCallArgs();
    expect(args.root).toBe(ROOT);
    expect(args.change).toBe(CHANGE);
    expect(args.onEvent).toBeInstanceOf(ChannelMock);
    // 发起成功即置种子（change_flow_start 提前 resolve 契约的权威摘要）
    expect(result.current.state).toEqual({
      runId: 'run-1727',
      status: 'running',
      phase: null,
      attempt: null,
      ask: null,
      confirmPhase: null,
      steps: [],
      finishedReason: null,
      liveEvents: {},
    });
  });

  it('空态发起（无运行 run 的常态路径）：发起后 RunUpdate 信封流入归并 state（null 态守卫不再吃掉首 run 信封——流程图 / 面板呈现）', async () => {
    const { result } = await mounted();
    expect(result.current.state).toBeNull();

    await act(async () => {
      await result.current.start(false);
    });

    deliver({ ipc: 'step', step: stepRow() });
    expect(result.current.state).toMatchObject({
      runId: 'run-1727',
      status: 'running',
      phase: 'implement',
      attempt: 1,
      steps: [stepRow()],
    });

    deliver(textMessage(0, '首 run 实时片段'));
    expect(result.current.state?.liveEvents['ses-exec-1']).toHaveLength(1);
  });

  it('终态后再发起（会话失败 / 中断后重启）：种子替换冻结终态面，新 run 信封照常归并、旧记因清空', async () => {
    stateResult = snapshot({ status: 'running', phase: 'implement', attempt: 2 });
    const { result } = await mounted();
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));

    deliver({ ipc: 'finished', status: 'failed', reason: '会话失败收敛' });
    expect(result.current.state).toMatchObject({
      status: 'failed',
      finishedReason: '会话失败收敛',
    });

    await act(async () => {
      await result.current.start(false);
    });

    // 冻结终态面被新 run 种子替换（终态守卫不再吞掉重启 run 的信封）
    expect(result.current.state).toMatchObject({
      runId: 'run-1727',
      status: 'running',
      finishedReason: null,
      steps: [],
    });

    deliver({ ipc: 'step', step: stepRow({ step: 'phaseStart', sessionId: null }) });
    expect(result.current.state).toMatchObject({
      status: 'running',
      phase: 'implement',
      attempt: 1,
      steps: [stepRow({ step: 'phaseStart', sessionId: null })],
    });
  });

  it('RunUpdate 信封流入归并 state（重挂恢复态上订阅）：Step 入步表、sessionEvent 入缓存、confirmWait 置等待相位', async () => {
    stateResult = snapshot({ status: 'running', phase: 'implement', attempt: 2 });
    const { result } = await mounted();
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));
    expect(result.current.state).not.toBeNull();

    await act(async () => {
      await result.current.start(false);
    });

    deliver({ ipc: 'step', step: stepRow() });
    expect(result.current.state).toMatchObject({
      phase: 'implement',
      attempt: 1,
      steps: [stepRow()],
    });

    deliver(textMessage(0, '实时片段'));
    expect(result.current.state?.liveEvents['ses-exec-1']).toHaveLength(1);

    deliver({ ipc: 'confirmWait', phase: 'implement' });
    expect(result.current.state).toMatchObject({
      status: 'waitingConfirm',
      confirmPhase: 'implement',
      phase: 'implement',
    });
  });

  it('停等与终态信封序列归并：ask 置载荷、finished 收口清停等态', async () => {
    stateResult = snapshot();
    const { result } = await mounted();
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));

    deliver({ ipc: 'ask', question: '选择哪个方向？', options: ['A', 'B'] });
    expect(result.current.state).toMatchObject({
      status: 'waitingAsk',
      ask: { question: '选择哪个方向？', options: ['A', 'B'] },
    });

    deliver({ ipc: 'finished', status: 'stopped', reason: '用户停止' });
    expect(result.current.state).toMatchObject({
      status: 'stopped',
      finishedReason: '用户停止',
      ask: null,
      confirmPhase: null,
    });
  });

  it('终态收口后到站信封不再改写 state：step / sessionEvent / ask / confirmWait 迟滞信封一律忽略（终态守卫——reducer 冻结态经订阅链路同样成立）', async () => {
    stateResult = snapshot({ status: 'running', phase: 'implement', attempt: 2 });
    const { result } = await mounted();
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));

    deliver({ ipc: 'finished', status: 'failed', reason: '写面漂移' });
    const frozen = result.current.state;
    expect(frozen).toMatchObject({ status: 'failed', finishedReason: '写面漂移' });

    deliver({ ipc: 'step', step: stepRow() });
    deliver(textMessage(0, '迟滞事件'));
    deliver({ ipc: 'ask', question: '迟滞提问？', options: ['A'] });
    deliver({ ipc: 'confirmWait', phase: 'test-gen' });

    expect(result.current.state).toBe(frozen);
    expect(result.current.state).toMatchObject({
      status: 'failed',
      finishedReason: '写面漂移',
      steps: [],
      liveEvents: {},
    });
  });

  it('重挂恢复已建立订阅时 start 复用既有 Channel（不重挂、双通道不叠加）', async () => {
    stateResult = snapshot({ status: 'running', phase: 'implement', attempt: 2 });
    const { result } = await mounted();
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));
    expect(ChannelMock.instances).toHaveLength(1);

    await act(async () => {
      await result.current.start(false);
    });

    expect(ChannelMock.instances).toHaveLength(1);
    expect(startCallArgs().onEvent).toBe(ChannelMock.instances[0]);
  });

  it('快照查询悬置时发起：Channel 由 start 侧惰性构造并被恢复路径复用——RunUpdate 经发起所建实例流入 state（发起侧订阅归并真实可达，非恢复侧代打）', async () => {
    let resolveState!: (value: ChangeRunSnapshot | null) => void;
    stateResult = new Promise<ChangeRunSnapshot | null>((resolve) => {
      resolveState = resolve;
    });
    const rendered = renderHook((input: Params) => useChangeFlowRun(input), {
      initialProps: { root: ROOT, change: CHANGE },
    });
    await act(async () => {});
    expect(rendered.result.current.state).toBeNull();
    expect(ChannelMock.instances).toHaveLength(0);

    // 恢复尚未落定即发起：ensureChannel 此刻才建实例（发起侧所建，非恢复侧）
    await act(async () => {
      await rendered.result.current.start(false);
    });
    expect(ChannelMock.instances).toHaveLength(1);
    expect(startCallArgs().onEvent).toBe(ChannelMock.instances[0]);

    // 恢复落定：state 非空、watch 复用发起侧所建实例（不重挂）
    await act(async () => {
      resolveState(snapshot({ status: 'running', phase: 'implement', attempt: 2 }));
    });
    expect(rendered.result.current.state?.status).toBe('running');
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));
    expect(calls('change_flow_watch')[0][1]).toHaveProperty('onEvent', ChannelMock.instances[0]);

    // 实时流经发起侧 Channel 实例归并 state：step 入步表、sessionEvent 入缓存
    deliver({ ipc: 'step', step: stepRow() });
    expect(rendered.result.current.state).toMatchObject({
      phase: 'implement',
      attempt: 1,
      steps: [stepRow()],
    });

    deliver(textMessage(0, '发起侧实时片段'));
    expect(rendered.result.current.state?.liveEvents['ses-exec-1']).toHaveLength(1);
  });
});

describe('useChangeFlowRun：start autoNextPhase 传参（停等节奏发起定格，D4）', () => {
  it('start(true) → invoke change_flow_start 携 root / change / 新 Channel 实例外增 autoNextPhase: true（经真实 bindings 生成物出线，commands 模块不 mock）', async () => {
    const { result } = await mounted();
    expect(ChannelMock.instances).toHaveLength(0);

    await act(async () => {
      await result.current.start(true);
    });

    expect(calls('change_flow_start')).toHaveLength(1);
    const args = startCallArgs();
    expect(args.autoNextPhase).toBe(true);
    expect(args.root).toBe(ROOT);
    expect(args.change).toBe(CHANGE);
    expect(args.onEvent).toBeInstanceOf(ChannelMock);
  });

  it('start(false) → autoNextPhase: false 显式出线（默认档亦为显式实参——签名必填无缺省，载荷键在场）', async () => {
    const { result } = await mounted();

    await act(async () => {
      await result.current.start(false);
    });

    expect(calls('change_flow_start')).toHaveLength(1);
    const args = startCallArgs();
    expect(args.autoNextPhase).toBe(false);
    expect(Object.keys(args)).toContain('autoNextPhase');
  });

  it('root / change 为 null 时 start(true / false) 均 no-op 零 invoke（守卫先行于传参）', async () => {
    const { result } = await mounted({ root: null, change: null });

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
  it('stop() → invoke change_flow_stop 携 { root, change } 原样透传', async () => {
    const { result } = await mounted();
    await act(async () => {
      await result.current.stop();
    });

    expect(calls('change_flow_stop')).toHaveLength(1);
    expect(calls('change_flow_stop')[0]).toEqual([
      'change_flow_stop',
      { root: ROOT, change: CHANGE },
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
      { root: ROOT, change: CHANGE, proceed: true },
    ]);
    expect(confirmCalls[1]).toEqual([
      'change_flow_confirm',
      { root: ROOT, change: CHANGE, proceed: false },
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
      { root: ROOT, change: CHANGE, answer: '继续推进 implement' },
    ]);
  });

  it('root / change 为 null（双 null 与混合 null 半边）时四操作均 no-op（零 invoke 零 Channel——守卫「||」两操作数各自独立可辨）', async () => {
    for (const params of [
      { root: null, change: null },
      { root: ROOT, change: null },
      { root: null, change: CHANGE },
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
  it('换根 rerender 后四操作以新 root 出线（旧闭包不滞留——start / stop / confirm / answer 依赖数组携带 root）', async () => {
    const ROOT2 = 'C:\\demo\\flow-2';
    const rendered = await mounted({ root: ROOT, change: CHANGE });
    rendered.rerender({ root: ROOT2, change: CHANGE });
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
      { root: ROOT2, change: CHANGE },
    ]);
    expect(calls('change_flow_confirm').at(-1)).toEqual([
      'change_flow_confirm',
      { root: ROOT2, change: CHANGE, proceed: true },
    ]);
    expect(calls('change_flow_answer').at(-1)).toEqual([
      'change_flow_answer',
      { root: ROOT2, change: CHANGE, answer: '换根后应答' },
    ]);
  });
});

describe('useChangeFlowRun：收口释放与重挂补订（D9）', () => {
  it('终态收口后卸载重挂：订阅引用释放（全新 Channel 实例）、change_flow_watch 重新补订、实时流不中断', async () => {
    stateResult = snapshot();
    const first = await mounted();
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(1));
    const firstChannel = ChannelMock.instances[0];

    deliver({ ipc: 'finished', status: 'completed', reason: null });
    expect(first.result.current.state?.status).toBe('completed');
    first.unmount();

    const second = renderHook((input: Params) => useChangeFlowRun(input), {
      initialProps: { root: ROOT, change: CHANGE },
    });
    await act(async () => {});
    await waitFor(() => expect(calls('change_flow_watch')).toHaveLength(2));

    expect(ChannelMock.instances).toHaveLength(2);
    expect(ChannelMock.instances[1]).not.toBe(firstChannel);

    deliver({ ipc: 'step', step: stepRow() });
    expect(second.result.current.state?.steps).toHaveLength(1);
  });
});

describe('useChangeFlowRun：恢复失败与迟到结果时序（disposed 收口补强）', () => {
  it('卸载后快照结果迟到：不建 Channel、不补订（disposed 弃投递——真实收口零残余 invoke）', async () => {
    let resolveState!: (value: ChangeRunSnapshot | null) => void;
    stateResult = new Promise<ChangeRunSnapshot | null>((resolve) => {
      resolveState = resolve;
    });
    const rendered = renderHook((input: Params) => useChangeFlowRun(input), {
      initialProps: { root: ROOT, change: CHANGE },
    });
    await act(async () => {});
    rendered.unmount();

    await act(async () => {
      resolveState(snapshot({ status: 'running', phase: 'implement', attempt: 2 }));
    });

    expect(calls('change_flow_watch')).toHaveLength(0);
    expect(ChannelMock.instances).toHaveLength(0);
  });

  it('恢复二次查询 reject 且 state 已非空：state 降级复位 null（catch 体与 disposed 取反守卫真实生效——非空态可见复位）', async () => {
    stateResult = snapshot({ status: 'running', phase: 'implement', attempt: 2 });
    const rendered = renderHook((input: Params) => useChangeFlowRun(input), {
      initialProps: { root: ROOT, change: CHANGE },
    });
    await act(async () => {});
    await waitFor(() => expect(rendered.result.current.state?.status).toBe('running'));
    expect(rendered.result.current.state).not.toBeNull();
    expect(calls('change_flow_watch')).toHaveLength(1);

    // 换根重挂：第二查询 reject → 空态降级覆盖既有非空 state
    stateResult = 'db: 快照查询失败';
    rendered.rerender({ root: 'C:\\demo\\flow-2', change: CHANGE });
    await act(async () => {});

    expect(rendered.result.current.state).toBeNull();
    // reject 不补订：watch 维持首次的一条，迟到路径零叠加
    expect(calls('change_flow_watch')).toHaveLength(1);
  });

  it('换根重挂时序：旧查询迟到结果不改写 state、不补订；新查询以全新 Channel 实例重建订阅（cleanup 释放 channelRef + disposed 置位真实生效）', async () => {
    let resolveFirst!: (value: ChangeRunSnapshot | null) => void;
    stateResult = new Promise<ChangeRunSnapshot | null>((resolve) => {
      resolveFirst = resolve;
    });
    const rendered = renderHook((input: Params) => useChangeFlowRun(input), {
      initialProps: { root: ROOT, change: CHANGE },
    });
    await act(async () => {});
    expect(rendered.result.current.state).toBeNull();

    // 换根：第二查询立即落定（waitingConfirm 快照）
    stateResult = snapshot({ status: 'waitingConfirm', phase: 'dev-design', attempt: 1 });
    rendered.rerender({ root: 'C:\\demo\\flow-2', change: CHANGE });
    await waitFor(() => expect(rendered.result.current.state?.status).toBe('waitingConfirm'));
    // channelRef 已被 cleanup 释放 → 新查询补订构造全新实例
    expect(ChannelMock.instances).toHaveLength(1);

    // 旧查询（属旧 root）迟到落定：不改写 state、不触发补订
    await act(async () => {
      resolveFirst(snapshot({ status: 'running', phase: 'implement', attempt: 9 }));
    });

    expect(rendered.result.current.state).toMatchObject({ status: 'waitingConfirm', attempt: 1 });
    expect(calls('change_flow_state')).toHaveLength(2);
    expect(calls('change_flow_watch')).toHaveLength(1);
    expect(ChannelMock.instances).toHaveLength(1);
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

  it('change_flow_state reject → 空态降级不崩（state null、不阻断页面、error 不误报）', async () => {
    stateResult = 'db: 快照查询失败';
    const { result } = await mounted();

    expect(result.current.state).toBeNull();
    expect(result.current.error).toBeNull();
    expect(calls('change_flow_watch')).toHaveLength(0);
  });

  it('start reject 携非 string（Error 对象 / 普通对象）：error 归一为 String(cause) 文本（readError 非字符串分支真实参与——error 恒为 string）', async () => {
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
  it('UseChangeFlowRunResult 键集恰为六键且形状不回退（state/start/stop/confirm/answer/error）', async () => {
    const { result } = await mounted();

    expect(Object.keys(result.current).sort()).toEqual(
      ['answer', 'confirm', 'error', 'start', 'state', 'stop'].sort(),
    );
    expect(result.current.state === null || typeof result.current.state === 'object').toBe(true);
    expect(typeof result.current.start).toBe('function');
    expect(typeof result.current.stop).toBe('function');
    expect(typeof result.current.confirm).toBe('function');
    expect(typeof result.current.answer).toBe('function');
    expect(result.current.error === null || typeof result.current.error === 'string').toBe(true);
  });
});
