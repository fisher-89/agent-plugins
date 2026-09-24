// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { toast } from 'sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import App from '../App';
import type {
  AgentEvent,
  AgentRunRecord,
  ChangeList,
  ExploreRecord,
  WorkspaceRecord,
} from '../types/dto';

// ---------------------------------------------------------------------------
// 集成关系「详情双恢复 → 会话链resume发送」（AC-9/AC-5/AC-4）：进入
// /explores/:name 详情后，文档路（read_explore）与对话路（agent_run_chain +
// 逐 run 事件重放）并行恢复、互不依赖；发送拼 stance、链尾 resume、来源
// 三元组在同一 invoke 参数里会合；run 终态定点重读文档。
//
// 进程边界 Mock：invoke 按命令名分发并记录入参；Channel mock 捕获 onmessage
// （测试内注入实时事件）；链路内组件/hooks/路由运行时全部真实实现。
// ---------------------------------------------------------------------------

const { ChannelMock, checkMock, getVersionMock, invokeMock, openMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return {
    ChannelMock,
    checkMock: vi.fn(),
    getVersionMock: vi.fn(),
    invokeMock: vi.fn(),
    openMock: vi.fn(),
  };
});

vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: checkMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

const FIRST: WorkspaceRecord = { root: 'C:\\demo\\alpha', name: 'alpha', addedAt: 1 };
const RECORD_ID = 7;
const RECORD_NAME = 'foo';

const fakeList: ChangeList = {
  active: [
    { name: 'add-feature', source: 'active', inventory: 'v2', created: null, unparsable: false },
  ],
  archiveGroups: [],
};

const exploreRecord: ExploreRecord = {
  id: RECORD_ID,
  root: FIRST.root,
  name: RECORD_NAME,
  createdAt: 1727000000000,
  updatedAt: 1727000000000,
};

function run(id: number, sessionId: string | null, parentRunId: number | null): AgentRunRecord {
  return {
    id,
    prompt: `explore 轮次 ${id}`,
    cwd: FIRST.root,
    env: 'default',
    permissionMode: 'bypassPermissions',
    status: 'completed',
    startedAt: 1727000000000 + id,
    finishedAt: 1727000001000 + id,
    numTurns: 1,
    costUsd: 0.1,
    durationMs: 500,
    sessionId,
    error: null,
    source: 'explore',
    sourceRef: String(RECORD_ID),
    parentRunId,
  };
}

function message(
  seq: number,
  blocks: Extract<AgentEvent, { kind: 'message' }>['blocks'],
  role = 'assistant',
): AgentEvent {
  return { seq, timestampMs: 1727000000000, kind: 'message', role, blocks, parentToolUseId: null };
}

/** 链 fixture：两条 parent_run_id 相连的 run；链尾带 sessionId 供 resume。 */
let chainRuns: AgentRunRecord[];
/** 每 run 的落库事件（四变体齐备在 run2）。 */
let eventsByRun: Map<number, AgentEvent[]>;
let docResult: { name: string; content: string } | null;
let startOutcome: AgentRunRecord | string | null;
let startCalls: Record<string, unknown>[];

function mockIpc() {
  ChannelMock.instances.length = 0;
  chainRuns = [run(11, 's-first', null), run(12, 's-tail', 11)];
  eventsByRun = new Map<number, AgentEvent[]>([
    [11, [message(0, [{ kind: 'text', text: '首轮结论' }])]],
    [
      12,
      [
        message(0, [{ kind: 'thinking', thinking: '续轮思考' }]),
        message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Read', input: { path: 'x' } }]),
        message(
          2,
          [{ kind: 'toolResult', id: 'tu_1', content: '文件内容', isError: false }],
          'user',
        ),
        message(3, [{ kind: 'text', text: '**续轮结论**' }]),
        {
          seq: 4,
          timestampMs: 1727000000000,
          kind: 'runResult',
          subtype: 'success',
          isError: false,
          numTurns: 2,
          durationMs: 800,
          costUsd: 0.2,
          usage: {},
          sessionId: 's-tail',
        },
      ],
    ],
  ]);
  docResult = { name: RECORD_NAME, content: '# 笔记初稿' };
  startOutcome = run(13, 's-new-tail', 12);
  startCalls = [];
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'list_workspaces') {
      return Promise.resolve([FIRST]);
    }
    if (command === 'list_changes') {
      return Promise.resolve(fakeList);
    }
    if (command === 'list_explore_records') {
      return Promise.resolve([exploreRecord]);
    }
    if (command === 'read_explore') {
      return Promise.resolve(docResult);
    }
    if (command === 'explore_doc_path') {
      return Promise.resolve(null);
    }
    if (command === 'watch_subscribe') {
      return Promise.resolve(1);
    }
    if (command === 'watch_unsubscribe') {
      return Promise.resolve(true);
    }
    if (command === 'agent_run_chain') {
      return Promise.resolve(chainRuns);
    }
    if (command === 'agent_run_events') {
      return Promise.resolve(eventsByRun.get(Number(params?.runId)) ?? []);
    }
    if (command === 'agent_start') {
      startCalls.push(params ?? {});
      return typeof startOutcome === 'string'
        ? Promise.reject(startOutcome)
        : Promise.resolve(startOutcome);
    }
    return Promise.resolve(null);
  });
}

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

function lastStartArgs(): Record<string, unknown> {
  if (startCalls.length === 0) throw new Error('agent_start 未被调用');
  return startCalls.at(-1)!;
}

/** 深链启动直达 /explores/foo 详情并等待双恢复取数发起（文档路 + 链路各一次）。 */
async function onDetail() {
  window.location.hash = `#/explores/${RECORD_NAME}`;
  render(<App />);
  await waitFor(() => expect(screen.getByTestId('explore-detail') !== null).toBe(true));
  await waitFor(() => {
    expect(countOf('read_explore')).toBe(1);
    expect(countOf('agent_run_chain')).toBe(1);
  });
}

/** 等待链事件重放完成（气泡渲染）。 */
async function waitForBubbles(count: number) {
  await waitFor(() =>
    expect(
      within(screen.getByTestId('explore-conversation')).getAllByTestId('chat-bubble'),
    ).toHaveLength(count),
  );
}

beforeEach(() => {
  window.location.hash = '';
  getVersionMock.mockReset();
  invokeMock.mockReset();
  openMock.mockReset();
  checkMock.mockReset();
  checkMock.mockResolvedValue(null);
  getVersionMock.mockResolvedValue('0.3.0');
  toast.dismiss();
  mockIpc();
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe = vi.fn();
      unobserve = vi.fn();
      disconnect = vi.fn();
    },
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// 双恢复与会话链（AC-9/AC-5）
// ---------------------------------------------------------------------------

describe('explore_session_pipeline：详情双恢复与全链重放（AC-9/AC-5）', () => {
  it('打开详情：read_explore 与 agent_run_chain 各发一次（互为前置不成立），文档与气泡同屏', async () => {
    await onDetail();

    expect(countOf('read_explore')).toBe(1);
    expect(countOf('agent_run_chain')).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith('agent_run_chain', {
      source: 'explore',
      sourceRef: String(RECORD_ID),
    });
    expect(screen.getByTestId('preview-doc')?.textContent).toContain('笔记初稿');
    expect(screen.getByTestId('explore-conversation') !== null).toBe(true);
  });

  it('全链重放按发起序、四变体气泡齐备（Text/Thinking/ToolUse+ToolResult/汇总卡）', async () => {
    await onDetail();
    await waitForBubbles(5);

    const conversation = screen.getByTestId('explore-conversation');
    const bubbles = within(conversation).getAllByTestId('chat-bubble');
    expect(bubbles).toHaveLength(5);
    expect(bubbles[0].getAttribute('data-role')).toBe('assistant');
    expect(within(conversation).getAllByTestId('block-text')[0]?.textContent).toContain('首轮结论');
    expect(within(conversation).getByTestId('block-thinking')?.textContent).toContain('续轮思考');
    expect(within(conversation).getByTestId('block-tool-use').getAttribute('data-tool-id')).toBe(
      'tu_1',
    );
    expect(within(conversation).getByTestId('block-tool-result')?.textContent).toContain(
      '文件内容',
    );
    const resultCard = within(conversation).getByTestId('result-summary');
    expect(resultCard.textContent).toContain('s-tail');
    // 事件重放序：首轮 text 气泡在续轮 thinking 之前（发起序拼接）
    const textIndex = bubbles.findIndex((bubble) => bubble.textContent?.includes('首轮结论'));
    const thinkingIndex = bubbles.findIndex((bubble) => bubble.textContent?.includes('续轮思考'));
    expect(textIndex).toBeLessThan(thinkingIndex);
  });

  it('read_explore 返回 null（文件缺失）：预览空态、对话重放不受影响（双恢复独立性）', async () => {
    docResult = null;
    await onDetail();
    await waitForBubbles(5);

    expect(screen.getByTestId('preview-empty') !== null).toBe(true);
    expect(screen.queryByTestId('preview-doc')).toBeNull();
    expect(
      within(screen.getByTestId('explore-conversation')).getAllByTestId('block-text').length,
    ).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// 发送续话与文档定点重读（AC-9，D4/D7）
// ---------------------------------------------------------------------------

describe('explore_session_pipeline：发送拼 stance + resume 链尾 + 三元组（AC-9）', () => {
  it('有链发送：prompt 以 stance 前导开头、resume 为链尾 sessionId、三元组齐全', async () => {
    await onDetail();

    fireEvent.change(screen.getByTestId('explore-prompt'), {
      target: { value: '继续追重试线索' },
    });
    fireEvent.click(screen.getByTestId('explore-send'));

    await waitFor(() => expect(startCalls.length).toBe(1));
    const args = lastStartArgs();
    expect(String(args.prompt).endsWith('继续追重试线索')).toBe(true);
    expect(String(args.prompt)).toContain('你在探索模式下工作');
    expect(args.resumeSessionId).toBe('s-tail');
    expect(args.source).toBe('explore');
    expect(args.sourceRef).toBe(String(RECORD_ID));
    expect(args.parentRunId).toBe(12);
    expect(args.root).toBe(FIRST.root);
    expect(args.onEvent).toBeInstanceOf(ChannelMock);
  });

  it('run 终态定点重读文档：agent_start resolve 后 read_explore 重发一次', async () => {
    await onDetail();
    expect(countOf('read_explore')).toBe(1);

    fireEvent.change(screen.getByTestId('explore-prompt'), { target: { value: '补一轮' } });
    fireEvent.click(screen.getByTestId('explore-send'));

    await waitFor(() => expect(countOf('read_explore')).toBe(2));
    expect(startOutcome).not.toBeNull();
  });

  it('无链记录发送：不携带 resume，起链后链增长、二次续话以新链尾为准（AC-9 多轮）', async () => {
    chainRuns = [];
    eventsByRun = new Map();
    await onDetail();
    await waitFor(() => expect(screen.getByTestId('conversation-empty') !== null).toBe(true));

    fireEvent.change(screen.getByTestId('explore-prompt'), { target: { value: '首轮起链' } });
    fireEvent.click(screen.getByTestId('explore-send'));
    await waitFor(() => expect(startCalls.length).toBe(1));
    expect(lastStartArgs().resumeSessionId).toBeNull();
    expect(lastStartArgs().parentRunId).toBeNull();

    fireEvent.change(screen.getByTestId('explore-prompt'), { target: { value: '二次续话' } });
    fireEvent.click(screen.getByTestId('explore-send'));
    await waitFor(() => expect(startCalls.length).toBe(2));
    expect(lastStartArgs().resumeSessionId).toBe('s-new-tail');
    expect(lastStartArgs().parentRunId).toBe(13);
  });

  it('AskUserQuestion 出现在链事件：静态卡片呈现、composer 可继续输入并以 resume 续话（MVP 闭环）', async () => {
    eventsByRun = new Map<number, AgentEvent[]>([
      [
        11,
        [
          message(0, [
            {
              kind: 'toolUse',
              id: 'tu_ask',
              name: 'AskUserQuestion',
              input: {
                questions: [
                  { question: '先调研哪个方向？', options: [{ label: '限流', description: null }] },
                ],
              },
            },
          ]),
        ],
      ],
    ]);
    await onDetail();
    await waitFor(() => expect(screen.getByTestId('ask-question-card') !== null).toBe(true));

    const card = screen.getByTestId('ask-question-card');
    expect(within(card).getByTestId('ask-question')?.textContent).toContain('先调研哪个方向？');
    expect(within(card).queryAllByRole('button')).toHaveLength(0);

    const composer = screen.getByTestId('explore-prompt');
    expect((composer as HTMLTextAreaElement).disabled).toBe(false);
    fireEvent.change(composer, { target: { value: '先看限流' } });
    fireEvent.click(screen.getByTestId('explore-send'));
    await waitFor(() => expect(startCalls.length).toBe(1));
    expect(lastStartArgs().resumeSessionId).toBe('s-tail');
  });

  it('双栏 resizable 拖分界：面板交互不重置会话状态（历史气泡与预览保留）', async () => {
    await onDetail();
    await waitForBubbles(5);
    const reads = countOf('read_explore');

    const handle = document.querySelector('[data-slot="resizable-handle"]');
    expect(handle !== null).toBe(true);
    await act(async () => {
      fireEvent.pointerDown(handle!, { button: 0, pointerId: 1 });
      fireEvent.pointerMove(handle!, { pointerId: 1, clientX: 200 });
      fireEvent.pointerUp(handle!, { pointerId: 1 });
    });

    expect(
      within(screen.getByTestId('explore-conversation')).getAllByTestId('block-text').length,
    ).toBeGreaterThan(0);
    expect(screen.getByTestId('preview-doc') !== null).toBe(true);
    expect(countOf('read_explore')).toBe(reads);
  });

  it('agent_start reject：running 呈现后复位、历史气泡保留、composer 不崩可再输入', async () => {
    startOutcome = '启动失败';
    await onDetail();
    await waitForBubbles(5);
    expect(screen.queryByTestId('conversation-running')).toBeNull();

    fireEvent.change(screen.getByTestId('explore-prompt'), { target: { value: '会失败的一轮' } });
    fireEvent.click(screen.getByTestId('explore-send'));

    await waitFor(() => expect(screen.getByTestId('conversation-running') !== null).toBe(true));
    await waitFor(() => expect(screen.queryByTestId('conversation-running')).toBeNull());
    expect(
      within(screen.getByTestId('explore-conversation')).getAllByTestId('chat-bubble').length,
    ).toBeGreaterThanOrEqual(5);
    expect(screen.getByTestId('explore-prompt') !== null).toBe(true);
  });

  it('运行中头部呈现流式状态、终态后复位（running 门闩在组件面的呈现）', async () => {
    let resolveStart: ((value: AgentRunRecord) => void) | null = null;
    startOutcome = null;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_start') {
        return new Promise<AgentRunRecord>((resolve) => {
          resolveStart = resolve;
        });
      }
      if (command === 'list_workspaces') return Promise.resolve([FIRST]);
      if (command === 'list_changes') return Promise.resolve(fakeList);
      if (command === 'list_explore_records') return Promise.resolve([exploreRecord]);
      if (command === 'agent_run_chain') return Promise.resolve(chainRuns);
      if (command === 'agent_run_events') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    await onDetail();

    fireEvent.change(screen.getByTestId('explore-prompt'), { target: { value: '运行中的一轮' } });
    fireEvent.click(screen.getByTestId('explore-send'));
    await waitFor(() => expect(countOf('agent_start')).toBe(1));
    expect(screen.getByTestId('conversation-running') !== null).toBe(true);

    await act(async () => {
      resolveStart?.(run(13, 's-new-tail', 12));
    });
    await waitFor(() => expect(screen.queryByTestId('conversation-running')).toBeNull());
  });
});
