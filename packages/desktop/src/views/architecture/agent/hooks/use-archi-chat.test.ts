import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { SessionPanelState } from '@/components/agent';
import type {
  AgentEvent,
  AgentPermissionMode,
  SessionPage,
  SessionSummary,
  TurnSummary,
} from '@/types/dto';

import { useArchiChat, type ArchiChatParams } from './use-archi-chat';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：`@tauri-apps/api/core` 的 invoke 按命令名分派
// （agent_start / agent_stop / session_detail / agent_session_transcript /
// agent_session_watch / list_archi_chat_sessions）；Channel 为可编程类，用例直投
// `{ ipc: 'record' }` 信封驱动 early-resolve 轮行与收口。`useAgentSession` /
// `@ai-sdk/react` / `agent-transport` 全真实组合（hook 即链路入口——归属参数经
// 真实链路到达 invoke）。
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
// fixture（serde camelCase 线格式；时钟不入 mock——updatedAt / startedAt 由
// fixture 显式给定，排序与显示断言取原始数值）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const OTHER_ROOT = 'C:\\demo\\beta';
const TS = 1_727_200_000_000;

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

function summary(turns: TurnSummary[], id: string): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'archi', sourceRef: null },
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

/** 清单应答函数（按 offset 分派 / 可切 reject）。 */
type ListHandler = (args: { offset: number; limit: number }) => Promise<SessionPage>;

interface IpcState {
  detail: SessionSummary | null | Error;
  transcript: AgentEvent[] | Error;
  watch: 'deferred' | 'resolve' | 'reject';
  start: TurnSummary | Error;
  list: ListHandler;
}

let ipc: IpcState;

const watchResolvers: (() => void)[] = [];

function resetIpc(overrides: Partial<IpcState> = {}): void {
  ipc = {
    // 缺省 detail：首发会话行（early-resolve 后的重放取数有据可查，避免回放空表
    // 清空镜像的竞态）
    detail: summary([turn(1, 'running', 'ses-1')], 'ses-1'),
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
    if (command === 'list_archi_chat_sessions') {
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

type Params = ArchiChatParams;

function baseParams(overrides: Partial<Params> = {}): Params {
  return {
    root: ROOT,
    ...overrides,
  };
}

async function mounted(overrides: Partial<Params> = {}) {
  const rendered = renderHook((props: Params) => useArchiChat(props), {
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
  text = '这个 workspace 的架构是什么',
  permissionMode: AgentPermissionMode = 'bypassPermissions',
): void {
  act(() => {
    send({ text, permissionMode });
  });
}

// ---------------------------------------------------------------------------
// 归属参数（AC-11）：source='archi' / sourceRef=null / cwd=null（后端回落主仓根）
// ---------------------------------------------------------------------------

describe('useArchiChat：归属参数', () => {
  it('空态发送：agent_start 携 source=archi / sourceRef=null / cwd=null / sessionId=null', async () => {
    const { result } = await mounted();

    sendInput(result.current.send);
    await act(async () => {});

    expect(callsOf('agent_start')).toHaveLength(1);
    expect(argsOf('agent_start')).toMatchObject({
      root: ROOT,
      prompt: '这个 workspace 的架构是什么',
      sessionId: null,
      source: 'archi',
      sourceRef: null,
      cwd: null,
      permissionMode: 'bypassPermissions',
    });
    expect(Object.keys(argsOf('agent_start')).sort(), '参数面逐字（三参归属 + 会话引用）').toEqual(
      [
        'onEvent',
        'root',
        'prompt',
        'permissionMode',
        'sessionId',
        'source',
        'sourceRef',
        'agent',
        'cwd',
      ].sort(),
    );
  });

  it('首发由后端铸行：early-resolve 轮行回写 currentSessionId，后续发送以该 id 续会话', async () => {
    const { result } = await mounted();

    sendInput(result.current.send);
    await act(async () => {});

    expect(result.current.currentSessionId, 'early-resolve 轮行回写会话 id').toBe('ses-1');
    expect(result.current.sessionId, '面板本地寻址输入仍为 null（id 以后端铸造为真相）').toBeNull();

    // 轮收口（`Record` 信封关流）后续会话：同源归属参数不改写
    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(1, 'completed', 'ses-1'));
    await act(async () => {});
    await act(async () => {});
    sendInput(result.current.send, '继续');
    await act(async () => {});
    expect(argsOf('agent_start')['sessionId']).toBe('ses-1');
    expect(argsOf('agent_start')['source']).toBe('archi');
  });
});

// ---------------------------------------------------------------------------
// 历史清单探测面（AC-12）：挂载零取数 / 展开取首屏 / 分页累加 / 两处重取事实
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

describe('useArchiChat：历史清单取数', () => {
  it('挂载零清单 invoke；展开取首屏一次；收起再展开再取一次（收起不清空）', async () => {
    ipc.list = pagedSessions(2);
    const { result } = await mounted();

    expect(callsOf('list_archi_chat_sessions'), '挂载零取数（零轮询 / 零首发取数）').toHaveLength(
      0,
    );

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(callsOf('list_archi_chat_sessions')).toHaveLength(1);
    expect(argsOf('list_archi_chat_sessions')).toEqual({ root: ROOT, offset: 0, limit: 20 });
    expect(result.current.listOpen).toBe(true);
    expect(result.current.sessions).toHaveLength(2);
    expect(result.current.total).toBe(2);

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.listOpen).toBe(false);
    expect(result.current.sessions, '收起不清空').toHaveLength(2);

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(callsOf('list_archi_chat_sessions'), '再展开再取一次首屏').toHaveLength(2);
    expect(argsOf('list_archi_chat_sessions')['offset']).toBe(0);
  });

  it('分页累加：loadMore 以已加载条数为 offset 追加；满载后不再发', async () => {
    ipc.list = pagedSessions(25);
    const { result } = await mounted();

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(20);
    expect(result.current.total).toBe(25);

    act(() => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(argsOf('list_archi_chat_sessions')).toEqual({ root: ROOT, offset: 20, limit: 20 });
    expect(result.current.sessions).toHaveLength(25);
    expect(result.current.total, 'total 保持 25').toBe(25);

    // 满载后「加载更多」入口的消失由骨架用例承载（`sessions.length < total` 判据）；
    // hook 侧口径：再次 loadMore 以已加载条数为 offset 取下一页，空页零追加、total 不变
    act(() => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(argsOf('list_archi_chat_sessions')).toEqual({ root: ROOT, offset: 25, limit: 20 });
    expect(result.current.sessions, '空页零追加').toHaveLength(25);
    expect(result.current.total).toBe(25);
  });

  it('两处重取事实：首发确立会话 id 与轮收口各重取一次首屏', async () => {
    ipc.list = pagedSessions(2);
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    const baseline = callsOf('list_archi_chat_sessions').length;

    // ① early-resolve 回写会话 id → 重取首屏（offset 0）
    sendInput(result.current.send);
    await act(async () => {});
    await act(async () => {});
    expect(result.current.currentSessionId).toBe('ses-1');
    expect(callsOf('list_archi_chat_sessions').length, '事实 ① 恰一次').toBe(baseline + 1);
    expect(argsOf('list_archi_chat_sessions')['offset']).toBe(0);

    // ② 轮收口（running true → false）→ 再重取首屏一次
    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(1, 'completed', 'ses-1'));
    await act(async () => {});
    await waitFor(() => expect(result.current.session.running).toBe(false));
    expect(callsOf('list_archi_chat_sessions').length, '事实 ② 恰一次').toBe(baseline + 2);
    expect(argsOf('list_archi_chat_sessions')['offset']).toBe(0);
  });

  it('清单失败：listError 为错误串且加载态归位；已加载条目保留', async () => {
    ipc.list = pagedSessions(25);
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(20);

    ipc.list = () => Promise.reject(new Error('库不可读'));
    act(() => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(result.current.listError).toContain('库不可读');
    expect(result.current.listLoading).toBe(false);
    expect(result.current.sessions, '分页失败保留已加载条目').toHaveLength(20);

    // 首屏失败（另一种形态）：items 为空
    resetIpc({ list: () => Promise.reject(new Error('库不可读')) });
    const fresh = await mounted();
    act(() => {
      fresh.result.current.toggleList();
    });
    await act(async () => {});
    expect(fresh.result.current.listError).toContain('库不可读');
    expect(fresh.result.current.listLoading).toBe(false);
    expect(fresh.result.current.sessions).toEqual([]);
    expect(fresh.result.current.total).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 新会话 / 历史点选（AC-12）：新会话零落库、历史以 sessionId 直查重放
// ---------------------------------------------------------------------------

describe('useArchiChat：新会话与历史点选', () => {
  it('newSession 回空态并清空会话镜像（零 agent_start、零写库类命令）', async () => {
    ipc.detail = summary([turn(11, 'completed', 'ses-1')], 'ses-1');
    const { result } = await mounted();
    act(() => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    await act(async () => {});
    expect(result.current.currentSessionId).toBe('ses-1');

    const startsBefore = callsOf('agent_start').length;
    act(() => {
      result.current.newSession();
    });
    await act(async () => {});

    expect(result.current.sessionId).toBeNull();
    expect(result.current.currentSessionId).toBeNull();
    expect(result.current.session.session, '会话镜像清空').toBeNull();
    expect(result.current.session.messages).toEqual([]);
    expect(result.current.session.events).toEqual([]);
    expect(result.current.session.chain).toEqual([]);
    expect(callsOf('agent_start').length, '新会话零落库').toBe(startsBefore);
    for (const writeCommand of [
      'archive_change',
      'create_change',
      'change_flow_start',
      'agent_stop',
    ]) {
      expect(callsOf(writeCommand), `${writeCommand} 不应触发`).toHaveLength(0);
    }
  });

  it('selectSession 以 sessionId 直查重放并收起清单（清单只作探测）', async () => {
    ipc.detail = summary([turn(12, 'completed', 'ses-2')], 'ses-2');
    const { result } = await mounted();
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    const listBefore = callsOf('list_archi_chat_sessions').length;

    act(() => {
      result.current.selectSession('ses-2');
    });
    await act(async () => {});
    await act(async () => {});

    expect(result.current.sessionId).toBe('ses-2');
    expect(result.current.currentSessionId).toBe('ses-2');
    expect(result.current.listOpen, '点选后清单收起').toBe(false);
    expect(argsOf('session_detail')).toEqual({ root: ROOT, sessionId: 'ses-2' });
    expect(Object.keys(argsOf('session_detail')).sort()).toEqual(['root', 'sessionId']);
    expect(argsOf('agent_session_transcript')).toEqual({ root: ROOT, sessionId: 'ses-2' });
    // 寻址恒走 session_detail（上两断言），清单只作探测：`currentSessionId` 由 null →
    // ses-2 属 D10「会话 id 确立」的首屏重取（`offset = 0`，与既有 `use-change-chat`
    // 的 `useListRefetch` 同构）——故取点选前后差值，而非绝对计数。
    const listCalls = callsOf('list_archi_chat_sessions');
    expect(
      listCalls.length - listBefore,
      '点选不额外做清单寻址（至多触发一次会话 id 确立的首屏重取）',
    ).toBeLessThanOrEqual(1);
    for (const [, listArgs] of listCalls) {
      expect(
        listArgs,
        '清单调用恒为 {root, offset: 0, limit: 20}（带 sessionId 即寻址违例）',
      ).toEqual({ root: ROOT, offset: 0, limit: 20 });
    }
  });
});

// ---------------------------------------------------------------------------
// 运行中禁发判据（AC-12）：唯一判据 = 本会话 running；历史浏览不受影响
// ---------------------------------------------------------------------------

describe('useArchiChat：运行中禁发', () => {
  it('running → canSend / canStart 皆 false、理由逐字、send 零第二发', async () => {
    ipc.detail = summary([turn(21, 'running', 'ses-1')], 'ses-1');
    const { result } = await mounted();
    act(() => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    await act(async () => {});

    expect(result.current.session.running).toBe(true);
    expect(result.current.canSend).toBe(false);
    expect(result.current.canStart).toBe(false);
    expect(result.current.disabledReason).toBe('本会话运行中，请等待收口');

    const startsBefore = callsOf('agent_start').length;
    sendInput(result.current.send);
    await act(async () => {});
    expect(callsOf('agent_start').length, '运行中禁发（零第二发）').toBe(startsBefore);
  });

  it('同拍历史浏览仍可用：toggleList / loadMore / selectSession 零拦截', async () => {
    ipc.detail = summary([turn(22, 'running', 'ses-1')], 'ses-1');
    ipc.list = pagedSessions(25);
    const { result } = await mounted();
    act(() => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    await act(async () => {});
    expect(result.current.session.running).toBe(true);

    const listBefore = callsOf('list_archi_chat_sessions').length;
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(
      callsOf('list_archi_chat_sessions').length - listBefore,
      '运行中仍可取清单（展开历史即发清单请求；点选确立会话 id 已先行触发过首屏重取）',
    ).toBeGreaterThan(0);
    expect(result.current.sessions).toHaveLength(20);

    act(() => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(result.current.sessions).toHaveLength(25);

    act(() => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    await act(async () => {});
    expect(result.current.sessionId).toBe('ses-1');
  });
});

// ---------------------------------------------------------------------------
// 状态行两态（`note`）：空态草案文案 / 有会话刷新提醒 / 运行中理由优先
// ---------------------------------------------------------------------------

const DRAFT_NOTE = '发送首条消息即开启新会话';
const REFRESH_NOTE = '模型改动不会自动刷新，改完点「刷新」';
const RUNNING_NOTE = '本会话运行中，请等待收口';

describe('useArchiChat：状态行 note', () => {
  it('空态 → 草案文案；有会话且非运行中 → 刷新提醒（恒占位）', async () => {
    const { result } = await mounted();
    expect(result.current.currentSessionId).toBeNull();
    expect(result.current.note).toBe(DRAFT_NOTE);

    sendInput(result.current.send);
    await act(async () => {});
    expect(result.current.currentSessionId, 'early-resolve 回写会话 id').toBe('ses-1');
    expect(result.current.note, '运行中理由优先').toBe(RUNNING_NOTE);

    ipc.detail = summary([turn(31, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(31, 'completed', 'ses-1'));
    await act(async () => {});
    await waitFor(() => expect(result.current.session.running).toBe(false));
    expect(result.current.currentSessionId).toBe('ses-1');
    expect(result.current.note, '有会话且非运行中 → 刷新提醒（不落 null）').toBe(REFRESH_NOTE);
  });

  it('理由优先于两种常态文案：运行中即便有空态 / 有会话两态均取理由', async () => {
    ipc.detail = summary([turn(32, 'running', 'ses-2')], 'ses-2');
    const { result } = await mounted();
    act(() => {
      result.current.selectSession('ses-2');
    });
    await act(async () => {});
    await act(async () => {});

    expect(result.current.disabledReason).toBe(RUNNING_NOTE);
    expect(result.current.note).toBe(RUNNING_NOTE);
    expect(result.current.note, '不落回草案 / 刷新提醒').not.toBe(DRAFT_NOTE);
    expect(result.current.note).not.toBe(REFRESH_NOTE);
  });
});

// ---------------------------------------------------------------------------
// root 两态与切根复位（AC-11 边界）：root = null → 全命令零 invoke；切根复位
// ---------------------------------------------------------------------------

describe('useArchiChat：root 两态与切根复位', () => {
  it('root = null → 全会话命令零 invoke，send / toggleList 为空操作不抛错', async () => {
    const { result } = await mounted({ root: null });

    sendInput(result.current.send);
    await act(async () => {});
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});

    for (const command of [
      'list_archi_chat_sessions',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
      'agent_start',
    ]) {
      expect(callsOf(command), `root = null → ${command} 零 invoke`).toHaveLength(0);
    }
    expect(result.current.sessionId).toBeNull();
    expect(result.current.note).toBe(DRAFT_NOTE);
  });

  it('root 由 null → 有值后取数恢复正常', async () => {
    ipc.list = pagedSessions(1);
    const { result, rerender } = await mounted({ root: null });

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(callsOf('list_archi_chat_sessions')).toHaveLength(0);

    await rerenderWith(rerender, { root: ROOT });
    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(callsOf('list_archi_chat_sessions')).toHaveLength(1);
    expect(result.current.sessions).toHaveLength(1);
  });

  it('root A → B：面板态复位、在途清单请求作废（A 库残留页不落 B 库）', async () => {
    const deferred: { resolve: ((page: SessionPage) => void) | null } = { resolve: null };
    ipc.list = () =>
      new Promise<SessionPage>((resolve) => {
        deferred.resolve = resolve;
      });
    const { result, rerender } = await mounted({ root: ROOT });

    act(() => {
      result.current.toggleList();
    });
    await act(async () => {});
    expect(result.current.sessions, 'A 库请求在途').toHaveLength(0);

    // 未落地前切根：旧根在途请求作废（票号复位）
    await rerenderWith(rerender, { root: OTHER_ROOT });
    await act(async () => {
      deferred.resolve?.({ sessions: [summary([], 'ses-a')], total: 1 });
    });
    expect(result.current.sessions, 'A 库残留页不落 B 库').toEqual([]);
    expect(result.current.total).toBe(0);
    expect(result.current.sessionId).toBeNull();
    expect(result.current.listOpen).toBe(false);
    expect(result.current.listLoading).toBe(false);
    expect(result.current.listError).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 骨架契约继承：返回对象即 SessionPanelState（12 字段）+ 面板专有面
// ---------------------------------------------------------------------------

describe('useArchiChat：骨架契约继承', () => {
  it('返回对象可赋给 SessionPanelState，且另含面板专有字段', async () => {
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
    for (const key of [
      'sessionId',
      'currentSessionId',
      'send',
      'stop',
      'canSend',
      'disabledReason',
    ] as const) {
      expect(result.current, `面板专有字段 ${key} 在场`).toHaveProperty(key);
    }
    expect(result.current.canSend).toBe(true);
    expect(result.current.disabledReason).toBeNull();
  });
});
