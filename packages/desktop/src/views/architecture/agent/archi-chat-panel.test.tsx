import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ResizablePanelGroup } from '@/components/ui/resizable';

import type { AgentEvent, SessionPage, SessionSummary, TurnSummary } from '../../../types/dto';
import { ArchiChatPanel, type ArchiChatPanelProps } from './archi-chat-panel';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：仅 mock `@tauri-apps/api/core`（invoke 按命令名分派
// 六命令 + 可编程 Channel）；`useArchiChat` / `SessionPanel` / `ArchiChatComposer` /
// `AgentSessionView` 全真实组合——组合用例入口即本组件（链路最上层调用方：面板 →
// hook → 会话基建 → transport → agent_start）。
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
// fixture（serde camelCase 线格式；时钟不入 mock——updatedAt / startedAt 由 fixture
// 显式给定）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
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

interface IpcState {
  detail: SessionSummary | null | Error;
  transcript: AgentEvent[] | Error;
  watch: 'deferred' | 'resolve';
  start: TurnSummary | Error;
  list: () => Promise<SessionPage>;
}

let ipc: IpcState;

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
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'session_detail') {
      return ipc.detail instanceof Error ? Promise.reject(ipc.detail) : Promise.resolve(ipc.detail);
    }
    if (command === 'agent_session_transcript') {
      return ipc.transcript instanceof Error
        ? Promise.reject(ipc.transcript)
        : Promise.resolve(ipc.transcript);
    }
    if (command === 'agent_session_watch') {
      return ipc.watch === 'resolve' ? Promise.resolve(null) : new Promise<null>(() => {});
    }
    if (command === 'agent_start') {
      return ipc.start instanceof Error ? Promise.reject(ipc.start) : Promise.resolve(ipc.start);
    }
    if (command === 'list_archi_chat_sessions') {
      return ipc.list();
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

function callsOf(name: string): unknown[][] {
  return invokeMock.mock.calls.filter(([command]) => command === name);
}

function argsOf(name: string, index = -1): Record<string, unknown> {
  const calls = callsOf(name);
  const call = index < 0 ? calls.at(-1) : calls[index];
  if (!call) throw new Error(`${name} 未被调用`);
  return call[1] as Record<string, unknown>;
}

interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
}

function deliverRecord(record: TurnSummary): void {
  const instance = ChannelMock.instances.at(-1) as ChannelLike | undefined;
  if (!instance) throw new Error('尚未建立流（无 Channel 实例）');
  act(() => {
    instance.onmessage?.({ ipc: 'record', record });
  });
}

function baseProps(overrides: Partial<ArchiChatPanelProps> = {}): ArchiChatPanelProps {
  return {
    root: ROOT,
    ...overrides,
  };
}

/** 分栏宿主（面板骨架自身即 `ResizablePanel`，须挂在 `ResizablePanelGroup` 内）。 */
function panelUi(overrides: Partial<ArchiChatPanelProps> = {}) {
  return (
    <ResizablePanelGroup orientation="horizontal">
      <ArchiChatPanel {...baseProps(overrides)} />
    </ResizablePanelGroup>
  );
}

/** 面板分栏元素（骨架默认收起 → 以窄条定位；展开体同在同一个分栏面板内）。 */
function panelRoot(): HTMLElement {
  const anchor =
    screen.queryByTestId('archi-chat-actions') ?? screen.getByTestId('archi-chat-rail');
  return anchor.closest<HTMLElement>('[data-slot="resizable-panel"]')!;
}

async function mountPanel(overrides: Partial<ArchiChatPanelProps> = {}) {
  const rendered = render(panelUi(overrides));
  await act(async () => {});
  fireEvent.click(screen.getByTestId('archi-chat-expand'));
  await act(async () => {});
  await act(async () => {});
  return rendered;
}

function typeText(text: string): void {
  fireEvent.change(screen.getByTestId('archi-chat-prompt'), { target: { value: text } });
}

async function send(text: string): Promise<void> {
  typeText(text);
  await act(async () => {
    fireEvent.click(screen.getByTestId('archi-chat-send'));
  });
  await act(async () => {});
}

async function openHistory(): Promise<void> {
  await act(async () => {
    fireEvent.click(screen.getByTestId('archi-chat-history'));
  });
  await act(async () => {});
}

describe('ArchiChatPanel：骨架装配面', () => {
  it('骨架面板 / 头区 / 会话体在场，标题逐字，composer 插槽呈 archi-chat 前缀', async () => {
    await mountPanel();

    expect(panelRoot()).toBeTruthy();
    expect(screen.getByTestId('archi-chat-actions').textContent).toContain('架构 Agent');
    expect(screen.getByTestId('archi-chat-session')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-composer')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-prompt')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-send')).toBeTruthy();
    expect(
      screen.queryAllByTestId('change-chat-rail'),
      'testIdPrefix 与 change 侧不串味',
    ).toHaveLength(0);
    expect(screen.queryByTestId('change-chat-actions')).toBeNull();
  });
});

describe('ArchiChatPanel：归属链路（组合）', () => {
  it('空态发送 → agent_start 六字段逐字（source / sourceRef / cwd / sessionId / prompt / 档位）', async () => {
    await mountPanel();

    await send('架构现状是什么');

    expect(callsOf('agent_start')).toHaveLength(1);
    expect(argsOf('agent_start')).toMatchObject({
      root: ROOT,
      prompt: '架构现状是什么',
      sessionId: null,
      source: 'archi',
      sourceRef: null,
      cwd: null,
      permissionMode: 'bypassPermissions',
    });
  });

  it('首铸回写：early-resolve 轮行 → 会话态（session-meta）+ 清单重取一次', async () => {
    await mountPanel();
    expect(callsOf('list_archi_chat_sessions'), '挂载零取数').toHaveLength(0);

    await send('先看模型');

    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());
    expect(screen.getByTestId('session-meta-id').textContent).toBe('ses-1');
    await waitFor(() => expect(callsOf('list_archi_chat_sessions')).toHaveLength(1));
    expect(argsOf('list_archi_chat_sessions')).toEqual({ root: ROOT, offset: 0, limit: 20 });
  });
});

describe('ArchiChatPanel：运行中禁发（组合）', () => {
  it('运行中 → 状态行理由、输入 / 发送 / 新会话同时禁用、零第二发，历史仍可展开', async () => {
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    ipc.list = () => Promise.resolve({ sessions: [summary([], 'ses-1')], total: 1 });
    await mountPanel();

    await openHistory();
    const [item] = screen.getAllByTestId('archi-chat-history-item');
    await act(async () => {
      fireEvent.click(item);
    });
    await act(async () => {});
    await act(async () => {});

    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());
    expect(screen.getByTestId('archi-chat-note').textContent).toBe('本会话运行中，请等待收口');
    expect(screen.getByTestId<HTMLTextAreaElement>('archi-chat-prompt').disabled).toBe(true);
    expect(screen.getByTestId<HTMLButtonElement>('archi-chat-send').disabled).toBe(true);
    expect(screen.getByTestId<HTMLButtonElement>('archi-chat-new').disabled).toBe(true);

    const startsBefore = callsOf('agent_start').length;
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-send'));
    });
    expect(callsOf('agent_start').length, '运行中零第二发').toBe(startsBefore);

    // 同拍历史浏览仍可用（禁发不影响清单取数）
    const listBefore = callsOf('list_archi_chat_sessions').length;
    await openHistory();
    expect(callsOf('list_archi_chat_sessions').length).toBeGreaterThan(listBefore);
  });
});

describe('ArchiChatPanel：历史与「新会话」（组合）', () => {
  it('展开历史 → 点选直查重放并收起；点新会话 → 回空态、零落库', async () => {
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
    await mountPanel();

    await openHistory();
    const items = screen.getAllByTestId('archi-chat-history-item');
    expect(items).toHaveLength(2);

    await act(async () => {
      fireEvent.click(items[1]);
    });
    await act(async () => {});
    await act(async () => {});

    expect(argsOf('session_detail')).toEqual({ root: ROOT, sessionId: 'ses-2' });
    expect(screen.queryByTestId('archi-chat-history-list'), '点选后清单收起').toBeNull();
    await waitFor(() =>
      expect(screen.getByTestId('archi-chat-session').textContent).toContain('历史会话正文'),
    );

    const startsBefore = callsOf('agent_start').length;
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-new'));
    });
    await act(async () => {});

    expect(screen.queryByTestId('session-meta'), '新会话回空态（会话镜像清空）').toBeNull();
    expect(screen.getByTestId('archi-chat-note').textContent).toContain('发送首条消息即开启新会话');
    expect(callsOf('agent_start').length, '「新会话」零落库').toBe(startsBefore);
  });
});

describe('ArchiChatPanel：状态行两态', () => {
  it('空态呈草案文案；有会话且非运行中呈刷新提醒（恒占位）', async () => {
    await mountPanel();
    expect(screen.getByTestId('archi-chat-note').textContent).toBe('发送首条消息即开启新会话');

    await send('开启会话');
    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());
    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(1, 'completed', 'ses-1'));
    await act(async () => {});
    await waitFor(() =>
      expect(screen.getByTestId('archi-chat-note').textContent).toBe(
        '模型改动不会自动刷新，改完点「刷新」',
      ),
    );
  });
});

describe('ArchiChatPanel：折叠（组合）', () => {
  it('挂载默认收起只呈窄条；点展开入口进入展开体；头区「收起Agent」回窄条', async () => {
    const rendered = render(panelUi());

    expect(screen.getByTestId('archi-chat-rail').textContent).toContain('架构 Agent');
    expect(screen.getByTestId('archi-chat-expand').getAttribute('title')).toBe('展开架构 Agent');
    expect(screen.queryByTestId('archi-chat-actions')).toBeNull();
    expect(screen.queryByTestId('archi-chat-composer')).toBeNull();

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-expand'));
    });
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-composer')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-collapse'));
    });
    expect(screen.getByTestId('archi-chat-rail')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-actions')).toBeNull();
    rendered.unmount();
  });
});

describe('ArchiChatPanel：root 两态与零自动动作', () => {
  it('root = null → 面板在场且零会话 invoke；root 恢复后发送可用', async () => {
    const { rerender } = await mountPanel({ root: null });

    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    await send('无根发送');
    for (const command of [
      'agent_start',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
      'list_archi_chat_sessions',
    ]) {
      expect(callsOf(command), `root = null → ${command} 零 invoke`).toHaveLength(0);
    }

    rerender(panelUi({ root: ROOT }));
    await act(async () => {});
    await send('有根发送');
    expect(callsOf('agent_start')).toHaveLength(1);
  });

  it('全流程零自动刷新 / 零自动提交：零 archi_graph、零提交与归档类命令', async () => {
    await mountPanel();
    await send('改一个 gate');
    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());
    deliverRecord(turn(1, 'completed', 'ses-1'));
    await act(async () => {});
    await openHistory();

    for (const command of [
      'archi_graph',
      'archi_set_gate',
      'archive_change',
      'change_flow_start',
      'create_change',
    ]) {
      expect(callsOf(command), `${command} 不应由面板触发`).toHaveLength(0);
    }
  });
});
