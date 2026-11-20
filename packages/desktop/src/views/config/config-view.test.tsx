// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionPage, SessionSummary, TurnSummary } from '../../types/dto';
import type {
  ConfigDiagnostic,
  WorkspaceConfig,
  WorkspaceConfigReport,
} from '../../types/generated/bindings';
import { ConfigView } from './config-view';

// ---------------------------------------------------------------------------
// 进程边界 Mock：IPC 收敛于 @tauri-apps/api/core invoke（useWorkspaceConfig 经
// 生成绑定 commands.workspaceConfig 调用，底层同模块 invoke，mock 切换后依旧
// 生效）；内部模块（useWorkspaceConfig / 五分区组件）不 mock，真实组合（本
// 文件兼作跨模块组合用例挂靠入口）。invoke 按命令名 workspace_config 分发
// （resolve / reject / 手动 pending 控制 loading 态），返回 camelCase
// WorkspaceConfigReport fixture（含合法 / fileMissing / readFailed /
// jsonInvalid / 违例 + defaultApplied 组合各一），深拷贝防用例间残留。
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

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
  Channel: ChannelMock,
}));

const ROOT = 'C:\\demo\\alpha';

function makeConfig(overrides: Partial<WorkspaceConfig> = {}): WorkspaceConfig {
  return {
    $schema: 'https://example.com/spec-driven.schema.json',
    schema: 'spec-driven',
    context: '桌面端插件仓库',
    rules: { proposal: ['提案规则一'], tasks: ['任务规则一'] },
    staticAnalysis: 'clippy',
    projects: [
      { root: 'packages/desktop', language: null, dependsOn: null },
      { root: 'packages/desktop/src-tauri', language: null, dependsOn: null },
    ],
    tests: [
      {
        root: 'packages/desktop',
        framework: 'vite-plus',
        cwd: 'packages/desktop',
        config: null,
        includes: ['src/**/*.test.tsx'],
        excludes: null,
        coverage: { lines: 80, branches: 70, functions: 75 },
        mutation: { cwd: null, score: 70 },
      },
      {
        root: 'src-tauri',
        framework: 'rust',
        cwd: 'src-tauri',
        config: 'Cargo.toml',
        includes: null,
        excludes: ['target/**'],
        coverage: { lines: 90, branches: 88, functions: 85 },
        mutation: { cwd: 'src-tauri', score: 75 },
      },
    ],
    writeProtection: { files: [{ glob: 'tests/golden/**', reason: '金样冻结' }] },
    extra: [{ key: 'customFlag', value: { nested: true } }],
    ...overrides,
  };
}

function diag(kind: ConfigDiagnostic['kind'], path: string, message: string): ConfigDiagnostic {
  return { kind, path, message };
}

/** 违例 + defaultApplied 组合报告：defaultedPaths 对位 schema 与 tests[0] 两处。 */
function makeReport(overrides: Partial<WorkspaceConfigReport> = {}): WorkspaceConfigReport {
  return {
    config: makeConfig(),
    diagnostics: [
      diag(
        'invalidValue',
        'tests[1].root',
        'suite root 必须为非空字符串且不含 glob 通配符（原值 "bad*"），该 suite 已被剔除。',
      ),
      diag('defaultApplied', 'schema', 'schema 字段未设置，使用默认值 "spec-driven"。'),
      diag('defaultApplied', 'tests[0].coverage.lines', '覆盖率阈值 lines 未设置，使用默认值 80。'),
      diag(
        'defaultApplied',
        'tests[0].mutation.score',
        '变异测试得分阈值 score 未设置，使用默认值 70。',
      ),
    ],
    ...overrides,
  };
}

/** 以 context 承载归属 root 的报告（切换用例区分新旧根数据用）。 */
function reportForRoot(root: string): WorkspaceConfigReport {
  return makeReport({ config: makeConfig({ context: root }) });
}

// ---------------------------------------------------------------------------
// 写回链路 fixture：会话转录里的 `propose_config` 可应用态卡片
// ---------------------------------------------------------------------------

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

function sessionSummary(): SessionSummary {
  return {
    row: {
      id: 'ses-config',
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'config', sourceRef: null },
      createdAt: TS,
      updatedAt: TS + 9,
    },
    stats: { turnCount: 1, totalDurationMs: null, inputTokens: null, outputTokens: null },
    turns: [turn(1, 'completed', 'ses-config')],
  };
}

/** `propose_config` 可应用态载荷（候选 = 写回目标，一处差异）。 */
const PROPOSE_PAYLOAD = JSON.stringify({
  current: { schema: 'spec-driven' },
  candidate: { schema: 'spec-driven', context: '写回后的上下文' },
  changes: [{ path: 'context', kind: 'added', before: null, after: '写回后的上下文' }],
  summary: '候选合法，共 1 处变更',
});

/** 含 `propose_config` 工具卡的转录（toolUse + toolResult 配成单卡）。 */
function proposeTranscript(): AgentEvent[] {
  return [
    {
      seq: 0,
      turnId: 1,
      timestampMs: TS,
      kind: 'message',
      role: 'assistant',
      blocks: [{ kind: 'toolUse', id: 'tu_cfg', name: 'propose_config', input: {} }],
      parentToolUseId: null,
    },
    {
      seq: 1,
      turnId: 1,
      timestampMs: TS,
      kind: 'message',
      role: 'tool',
      blocks: [{ kind: 'toolResult', id: 'tu_cfg', content: PROPOSE_PAYLOAD, isError: false }],
      parentToolUseId: null,
    },
  ];
}

function sessionPage(): SessionPage {
  return { sessions: [sessionSummary()], total: 1 };
}

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

/** 以 data-suite-root 定位 suite 卡片。 */
function suiteCardByRoot(root: string): HTMLElement {
  const hit = screen
    .getAllByTestId('config-suite')
    .find((card) => card.getAttribute('data-suite-root') === root);
  if (!hit) throw new Error(`data-suite-root 为 ${root} 的 suite 卡片不存在`);
  return hit;
}

beforeEach(() => {
  invokeMock.mockReset();
  ChannelMock.instances.length = 0;
});

describe('ConfigView：页面编排（AC-5 / AC-6）', () => {
  it('完整报告：五分区依序呈现（diagnostics 置顶 → 基础配置 → tests → write_protection → 未知字段），页面根 config-view 在场', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-extra') !== null).toBe(true));
    expect(screen.getByTestId('config-view') !== null).toBe(true);

    const view = screen.getByTestId('config-view');
    const diagnostics = within(view).getByTestId('config-diagnostics');
    const basic = within(view).getByTestId('config-basic');
    const tests = within(view).getByTestId('config-tests');
    const protection = within(view).getByTestId('config-write-protection');
    const extra = within(view).getByTestId('config-extra');
    expect(
      diagnostics.compareDocumentPosition(basic) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(basic.compareDocumentPosition(tests) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(
      tests.compareDocumentPosition(protection) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      protection.compareDocumentPosition(extra) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it('挂载以当前 root 发起一次解析（workspace_config { root }），loading 行在场、数据到达后卸载', async () => {
    let resolveLoad!: (value: WorkspaceConfigReport) => void;
    invokeMock.mockImplementation(() => {
      return new Promise<WorkspaceConfigReport>((resolve) => {
        resolveLoad = resolve;
      });
    });
    render(<ConfigView root={ROOT} />);

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('workspace_config', { root: ROOT });
    expect(screen.getByTestId('config-loading') !== null).toBe(true);

    await act(async () => {
      resolveLoad(makeReport());
    });
    expect(screen.queryByTestId('config-loading')).toBeNull();
  });

  it('defaultedPaths 派生对位：DefaultApplied path 驱动对应位置「未设（默认 N）」，未对位字段不标注', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-tests') !== null).toBe(true));

    // 基础配置区：schema 在 defaultedPaths → 「未设（默认 spec-driven）」
    const basic = screen.getByTestId('config-basic');
    expect(within(basic).getByText('未设（默认 spec-driven）') !== null).toBe(true);

    // tests[0] 卡片：coverage.lines → 未设（默认 80）；mutation.score → 未设（默认 70）
    const card0 = suiteCardByRoot('packages/desktop');
    expect(within(card0).getAllByText('未设（默认 80）')).toHaveLength(1);
    expect(within(card0).getByText('未设（默认 70）') !== null).toBe(true);

    // tests[1] 卡片同字段不受波及（无任何标注）
    const card1 = suiteCardByRoot('src-tauri');
    expect(within(card1).queryByText(/未设（默认/)).toBeNull();
  });
});

describe('ConfigView：状态面（AC-7）', () => {
  it('命令 reject：config-error inline 持久呈现（testid 承载、无 toast 顶替），分区不渲染', async () => {
    invokeMock.mockRejectedValue(new Error('root 无效（C:\\demo\\alpha）：不存在'));

    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-error') !== null).toBe(true));
    expect(screen.getByTestId('config-error').textContent).toContain('解析失败');
    expect(screen.getByTestId('config-error').textContent).toContain('root 无效');

    // inline 持久呈现：静置后仍在场，且无 toast 顶替（无 toast 节点出现）
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(screen.getByTestId('config-error') !== null).toBe(true);
    expect(document.querySelector('[data-sonner-toast]')).toBeNull();

    // 分区不渲染、loading 复位
    expect(screen.queryByTestId('config-basic')).toBeNull();
    expect(screen.queryByTestId('config-tests')).toBeNull();
    expect(screen.queryByTestId('config-loading')).toBeNull();
  });

  it('文件级 fatal 诊断（readFailed / jsonInvalid）：config-error inline 呈现（双来源之文件侧）', async () => {
    invokeMock.mockResolvedValue(
      makeReport({
        diagnostics: [diag('readFailed', '$', '工作区配置文件读取失败（权限），呈现全部默认值。')],
      }),
    );
    const failed = render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-error') !== null).toBe(true));
    expect(screen.getByTestId('config-error').textContent).toContain('配置文件读取或解析失败');
    expect(screen.queryByTestId('config-empty')).toBeNull();
    expect(screen.queryByTestId('config-basic')).toBeNull();
    failed.unmount();

    invokeMock.mockResolvedValue(
      makeReport({
        diagnostics: [
          diag('jsonInvalid', '$', '工作区配置文件不是合法 JSON（…），呈现全部默认值。'),
        ],
      }),
    );
    render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-error') !== null).toBe(true));
    expect(screen.getByTestId('config-error').textContent).toContain('配置文件读取或解析失败');
  });

  it('fileMissing 诊断：config-empty 空态（非错误、无 error 语义），文案说明默认值行为，与 diagnostics 警示区分', async () => {
    invokeMock.mockResolvedValue(
      makeReport({
        config: makeConfig({ $schema: null, staticAnalysis: null }),
        diagnostics: [diag('fileMissing', '$', '工作区配置文件不存在，呈现全部默认值。')],
      }),
    );

    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-empty') !== null).toBe(true));
    expect(screen.getByTestId('config-empty').textContent).toContain('无配置文件');
    expect(screen.getByTestId('config-empty').textContent).toContain('默认值');
    expect(screen.queryByTestId('config-error')).toBeNull();
    // 空态非分区呈现态：五分区与 diagnostics 警示区均不在场
    expect(screen.queryByTestId('config-diagnostics')).toBeNull();
    expect(screen.queryByTestId('config-basic')).toBeNull();
  });
});

describe('ConfigView：刷新交互与切换工作区（AC-6）', () => {
  it('点击 config-refresh 恰再发一次解析；loading 期间刷新钮 disabled', async () => {
    let resolveLoad!: (value: WorkspaceConfigReport) => void;
    invokeMock.mockImplementation(() => {
      return new Promise<WorkspaceConfigReport>((resolve) => {
        resolveLoad = resolve;
      });
    });
    render(<ConfigView root={ROOT} />);

    const refresh = screen.getByTestId<HTMLButtonElement>('config-refresh');
    expect(refresh.disabled).toBe(true);

    await act(async () => {
      resolveLoad(makeReport());
    });
    await waitFor(() => expect(refresh.disabled).toBe(false));

    const before = countOf('workspace_config');
    fireEvent.click(refresh);
    await waitFor(() => expect(countOf('workspace_config')).toBe(before + 1));
    expect(invokeMock).toHaveBeenLastCalledWith('workspace_config', { root: ROOT });
  });

  it('root prop 变化：以新根重取，旧根报告不呈现（hook 归属抑制经页面可见）', async () => {
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === ROOT) {
        return Promise.resolve(reportForRoot(ROOT));
      }
      return new Promise<WorkspaceConfigReport>(() => {});
    });
    const { rerender } = render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));
    expect(screen.getByTestId('config-basic').textContent).toContain(ROOT);

    rerender(<ConfigView root={'C:\\demo\\beta'} />);
    await act(async () => {});

    // 过渡轮：旧根报告不呈现（分区隐藏、loading 行接管）
    expect(screen.queryByTestId('config-basic')).toBeNull();
    expect(screen.getByTestId('config-loading') !== null).toBe(true);
    expect(invokeMock).toHaveBeenLastCalledWith('workspace_config', { root: 'C:\\demo\\beta' });
  });
});

describe('ConfigView：写回边界（AC-8）', () => {
  it('页面零直接写控件（无文本输入 / 保存 / 删除）；缺省收起态零会话类 invoke，全程无写命令发起', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-extra') !== null).toBe(true));

    // 页面级控件只有刷新钮（写回入口在左栏提案卡上，不属页面本体）
    const rightColumn = screen.getByTestId('config-refresh');
    expect(rightColumn).toBeTruthy();
    expect(screen.queryByTestId('config-save')).toBeNull();
    expect(screen.queryAllByRole('textbox'), '缺省收起态 composer 不在场').toHaveLength(0);
    expect(screen.queryByText('保存')).toBeNull();
    expect(screen.queryByText('删除')).toBeNull();

    // 缺省收起态：零会话类 invoke、零写命令
    for (const command of [
      'list_config_chat_sessions',
      'session_detail',
      'agent_session_transcript',
      'agent_session_watch',
      'agent_start',
      'workspace_config_apply',
    ]) {
      expect(countOf(command), `${command} 零调用`).toBe(0);
    }
    expect(invokeMock.mock.calls.every(([name]) => name === 'workspace_config')).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 本变更：横向分栏（左会话面板 → 右页头 + 五分区）+ 应用后单路重取
//
// 进程边界：仅 @tauri-apps/api/core 的 invoke（按命令名分派，含会话面命令）；
// `ConfigChatPanel` / `useConfigChat` / 五分区全真实组合——本文件兼作跨模块组合
// 用例挂靠入口（写回方向：卡片 → workspace_config_apply → 页面 refresh）。
// ---------------------------------------------------------------------------

/** 会话面命令（缺省收起态应零调用）。 */
const SESSION_COMMANDS = [
  'list_config_chat_sessions',
  'session_detail',
  'agent_session_transcript',
  'agent_session_watch',
  'agent_start',
  'agent_stop',
];

/** 页面源码（结构锚用例的读取面）。 */
function configViewSource(): string {
  return readFileSync(resolve(process.cwd(), 'src/views/config/config-view.tsx'), 'utf8');
}

describe('ConfigView：横向分栏结构（AC-5）', () => {
  it('左栏 config-chat-* 骨架与右栏页头 + 五分区同屏，config-view testid 保留', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));

    expect(screen.getByTestId('config-view')).toBeTruthy();
    // 左栏（缺省收起 → 窄条 + 展开入口）
    expect(screen.getByTestId('config-chat-rail')).toBeTruthy();
    expect(screen.getByTestId('config-chat-expand')).toBeTruthy();
    // 右栏：页头 + 状态面 + 五分区
    expect(screen.getByTestId('config-refresh')).toBeTruthy();
    for (const id of [
      'config-diagnostics',
      'config-basic',
      'config-tests',
      'config-write-protection',
      'config-extra',
    ]) {
      expect(screen.getByTestId(id), `分区 ${id} 在右栏`).toBeTruthy();
    }
  });

  it('展开左栏后 composer 在案（config-chat 前缀）且分区不受影响', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));

    await act(async () => {
      fireEvent.click(screen.getByTestId('config-chat-expand'));
    });

    expect(screen.getByTestId('config-chat-composer')).toBeTruthy();
    expect(screen.getByTestId('config-chat-prompt')).toBeTruthy();
    expect(screen.getByTestId('config-basic')).toBeTruthy();
  });
});

describe('ConfigView：写回链路（卡片 → apply → refresh 单路）', () => {
  it('页面把 `state.refresh` 作为唯一 onApplied 传给会话面板（结构锚：单路重取入口恰一处，不采信 apply 返回值）', () => {
    const source = configViewSource();
    // 插桩不敏感形态：只锚「元素 + 属性表达式」两段，不整行逐字比对
    // （Stryker 会重写 JSX 行内字符串字面量，整行比较在沙箱内会失效）。
    expect(source.match(/onApplied=\{state\.refresh\}/g) ?? [], '单路重取入口恰一处').toHaveLength(
      1,
    );
    expect(source).toMatch(/<ConfigChatPanel[^>]*onApplied=\{state\.refresh\}/);
    // 不采信 apply 返回值作第二真相源：页面零 apply 调用
    expect(source).not.toContain('workspaceConfigApply');
    expect(source).not.toContain('workspace_config_apply');
  });

  it('提案卡点「应用」成功 → workspace_config 恰增加一次（只经 refresh 单路，新值就地呈现）', async () => {
    let configCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'workspace_config') {
        configCalls += 1;
        // 首次挂载 = 旧值；refresh 后 = 写回后的新值。
        return Promise.resolve(
          makeReport({
            config: makeConfig({
              context: configCalls === 1 ? '写回前的上下文' : '写回后的上下文',
            }),
          }),
        );
      }
      if (command === 'list_config_chat_sessions') return Promise.resolve(sessionPage());
      if (command === 'session_detail') return Promise.resolve(sessionSummary());
      if (command === 'agent_session_transcript') return Promise.resolve(proposeTranscript());
      if (command === 'agent_session_watch') return new Promise<null>(() => {});
      if (command === 'workspace_config_apply') {
        // 返回值刻意带第三份内容：页面若采信它作真相源，右栏就会呈现「apply 返回值」。
        return Promise.resolve(
          makeReport({ config: makeConfig({ context: 'apply 返回值（不应呈现）' }) }),
        );
      }
      return Promise.resolve(null);
    });

    render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));
    expect(countOf('workspace_config'), '挂载恰一次解析').toBe(1);

    // 展开左栏 → 打开历史 → 选中含提案卡的会话
    await act(async () => {
      fireEvent.click(screen.getByTestId('config-chat-expand'));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('config-chat-history'));
    });
    await act(async () => {});
    const [item] = screen.getAllByTestId('config-chat-history-item');
    await act(async () => {
      fireEvent.click(item);
    });
    await act(async () => {});
    await act(async () => {});

    const applyButton = await waitFor(() => {
      const button = screen.getByTestId<HTMLButtonElement>('config-propose-apply');
      expect(button.disabled).toBe(false);
      return button;
    });
    await act(async () => {
      fireEvent.click(applyButton);
    });
    await act(async () => {});

    // 写回一次、页面重取恰一次（新值就地呈现）
    expect(countOf('workspace_config_apply'), '应用恰一次').toBe(1);
    const [applyCall] = invokeMock.mock.calls.filter(([name]) => name === 'workspace_config_apply');
    expect(applyCall?.[1]).toMatchObject({
      root: ROOT,
      candidate: { schema: 'spec-driven', context: '写回后的上下文' },
    });
    await waitFor(() => expect(countOf('workspace_config')).toBe(2));
    await waitFor(() =>
      expect(screen.getByTestId('config-basic').textContent).toContain('写回后的上下文'),
    );
    // 单路真相源：apply 的返回值不入呈现面
    expect(screen.getByTestId('config-basic').textContent).not.toContain('apply 返回值');
  });

  it('缺省收起态：零会话类 invoke（面板不自取数）', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));

    for (const command of SESSION_COMMANDS) {
      expect(countOf(command), `${command} 零调用`).toBe(0);
    }
    expect(countOf('workspace_config')).toBe(1);
  });

  it('root 切换 → 会话面板复位（无残留会话 id）且页面按新根重取', async () => {
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === ROOT) return Promise.resolve(reportForRoot(ROOT));
      return Promise.resolve(reportForRoot('C:demo\beta'));
    });
    const { rerender } = render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));

    rerender(<ConfigView root={'C:demo\beta'} />);
    await act(async () => {});

    expect(invokeMock).toHaveBeenLastCalledWith('workspace_config', {
      root: 'C:demo\beta',
    });
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));
    expect(screen.getByTestId('config-basic').textContent).toContain('C:demo\beta');
    // 分栏骨架不因切根卸载（会话面板恒挂载，内部状态复位归 hook）
    expect(screen.getByTestId('config-chat-rail')).toBeTruthy();
  });
});

describe('ConfigView：状态面在分栏后仍在右栏（AC-7 零改判）', () => {
  it('报告命令 reject → config-error 在案、右侧不崩、会话面板照常挂载', async () => {
    invokeMock.mockRejectedValue(new Error('root 无效（C:demoalpha）'));
    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-error') !== null).toBe(true));
    expect(screen.getByTestId('config-error').textContent).toContain('解析失败');
    expect(screen.getByTestId('config-chat-rail')).toBeTruthy();
    expect(screen.queryByTestId('config-basic')).toBeNull();
  });

  it.each([
    [
      '文件缺失',
      makeReport({
        config: makeConfig({ $schema: null, staticAnalysis: null }),
        diagnostics: [diag('fileMissing', '$', '工作区配置文件不存在，呈现全部默认值。')],
      }),
      'config-empty',
    ],
    [
      'fatal 诊断',
      makeReport({
        diagnostics: [diag('readFailed', '$', '工作区配置文件读取失败（权限）。')],
      }),
      'config-error',
    ],
  ])('%s 形态：状态面在案、分区隐藏，分栏与会话面板仍在', async (_label, report, testId) => {
    invokeMock.mockResolvedValue(report);
    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId(testId) !== null).toBe(true));
    expect(screen.getByTestId('config-chat-rail')).toBeTruthy();
    expect(screen.queryByTestId('config-basic')).toBeNull();
    expect(screen.queryByTestId('config-tests')).toBeNull();
  });
});

describe('ConfigView：结构锚（折叠真相单一 / 页头改判）', () => {
  it('源码零折叠 state、零 onResize 像素派生、零面板显隐开关（折叠全在骨架内）', () => {
    const source = configViewSource();
    for (const forbidden of [
      'onResize',
      'useState',
      'isCollapsed',
      'setIsCollapsed',
      'collapsed',
    ]) {
      expect(source, `页面本体零 ${forbidden}`).not.toContain(forbidden);
    }
    // 折叠能力由骨架自带（ResizablePanelGroup + SessionPanel 内建）
    expect(source).toContain('ResizablePanelGroup');
    expect(source).toContain('ConfigChatPanel');
  });

  it('页头文案改判为会话 + 应用语义、零「只读页面」字样，config-* testid 不变', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('config-basic') !== null).toBe(true));

    expect(document.body.textContent).not.toContain('只读');
    const source = configViewSource();
    expect(source).not.toContain('只读页面');
    expect(source).toContain('写回的唯一入口在左栏提案卡上');
    expect(screen.getByTestId('config-refresh').textContent).toContain('刷新');
  });
});
