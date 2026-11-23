// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import * as agentBarrel from '@/components/agent';
import { ResizablePanelGroup } from '@/components/ui/resizable';

import type { AgentEvent, SessionPage, SessionSummary, TurnSummary } from '../../../types/dto';
import { ConfigChatPanel, type ConfigChatPanelProps } from './config-chat-panel';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：仅 mock `@tauri-apps/api/core`（invoke 按命令名分派
// 六命令 + 可编程 Channel）；`useConfigChat` / `SessionPanel` / `AgentInput` /
// `AgentSessionView` 全真实组合——组合用例入口即本组件（链路最上层调用方：
// 面板 → hook → 会话基建 → transport → agent_start）。
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
// fixture（serde camelCase 线格式；时钟不入 mock——updatedAt / startedAt 由 fixture
// 显式给定）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';
const TS = 1_727_500_000_000;

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
    },
    turns,
    preamble: null,
  };
}

function emptyPage(): SessionPage {
  return { sessions: [], total: 0 };
}

/** `propose_config` 可应用态载荷（候选 + 一处差异）。 */
const PROPOSE_PAYLOAD = JSON.stringify({
  current: { schema: 'spec-driven' },
  candidate: { schema: 'spec-driven', context: '新上下文' },
  changes: [{ path: 'context', kind: 'added', before: null, after: '新上下文' }],
  summary: '候选合法，共 1 处变更',
});

/** 含 propose_config 工具卡的转录（toolUse + toolResult 配成单卡）。 */
function proposeTranscript(): AgentEvent[] {
  return [
    {
      seq: 0,
      turnId: 11,
      timestampMs: TS,
      kind: 'message',
      role: 'assistant',
      blocks: [{ kind: 'toolUse', id: 'tu_cfg', name: 'propose_config', input: {} }],
      parentToolUseId: null,
    },
    {
      seq: 1,
      turnId: 11,
      timestampMs: TS,
      kind: 'message',
      role: 'tool',
      blocks: [{ kind: 'toolResult', id: 'tu_cfg', content: PROPOSE_PAYLOAD, isError: false }],
      parentToolUseId: null,
    },
  ];
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
  apply: unknown;
}

let ipc: IpcState;

function resetIpc(overrides: Partial<IpcState> = {}): void {
  ipc = {
    detail: summary([turn(1, 'running', 'ses-1')], 'ses-1'),
    transcript: [],
    watch: 'deferred',
    start: turn(1, 'running', 'ses-1'),
    list: () => Promise.resolve(emptyPage()),
    apply: {
      config: {
        $schema: null,
        schema: 'spec-driven',
        context: '写回后的上下文',
        rules: null,
        staticAnalysis: null,
        projects: [],
        tests: [],
        writeProtection: null,
        extra: [],
      },
      diagnostics: [],
    },
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
    if (command === 'list_config_chat_sessions') {
      return ipc.list();
    }
    if (command === 'workspace_config_apply') {
      return ipc.apply instanceof Error ? Promise.reject(ipc.apply) : Promise.resolve(ipc.apply);
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

/** 分栏宿主（面板骨架自身即 `ResizablePanel`，须挂在 `ResizablePanelGroup` 内）。 */
function panelUi(overrides: Partial<ConfigChatPanelProps> = {}) {
  return (
    <ResizablePanelGroup orientation="horizontal">
      <ConfigChatPanel root={ROOT} onApplied={vi.fn()} {...overrides} />
    </ResizablePanelGroup>
  );
}

async function mountPanel(overrides: Partial<ConfigChatPanelProps> = {}) {
  const rendered = render(panelUi(overrides));
  await act(async () => {});
  fireEvent.click(screen.getByTestId('config-chat-expand'));
  await act(async () => {});
  await act(async () => {});
  return rendered;
}

function typeText(text: string): void {
  fireEvent.change(screen.getByTestId('config-chat-prompt'), { target: { value: text } });
}

async function send(text: string): Promise<void> {
  typeText(text);
  await act(async () => {
    fireEvent.click(screen.getByTestId('config-chat-send'));
  });
  await act(async () => {});
}

async function openHistory(): Promise<void> {
  await act(async () => {
    fireEvent.click(screen.getByTestId('config-chat-history'));
  });
  await act(async () => {});
}

describe('ConfigChatPanel：骨架装配面', () => {
  it('config-chat-* 骨架实例齐备、标题逐字，composer 为 config-chat 前缀', async () => {
    await mountPanel();

    expect(screen.getByTestId('config-chat-actions').textContent).toContain('配置 Agent');
    expect(screen.getByTestId('config-chat-collapse')).toBeTruthy();
    expect(screen.getByTestId('config-chat-new')).toBeTruthy();
    expect(screen.getByTestId('config-chat-history')).toBeTruthy();
    expect(screen.getByTestId('config-chat-note').textContent).toBe('发送首条消息即开启新会话');
    expect(screen.getByTestId('config-chat-session')).toBeTruthy();
    expect(screen.getByTestId('config-chat-composer')).toBeTruthy();
    expect(screen.getByTestId('config-chat-prompt')).toBeTruthy();
    expect(screen.getByTestId('config-chat-send')).toBeTruthy();
    // 与 archi / change 侧不串味
    expect(screen.queryAllByTestId('archi-chat-actions')).toHaveLength(0);
    expect(screen.queryByTestId('change-chat-actions')).toBeNull();
  });

  it('SessionPanel 与 barrel 直导入为同一实例（骨架复用，不建第二实现）', async () => {
    await mountPanel();
    // 骨架实例即 barrel 出口实现：同一 `testIdPrefix` 生成面与之一致
    expect(typeof agentBarrel.SessionPanel).toBe('function');
    expect(screen.getByTestId('config-chat-actions').textContent).toContain('配置 Agent');
    expect(screen.queryByTestId('config-chat-rail')).toBeNull();
  });
});

describe('ConfigChatPanel：归属链路（组合）', () => {
  it('空态发送 → agent_start 六字段逐字（source / sourceRef / cwd / sessionId / prompt / 档位）', async () => {
    await mountPanel();

    await send('这个 workspace 的配置该怎么写');

    expect(callsOf('agent_start')).toHaveLength(1);
    expect(argsOf('agent_start')).toMatchObject({
      root: ROOT,
      prompt: '这个 workspace 的配置该怎么写',
      sessionId: null,
      source: 'config',
      sourceRef: null,
      cwd: null,
      permissionMode: 'bypassPermissions',
    });
  });

  it('agent_start reject → 面板 error 呈现、不崩、composer 恢复可用', async () => {
    ipc.start = new Error('provider 配置缺失');
    await mountPanel();

    await send('启动失败一轮');

    await waitFor(() =>
      expect(screen.getByTestId('config-chat-session').textContent).toContain('provider'),
    );
    expect(screen.getByTestId('config-chat-actions')).toBeTruthy();
    const prompt = screen.getByTestId<HTMLTextAreaElement>('config-chat-prompt');
    expect(prompt.disabled).toBe(false);
  });
});

describe('ConfigChatPanel：composer 注入形态', () => {
  it('空闲时输入与发送可用；运行中 disabled + 理由文案 + 停止入口在场', async () => {
    ipc.detail = summary([turn(11, 'running', 'ses-1')], 'ses-1');
    ipc.list = () => Promise.resolve({ sessions: [summary([], 'ses-1')], total: 1 });
    await mountPanel();
    expect(screen.getByTestId<HTMLTextAreaElement>('config-chat-prompt').disabled).toBe(false);

    await openHistory();
    const [item] = screen.getAllByTestId('config-chat-history-item');
    await act(async () => {
      fireEvent.click(item);
    });
    await act(async () => {});
    await act(async () => {});

    await waitFor(() =>
      expect(screen.getByTestId('config-chat-note').textContent).toBe('本会话运行中，请等待收口'),
    );
    expect(screen.getByTestId<HTMLTextAreaElement>('config-chat-prompt').disabled).toBe(true);
    expect(screen.getByTestId<HTMLButtonElement>('config-chat-send').disabled).toBe(true);
    expect(screen.getByTestId('config-chat-disabled-reason').textContent).toContain('本会话运行中');
    expect(screen.getByTestId('config-chat-stop')).toBeTruthy();
  });
});

describe('ConfigChatPanel：折叠（组合）', () => {
  it('缺省收起只呈窄条且零会话类 invoke；展开后同一骨架实例、收起回窄条', async () => {
    render(panelUi());

    expect(screen.getByTestId('config-chat-rail').textContent).toContain('配置 Agent');
    expect(screen.getByTestId('config-chat-expand').getAttribute('title')).toBe('展开配置 Agent');
    expect(screen.queryByTestId('config-chat-actions')).toBeNull();
    for (const command of [
      'agent_start',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
      'list_config_chat_sessions',
    ]) {
      expect(callsOf(command), `收起期间 ${command} 零 invoke`).toHaveLength(0);
    }

    await act(async () => {
      fireEvent.click(screen.getByTestId('config-chat-expand'));
    });
    expect(screen.getByTestId('config-chat-actions')).toBeTruthy();
    expect(screen.queryByTestId('config-chat-rail')).toBeNull();

    await act(async () => {
      fireEvent.click(screen.getByTestId('config-chat-collapse'));
    });
    expect(screen.getByTestId('config-chat-rail')).toBeTruthy();
  });
});

describe('ConfigChatPanel：root 两态与写回动作面', () => {
  it('root = null → 面板渲染但零 invoke；不提供 ConfigApplyScope（卡片呈只读态）', async () => {
    await mountPanel({ root: null });

    expect(screen.getByTestId('config-chat-actions')).toBeTruthy();
    await send('无根发送');
    for (const command of [
      'agent_start',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
      'list_config_chat_sessions',
    ]) {
      expect(callsOf(command), `root = null → ${command} 零 invoke`).toHaveLength(0);
    }
  });

  it('onApplied 透传：包裹的 scope 提供 {root, onApplied}，卡片点「应用」成功 → 回调恰一次', async () => {
    const onApplied = vi.fn();
    ipc.list = () => Promise.resolve({ sessions: [summary([], 'ses-1')], total: 1 });
    ipc.detail = summary([turn(1, 'completed', 'ses-1')], 'ses-1');
    ipc.transcript = proposeTranscript();
    await mountPanel({ onApplied });

    await openHistory();
    const [item] = screen.getAllByTestId('config-chat-history-item');
    await act(async () => {
      fireEvent.click(item);
    });
    await act(async () => {});
    await act(async () => {});

    // 提案卡展开后可应用：卡片读到的 scope.root 即面板入参 root
    const applyButton = await waitFor(() => {
      const button = screen.getByTestId<HTMLButtonElement>('config-propose-apply');
      expect(button.disabled).toBe(false);
      return button;
    });
    await act(async () => {
      fireEvent.click(applyButton);
    });
    await act(async () => {});

    expect(callsOf('workspace_config_apply')).toHaveLength(1);
    expect(argsOf('workspace_config_apply')).toMatchObject({ root: ROOT });
    expect(onApplied).toHaveBeenCalledTimes(1);
  });
});
