import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { SessionPanelState } from '@/components/agent';
import { driftCount, driftReports, resetDrift } from '@/lib/ipc-drift';
import type {
  AgentEvent,
  AgentPermissionMode,
  SessionPage,
  SessionSummary,
  TurnSummary,
} from '@/types/dto';

import { useChangeChat, type ChangeChatParams } from './use-change-chat';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：`@tauri-apps/api/core` 的 invoke 按命令名分派
// （agent_start / agent_stop / session_detail / agent_session_transcript /
// agent_session_watch / list_change_chat_sessions）；Channel 为可编程类，测试
// 直投 `{ ipc: 'record' }` 信封驱动 early-resolve 轮行与收口。useAgentSession /
// @ai-sdk/react / agent-transport 全部真实组合（hook 即链路入口）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((message: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式；时钟不入 mock——updatedAt / createdAt 由
// fixture 显式给定，排序与显示断言取原始数值）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const CHANGE_ID = 'chg-1';
const WORKTREE = 'C:\\demo\\wt';
const TS = 1_727_000_000_000;

function turn(turnId: number, status: TurnSummary['status'], sessionId: string): TurnSummary {
  return {
    turnId,
    sessionId,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 900,
    numTurns: status === 'running' ? null : 1,
    costUsd: status === 'running' ? null : 0.1,
    durationMs: status === 'running' ? null : 500,
    error: null,
  };
}

function summary(
  turns: TurnSummary[],
  id: string,
  sourceRef: string | null = CHANGE_ID,
): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'change', sourceRef },
      createdAt: TS,
      updatedAt: TS + 9,
    },
    stats: {
      turnCount: turns.length,
      totalDurationMs: null,
      inputTokens: null,
      outputTokens: null,
    },
    turns,
  };
}

function emptyPage(): SessionPage {
  return { sessions: [], total: 0 };
}

// ---------------------------------------------------------------------------
// 可编程 IPC 应答（按命令名分派）
// ---------------------------------------------------------------------------

/** 清单应答函数（按 args.offset 分派 / 可切 reject）。 */
type ListHandler = (args: { offset: number; limit: number }) => Promise<SessionPage>;

interface IpcState {
  detail: SessionSummary | null | Error;
  transcript: AgentEvent[] | Error;
  /** 订阅命令：deferred = 挂起（用例直投信封）/ resolve / reject */
  watch: 'deferred' | 'resolve' | 'reject';
  start: TurnSummary | Error;
  list: ListHandler;
}

let ipc: IpcState;

const watchResolvers: (() => void)[] = [];

function resetIpc(overrides: Partial<IpcState> = {}): void {
  ipc = {
    detail: null,
    transcript: [],
    watch: 'deferred',
    start: turn(1, 'running', 'ses-1'),
    list: () => Promise.resolve(emptyPage()),
    ...overrides,
  };
  ChannelMock.instances.length = 0;
  watchResolvers.length = 0;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, args: Record<string, unknown> = {}) => {
    if (command === 'session_detail') {
      return ipc.detail instanceof Error ? Promise.reject(ipc.detail) : Promise.resolve(ipc.detail);
    }
    if (command === 'agent_session_transcript') {
      return ipc.transcript instanceof Error
        ? Promise.reject(ipc.transcript)
        : Promise.resolve(ipc.transcript);
    }
    if (command === 'agent_session_watch') {
      if (ipc.watch === 'reject') return Promise.reject(new Error('watch 命令失败'));
      if (ipc.watch === 'resolve') return Promise.resolve(null);
      return new Promise<null>((resolve) => {
        watchResolvers.push(() => resolve(null));
      });
    }
    if (command === 'agent_start') {
      return ipc.start instanceof Error ? Promise.reject(ipc.start) : Promise.resolve(ipc.start);
    }
    if (command === 'list_change_chat_sessions') {
      return ipc.list({
        offset: Number(args['offset']),
        limit: Number(args['limit']),
      });
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  resetIpc();
});

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('尚未建立流（无 Channel 实例）');
  return instance;
}

function deliverRecord(record: TurnSummary): void {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'record', record });
  });
}

function callsOf(name: string): unknown[][] {
  return invokeMock.mock.calls.filter(([command]) => command === name);
}

function argsOf(name: string, index = -1): Record<string, unknown> {
  const calls = callsOf(name);
  const call = index < 0 ? calls.at(-1) : calls[index];
  if (!call) throw new Error(`${name} 未被调用`);
  return call[1] as Record<string, unknown>;
}

type Params = ChangeChatParams;

function baseParams(overrides: Partial<Params> = {}): Params {
  return {
    root: ROOT,
    changeId: CHANGE_ID,
    worktree: WORKTREE,
    runActive: false,
    archived: false,
    ...overrides,
  };
}

async function mounted(overrides: Partial<Params> = {}) {
  const rendered = renderHook((props: Params) => useChangeChat(props), {
    initialProps: baseParams(overrides),
  });
  await act(async () => {});
  await act(async () => {});
  return rendered;
}

function rerenderWith(
  rerender: (props: Params) => void,
  overrides: Partial<Params>,
): Promise<void> {
  return act(async () => {
    rerender(baseParams(overrides));
  });
}

function sendInput(
  send: (input: { text: string; permissionMode: AgentPermissionMode }) => void,
  text = '首轮提问',
  permissionMode: AgentPermissionMode = 'default',
): void {
  act(() => {
    send({ text, permissionMode });
  });
}

// ---------------------------------------------------------------------------
// 打开即空态：挂载零取数（AC-4 / AC-5）
// ---------------------------------------------------------------------------

describe('useChangeChat：打开即空态', () => {
  it('挂载后 sessionId 为 null、清单空、未展开，且零会话类 invoke', async () => {
    const { result } = await mounted();

    expect(result.current.sessionId).toBeNull();
    expect(result.current.currentSessionId).toBeNull();
    expect(result.current.sessions).toEqual([]);
    expect(result.current.total).toBe(0);
    expect(result.current.listOpen).toBe(false);
    expect(result.current.listLoading).toBe(false);
    expect(result.current.listError).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
    expect(callsOf('list_change_chat_sessions')).toHaveLength(0);
    expect(callsOf('session_detail')).toHaveLength(0);
    expect(callsOf('agent_session_transcript')).toHaveLength(0);
  });

  it('空态不自动恢复最近会话：页面重挂后仍为空态（不选中任何会话）', async () => {
    ipc.list = () => Promise.resolve({ sessions: [summary([], 'ses-old')], total: 1 });
    const first = await mounted();
    expect(first.result.current.sessions).toEqual([]);
    expect(first.result.current.sessionId).toBeNull();
    first.unmount();

    const second = await mounted();
    expect(second.result.current.sessionId).toBeNull();
    expect(second.result.current.currentSessionId).toBeNull();
    expect(callsOf('session_detail')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 归属与执行根注入（AC-1 / AC-3 / AC-4）
// ---------------------------------------------------------------------------

describe('useChangeChat：归属与执行根注入', () => {
  it('空态首发携 source=change / sourceRef=changeId / cwd=worktree / sessionId=null', async () => {
    const { result } = await mounted();

    sendInput(result.current.send);
    await act(async () => {});

    expect(callsOf('agent_start')).toHaveLength(1);
    expect(argsOf('agent_start')).toMatchObject({
      root: ROOT,
      prompt: '首轮提问',
      sessionId: null,
      source: 'change',
      sourceRef: CHANGE_ID,
      cwd: WORKTREE,
      permissionMode: 'default',
    });
  });

  it('worktree 为 null（legacy）→ cwd 传 null，后端回落 root', async () => {
    const { result } = await mounted({ worktree: null });
    expect(result.current.cwd).toBeNull();

    sendInput(result.current.send);
    await act(async () => {});

    expect(argsOf('agent_start')['cwd']).toBeNull();
  });

  it('归档 change 不传悬空 worktree（cwd 恒 null）', async () => {
    const { result } = await mounted({ archived: true });
    expect(result.current.cwd).toBeNull();

    sendInput(result.current.send);
    await act(async () => {});

    expect(callsOf('agent_start'), '归档只读零发起').toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 禁发判据单源（AC-6 矩阵四行 + 优先级；`sessionId === null` 不入判据）
// ---------------------------------------------------------------------------

describe('useChangeChat：禁发判据单源', () => {
  it('row3：无 run 的 active 且会话未运行 → 理由 null、可发可起（空态同判）', async () => {
    const { result } = await mounted();

    expect(result.current.disabledReason).toBeNull();
    expect(result.current.canSend).toBe(true);
    expect(result.current.canStart).toBe(true);
    expect(result.current.sessionId, '空态即草案态（不单独构成禁用理由）').toBeNull();
  });

  it('row1：流程运行中 → 理由「流程运行中」、全禁发且 send 零 agent_start', async () => {
    const { result } = await mounted({ runActive: true });

    expect(result.current.disabledReason).toBe('流程运行中');
    expect(result.current.canSend).toBe(false);
    expect(result.current.canStart).toBe(false);

    sendInput(result.current.send);
    await act(async () => {});
    expect(callsOf('agent_start')).toHaveLength(0);
  });

  it('row4：归档只读 → 理由「已归档 change 只读」、全禁发且 send 零 agent_start', async () => {
    const { result } = await mounted({ archived: true });

    expect(result.current.disabledReason).toBe('已归档 change 只读');
    expect(result.current.canSend).toBe(false);
    expect(result.current.canStart).toBe(false);

    sendInput(result.current.send);
    await act(async () => {});
    expect(callsOf('agent_start')).toHaveLength(0);
  });

  it('row2：本会话 running → 理由仍 null（running 不进理由）、可发不可起', async () => {
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    const { result } = await mounted();

    act(() => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    await act(async () => {});

    expect(result.current.sessionId).toBe('ses-1');
    expect(result.current.session.running).toBe(true);
    expect(result.current.disabledReason).toBeNull();
    expect(result.current.canSend).toBe(true);
    expect(result.current.canStart).toBe(false);
  });

  it('优先级：归档与 run 同时在场 → 归档理由优先（三元序）', async () => {
    const { result } = await mounted({ archived: true, runActive: true });

    expect(result.current.disabledReason).toBe('已归档 change 只读');
    expect(result.current.canSend).toBe(false);
    expect(result.current.canStart).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 历史清单探测面：展开取数 / 每次展开重取 / 分页累加 / 失败降级 / 票号收敛
// ---------------------------------------------------------------------------

/** 按 offset 分页应答的清单 fixture（`total` 为本过滤集全量）。 */
function pagedSessions(total: number): ListHandler {
  return ({ offset, limit }) => {
    const sessions: SessionSummary[] = [];
    for (let index = offset; index < Math.min(offset + limit, total); index += 1) {
      sessions.push(summary([], `ses-${String(index).padStart(3, '0')}`));
    }
    return Promise.resolve({ sessions, total });
  };
}

describe('useChangeChat：历史清单取数', () => {
  it('toggleList 展开 → list_change_chat_sessions 恰一次且参数 { root, id, offset: 0, limit: 20 }', async () => {
    ipc.list = pagedSessions(2);
    const { result } = await mounted();

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});

    expect(callsOf('list_change_chat_sessions')).toHaveLength(1);
    expect(argsOf('list_change_chat_sessions')).toEqual({
      root: ROOT,
      id: CHANGE_ID,
      offset: 0,
      limit: 20,
    });
    expect(result.current.listOpen).toBe(true);
    expect(result.current.listLoading).toBe(false);
    expect(result.current.listError).toBeNull();
    expect(result.current.sessions).toHaveLength(2);
    expect(result.current.total).toBe(2);
  });

  it('每次展开都重取首屏（收起不清空、再次展开仍在飞一跳）', async () => {
    ipc.list = pagedSessions(2);
    const { result } = await mounted();

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(callsOf('list_change_chat_sessions')).toHaveLength(1);
    const firstPage = result.current.sessions;

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.listOpen).toBe(false);
    expect(result.current.sessions, '收起不清空').toEqual(firstPage);

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(callsOf('list_change_chat_sessions'), '每次展开重取（非缓存）').toHaveLength(2);
    expect(argsOf('list_change_chat_sessions')['offset']).toBe(0);
  });

  it('loadMore 按 offset = 已加载条数累加，连续取到 total 为止（offset 序列 20 / 40）', async () => {
    ipc.list = pagedSessions(45);
    const { result } = await mounted();

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(20);
    expect(result.current.total).toBe(45);

    act(() => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(argsOf('list_change_chat_sessions')['offset']).toBe(20);
    expect(result.current.sessions).toHaveLength(40);

    act(() => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(argsOf('list_change_chat_sessions')['offset']).toBe(40);
    expect(result.current.sessions).toHaveLength(45);
    expect(result.current.sessions.length).toBe(result.current.total);
  });

  it('页宽常量单源：任一取数 limit 恒为 20（超长清单连续 loadMore 无跳页无重复）', async () => {
    ipc.list = pagedSessions(120);
    const { result } = await mounted();

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    for (let round = 0; round < 5; round += 1) {
      act(() => {
        result.current.loadMore();
      });
      await act(async () => {});
    }

    const offsets = callsOf('list_change_chat_sessions').map(
      ([, args]) => (args as { offset: number }).offset,
    );
    expect(offsets).toEqual([0, 20, 40, 60, 80, 100]);
    expect(
      callsOf('list_change_chat_sessions').every(
        ([, args]) => (args as { limit: number }).limit === 20,
      ),
      'limit 恒为 CHANGE_CHAT_PAGE_SIZE',
    ).toBe(true);
    const ids = result.current.sessions.map((session) => session.row.id);
    expect(new Set(ids).size, '不重不漏').toBe(ids.length);
    expect(result.current.sessions).toHaveLength(120);
  });

  it('清单取数 reject → listError 置位、loading 复位、sessions 保持前值（不清空）', async () => {
    ipc.list = pagedSessions(2);
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(2);

    ipc.list = () => Promise.reject('清单取数失败');
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});

    expect(result.current.listError).toBe('清单取数失败');
    expect(result.current.listLoading).toBe(false);
    expect(result.current.sessions, '失败降级不清空既有页').toHaveLength(2);
    expect(result.current.canSend, '判据不受清单失败影响').toBe(true);
  });

  it('空页应答 → sessions 空、total 0，重复展开状态稳定不抖动', async () => {
    ipc.list = () => Promise.resolve(emptyPage());
    const { result } = await mounted();

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toEqual([]);
    expect(result.current.total).toBe(0);

    act(() => {
      result.current.toggleList();
    });
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toEqual([]);
    expect(result.current.total).toBe(0);
    expect(result.current.listError).toBeNull();
  });

  it('在飞旧页应答不覆盖新页（请求票号收敛，连点历史与重取交错）', async () => {
    const deferred: { resolve: ((page: SessionPage) => void) | null } = { resolve: null };
    ipc.list = pagedSessions(20);
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(20);

    // 第二拍（加载更多）挂起不落定
    ipc.list = () =>
      new Promise<SessionPage>((resolve) => {
        deferred.resolve = resolve;
      });
    act(() => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(result.current.listLoading).toBe(true);

    // 第三拍（收起再展开）以新页落地
    ipc.list = () => Promise.resolve({ sessions: [], total: 0 });
    act(() => {
      result.current.toggleList();
    });
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toEqual([]);

    // 陈旧的第二拍此刻落定：票号已作废，不得写入
    await act(async () => {
      deferred.resolve?.({ sessions: [summary([], 'ses-stale')], total: 1 });
    });
    expect(result.current.sessions, '旧页不覆盖新页').toEqual([]);
    expect(result.current.total).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 三处事实驱动重取（D11）：首发后会话 id 确立 / 轮收口（挂载与展开之外）
// ---------------------------------------------------------------------------

describe('useChangeChat：清单重取时机', () => {
  it('空态首发后轮行确立会话 id → currentSessionId 非空且清单重取一次', async () => {
    ipc.detail = summary([turn(1, 'running', 'ses-1')], 'ses-1');
    const { result } = await mounted();
    expect(callsOf('list_change_chat_sessions')).toHaveLength(0);

    sendInput(result.current.send);
    await act(async () => {});
    await act(async () => {});

    expect(result.current.currentSessionId, '会话 id 以后端铸造回写为真相').toBe('ses-1');
    expect(callsOf('list_change_chat_sessions'), '首发后重取（D11 ②）').toHaveLength(1);
  });

  it('轮收口（库态 running → completed）→ 清单再重取一次', async () => {
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    const { result } = await mounted();
    act(() => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    await act(async () => {});
    expect(result.current.session.running).toBe(true);
    const before = callsOf('list_change_chat_sessions').length;

    // 库态同步转收口（真实语义：轮收口即库内轮行翻 completed）后再投终态信封
    ipc.detail = summary([turn(11, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(11, 'completed', 'ses-1'));
    await act(async () => {});
    await waitFor(() => expect(result.current.session.running).toBe(false));

    expect(callsOf('list_change_chat_sessions').length, '收口重取（D11 ③）').toBe(before + 1);
  });
});

// ---------------------------------------------------------------------------
// 新会话 / 历史点选 / 身份源切换
// ---------------------------------------------------------------------------

describe('useChangeChat：新会话与历史点选', () => {
  it('selectSession 直查会话（参数恰 { root, sessionId }，零 source / sourceRef 反查）', async () => {
    ipc.detail = summary([turn(21, 'completed', 'ses-2')], 'ses-2');
    const { result } = await mounted();

    act(() => {
      result.current.selectSession('ses-2');
    });
    await act(async () => {});
    await act(async () => {});

    expect(result.current.sessionId).toBe('ses-2');
    expect(result.current.currentSessionId).toBe('ses-2');
    expect(argsOf('session_detail')).toEqual({ root: ROOT, sessionId: 'ses-2' });
    expect(Object.keys(argsOf('session_detail')).sort()).toEqual(['root', 'sessionId']);
    expect(argsOf('agent_session_transcript')).toEqual({ root: ROOT, sessionId: 'ses-2' });
    expect(Object.keys(argsOf('agent_session_transcript')).sort()).toEqual(['root', 'sessionId']);
  });

  it('newSession 回空态可再发；旧会话条目保留（仍可重开）', async () => {
    ipc.detail = summary([turn(21, 'completed', 'ses-2')], 'ses-2');
    ipc.list = () => Promise.resolve({ sessions: [summary([], 'ses-2')], total: 1 });
    const { result } = await mounted();
    act(() => {
      result.current.selectSession('ses-2');
    });
    await act(async () => {});
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(1);

    act(() => {
      result.current.newSession();
    });
    await act(async () => {});

    expect(result.current.sessionId).toBeNull();
    expect(result.current.currentSessionId).toBeNull();
    expect(result.current.canSend, '回同一可发送空态').toBe(true);
    expect(result.current.sessions, '旧会话仍可重开').toHaveLength(1);

    invokeMock.mockClear();
    sendInput(result.current.send, '新会话首轮');
    await act(async () => {});
    expect(argsOf('agent_start')['sessionId']).toBeNull();
  });

  it('change id 变化 → 会话 id / 展开态 / 清单复位，随后以新 id 取数', async () => {
    ipc.detail = summary([turn(21, 'completed', 'ses-1')], 'ses-1');
    ipc.list = () => Promise.resolve({ sessions: [summary([], 'ses-1')], total: 1 });
    const { result, rerender } = await mounted();
    act(() => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(1);

    await rerenderWith(rerender, { changeId: 'chg-2' });

    expect(result.current.sessionId).toBeNull();
    expect(result.current.listOpen).toBe(false);
    expect(result.current.sessions).toEqual([]);
    expect(result.current.total).toBe(0);

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(argsOf('list_change_chat_sessions')['id']).toBe('chg-2');
  });
});

// ---------------------------------------------------------------------------
// 参数守卫（root / changeId 缺席即零 invoke）
// ---------------------------------------------------------------------------

describe('useChangeChat：参数守卫', () => {
  it('root = null → 展开零 invoke、发送零 agent_start、清单空不崩', async () => {
    const { result } = await mounted({ root: null });

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    sendInput(result.current.send);
    await act(async () => {});

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.sessions).toEqual([]);
    expect(result.current.total).toBe(0);
  });

  it('changeId = null → 展开零 invoke（不拼 source_ref），发送仍按矩阵判定且 sourceRef 为 null', async () => {
    const { result } = await mounted({ changeId: null });

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(callsOf('list_change_chat_sessions')).toHaveLength(0);

    expect(result.current.canSend).toBe(true);
    sendInput(result.current.send);
    await act(async () => {});
    expect(callsOf('agent_start')).toHaveLength(1);
    expect(argsOf('agent_start')['sourceRef']).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：`listChangeChatSessions`
// 的四形参分页面放行、结果面 soft-report 与既有分页累积语义。
// ---------------------------------------------------------------------------

describe('useChangeChat：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('首取 listChangeChatSessions(root, changeId, 0, PAGE_SIZE) → 放行、键名逐字命中、零漂移', async () => {
    resetIpc({ list: pagedSessions(3) });
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await waitFor(() => expect(result.current.sessions).toHaveLength(3));

    expect(argsOf('list_change_chat_sessions', 0)).toMatchObject({
      root: ROOT,
      id: CHANGE_ID,
      offset: 0,
    });
    expect(Object.keys(argsOf('list_change_chat_sessions', 0))).toEqual([
      'root',
      'id',
      'offset',
      'limit',
    ]);
    expect(driftCount()).toBe(0);
  });

  it('结果含 null 可空字段 → 零漂移、既有消息列表映射不变', async () => {
    resetIpc({
      list: () => Promise.resolve({ sessions: [summary([], 'ses-null')], total: 1 }),
      detail: null,
    });
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));

    expect(result.current.total).toBe(1);
    expect(driftCount()).toBe(0);
  });

  it('reject 串 → 既有 error 态文案逐字不变、零漂移', async () => {
    resetIpc({ list: () => Promise.reject('查询会话失败：db locked') });
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await waitFor(() => expect(result.current.listError).not.toBeNull());

    expect(result.current.listError).toBe('查询会话失败：db locked');
    expect(driftCount()).toBe(0);
  });

  it('结果面脏（sessions 元素缺字段）→ 漂移恰 1 条 + 既有分页合并不崩', async () => {
    resetIpc({
      list: () =>
        Promise.resolve({
          sessions: [{ id: 'ses-broken', updatedAt: TS } as unknown as SessionSummary],
          total: 1,
        }),
    });
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('listChangeChatSessions');
    expect(driftReports()[0].path.startsWith('#/sessions/0')).toBe(true);
    expect(result.current.listError).toBeNull();
  });

  it('空页（sessions: []）→ 零漂移、既有 hasMore 语义不变', async () => {
    resetIpc({ list: () => Promise.resolve(emptyPage()) });
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await waitFor(() => expect(result.current.listLoading).toBe(false));

    expect(result.current.sessions).toEqual([]);
    expect(result.current.total).toBe(0);
    expect(driftCount()).toBe(0);
  });

  it('offset 递增连续两次取数（0 → PAGE_SIZE）→ 两页均零误拒、漂移保持 0', async () => {
    resetIpc({ list: pagedSessions(40) });
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await waitFor(() => expect(result.current.sessions.length).toBeGreaterThan(0));
    const pageSize = result.current.sessions.length;

    act(() => {
      result.current.loadMore();
    });
    await waitFor(() => expect(result.current.sessions).toHaveLength(pageSize * 2));

    const offsets = callsOf('list_change_chat_sessions').map(
      ([, args]) => (args as Record<string, unknown>)['offset'],
    );
    expect(offsets).toEqual([0, pageSize]);
    expect(driftCount()).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 状态行 `note`（骨架契约：`disabledReason ?? （空态 ? 草案文案 : null）`）
// ——文案生成规则留在本 hook（骨架只渲染），四象限覆盖 string / null 两 value
// 形态；错误态不新增第三档文案。
// ---------------------------------------------------------------------------

const DRAFT_NOTE = '发送首条消息即开启新会话';
const ARCHIVED_NOTE = '已归档 change 只读';
const RUN_NOTE = '流程运行中';

describe('useChangeChat：状态行 note', () => {
  it('空态且无理由 → 草案文案逐字', async () => {
    const { result } = await mounted();

    expect(result.current.sessionId).toBeNull();
    expect(result.current.currentSessionId).toBeNull();
    expect(result.current.note).toBe(DRAFT_NOTE);
  });

  it('首发后确立会话 id 且无理由 → note 为 null（不占位）', async () => {
    const { result } = await mounted();
    sendInput(result.current.send);
    await act(async () => {});
    await act(async () => {});

    expect(result.current.currentSessionId, '后端铸造会话 id 回写').toBe('ses-1');
    expect(result.current.session.running).toBe(true);
    expect(result.current.disabledReason, 'running 不进理由（change 侧判据矩阵）').toBeNull();
    expect(result.current.note, '有会话且无理由 → 状态行不占位').toBeNull();
  });

  it('禁用理由优先于草案文案（空态 / 有会话两态 × 归档 / 流程运行中两理由）', async () => {
    const archivedEmpty = await mounted({ archived: true });
    expect(archivedEmpty.result.current.sessionId).toBeNull();
    expect(archivedEmpty.result.current.note, '归档理由高于草案文案').toBe(ARCHIVED_NOTE);
    archivedEmpty.unmount();

    const runEmpty = await mounted({ runActive: true });
    expect(runEmpty.result.current.note).toBe(RUN_NOTE);
    runEmpty.unmount();

    // 有会话同理由矩阵：note 仍取理由（不落回 null）
    ipc.detail = summary([turn(32, 'completed', 'ses-2')], 'ses-2');
    const archivedWithSession = await mounted({ archived: true });
    act(() => {
      archivedWithSession.result.current.selectSession('ses-2');
    });
    await act(async () => {});
    await act(async () => {});
    expect(archivedWithSession.result.current.currentSessionId).toBe('ses-2');
    expect(archivedWithSession.result.current.note).toBe(ARCHIVED_NOTE);
    archivedWithSession.unmount();

    const runWithSession = await mounted({ runActive: true });
    act(() => {
      runWithSession.result.current.selectSession('ses-2');
    });
    await act(async () => {});
    await act(async () => {});
    expect(runWithSession.result.current.note).toBe(RUN_NOTE);
  });

  it('类型面：四象限只产出 string 或 null（不返回空串 / undefined）', async () => {
    const quadrants: (string | null)[] = [];

    const empty = await mounted();
    quadrants.push(empty.result.current.note);
    empty.unmount();

    const emptyWithReason = await mounted({ archived: true });
    quadrants.push(emptyWithReason.result.current.note);
    emptyWithReason.unmount();

    ipc.detail = summary([turn(33, 'completed', 'ses-3')], 'ses-3');
    const withSession = await mounted();
    act(() => {
      withSession.result.current.selectSession('ses-3');
    });
    await act(async () => {});
    quadrants.push(withSession.result.current.note);
    withSession.unmount();

    const withSessionAndReason = await mounted({ runActive: true });
    act(() => {
      withSessionAndReason.result.current.selectSession('ses-3');
    });
    await act(async () => {});
    quadrants.push(withSessionAndReason.result.current.note);

    expect(quadrants.filter((note) => note === null)).toHaveLength(1);
    expect(quadrants.filter((note) => typeof note === 'string')).toHaveLength(3);
    for (const note of quadrants) {
      expect(note === null || note.length > 0, `note 不得为空串：${JSON.stringify(note)}`).toBe(
        true,
      );
    }
  });

  it('骨架契约继承：返回对象可赋给 SessionPanelState（12 字段逐字段在场）', async () => {
    const { result } = await mounted();
    const state: SessionPanelState = result.current;

    for (const key of [
      'session',
      'sessions',
      'total',
      'listOpen',
      'listLoading',
      'listError',
      'toggleList',
      'loadMore',
      'newSession',
      'selectSession',
      'canStart',
      'note',
    ] as const) {
      expect(state, `骨架契约字段 ${key} 在场`).toHaveProperty(key);
    }
    expect(result.current.disabledReason).toBeNull();
    expect(result.current.canSend).toBe(true);
  });

  it('取数失败不干扰 note：清单 / 会话面异常下仍取「理由 ?? 空态草案」', async () => {
    ipc.list = () => Promise.reject(new Error('库不可读'));
    const { result } = await mounted();

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.listError).toContain('库不可读');
    expect(result.current.listLoading).toBe(false);
    expect(result.current.note, '清单失败不改写状态行').toBe(DRAFT_NOTE);

    ipc.detail = new Error('会话读取失败');
    act(() => {
      result.current.selectSession('ses-err');
    });
    await act(async () => {});
    await act(async () => {});
    expect(result.current.session.error).toContain('会话读取失败');
    expect(result.current.note, '会话面失败不新增第三档文案').toBe(DRAFT_NOTE);
    expect(result.current.disabledReason).toBeNull();
  });
});
