import { readFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';

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

import { useConfigChat, type ConfigChatParams } from './use-config-chat';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：`@tauri-apps/api/core` 的 invoke 按命令名分派
// （agent_start / agent_stop / session_detail / agent_session_transcript /
// agent_session_watch / list_config_chat_sessions）；Channel 为可编程类，用例直投
// `{ ipc: 'record' }` 信封驱动 early-resolve 轮行与收口。`useAgentSession` /
// `@ai-sdk/react` / `agent-transport` 全真实组合（hook 即链路入口——归属参数经
// 真实链路到达 invoke）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    public onmessage: ((message: unknown) => void) | null = null;
    public static instances: ChannelMock[] = [];
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
const TS = 1_727_400_000_000;

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
      provenance: { source: 'config', sourceRef: null },
      createdAt: TS,
      updatedAt: TS + 9,
    },
    stats: {
      turnCount: turns.length,
      totalDurationMs: null,
      inputTokens: null,
      outputTokens: null,
      cachedInputTokens: null,
      cacheCreationInputTokens: null,
    },
    turns,
    preamble: null,
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
    // 缺省 detail：首发会话行（early-resolve 后的重放取数有据可查）
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
    if (command === 'list_config_chat_sessions') {
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

type Params = ConfigChatParams;

function baseParams(overrides: Partial<Params> = {}): Params {
  return { root: ROOT, ...overrides };
}

async function mounted(overrides: Partial<Params> = {}) {
  const rendered = renderHook((props: Params) => useConfigChat(props), {
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
  text = '这个 workspace 的配置该怎么写',
  permissionMode: AgentPermissionMode = 'bypassPermissions',
): void {
  act(() => {
    send({ text, permissionMode });
  });
}

async function openHistory(result: { current: { onOpenHistory: () => void } }): Promise<void> {
  await act(async () => {
    result.current.onOpenHistory();
  });
  await act(async () => {});
}

// ---------------------------------------------------------------------------
// 归属参数（AC-7）：source='config' / sourceRef=null / cwd=null（后端回落主仓根）
// ---------------------------------------------------------------------------

describe('useConfigChat：归属参数', () => {
  it('空态发送：agent_start 携 source=config / sourceRef=null / cwd=null / sessionId=null', async () => {
    const { result } = await mounted();
    expect(callsOf('agent_start')).toHaveLength(0);

    sendInput(result.current.send, '先看盘面');

    await waitFor(() => expect(callsOf('agent_start')).toHaveLength(1));
    expect(argsOf('agent_start')).toMatchObject({
      root: ROOT,
      prompt: '先看盘面',
      sessionId: null,
      source: 'config',
      sourceRef: null,
      cwd: null,
      permissionMode: 'bypassPermissions',
    });
  });

  it('首发由后端铸行：early-resolve 轮行回写 currentSessionId，收口后以该 id 续会话', async () => {
    const { result } = await mounted();
    sendInput(result.current.send, '第一条');
    await waitFor(() => expect(result.current.currentSessionId).toBe('ses-1'));

    // 首轮收口（运行中禁发是判据本体，此处先让轮行收口再发第二条）
    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(1, 'completed', 'ses-1'));
    await waitFor(() => expect(result.current.session.running).toBe(false));

    sendInput(result.current.send, '第二条');
    await waitFor(() => expect(callsOf('agent_start')).toHaveLength(2));
    expect(argsOf('agent_start')).toMatchObject({ sessionId: 'ses-1', source: 'config' });
  });
});

// ---------------------------------------------------------------------------
// 历史清单取数（AC-7）：打开才取、分页累加、两处事实驱动重取
// ---------------------------------------------------------------------------

describe('useConfigChat：历史清单取数', () => {
  it('挂载零清单 invoke；每次打开重取首屏一次（替换而非累加）', async () => {
    let page = 0;
    ipc.list = () => {
      page += 1;
      return Promise.resolve({
        sessions: [summary([], 'ses-1'), summary([], 'ses-2')],
        total: 2,
      });
    };
    const { result } = await mounted();
    expect(callsOf('list_config_chat_sessions'), '挂载零取数').toHaveLength(0);

    await openHistory(result);
    expect(callsOf('list_config_chat_sessions')).toHaveLength(1);
    expect(argsOf('list_config_chat_sessions')).toEqual({ root: ROOT, offset: 0, limit: 20 });
    expect(result.current.sessions).toHaveLength(2);
    expect(result.current.total).toBe(2);

    await openHistory(result);
    expect(callsOf('list_config_chat_sessions')).toHaveLength(2);
    expect(result.current.sessions, 'offset = 0 走替换（非累加）').toHaveLength(2);
    expect(page).toBe(2);
  });

  it('分页追加：loadMore 以已加载条数为 offset 累加；满载后判据关闭', async () => {
    const all = Array.from({ length: 25 }, (_, index) => summary([], `ses-${index}`));
    ipc.list = ({ offset, limit }) =>
      Promise.resolve({ sessions: all.slice(offset, offset + limit), total: all.length });
    const { result } = await mounted();

    await openHistory(result);
    expect(result.current.sessions).toHaveLength(20);
    expect(result.current.total).toBe(25);
    expect(result.current.sessions.length < result.current.total).toBe(true);

    await act(async () => {
      result.current.loadMore();
    });
    await act(async () => {});
    expect(argsOf('list_config_chat_sessions')).toEqual({
      root: ROOT,
      offset: 20,
      limit: 20,
    });
    expect(result.current.sessions).toHaveLength(25);
    expect(result.current.sessions.length < result.current.total, '满载后不再有下一页').toBe(false);
  });

  it('两处重取事实：首发确立会话 id 与轮收口各重取一次首屏', async () => {
    const { result } = await mounted();
    expect(callsOf('list_config_chat_sessions')).toHaveLength(0);

    sendInput(result.current.send, '开启会话');
    await waitFor(() => expect(result.current.currentSessionId).toBe('ses-1'));
    await waitFor(() => expect(callsOf('list_config_chat_sessions')).toHaveLength(1));
    expect(argsOf('list_config_chat_sessions')).toEqual({ root: ROOT, offset: 0, limit: 20 });

    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(1, 'completed', 'ses-1'));
    await waitFor(() => expect(callsOf('list_config_chat_sessions')).toHaveLength(2));
    expect(argsOf('list_config_chat_sessions')).toEqual({ root: ROOT, offset: 0, limit: 20 });
  });

  it('清单失败：listError 为错误串且加载态归位；下次打开重取成功即清错', async () => {
    ipc.list = () => Promise.reject(new Error('canonicalize 失败'));
    const { result } = await mounted();

    await openHistory(result);
    await waitFor(() => expect(result.current.listError).toContain('canonicalize'));
    expect(result.current.listLoading).toBe(false);
    expect(result.current.sessions).toHaveLength(0);

    ipc.list = () => Promise.resolve({ sessions: [summary([], 'ses-1')], total: 1 });
    await openHistory(result);
    await waitFor(() => expect(result.current.listError).toBeNull());
    expect(result.current.sessions).toHaveLength(1);
  });

  it('连点历史与收口重取交错 → 只有最新一拍落地（旧页不覆盖新页）', async () => {
    const stale = summary([], 'ses-stale');
    const fresh = summary([], 'ses-fresh');
    const pending: ((page: SessionPage) => void)[] = [];
    ipc.list = () =>
      new Promise<SessionPage>((resolve) => {
        pending.push(resolve);
      });
    const { result } = await mounted();

    await act(async () => {
      result.current.onOpenHistory();
    });
    await act(async () => {
      result.current.onOpenHistory();
    });
    expect(pending).toHaveLength(2);

    // 先回旧请求、后回新请求：票号保证旧页作废
    await act(async () => {
      pending[0]!({ sessions: [stale], total: 1 });
    });
    await act(async () => {
      pending[1]!({ sessions: [fresh], total: 1 });
      await Promise.resolve();
    });

    expect(result.current.sessions.map((item) => item.row.id)).toEqual(['ses-fresh']);
  });
});

// ---------------------------------------------------------------------------
// 新会话与历史点选
// ---------------------------------------------------------------------------

describe('useConfigChat：新会话与历史点选', () => {
  it('newSession 回空态并清空会话镜像（零 agent_start、零铸造 invoke）', async () => {
    const { result } = await mounted();
    sendInput(result.current.send, '开启会话');
    await waitFor(() => expect(result.current.currentSessionId).toBe('ses-1'));
    const startsBefore = callsOf('agent_start').length;

    await act(async () => {
      result.current.newSession();
    });
    await act(async () => {});

    expect(result.current.sessionId).toBeNull();
    expect(result.current.currentSessionId).toBeNull();
    expect(callsOf('agent_start').length, '「新会话」零落库').toBe(startsBefore);
    expect(result.current.note).toBe('发送首条消息即开启新会话');
  });

  it('selectSession 以该 id 直查重放（零 source 反查）', async () => {
    ipc.list = () =>
      Promise.resolve({ sessions: [summary([], 'ses-1'), summary([], 'ses-2')], total: 2 });
    ipc.detail = summary([turn(21, 'completed', 'ses-2')], 'ses-2');
    ipc.transcript = [
      {
        seq: 0,
        timestampMs: TS,
        kind: 'message',
        role: 'assistant',
        blocks: [{ kind: 'text', text: '历史会话正文' }],
        parentToolUseId: null,
      },
    ];
    const { result } = await mounted();
    await openHistory(result);

    await act(async () => {
      result.current.selectSession('ses-2');
    });
    await act(async () => {});
    await act(async () => {});

    expect(result.current.sessionId).toBe('ses-2');
    expect(argsOf('session_detail')).toEqual({ root: ROOT, sessionId: 'ses-2' });
    expect(argsOf('agent_session_transcript')).toEqual({
      root: ROOT,
      sessionId: 'ses-2',
    });
    // 零 source 反查：清单命令只出现在打开历史的取数里，无「按来源查会话」的调用
    const listCalls = callsOf('list_config_chat_sessions').length;
    expect(listCalls).toBeGreaterThan(0);
    for (const [, args] of callsOf('list_config_chat_sessions') as [string, object][]) {
      expect(Object.keys(args).sort()).toEqual(['limit', 'offset', 'root']);
    }
  });

  it('历史选中的会话不存在（session_detail reject）→ error 呈现、sessionId 保留可重试', async () => {
    ipc.detail = new Error('查无此 id');
    const { result } = await mounted();

    await act(async () => {
      result.current.selectSession('ses-gone');
    });
    await act(async () => {});
    await act(async () => {});

    await waitFor(() => expect(result.current.session.error).toContain('查无此 id'));
    expect(result.current.sessionId, '寻址保留（可重试）').toBe('ses-gone');
    expect(result.current.listError, '清单不受污染').toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 禁发矩阵与状态行（单源判据）
// ---------------------------------------------------------------------------

describe('useConfigChat：运行中禁发与状态行', () => {
  it('运行中 → canSend / canStart 皆 false、理由逐字、send 零第二发', async () => {
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    const { result } = await mounted();

    await act(async () => {
      result.current.selectSession('ses-1');
    });
    await act(async () => {});
    await act(async () => {});

    await waitFor(() => expect(result.current.session.running).toBe(true));
    expect(result.current.canSend).toBe(false);
    expect(result.current.canStart).toBe(false);
    expect(result.current.disabledReason).toBe('本会话运行中，请等待收口');
    expect(result.current.note).toBe(result.current.disabledReason);

    const startsBefore = callsOf('agent_start').length;
    sendInput(result.current.send, '运行中不该发出去');
    expect(callsOf('agent_start').length, '运行中零第二发').toBe(startsBefore);
  });

  it('空态即可发送：canSend / canStart true、disabledReason null', async () => {
    const { result } = await mounted();
    expect(result.current.canSend).toBe(true);
    expect(result.current.canStart).toBe(true);
    expect(result.current.disabledReason).toBeNull();
  });

  it.each([
    ['空态', null, '发送首条消息即开启新会话'],
    ['已开会话空闲', 'ses-1', '配置写回只在提案卡上点「应用」生效'],
  ])('状态行三态之一（%s）→ note 逐字', async (_label, sessionId, expected) => {
    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    const { result } = await mounted();
    if (sessionId !== null) {
      await act(async () => {
        result.current.selectSession(sessionId);
      });
      await act(async () => {});
      await act(async () => {});
      await waitFor(() => expect(result.current.currentSessionId).toBe('ses-1'));
    }
    expect(result.current.note).toBe(expected);
  });
});

// ---------------------------------------------------------------------------
// root 两态与切根复位
// ---------------------------------------------------------------------------

describe('useConfigChat：root 两态与切根复位', () => {
  it('root = null → 会话命令零 invoke；canSend 不被禁用（唯一判据是 running）', async () => {
    const { result } = await mounted({ root: null });

    sendInput(result.current.send, '无根发送');
    await openHistory(result);

    for (const command of [
      'agent_start',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
      'list_config_chat_sessions',
    ]) {
      expect(callsOf(command), `root = null → ${command} 零 invoke`).toHaveLength(0);
    }
    expect(result.current.canSend).toBe(true);
    expect(result.current.disabledReason).toBeNull();
  });

  it('root 由 null → 有值后取数恢复正常', async () => {
    const { result, rerender } = await mounted({ root: null });
    await rerenderWith(rerender, { root: ROOT });

    await openHistory(result);
    expect(callsOf('list_config_chat_sessions')).toHaveLength(1);
    expect(argsOf('list_config_chat_sessions')).toMatchObject({ root: ROOT });

    sendInput(result.current.send, '有根发送');
    await waitFor(() => expect(callsOf('agent_start')).toHaveLength(1));
  });

  it('root A → B：面板态复位、在途清单请求作废（A 库残留页不落 B 库）', async () => {
    ipc.list = ({ offset }) =>
      Promise.resolve({
        sessions: [summary([], `ses-a-${offset}`)],
        total: 1,
      });
    const { result, rerender } = await mounted();
    await openHistory(result);
    expect(result.current.sessions).toHaveLength(1);

    // 在途请求：切根前发出的页不得落到新根
    const pending: ((page: SessionPage) => void)[] = [];
    ipc.list = () =>
      new Promise<SessionPage>((resolve) => {
        pending.push(resolve);
      });
    await act(async () => {
      result.current.onOpenHistory();
    });
    expect(pending).toHaveLength(1);

    await rerenderWith(rerender, { root: OTHER_ROOT });
    expect(result.current.sessions, '切根即复位清单').toHaveLength(0);
    expect(result.current.sessionId).toBeNull();

    await act(async () => {
      pending[0]!({ sessions: [summary([], 'ses-a-stale')], total: 1 });
      await Promise.resolve();
    });
    expect(result.current.sessions, '旧根在途页作废').toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 边界：分页可用性与发送纪律结构锚
// ---------------------------------------------------------------------------

describe('useConfigChat：分页可用性与发送纪律', () => {
  it.each([
    ['空清单', 0, 0, false],
    ['超出页宽', 20, 25, true],
  ])(
    '分页判据（%s）：sessions.length < total 驱动 loadMore',
    async (_label, loaded, total, more) => {
      const all = Array.from({ length: total }, (_, index) => summary([], `ses-${index}`));
      ipc.list = ({ offset, limit }) =>
        Promise.resolve({ sessions: all.slice(offset, offset + limit), total });
      const { result } = await mounted();
      await openHistory(result);
      expect(result.current.sessions).toHaveLength(loaded);
      expect(result.current.sessions.length < result.current.total).toBe(more);
    },
  );

  it('发送纪律结构锚：源码零 `commands.` 直调（一律经 ipc），来源常量字面量为 config', () => {
    const source = readFileSync(
      resolvePath(process.cwd(), 'src/views/config/hooks/use-config-chat.ts'),
      'utf8',
    );
    expect(source).not.toContain('commands.');
    expect(source).toContain('listConfigChatSessions');
    // 插桩不敏感形态：Stryker 把初始化式包成
    // `stryMutAct_*(...) ? "" : (stryCov_*(...), 'config')`，逐字整行比对在沙箱内
    // 会失效——只锚「声明 + 同行字面量」两段，归属值的行为判据另由 agent_start 入参断言承担。
    expect(source).toMatch(/const CONFIG_CHAT_SOURCE =[^\n]*['"]config['"]/);
  });
});

/** 编译期锚定：返回对象可赋给 `SessionPanelState` 且另含面板专有字段。 */
describe('useConfigChat：骨架契约继承', () => {
  it('返回对象满足 SessionPanelState 并带 sessionId / currentSessionId / send / stop', async () => {
    const { result } = await mounted();
    const asPanel: SessionPanelState = result.current;
    expect(asPanel.canStart).toBe(true);
    expect(asPanel.note).toBe('发送首条消息即开启新会话');
    expect(result.current.sessionId).toBeNull();
    expect(result.current.currentSessionId).toBeNull();
    expect(typeof result.current.send).toBe('function');
    expect(typeof result.current.stop).toBe('function');
  });
});
