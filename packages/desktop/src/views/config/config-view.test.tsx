// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

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

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

const ROOT = 'C:\\demo\\alpha';

function makeConfig(overrides: Partial<WorkspaceConfig> = {}): WorkspaceConfig {
  return {
    $schema: 'https://example.com/spec-driven.schema.json',
    schema: 'spec-driven',
    context: '桌面端插件仓库',
    rules: { proposal: ['提案规则一'], tasks: ['任务规则一'] },
    staticAnalysis: 'clippy',
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

describe('ConfigView：只读边界（AC-8）', () => {
  it('页面无任何写回入口：唯一可交互控件为刷新钮，无文本输入 / 保存 / 删除控件，全程无写命令发起', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<ConfigView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('config-extra') !== null).toBe(true));

    // 唯一可交互控件：刷新钮
    const buttons = screen.getAllByRole('button');
    expect(buttons).toHaveLength(1);
    expect(buttons[0].getAttribute('data-testid')).toBe('config-refresh');

    // 无文本输入 / 编辑 / 保存 / 删除控件
    expect(screen.queryAllByRole('textbox')).toHaveLength(0);
    expect(screen.queryByText('保存')).toBeNull();
    expect(screen.queryByText('删除')).toBeNull();

    // 全程无写命令发起：全部 invoke 均为 workspace_config 只读解析
    expect(invokeMock.mock.calls.every(([name]) => name === 'workspace_config')).toBe(true);
  });
});
