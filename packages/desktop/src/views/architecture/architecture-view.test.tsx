import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiElement, SessionPage, TurnSummary } from '../../types/dto';
import { ArchitectureView } from './architecture-view';

/**
 * `ArchitectureView` 单测：整宽页头（标题 / 面板开关 / 刷新）+ 横向分栏（左栏通用
 * Agent 面板默认收起为窄条、右栏画布全宽）+ 显式刷新语义零变化（`key` 重挂恰一次
 * `archi_graph`）+ 收起态零会话取数 + 画布零放宽（AC-9）+ 写后不自动刷新（AC-8 /
 * AC-13）。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（archi 两命令 + 会话六命令按名
 * 分派）+ 可编程 `Channel`；xyflow / 分栏度量垫片照抄既有先例（环境垫片，非业务
 * mock）。
 */

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

/** 分栏 / xyflow 共用的 ResizeObserver 垫片（可重放观测，尺寸取 offsetWidth）。 */
class ResizeObserverStub {
  private elements: Element[] = [];
  static instances: ResizeObserverStub[] = [];

  constructor(private readonly callback: ResizeObserverCallback) {
    ResizeObserverStub.instances.push(this);
  }

  observe = (target: Element): void => {
    this.elements.push(target);
  };

  unobserve = (target: Element): void => {
    this.elements = this.elements.filter((element) => element !== target);
  };

  disconnect = (): void => {
    this.elements = [];
  };

  /** 重放全部观测目标（分栏读 `borderBoxSize[0]` 判分支，垫片同步给出报告尺寸）。 */
  static flush(): void {
    for (const observer of ResizeObserverStub.instances) {
      if (observer.elements.length === 0) continue;
      const entries = observer.elements.map(
        (target) =>
          ({
            target,
            contentRect: {
              width: target.clientWidth,
              height: target.clientHeight,
              x: 0,
              y: 0,
              top: 0,
              left: 0,
              bottom: 0,
              right: 0,
            },
            borderBoxSize: [
              {
                inlineSize: (target as HTMLElement).offsetWidth,
                blockSize: (target as HTMLElement).offsetHeight,
              },
            ],
          }) as unknown as ResizeObserverEntry,
      );
      observer.callback(entries, observer);
    }
  }
}

const nativeOffsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
const nativeOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
const nativeFontSize = document.documentElement.style.fontSize;

const ROOT = 'C:\\demo\\alpha';
const TS = 1_727_200_000_000;

function element(fqn: string): ArchiElement {
  return {
    fqn,
    kind: 'module',
    gate: 'off',
    path: [],
    summary: null,
    metadata: {},
    modelFile: '.dev-team/architecture/models/01-core.toml',
    modelLine: 2,
    mark: 'none',
  };
}

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

/** `archi_graph` 应答形态切换点（停摆诊断 / 正常图）。 */
let graphDiagnostics: {
  kind: string;
  message: string;
  file: string;
  line: number;
  column: null;
}[] = [];
let sessionPage: SessionPage = { sessions: [], total: 0 };
let sessionDetail: unknown = null;

function callsOf(command: string): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

function lastChannel(): { onmessage: ((message: unknown) => void) | null } {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('尚未建立流（无 Channel 实例）');
  return instance;
}

beforeEach(() => {
  graphDiagnostics = [];
  sessionPage = { sessions: [], total: 0 };
  sessionDetail = null;
  ChannelMock.instances.length = 0;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'archi_graph') {
      return Promise.resolve({
        elements: [element('App'), element('App.Auth')],
        relations: [{ from: 'App.Auth', to: 'App', description: null, mark: 'none', reason: null }],
        diagnostics: graphDiagnostics,
      });
    }
    if (command === 'archi_module') {
      const fqn = typeof params?.element === 'string' ? params.element : '';
      return Promise.resolve({
        element: element(fqn),
        dependsOn: [],
        dependedBy: [],
        children: [],
        docs: [],
        warnings: [],
      });
    }
    if (command === 'session_detail') return Promise.resolve(sessionDetail);
    if (command === 'agent_session_transcript') return Promise.resolve([]);
    if (command === 'agent_session_watch') return new Promise<null>(() => {});
    if (command === 'agent_start') {
      return Promise.resolve(turn(1, 'running', 'ses-archi-1'));
    }
    if (command === 'list_archi_chat_sessions') return Promise.resolve(sessionPage);
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });

  ResizeObserverStub.instances = [];
  vi.stubGlobal('ResizeObserver', ResizeObserverStub);
  // jsdom 无 CSS 引擎：`3rem` 收起尺寸需根字号可解析（浏览器默认 16px 口径）
  document.documentElement.style.fontSize = '16px';
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
    configurable: true,
    get: () => 100,
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
    configurable: true,
    get: () => 100,
  });
  vi.stubGlobal(
    'DOMMatrixReadOnly',
    class {
      m22 = 1;
    },
  );
  const svgPrototype = globalThis.SVGElement?.prototype as unknown as
    | Record<string, unknown>
    | undefined;
  if (svgPrototype !== undefined && typeof svgPrototype.getBBox !== 'function') {
    svgPrototype.getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 });
  }
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  if (nativeOffsetWidth !== undefined) {
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', nativeOffsetWidth);
  }
  if (nativeOffsetHeight !== undefined) {
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', nativeOffsetHeight);
  }
  document.documentElement.style.fontSize = nativeFontSize;
});

/** 挂载并等画布首帧（archi_graph 已发）。 */
async function mountView(root: string | null = ROOT) {
  const rendered = render(<ArchitectureView root={root} />);
  if (root !== null) {
    await waitFor(() => expect(screen.getByTestId('archi-canvas')).toBeTruthy());
  } else {
    await waitFor(() => expect(screen.getByTestId('archi-canvas-empty')).toBeTruthy());
  }
  await act(async () => {});
  return rendered;
}

describe('ArchitectureView：页头与分栏结构', () => {
  it('页头三件（标题 → 面板开关 → 刷新）在分栏之上，分栏下左栏面板 / 右栏画布', async () => {
    await mountView();

    const title = screen.getByText('架构');
    const toggle = screen.getByTestId('archi-chat-toggle');
    const refresh = screen.getByTestId('archi-refresh');
    const group = document.querySelector('[data-slot="resizable-panel-group"]');
    expect(group, '分栏组在场').not.toBeNull();

    // 可观察次序：标题 → 开关 → 刷新，且三者均位于分栏之前
    const order = [title, toggle, refresh];
    for (let index = 1; index < order.length; index += 1) {
      expect(
        (order[index - 1].compareDocumentPosition(order[index]) &
          Node.DOCUMENT_POSITION_FOLLOWING) !==
          0,
        `第 ${index} 件位于前一件之后`,
      ).toBe(true);
    }
    expect(
      (refresh.compareDocumentPosition(group as Node) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
      '页头整宽留在分栏之上',
    ).toBe(true);

    const panel = screen.getByTestId('archi-chat-panel');
    expect(panel.closest('[data-slot="resizable-panel"]'), '左栏为面板').not.toBeNull();
    expect(
      screen.getByTestId('archi-canvas').closest('[data-slot="resizable-panel"]'),
      '右栏为画布',
    ).not.toBeNull();
    expect(panel.closest('[data-slot="resizable-panel"]')).not.toBe(
      screen.getByTestId('archi-canvas').closest('[data-slot="resizable-panel"]'),
    );
  });
});

describe('ArchitectureView：默认收起与开关', () => {
  it('首拍即窄条（展开体缺席），且挂载收紧一次（分栏份额 = 收起尺寸）', async () => {
    await mountView();
    ResizeObserverStub.flush();
    await act(async () => {});

    expect(screen.getByTestId('archi-chat-rail')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-expand')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-actions'), '收起后展开体不在场').toBeNull();
    expect(screen.queryByTestId('archi-chat-prompt')).toBeNull();
    expect(screen.getByTestId('archi-chat-toggle').textContent).toContain('显示架构 Agent');

    // 收紧的可观察后果：默认窄条份额 < 展开份额（像素无关的相对判据——
    // 折叠真相取页面 state，不取分栏 resize 回调的像素派生）
    const panelElement = screen
      .getByTestId('archi-chat-panel')
      .closest<HTMLElement>('[data-panel]');
    expect(panelElement).not.toBeNull();
    const collapsedShare = Number(panelElement?.style.flexGrow);
    expect(collapsedShare, '挂载即收紧（窄条份额为正数）').toBeGreaterThan(0);

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });
    ResizeObserverStub.flush();
    await act(async () => {});
    const expandedShare = Number(panelElement?.style.flexGrow);
    expect(expandedShare, '展开后份额大于窄条份额').toBeGreaterThan(collapsedShare);
  });

  it('页面开关两拍互逆：展开 → 展开体在场；再点 → 回窄条（state 为唯一真源）', async () => {
    await mountView();

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-prompt')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();
    expect(screen.getByTestId('archi-chat-toggle').textContent).toContain('隐藏架构 Agent');

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });
    expect(screen.getByTestId('archi-chat-rail')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-actions')).toBeNull();
  });

  it('窄条展开入口与页面开关同一入口语义（两入口等价）', async () => {
    await mountView();

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-expand'));
    });
    expect(screen.getByTestId('archi-chat-actions')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-session')).toBeTruthy();
    expect(screen.queryByTestId('archi-chat-rail')).toBeNull();
  });
});

describe('ArchitectureView：会话取数与刷新语义', () => {
  it('收起态零会话取数：唯一启动 invoke 为 archi_graph；展开面板不开历史亦零清单 invoke', async () => {
    await mountView();

    expect(countOf('archi_graph'), '画布启动取数').toBe(1);
    for (const command of [
      'list_archi_chat_sessions',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
    ]) {
      expect(countOf(command), `收起态 ${command} 零调用`).toBe(0);
    }

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });
    for (const command of [
      'list_archi_chat_sessions',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
      'agent_start',
    ]) {
      expect(countOf(command), `展开但不开历史 → ${command} 仍零调用`).toBe(0);
    }
  });

  it('点刷新 → archi_graph 以同 root 重发恰一次；面板开关不触发 archi_graph', async () => {
    await mountView();
    expect(countOf('archi_graph')).toBe(1);

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-refresh'));
    });
    await waitFor(() => expect(countOf('archi_graph')).toBe(2));
    expect(callsOf('archi_graph')[1]).toEqual({ root: ROOT });

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });
    expect(countOf('archi_graph'), '面板开关不触发画布重取').toBe(2);
  });
});

describe('ArchitectureView：画布零放宽（AC-9）', () => {
  it('full 模式：readonly 文案逐字、无改档控件、零 archi_set_gate', async () => {
    await mountView();

    await waitFor(() => expect(screen.getByTestId('archi-gate-readonly')).toBeTruthy());
    expect(screen.getByTestId('archi-gate-readonly').textContent).toContain(
      '改档需要开一个 change',
    );
    expect(screen.queryByTestId('archi-gate-select')).toBeNull();
    expect(screen.queryByTestId('archi-gate-submit')).toBeNull();
    expect(countOf('archi_set_gate')).toBe(0);
  });

  it('写后不自动刷新：面板侧发送并收口后 archi_graph 零重发，假计时器 120s 亦零新增', async () => {
    await mountView();
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });

    fireEvent.change(screen.getByTestId('archi-chat-prompt'), {
      target: { value: '把 Auth 的 gate 抬到 strict' },
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-send'));
    });
    await act(async () => {});
    await waitFor(() => expect(countOf('agent_start')).toBe(1));

    // 收口信封：画布不因此重取（零 watch 零轮询，刷新只由按钮驱动）
    await act(async () => {
      lastChannel().onmessage?.({ ipc: 'record', record: turn(1, 'completed', 'ses-archi-1') });
    });
    await act(async () => {});
    expect(countOf('archi_graph'), '收口后画布零重发').toBe(1);

    vi.useFakeTimers();
    try {
      await act(async () => {
        vi.advanceTimersByTime(120_000);
      });
      expect(countOf('archi_graph'), '假计时器推进下零新增').toBe(1);
      expect(countOf('agent_session_watch'), '会话面无轮询').toBeLessThanOrEqual(1);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('ArchitectureView：假计时器零取数', () => {
  it('挂载后推进 120s：画布与会话双面零新增（零 watch 零轮询）', async () => {
    await mountView();
    const graphBefore = countOf('archi_graph');

    vi.useFakeTimers();
    try {
      await act(async () => {
        vi.advanceTimersByTime(120_000);
      });
      expect(countOf('archi_graph'), '画布零轮询').toBe(graphBefore);
      for (const command of [
        'list_archi_chat_sessions',
        'session_detail',
        'agent_session_transcript',
        'agent_session_watch',
      ]) {
        expect(countOf(command), `会话面 ${command} 零轮询`).toBe(0);
      }
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('ArchitectureView：root / 停摆诊断', () => {
  it('root = null → 零 invoke + 画布空态，面板与开关仍在场（可展开不白屏）', async () => {
    await mountView(null);

    expect(countOf('archi_graph')).toBe(0);
    expect(countOf('archi_set_gate')).toBe(0);
    expect(screen.getByTestId('archi-canvas-empty')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-toggle')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-rail')).toBeTruthy();

    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-chat-toggle'));
    });
    expect(screen.getByTestId('archi-chat-panel')).toBeTruthy();
    expect(screen.getByTestId('archi-chat-prompt')).toBeTruthy();
  });

  it('archi_graph 返回诊断 → 画布停摆面板呈 file:line 清单；刷新后从正常切到停摆', async () => {
    await mountView();
    expect(screen.queryByTestId('archi-error-panel')).toBeNull();

    graphDiagnostics = [
      {
        kind: 'missingField',
        message: '缺少必填字段 kind',
        file: '.dev-team/architecture/models/02-bad.toml',
        line: 7,
        column: null,
      },
    ];
    await act(async () => {
      fireEvent.click(screen.getByTestId('archi-refresh'));
    });
    await waitFor(() => expect(screen.getByTestId('archi-error-panel')).toBeTruthy());
    const rows = screen.getAllByTestId('archi-error-row');
    expect(rows).toHaveLength(1);
    expect(rows[0].textContent).toContain('.dev-team/architecture/models/02-bad.toml:7');
    expect(rows[0].textContent).toContain('missingField');
    expect(
      screen.getByTestId('archi-canvas').querySelector('[data-testid="archi-gate-readonly"]'),
      '停摆态零图数据（抽屉 / 节点全缺席）',
    ).toBeNull();
  });
});
