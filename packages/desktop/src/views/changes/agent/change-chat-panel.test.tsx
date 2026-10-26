import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionPage, SessionSummary, TurnSummary } from '../../../types/dto';
import { ChangeChatPanel, type ChangeChatPanelProps } from './change-chat-panel';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：仅 mock `@tauri-apps/api/core`（invoke 按命令名
// 分派 + Channel 可编程类），面板 / useChangeChat / AgentSessionView /
// ChangeChatComposer / AgentInput 全部真实组合——组合用例入口即本组件（链路
// 最上层调用方：面板 → hook → 会话基建 → transport → agent_start）。
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
// fixture（serde camelCase 线格式）
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

function textEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

/** 历史清单项 fixture（id 前缀 8 字符呈于行内，完整 id 落 title）。 */
function historyItem(id: string): SessionSummary {
  return summary([turn(1, 'completed', id)], id);
}

// ---------------------------------------------------------------------------
// 可编程 IPC 应答（按命令名分派；用例内按需重置）
// ---------------------------------------------------------------------------

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
    detail: null,
    transcript: [],
    watch: 'deferred',
    start: turn(1, 'running', 'ses-1'),
    list: () => Promise.resolve({ sessions: [], total: 0 }),
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

function baseProps(overrides: Partial<ChangeChatPanelProps> = {}): ChangeChatPanelProps {
  return {
    root: ROOT,
    changeId: CHANGE_ID,
    cwd: WORKTREE,
    readOnly: false,
    runActive: false,
    collapsed: false,
    onToggleCollapsed: vi.fn(),
    ...overrides,
  };
}

async function mountPanel(overrides: Partial<ChangeChatPanelProps> = {}) {
  const rendered = render(<ChangeChatPanel {...baseProps(overrides)} />);
  await act(async () => {});
  await act(async () => {});
  return rendered;
}

/** 面板重渲染（change id / 只读 / run 态切换场景）。 */
async function rerenderPanel(
  rendered: ReturnType<typeof render>,
  overrides: Partial<ChangeChatPanelProps>,
): Promise<void> {
  await act(async () => {
    rendered.rerender(<ChangeChatPanel {...baseProps(overrides)} />);
  });
}

/** 输入框（面板正文挂载时方在场）。 */
function promptBox(): HTMLTextAreaElement {
  return screen.getByTestId('change-chat-prompt') as HTMLTextAreaElement;
}

function typeText(text: string): void {
  fireEvent.change(promptBox(), { target: { value: text } });
}

function sendButton(): HTMLButtonElement {
  return screen.getByTestId('change-chat-send') as HTMLButtonElement;
}

async function expandHistory(): Promise<void> {
  fireEvent.click(screen.getByTestId('change-chat-history'));
  await act(async () => {});
  await act(async () => {});
}

/** 选中第 index 条历史项（0 起）并落定重放取数。 */
async function selectHistoryItem(index: number): Promise<void> {
  const items = screen.getAllByTestId('change-chat-history-item');
  fireEvent.click(items[index]);
  await act(async () => {});
  await act(async () => {});
}

// ---------------------------------------------------------------------------
// 打开即空态（AC-4 / AC-5）：面板呈现、可发送、零会话类 invoke
// ---------------------------------------------------------------------------

describe('ChangeChatPanel：打开即空态', () => {
  it('挂载后空态可发送，且 session_detail / 转录 / 清单 invoke 均为零次', async () => {
    await mountPanel();

    expect(screen.getByTestId('change-chat-panel')).toBeTruthy();
    expect(promptBox().disabled).toBe(false);
    expect(sendButton().disabled, '空文本禁发（既有守卫）').toBe(true);

    typeText('首轮提问');
    expect(sendButton().disabled).toBe(false);

    expect(callsOf('session_detail')).toHaveLength(0);
    expect(callsOf('agent_session_transcript')).toHaveLength(0);
    expect(callsOf('list_change_chat_sessions')).toHaveLength(0);
    expect(screen.getByTestId('change-chat-note').textContent).toBe('发送首条消息即开启新会话');
  });

  it('空态首发即 New：agent_start 参数携 source / sourceRef / cwd 与 sessionId=null', async () => {
    await mountPanel();
    typeText('首轮提问');
    fireEvent.click(sendButton());
    await act(async () => {});
    await act(async () => {});

    expect(callsOf('agent_start')).toHaveLength(1);
    expect(argsOf('agent_start')).toMatchObject({
      root: ROOT,
      prompt: '首轮提问',
      sessionId: null,
      source: 'change',
      sourceRef: CHANGE_ID,
      cwd: WORKTREE,
    });
  });

  it('首发后轮行回写会话 id：session-meta-id 呈ses-1（不靠本地 sessionId 判空态）', async () => {
    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    ipc.transcript = [textEvent(0, '收到，开始读代码')];
    await mountPanel();
    typeText('首轮提问');
    fireEvent.click(sendButton());
    await act(async () => {});
    await act(async () => {});

    deliverRecord(turn(1, 'completed', 'ses-1'));
    await act(async () => {});
    await act(async () => {});

    await waitFor(() => expect(screen.getByTestId('session-meta-id').textContent).toBe('ses-1'));
    // messages 通知有节流窗：重放正文以终态等待
    await waitFor(() =>
      expect(screen.getByTestId('change-chat-panel').textContent).toContain('收到，开始读代码'),
    );
    expect(screen.queryByTestId('change-chat-note'), '有会话且无理由时不占位').toBeNull();
  });

  it('root = null → 面板在场但发送零 agent_start（不崩）', async () => {
    await mountPanel({ root: null });
    typeText('无 root');
    fireEvent.click(sendButton());
    await act(async () => {});

    expect(screen.getByTestId('change-chat-panel')).toBeTruthy();
    expect(callsOf('agent_start')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 历史清单面（AC-4）：展开取数 / 点选直查 / 新会话重置 / 分页 / 降级
// ---------------------------------------------------------------------------

describe('ChangeChatPanel：历史清单面', () => {
  it('展开历史 → 清单恰一次且参数 { root, id, offset: 0, limit: 20 }，应答 2 条呈 2 行', async () => {
    ipc.list = () =>
      Promise.resolve({ sessions: [historyItem('ses-1'), historyItem('ses-2')], total: 2 });
    await mountPanel();

    await expandHistory();

    expect(callsOf('list_change_chat_sessions')).toHaveLength(1);
    expect(argsOf('list_change_chat_sessions')).toEqual({
      root: ROOT,
      id: CHANGE_ID,
      offset: 0,
      limit: 20,
    });
    expect(screen.getAllByTestId('change-chat-history-item')).toHaveLength(2);
  });

  it('点选历史 → 会话直查重放（参数恰 { root, sessionId }，零 source / sourceRef）', async () => {
    ipc.list = () =>
      Promise.resolve({ sessions: [historyItem('ses-1'), historyItem('ses-2')], total: 2 });
    ipc.detail = summary([turn(21, 'completed', 'ses-2')], 'ses-2');
    ipc.transcript = [textEvent(0, '历史会话正文')];
    await mountPanel();
    await expandHistory();

    await selectHistoryItem(1);

    expect(callsOf('session_detail')).toHaveLength(1);
    expect(argsOf('session_detail')).toEqual({ root: ROOT, sessionId: 'ses-2' });
    expect(Object.keys(argsOf('session_detail')).sort()).toEqual(['root', 'sessionId']);
    expect(callsOf('agent_session_transcript')).toHaveLength(1);
    expect(Object.keys(argsOf('agent_session_transcript')).sort()).toEqual(['root', 'sessionId']);
    await waitFor(() => expect(screen.getByTestId('session-meta-id').textContent).toBe('ses-2'));
    // messages 通知有节流窗：重放正文以终态等待
    await waitFor(() =>
      expect(screen.getByTestId('change-chat-panel').textContent).toContain('历史会话正文'),
    );
  });

  it('新会话 → 对话回空、下一次发送 sessionId 为 null、旧会话条目仍可重开', async () => {
    ipc.list = () =>
      Promise.resolve({ sessions: [historyItem('ses-1'), historyItem('ses-2')], total: 2 });
    ipc.detail = summary([turn(21, 'completed', 'ses-2')], 'ses-2');
    ipc.transcript = [textEvent(0, '历史会话正文')];
    await mountPanel();
    await expandHistory();
    await selectHistoryItem(1);
    // messages 通知有节流窗：重放正文以终态等待
    await waitFor(() =>
      expect(screen.getByTestId('change-chat-panel').textContent).toContain('历史会话正文'),
    );

    fireEvent.click(screen.getByTestId('change-chat-new'));
    await act(async () => {});
    await act(async () => {});

    expect(screen.queryByTestId('session-meta-id'), '对话回空').toBeNull();
    expect(screen.getByTestId('change-chat-note').textContent).toBe('发送首条消息即开启新会话');

    typeText('新会话首轮');
    fireEvent.click(sendButton());
    await act(async () => {});
    expect(argsOf('agent_start')['sessionId']).toBeNull();

    // 旧会话仍可重开（清单条目保留，重新展开可点选）
    await expandHistory();
    expect(screen.getAllByTestId('change-chat-history-item')).toHaveLength(2);
  });

  it('清单空页 → 零条目 + 空占位文案，面板不崩', async () => {
    await mountPanel();
    await expandHistory();

    expect(screen.queryAllByTestId('change-chat-history-item')).toHaveLength(0);
    expect(screen.getByTestId('change-chat-history-empty').textContent).toBe('（暂无历史会话）');
    expect(promptBox().disabled).toBe(false);
  });

  it('分页累加：首屏 20 条呈「加载更多」，点击后 offset=20 累加到 25 条且入口消失', async () => {
    ipc.list = ({ offset, limit }) => {
      const sessions: SessionSummary[] = [];
      for (let index = offset; index < Math.min(offset + limit, 25); index += 1) {
        sessions.push(historyItem(`ses-${String(index).padStart(3, '0')}`));
      }
      return Promise.resolve({ sessions, total: 25 });
    };
    await mountPanel();
    await expandHistory();

    expect(screen.getAllByTestId('change-chat-history-item')).toHaveLength(20);
    expect(screen.getByTestId('change-chat-history-more')).toBeTruthy();

    fireEvent.click(screen.getByTestId('change-chat-history-more'));
    await act(async () => {});
    await act(async () => {});

    expect(argsOf('list_change_chat_sessions')['offset']).toBe(20);
    expect(screen.getAllByTestId('change-chat-history-item')).toHaveLength(25);
    expect(screen.queryByTestId('change-chat-history-more')).toBeNull();
  });

  it('清单取数失败 → 错误文案原样呈现，面板 / 历史入口 / 输入发送不受影响', async () => {
    ipc.list = () => Promise.reject('清单取数失败');
    await mountPanel();
    await expandHistory();

    expect(screen.getByTestId('change-chat-history-error').textContent).toContain('清单取数失败');
    expect(screen.getByTestId('change-chat-history')).toBeTruthy();
    expect(promptBox().disabled).toBe(false);
    typeText('失败后仍可发');
    expect(sendButton().disabled).toBe(false);
  });

  it('change id 变化 → 回空态、清单收起且旧条目不再场；再展开以新 id 取数', async () => {
    ipc.list = () => Promise.resolve({ sessions: [historyItem('ses-1')], total: 1 });
    ipc.detail = summary([turn(21, 'completed', 'ses-1')], 'ses-1');
    const rendered = await mountPanel();
    await expandHistory();
    await selectHistoryItem(0);
    await expandHistory();
    expect(screen.getAllByTestId('change-chat-history-item')).toHaveLength(1);

    await rerenderPanel(rendered, { changeId: 'chg-2' });

    expect(screen.queryByTestId('change-chat-history-item')).toBeNull();
    expect(screen.queryByTestId('session-meta-id')).toBeNull();

    await expandHistory();
    expect(argsOf('list_change_chat_sessions')['id']).toBe('chg-2');
  });
});

// ---------------------------------------------------------------------------
// 禁发与只读矩阵（AC-6 四行）+ 空态不豁免
// ---------------------------------------------------------------------------

describe('ChangeChatPanel：禁发与只读矩阵', () => {
  it('row1 流程运行中：理由行「流程运行中」，输入 / 发送 / 新会话禁用，历史仍可切', async () => {
    ipc.list = () => Promise.resolve({ sessions: [historyItem('ses-1')], total: 1 });
    await mountPanel({ runActive: true });

    expect(screen.getByTestId('change-chat-disabled-reason').textContent).toBe('流程运行中');
    expect(promptBox().disabled).toBe(true);
    expect(sendButton().disabled).toBe(true);
    expect(screen.getByTestId<HTMLInputElement>('change-chat-new').disabled, '新会话一并禁').toBe(
      true,
    );

    await expandHistory();
    expect(callsOf('list_change_chat_sessions'), '历史可切').toHaveLength(1);
    expect(screen.getAllByTestId('change-chat-history-item')).toHaveLength(1);
  });

  it('row4 归档只读：理由行「已归档 change 只读」，全禁发且点击发送零 agent_start', async () => {
    ipc.list = () => Promise.resolve({ sessions: [historyItem('ses-1')], total: 1 });
    await mountPanel({ readOnly: true });

    expect(screen.getByTestId('change-chat-disabled-reason').textContent).toBe(
      '已归档 change 只读',
    );
    expect(promptBox().disabled).toBe(true);
    expect(sendButton().disabled).toBe(true);
    expect(screen.getByTestId<HTMLInputElement>('change-chat-new').disabled).toBe(true);

    fireEvent.click(sendButton());
    await act(async () => {});
    expect(callsOf('agent_start')).toHaveLength(0);

    await expandHistory();
    expect(callsOf('list_change_chat_sessions')).toHaveLength(1);
  });

  it('空态不豁免禁发：runActive / readOnly 两档下空态同样禁用且理由在场', async () => {
    for (const overrides of [
      { runActive: true },
      { readOnly: true },
    ] as Partial<ChangeChatPanelProps>[]) {
      resetIpc();
      const rendered = await mountPanel(overrides);
      expect(screen.queryByTestId('session-meta-id'), '确为空态').toBeNull();
      expect(screen.getByTestId('change-chat-disabled-reason')).toBeTruthy();
      expect(promptBox().disabled).toBe(true);
      expect(sendButton().disabled).toBe(true);
      rendered.unmount();
    }
  });

  it('row2 本会话 running：发送禁用 + 停止入口在场、理由行不在场、新会话禁用', async () => {
    ipc.list = () => Promise.resolve({ sessions: [historyItem('ses-1')], total: 1 });
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    await mountPanel();
    await expandHistory();

    await selectHistoryItem(0);

    await waitFor(() => expect(screen.getByTestId('change-chat-stop')).toBeTruthy());
    expect(sendButton().disabled).toBe(true);
    expect(
      screen.queryByTestId('change-chat-disabled-reason'),
      'disabled 不顶替 running',
    ).toBeNull();
    expect(screen.getByTestId<HTMLInputElement>('change-chat-new').disabled).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 折叠窄条
// ---------------------------------------------------------------------------

describe('ChangeChatPanel：折叠形态', () => {
  it('cwd 两态（worktree / null）面板均正常渲染；执行根文案行已退役（详情头承接）', async () => {
    const withWorktree = await mountPanel();
    expect(screen.getByTestId('change-chat-note').textContent).toBe('发送首条消息即开启新会话');
    expect(screen.queryByTestId('change-chat-cwd')).toBeNull();
    withWorktree.unmount();

    await mountPanel({ cwd: null });
    expect(screen.getByTestId('change-chat-panel')).toBeTruthy();
    expect(screen.getByTestId('change-chat-note').textContent).toBe('发送首条消息即开启新会话');
    expect(screen.queryByTestId('change-chat-cwd')).toBeNull();
  });

  it('collapsed 折叠窄条：展开入口在场、正文（输入框）不在场，点击回调恰一次', async () => {
    const onToggleCollapsed = vi.fn();
    await mountPanel({ collapsed: true, onToggleCollapsed });

    expect(screen.getByTestId('change-chat-expand')).toBeTruthy();
    expect(screen.getByTestId('change-chat-rail')).toBeTruthy();
    expect(screen.queryByTestId('change-chat-prompt')).toBeNull();

    fireEvent.click(screen.getByTestId('change-chat-expand'));
    expect(onToggleCollapsed).toHaveBeenCalledTimes(1);
  });

  it('collapsed 为 false 时呈完整骨架（新会话 / 历史 / 输入），展开入口不在场', async () => {
    await mountPanel();

    expect(screen.getByTestId('change-chat-panel')).toBeTruthy();
    expect(screen.getByTestId('change-chat-new')).toBeTruthy();
    expect(screen.getByTestId('change-chat-history')).toBeTruthy();
    expect(promptBox()).toBeTruthy();
    expect(screen.queryByTestId('change-chat-expand')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 发起失败降级与首发后重取
// ---------------------------------------------------------------------------

describe('ChangeChatPanel：发起失败与重取', () => {
  it('agent_start reject → 原始错误串呈现、无半截气泡、输入仍可用', async () => {
    ipc.start = new Error('worktree 目录不存在（可能已被手动删除）: C:\\gone');
    await mountPanel();
    typeText('会失败的一轮');
    fireEvent.click(sendButton());
    await act(async () => {});
    await act(async () => {});

    const error = await screen.findByTestId('agent-session-error');
    expect(error.textContent).toContain('worktree 目录不存在（可能已被手动删除）');
    expect(screen.queryAllByTestId('chat-bubble')).toHaveLength(0);
    expect(promptBox().disabled).toBe(false);
  });

  it('首发确立会话 id 与轮收口各触发一次清单重取（新铸会话进历史 ▾）', async () => {
    ipc.detail = summary([turn(1, 'running', 'ses-1')], 'ses-1');
    await mountPanel();
    typeText('首轮提问');
    fireEvent.click(sendButton());
    await act(async () => {});
    await act(async () => {});
    await waitFor(() => expect(screen.getByTestId('session-meta-id').textContent).toBe('ses-1'));
    const afterStart = callsOf('list_change_chat_sessions').length;
    expect(afterStart, '会话 id 确立即重取一次').toBe(1);

    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    deliverRecord(turn(1, 'completed', 'ses-1'));
    await act(async () => {});
    await act(async () => {});

    await waitFor(() =>
      expect(callsOf('list_change_chat_sessions').length, '收口再重取').toBe(afterStart + 1),
    );
  });
});

// ---------------------------------------------------------------------------
// ChangeChatComposer：testid 前缀 / 理由透传 / 档位 / 停止入口（插槽即唯一
// 挂载面——design 定稿并入本文件，不新建 composer 测试文件）
// ---------------------------------------------------------------------------

describe('ChangeChatComposer：前缀与判据透传', () => {
  it('change-chat 前缀落到输入 / 档位 / 发送 / composer 容器，理由行文本即上层理由', async () => {
    await mountPanel({ runActive: true });

    expect(screen.getByTestId('change-chat-composer')).toBeTruthy();
    expect(promptBox().id).toBe('change-chat-prompt');
    expect(screen.getByTestId('change-chat-permission-mode').id).toBe(
      'change-chat-permission-mode',
    );
    expect(screen.getByTestId('change-chat-send') !== null, '发送按钮前缀化').toBe(true);
    expect(screen.getByTestId('change-chat-disabled-reason').textContent).toBe('流程运行中');
  });

  it('档位切到 default 后发送：agent_start 的 permissionMode 为 default（文本不改写）', async () => {
    await mountPanel();
    fireEvent.change(screen.getByTestId('change-chat-permission-mode'), {
      target: { value: 'default' },
    });
    typeText('默认档提问');
    fireEvent.click(sendButton());
    await act(async () => {});

    expect(argsOf('agent_start')).toMatchObject({
      prompt: '默认档提问',
      permissionMode: 'default',
    });
  });

  it('无理由态（disabled false + 无理由）→ 理由行不在场（不渲染空节点）', async () => {
    await mountPanel();

    expect(screen.queryByTestId('change-chat-disabled-reason')).toBeNull();
    expect(screen.getByTestId('change-chat-composer').textContent).not.toContain('undefined');
  });

  it('running 与 disabled 正交：running+可发 → 停止入口在场；running+禁用 → 停止入口不在场且理由在场', async () => {
    ipc.list = () => Promise.resolve({ sessions: [historyItem('ses-1')], total: 1 });
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    const rendered = await mountPanel();
    await expandHistory();
    await selectHistoryItem(0);
    await waitFor(() => expect(screen.getByTestId('change-chat-stop')).toBeTruthy());
    expect(sendButton().disabled).toBe(true);
    expect(screen.queryByTestId('change-chat-disabled-reason')).toBeNull();

    await rerenderPanel(rendered, { runActive: true });

    expect(screen.queryByTestId('change-chat-stop'), '整面板禁用不呈停止入口').toBeNull();
    expect(screen.getByTestId('change-chat-disabled-reason').textContent).toBe('流程运行中');
    expect(sendButton().disabled).toBe(true);
  });

  it('点击停止 → agent_stop 携 { root, sessionId }（停止面不携执行根）', async () => {
    ipc.list = () => Promise.resolve({ sessions: [historyItem('ses-1')], total: 1 });
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    await mountPanel();
    await expandHistory();
    await selectHistoryItem(0);
    await waitFor(() => expect(screen.getByTestId('change-chat-stop')).toBeTruthy());

    fireEvent.click(screen.getByTestId('change-chat-stop'));
    await act(async () => {});

    expect(callsOf('agent_stop')).toHaveLength(1);
    expect(argsOf('agent_stop')).toEqual({ root: ROOT, sessionId: 'ses-1' });
    expect(Object.keys(argsOf('agent_stop')).sort()).toEqual(['root', 'sessionId']);
  });
});
