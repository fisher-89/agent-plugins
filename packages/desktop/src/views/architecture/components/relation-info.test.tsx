import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiRelation, ArchiRelationDetail } from '../../../types/generated/bindings';
import type { ArchiRelationState, RelationEdge } from '../hooks/use-archi-relation';
import { RelationInfo } from './relation-info';

/**
 * `RelationInfo` 单测（`relation-api-dependency-check` / AC-7 / D13 / D14 / D19）：七段
 * 固定序 + 四空态优先级（失败 > 豁免 > 未声明 > 无 import）+ 行内错误与重试；数据全由
 * 调用方注入——组件不自行求差、不读漂移报告、不发 `archi_relation`。
 *
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`，只为「模型段查看」与「零报告
 * invoke」判据记录调用（`FileViewer` 复用真实组件）。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const EDGE: RelationEdge = { from: 'App.Frontend.Canvas', to: 'App.Frontend.GeneratedBindings' };

/** 图里的关系投影（声明清单是面板的图侧数据源，零取数）。 */
function relation(overrides: Partial<ArchiRelation> = {}): ArchiRelation {
  return {
    from: EDGE.from,
    to: EDGE.to,
    description: '画布经生成绑定调用命令',
    mark: 'none',
    reason: null,
    apis: ['AgentInput', 'SessionPanel'],
    ...overrides,
  };
}

/** 关系详情投影（`archi_relation` 的出参）。 */
function detail(overrides: Partial<ArchiRelationDetail> = {}): ArchiRelationDetail {
  return {
    from: EDGE.from,
    to: EDGE.to,
    description: '画布经生成绑定调用命令',
    mark: 'none',
    reason: null,
    modelFile: '.dev-team/architecture/graph/model/06-frontend.toml',
    modelLine: 188,
    declared: ['AgentInput', 'SessionPanel'],
    used: ['AgentInput'],
    usedNotDeclared: [],
    declaredNotUsed: ['SessionPanel'],
    occurrences: [
      {
        file: 'src/views/architecture/hooks/use-archi-relation.ts',
        line: 12,
        specifier: '../bindings',
        typeOnly: false,
      },
    ],
    ...overrides,
  };
}

/** 取数状态面（四字段恒在场）。 */
function state(overrides: Partial<ArchiRelationState> = {}): ArchiRelationState {
  return {
    detail: detail(),
    loading: false,
    error: null,
    retry: () => undefined,
    ...overrides,
  };
}

interface RenderOptions {
  root?: string | null;
  edge?: RelationEdge;
  relation?: ArchiRelation;
  state?: ArchiRelationState;
  exemptVia?: 'config' | 'inline' | null;
  onRetry?: () => void;
}

/** 统一渲染面（六字段全注入）。 */
function renderPanel(options: RenderOptions = {}): void {
  render(
    <RelationInfo
      root={options.root === undefined ? 'C:\\demo\\alpha' : options.root}
      edge={options.edge ?? EDGE}
      relation={options.relation ?? relation()}
      state={options.state ?? state()}
      exemptVia={options.exemptVia ?? null}
      onRetry={options.onRetry ?? (() => undefined)}
    />,
  );
}

/** 某命令的调用入参清单。 */
function callsOf(command: string): Record<string, unknown>[] {
  return invokeMock.mock.calls
    .filter(([name]) => name === command)
    .map(([, params]) => (params ?? {}) as Record<string, unknown>);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'read_file') {
      const content = '# 前端模型\n\n画布经生成绑定调用命令。\n';
      return Promise.resolve({
        content,
        path: typeof params?.path === 'string' ? params.path : '',
        stat: { size: content.length, modifiedMs: null, isFile: true, isDir: false },
        offset: 0,
        nextOffset: content.length,
        truncated: false,
        encoding: 'utf8',
      });
    }
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
});

afterEach(() => {
  cleanup();
});

describe('七段固定序', () => {
  it('应在健康详情下逐段在场（关系 / 标记 / 说明 / 声明 / 实际使用 / 两桶 / 命中点 / 模型）', () => {
    renderPanel();
    const panel = screen.getByTestId('archi-relation-info');
    expect(panel.getAttribute('data-from')).toBe(EDGE.from);
    expect(panel.getAttribute('data-to')).toBe(EDGE.to);
    expect(within(panel).getByTestId('archi-relation-fqn').textContent).toBe(
      `${EDGE.from} → ${EDGE.to}`,
    );
    expect(within(panel).getByText('none')).toBeTruthy();
    expect(within(panel).getByText('画布经生成绑定调用命令')).toBeTruthy();
    expect(within(panel).getByTestId('archi-relation-declared').textContent).toBe(
      'AgentInput · SessionPanel',
    );
    expect(within(panel).getByTestId('archi-relation-used').textContent).toBe('AgentInput');
    expect(within(panel).getByTestId('archi-relation-used-not-declared').textContent).toContain(
      '—',
    );
    expect(within(panel).getByTestId('archi-relation-declared-not-used').textContent).toContain(
      'SessionPanel',
    );
    expect(within(panel).getByTestId('archi-relation-occurrence').textContent).toContain(
      'src/views/architecture/hooks/use-archi-relation.ts:12',
    );
    expect(within(panel).getByTestId('archi-relation-model').textContent).toContain(
      '.dev-team/architecture/graph/model/06-frontend.toml:188',
    );
  });

  it('应在 state.detail 为 null 时仍按图载荷渲染声明段（声明零取数）', () => {
    renderPanel({ state: state({ detail: null }) });
    expect(screen.getByTestId('archi-relation-declared').textContent).toBe(
      'AgentInput · SessionPanel',
    );
    expect(screen.getByTestId('archi-relation-used').textContent).toBe('—');
    expect(screen.queryByTestId('archi-relation-model')).toBeNull();
    expect(callsOf('archi_relation')).toHaveLength(0);
  });

  it('应逐条渲染命中点并按入参序（不重排）', () => {
    renderPanel({
      state: state({
        detail: detail({
          occurrences: [
            { file: 'src/b.ts', line: 9, specifier: '@/b', typeOnly: false },
            { file: 'src/a.ts', line: 2, specifier: null, typeOnly: true },
          ],
        }),
      }),
    });
    const rows = screen.getAllByTestId('archi-relation-occurrence');
    expect(rows.map((row) => row.textContent)).toEqual(['src/b.ts:9  @/b', 'src/a.ts:2']);
  });

  it('应在点击查看时取模型文件正文（载荷 = root + 模型文件路径）', async () => {
    renderPanel();
    const panel = screen.getByTestId('archi-relation-info');
    fireEvent.click(within(panel).getByRole('button', { name: '查看' }));
    await waitFor(() => expect(callsOf('read_file')).toHaveLength(1));
    expect(callsOf('read_file')[0]?.path).toBe(
      `${'C:\\demo\\alpha'}/.dev-team/architecture/graph/model/06-frontend.toml`,
    );
  });

  it('应在 root 为 null 时照常渲染且查看不可点（零 read_file）', () => {
    renderPanel({ root: null });
    const button = screen.getByRole('button', { name: '查看' });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
    expect(callsOf('read_file')).toHaveLength(0);
    expect(screen.getByTestId('archi-relation-declared')).toBeTruthy();
  });

  it('应在说明为 null 时不占文案（不渲空串、不塌相邻段）', () => {
    renderPanel({ relation: relation({ description: null }) });
    const panel = screen.getByTestId('archi-relation-info');
    expect(within(panel).getByTestId('archi-relation-declared')).toBeTruthy();
    expect(panel.textContent).not.toContain('null');
  });

  it('应在超长名字与特殊字符时原样渲染且不解析为 HTML', () => {
    const long = 'N'.repeat(1100);
    renderPanel({
      relation: relation({ description: '<b>不是标签</b>', apis: [long, 'Agent🙂'] }),
    });
    const declared = screen.getByTestId('archi-relation-declared');
    expect(declared.textContent).toContain(long);
    expect(declared.textContent).toContain('Agent🙂');
    expect(screen.getByTestId('archi-relation-info').querySelector('b')).toBeNull();
  });

  it('应在两桶皆空时各显占位（不塌段）', () => {
    renderPanel({
      state: state({
        detail: detail({ usedNotDeclared: [], declaredNotUsed: [] }),
      }),
    });
    expect(screen.getByTestId('archi-relation-used-not-declared').textContent).toContain('—');
    expect(screen.getByTestId('archi-relation-declared-not-used').textContent).toContain('—');
    expect(screen.queryByTestId('archi-relation-declared-not-used-note')).toBeNull();
  });

  it('应在声明没用时并列两条成因（不只写「没用」）', () => {
    renderPanel();
    const note = screen.getByTestId('archi-relation-declared-not-used-note');
    expect(note.textContent).toContain('不再依赖');
    expect(note.textContent).toContain('名字写错');
    expect(note.textContent).toContain('glob');
  });

  it('应在命中点行号为 0 时原样呈现文件路径（零 panic、不伪造行号）', () => {
    renderPanel({
      state: state({
        detail: detail({
          occurrences: [{ file: 'src/a.ts', line: 0, specifier: null, typeOnly: false }],
        }),
      }),
    });
    const row = screen.getByTestId('archi-relation-occurrence');
    expect(row.textContent).toContain('src/a.ts');
  });
});

describe('四空态与优先级', () => {
  it('应在关系未声明 apis 时只渲空态①（不渲染事实四段）', () => {
    renderPanel({ relation: relation({ apis: [] }) });
    expect(screen.getByTestId('archi-relation-undeclared').textContent).toContain('未声明依赖 API');
    expect(screen.queryByTestId('archi-relation-used')).toBeNull();
    expect(screen.queryByTestId('archi-relation-used-not-declared')).toBeNull();
    expect(screen.queryByTestId('archi-relation-occurrence')).toBeNull();
    expect(screen.queryByTestId('archi-relation-model')).toBeNull();
    expect(screen.queryByTestId('archi-relation-no-import')).toBeNull();
  });

  it('应在扫描面无命中时渲空态②（声明段照常）', () => {
    renderPanel({ state: state({ detail: detail({ occurrences: [] }) }) });
    expect(screen.getByTestId('archi-relation-no-import').textContent).toContain(
      '扫描面无该关系的 import',
    );
    expect(screen.getByTestId('archi-relation-declared')).toBeTruthy();
  });

  it('应在豁免边渲空态③（含来源与理由）', () => {
    for (const via of ['config', 'inline'] as const) {
      cleanup();
      renderPanel({
        relation: relation({ mark: 'exempt', reason: '迁移期允许' }),
        exemptVia: via,
      });
      const exempt = screen.getByTestId('archi-relation-exempt');
      expect(exempt.textContent).toContain('已豁免');
      expect(exempt.textContent).toContain(via);
      expect(exempt.textContent).toContain('迁移期允许');
    }
  });

  it('应在豁免来源缺席时仍渲空态③（来源句退化为未知来源）', () => {
    renderPanel({ relation: relation({ mark: 'exempt', reason: '迁移期允许' }) });
    const exempt = screen.getByTestId('archi-relation-exempt');
    expect(exempt.textContent).toContain('未知来源');
    expect(exempt.textContent).toContain('迁移期允许');
  });

  it('应在失败时渲空态④并提供重试（点击触发 onRetry 恰一次）', () => {
    const onRetry = vi.fn();
    renderPanel({ state: state({ detail: null, error: '命令失败' }), onRetry });
    expect(screen.getByTestId('archi-relation-error').textContent).toContain('命令失败');
    fireEvent.click(screen.getByTestId('archi-relation-retry'));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('应在失败与未声明同时成立时只渲错误态（优先级 ① > ③）', () => {
    renderPanel({
      relation: relation({ apis: [] }),
      state: state({ detail: null, error: '命令失败' }),
    });
    expect(screen.getByTestId('archi-relation-error')).toBeTruthy();
    expect(screen.queryByTestId('archi-relation-undeclared')).toBeNull();
  });

  it('应在失败与豁免同时成立时只渲错误态（优先级 ① > ②）', () => {
    renderPanel({
      relation: relation({ mark: 'exempt', reason: '迁移期允许' }),
      exemptVia: 'config',
      state: state({ detail: null, error: '命令失败' }),
    });
    expect(screen.getByTestId('archi-relation-error')).toBeTruthy();
    expect(screen.queryByTestId('archi-relation-exempt')).toBeNull();
  });

  it('应在错误与陈旧详情同时非空时只渲错误态（事实四段缺席、零 panic）', () => {
    renderPanel({
      state: state({ detail: detail(), error: '命令失败' }),
    });
    expect(screen.getByTestId('archi-relation-error').textContent).toContain('命令失败');
    for (const testId of [
      'archi-relation-used',
      'archi-relation-used-not-declared',
      'archi-relation-occurrence',
      'archi-relation-model',
    ]) {
      expect(screen.queryByTestId(testId)).toBeNull();
    }
    expect(screen.getByTestId('archi-relation-fqn').textContent).toBe(`${EDGE.from} → ${EDGE.to}`);
    expect(screen.getByTestId('archi-relation-declared').textContent).toContain('AgentInput');
  });

  it('应在豁免与未声明同时成立时只渲豁免态（优先级 ② > ③）', () => {
    renderPanel({
      relation: relation({ mark: 'exempt', reason: '迁移期允许', apis: [] }),
      exemptVia: 'inline',
    });
    expect(screen.getByTestId('archi-relation-exempt')).toBeTruthy();
    expect(screen.queryByTestId('archi-relation-undeclared')).toBeNull();
  });

  it('应在取数中独立呈现加载态（不占四空态槽位）', () => {
    renderPanel({ state: state({ detail: null, loading: true }) });
    expect(screen.getByTestId('archi-relation-loading').textContent).toContain('取数中');
    expect(screen.queryByTestId('archi-relation-error')).toBeNull();
    expect(screen.queryByTestId('archi-relation-exempt')).toBeNull();
    expect(screen.queryByTestId('archi-relation-undeclared')).toBeNull();
    expect(screen.queryByTestId('archi-relation-no-import')).toBeNull();
  });

  it('应在错误清除后恢复七段（四空态 testid 全缺席）', () => {
    const { rerender } = render(
      <RelationInfo
        root={'C:\\demo\\alpha'}
        edge={EDGE}
        relation={relation()}
        state={state({ detail: null, error: '命令失败' })}
        exemptVia={null}
        onRetry={() => undefined}
      />,
    );
    expect(screen.getByTestId('archi-relation-error')).toBeTruthy();

    rerender(
      <RelationInfo
        root={'C:\\demo\\alpha'}
        edge={EDGE}
        relation={relation()}
        state={state()}
        exemptVia={null}
        onRetry={() => undefined}
      />,
    );
    expect(screen.queryByTestId('archi-relation-error')).toBeNull();
    expect(screen.queryByTestId('archi-relation-exempt')).toBeNull();
    expect(screen.queryByTestId('archi-relation-undeclared')).toBeNull();
    expect(screen.queryByTestId('archi-relation-no-import')).toBeNull();
    expect(screen.getByTestId('archi-relation-used')).toBeTruthy();
  });
});

describe('注入面与职责边界', () => {
  it('应在每一对注入值下渲染对应该值（面板零自取数）', () => {
    renderPanel({
      relation: relation({ from: 'App.A', to: 'App.B', apis: ['OnlyName'] }),
      state: state({ detail: detail({ used: ['OtherName'] }) }),
    });
    expect(screen.getByTestId('archi-relation-fqn').textContent).toBe('App.A → App.B');
    expect(screen.getByTestId('archi-relation-declared').textContent).toBe('OnlyName');
    expect(screen.getByTestId('archi-relation-used').textContent).toBe('OtherName');
    expect(callsOf('archi_relation')).toHaveLength(0);
    expect(callsOf('archi_drift')).toHaveLength(0);
  });

  it('应在陈旧回包（detail 的元素对与 edge 不一致）时仍按 edge 渲染关系行', () => {
    renderPanel({
      state: state({ detail: detail({ from: 'App.Stale', to: 'App.Stale.To' }) }),
    });
    expect(screen.getByTestId('archi-relation-fqn').textContent).toBe(`${EDGE.from} → ${EDGE.to}`);
    expect(screen.getByTestId('archi-relation-declared').textContent).toContain('AgentInput');
  });

  it('应在渲染期间零 invoke（数据全来自 prop）', () => {
    renderPanel();
    expect(invokeMock.mock.calls).toHaveLength(0);
  });

  it('应在源码内零二次求差与零报告读取（职责边界机械锚）', () => {
    const source = readFileSync(
      resolve(process.cwd(), 'src/views/architecture/components/relation-info.tsx'),
      'utf8',
    );
    for (const forbidden of [
      "from '@tauri-apps/api/core'",
      'commands.',
      'invoke(',
      'useArchiRelation(',
      'api_buckets',
      'Set(',
    ]) {
      expect(source).not.toContain(forbidden);
    }
    expect(source).toContain('relation.apis');
    expect(source).toContain('state.detail');
  });
});
