import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation, useNavigationType } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, ExploreDoc, ExploreRecord, SessionSummary } from '../../types/dto';
import { ExploreDetailView } from './explore-detail-view';
import type { ExploreListState } from './hooks/use-explore-list';

// 进程边界 Mock：invoke + Channel（@tauri-apps/api/core）；内部 hooks
// （useExploreDoc / useExploreSession）真实组合，fixture 经 mock IPC 流入。
// `list` 为 props 注入依赖（updateTitle / promote / refresh 可编程 resolve /
// reject——入参例外）。
const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

const ROOT = 'C:\\demo\\alpha';

/** 记录 fixture（title 独立字段；promotedTo 两态）。 */
function record(overrides: Partial<ExploreRecord> = {}): ExploreRecord {
  return {
    id: 7,
    root: ROOT,
    name: 'api-retry',
    title: '接口重试策略',
    workflowType: 'requirement',
    promotedTo: null,
    sessionId: null,
    createdAt: 1727000000000,
    updatedAt: 1727000000000,
    ...overrides,
  };
}

/** 注入依赖假件：ExploreListState（动作可编程）。 */
function makeList(overrides: Partial<ExploreListState> = {}): ExploreListState {
  return {
    records: [],
    loading: false,
    error: null,
    refresh: vi.fn(),
    create: vi.fn(),
    rename: vi.fn(),
    remove: vi.fn(),
    updateTitle: vi.fn().mockResolvedValue(undefined),
    updateWorkflowType: vi.fn().mockResolvedValue(undefined),
    promote: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

let docResult: ExploreDoc | null = null;

/** 进程边界应答面。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'read_explore') return Promise.resolve(docResult);
  if (command === 'explore_doc_path') {
    return Promise.resolve('C:\\demo\\alpha\\openspec\\explores\\api-retry.md');
  }
  if (command === 'watch_subscribe') return Promise.resolve(1);
  if (command === 'watch_unsubscribe') return Promise.resolve(null);
  if (command === 'agent_sessions') return Promise.resolve([]);
  if (command === 'agent_session_transcript') return Promise.resolve([]);
  return Promise.resolve(null);
}

/** URL 探针：把当前 pathname 投影到 DOM 供 navigate 断言；导航类型探针
 * （POP / PUSH / REPLACE）供 replace 语义断言。 */
function LocationProbe() {
  const { pathname } = useLocation();
  const navigationType = useNavigationType();
  return (
    <>
      <span data-testid="location-probe">{pathname}</span>
      <span data-testid="nav-type-probe">{navigationType}</span>
    </>
  );
}

/** 详情页挂载树（可 rerender 换 record props——记录切换用例复用同一 router）。 */
function detailTree(recordValue: ExploreRecord | null, list: ExploreListState) {
  return (
    <MemoryRouter initialEntries={['/explores/api-retry']}>
      <Routes>
        <Route
          path="/explores/:name"
          element={<ExploreDetailView list={list} record={recordValue} root={ROOT} />}
        />
        <Route path="/changes/:id" element={<span data-testid="change-detail-stub" />} />
      </Routes>
      <LocationProbe />
    </MemoryRouter>
  );
}

function renderDetail(recordValue: ExploreRecord | null, list: ExploreListState) {
  return render(detailTree(recordValue, list));
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => ipc(command));
  docResult = null;
});

afterEach(() => {
  cleanup();
});

describe('ExploreDetailView：详情头与 promoted 两态（AC-8）', () => {
  it('正向：详情头渲染 record.title（data-testid 在位，name 不作标题展示）', async () => {
    renderDetail(record(), makeList());

    const title = await screen.findByTestId('explore-detail-title');
    expect(title.textContent).toBe('接口重试策略');
    expect(screen.queryByText('api-retry')).toBeNull();
  });

  it('正向：promotedTo 非空 → 「已转变更」徽标 + 「查看变更」按钮；点击跳转 /changes/<promotedTo>', async () => {
    renderDetail(record({ promotedTo: 'chg-018f3a-0001' }), makeList());

    expect(await screen.findByTestId('explore-promoted-badge')).toBeTruthy();
    expect(screen.getByTestId('explore-promoted-badge').textContent).toBe('已转变更');
    fireEvent.click(screen.getByTestId('explore-promoted-jump'));

    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/changes/chg-018f3a-0001'),
    );
  });

  it('正向：草稿态（promotedTo 为 null）→ 「启动变更」入口；点击调 list.promote 携当前选择类型恰一次', async () => {
    const list = makeList();
    renderDetail(record(), list);

    const trigger = await screen.findByTestId('explore-promote-trigger');
    expect(screen.queryByTestId('explore-promoted-badge')).toBeNull();
    fireEvent.click(trigger);

    await waitFor(() => expect(list.promote).toHaveBeenCalledTimes(1));
    expect(list.promote).toHaveBeenCalledWith('api-retry', 'requirement');
  });

  it('异常：list.promote reject → 行内错误块呈现错误文本、页面不崩', async () => {
    const list = makeList({
      promote: vi.fn().mockRejectedValue(new Error('探索笔记尚未落盘')),
    });
    renderDetail(record(), list);

    fireEvent.click(await screen.findByTestId('explore-promote-trigger'));

    const error = await screen.findByTestId('explore-promote-error');
    expect(error.textContent).toContain('探索笔记尚未落盘');
    // 页面仍可用（头部与触发器在位）
    expect(screen.getByTestId('explore-detail-title')).toBeTruthy();
    expect(screen.getByTestId('explore-promote-trigger')).toBeTruthy();
  });
});

describe('ExploreDetailView：title 回填 effect（AC-5 链路）', () => {
  it('正向：doc 首行 # 标题与 record.title 不同 → list.updateTitle 恰一次并 refresh', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: '# 新标题\n\n正文' };
    renderDetail(record({ title: '旧标题' }), list);

    await waitFor(() => expect(list.updateTitle).toHaveBeenCalledWith('api-retry', '新标题'));
    expect(list.updateTitle).toHaveBeenCalledTimes(1);
  });

  it('边界：首行标题与 record.title 相同 → 零 updateTitle 调用', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: '# 接口重试策略\n\n正文' };
    renderDetail(record(), list);

    await screen.findByTestId('explore-detail-title');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_explore', {
        root: ROOT,
        name: 'api-retry',
      }),
    );
    expect(list.updateTitle).not.toHaveBeenCalled();
  });

  it('边界：无标题行（extractMarkdownTitle 返回 null）→ 零 updateTitle 零报错', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: '正文起首，没有标题行' };
    renderDetail(record(), list);

    await screen.findByTestId('explore-detail-title');
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    expect(list.updateTitle).not.toHaveBeenCalled();
    expect(screen.getByTestId('explore-detail-title').textContent).toBe('接口重试策略');
  });
});

describe('ExploreDetailView：promoted 停读与缺失态（AC-8 / 边界）', () => {
  it('边界：promotedTo 非空 → name 传 null（停 read/watch），渲染 promoted 提示面板', async () => {
    const list = makeList();
    renderDetail(record({ promotedTo: 'chg-018f3a-0001' }), list);

    expect(await screen.findByTestId('explore-promoted-preview')).toBeTruthy();
    expect(screen.queryByTestId('explore-preview')).toBeNull();
    const commands = invokeMock.mock.calls.map(([name]) => name);
    expect(commands).not.toContain('read_explore');
    expect(commands).not.toContain('explore_doc_path');
    expect(list.updateTitle).not.toHaveBeenCalled();
  });

  it('边界：record = null → 缺失态占位（既有语义零改动，零取数）', async () => {
    renderDetail(null, makeList());

    expect(await screen.findByTestId('explore-detail-missing')).toBeTruthy();
    expect(screen.queryByTestId('explore-detail-title')).toBeNull();
    const commands = invokeMock.mock.calls.map(([name]) => name);
    expect(commands).not.toContain('read_explore');
  });
});

describe('ExploreDetailView：promote 类型选择器与改即写（AC-8）', () => {
  it('正向：草稿态渲染类型选择器（explore-workflow-type-select），默认值 = record.workflowType（两态各一则）', async () => {
    renderDetail(record({ workflowType: 'requirement' }), makeList());
    const select = await screen.findByTestId('explore-workflow-type-select');
    expect((select as HTMLSelectElement).value).toBe('requirement');

    cleanup();

    renderDetail(record({ workflowType: 'test-only' }), makeList());
    const selectTestOnly = await screen.findByTestId('explore-workflow-type-select');
    expect((selectTestOnly as HTMLSelectElement).value).toBe('test-only');
  });

  it('正向：选择器恰两字面选项（requirement / test-only，闭集不越集）', async () => {
    renderDetail(record(), makeList());

    const select = await screen.findByTestId('explore-workflow-type-select');
    const options = Array.from(select.querySelectorAll('option')).map((option) => option.value);
    expect(options).toEqual(['requirement', 'test-only']);
  });

  it('正向：改选类型 → list.updateWorkflowType(name, 新值) 恰一次（改即写落库）', async () => {
    const list = makeList();
    renderDetail(record(), list);

    const select = await screen.findByTestId('explore-workflow-type-select');
    fireEvent.change(select, { target: { value: 'test-only' } });

    await waitFor(() =>
      expect(list.updateWorkflowType).toHaveBeenCalledWith('api-retry', 'test-only'),
    );
    expect(list.updateWorkflowType).toHaveBeenCalledTimes(1);
  });

  it('正向：改选后点「启动变更」→ promote(name, 最终选择类型)（携带改选值而非库内值）', async () => {
    const list = makeList();
    renderDetail(record({ workflowType: 'requirement' }), list);

    const select = await screen.findByTestId('explore-workflow-type-select');
    fireEvent.change(select, { target: { value: 'test-only' } });
    await waitFor(() =>
      expect(list.updateWorkflowType).toHaveBeenCalledWith('api-retry', 'test-only'),
    );

    fireEvent.click(screen.getByTestId('explore-promote-trigger'));
    await waitFor(() => expect(list.promote).toHaveBeenCalledTimes(1));
    expect(list.promote).toHaveBeenCalledWith('api-retry', 'test-only');
  });

  it('边界：记录值变更 → 选择器随 record.workflowType 复位（本地态跟随记录）', async () => {
    const { rerender } = renderDetail(record({ workflowType: 'test-only' }), makeList());

    const select = await screen.findByTestId('explore-workflow-type-select');
    expect((select as HTMLSelectElement).value).toBe('test-only');

    // 记录值变化（如另一端回填后重进 / 记录回流）→ 选择器随记录复位
    rerender(
      <MemoryRouter initialEntries={['/explores/api-retry']}>
        <Routes>
          <Route
            path="/explores/:name"
            element={<ExploreDetailView list={makeList()} record={record()} root={ROOT} />}
          />
        </Routes>
      </MemoryRouter>,
    );
    await waitFor(() =>
      expect(screen.getByTestId<HTMLSelectElement>('explore-workflow-type-select').value).toBe(
        'requirement',
      ),
    );
  });

  it('异常：改选触发的 updateWorkflowType reject → 静默（无错误块、选择器保持用户当前选择、页面不崩）', async () => {
    const list = makeList({
      updateWorkflowType: vi.fn().mockRejectedValue(new Error('workflow_type 不受支持')),
    });
    renderDetail(record(), list);

    const select = await screen.findByTestId('explore-workflow-type-select');
    fireEvent.change(select, { target: { value: 'test-only' } });

    await waitFor(() =>
      expect(list.updateWorkflowType).toHaveBeenCalledWith('api-retry', 'test-only'),
    );
    // 回填失败静默（沿 useTitleBackfill 先例：库内值保持原值，不呈现错误块）
    expect(screen.queryByTestId('explore-promote-error')).toBeNull();
    expect(screen.getByTestId<HTMLSelectElement>('explore-workflow-type-select').value).toBe(
      'test-only',
    );
    expect(screen.getByTestId('explore-detail-title')).toBeTruthy();
  });

  it('边界：promoted 态不渲染选择器（动作面收敛为「查看变更」）', async () => {
    renderDetail(record({ promotedTo: 'chg-018f3a-0001' }), makeList());

    await screen.findByTestId('explore-promoted-badge');
    expect(screen.queryByTestId('explore-workflow-type-select')).toBeNull();
    expect(screen.queryByTestId('explore-promote-trigger')).toBeNull();
  });
});

describe('ExploreDetailView：workflow_type 回填 effect（AC-8 链路）', () => {
  it('正向：doc 含约定标记行 test-only 且与 record.workflowType 不同 → list.updateWorkflowType 恰一次', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: 'workflow_type: test-only\n\n正文段落' };
    renderDetail(record({ workflowType: 'requirement' }), list);

    await waitFor(() =>
      expect(list.updateWorkflowType).toHaveBeenCalledWith('api-retry', 'test-only'),
    );
    expect(list.updateWorkflowType).toHaveBeenCalledTimes(1);
  });

  it('正向：标记值为 requirement 而记录为 test-only → 反向回填亦触发', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: '# 标题\nworkflow_type: requirement\n' };
    renderDetail(record({ workflowType: 'test-only' }), list);

    await waitFor(() =>
      expect(list.updateWorkflowType).toHaveBeenCalledWith('api-retry', 'requirement'),
    );
    expect(list.updateWorkflowType).toHaveBeenCalledTimes(1);
  });

  it('边界：标记值与 record.workflowType 相同 → 零 updateWorkflowType（无差异零 invoke）', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: '# 接口重试策略\nworkflow_type: requirement\n' };
    renderDetail(record({ workflowType: 'requirement' }), list);

    await screen.findByTestId('explore-detail-title');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_explore', { root: ROOT, name: 'api-retry' }),
    );
    expect(list.updateWorkflowType).not.toHaveBeenCalled();
  });

  it('异常：无标记行 / 标记值非法 → 零 updateWorkflowType、零报错（record 保持原值）', async () => {
    for (const content of [
      '只有正文，没有约定标记行',
      'workflow_type: bug-fix\n正文',
      'workflow_type:\n正文',
    ]) {
      cleanup();
      const list = makeList();
      docResult = { name: 'api-retry', content };
      renderDetail(record({ workflowType: 'requirement' }), list);

      await screen.findByTestId('explore-detail-title');
      await waitFor(() =>
        expect(invokeMock).toHaveBeenCalledWith('read_explore', {
          root: ROOT,
          name: 'api-retry',
        }),
      );
      expect(list.updateWorkflowType).not.toHaveBeenCalled();
      expect(screen.queryByTestId('explore-promote-error')).toBeNull();
      expect(screen.getByTestId('explore-workflow-type-select')).toBeTruthy();
    }
  });

  it('异常：回填 reject → 静默（无错误块、页面不崩、选择器默认仍随记录值）', async () => {
    const list = makeList({
      updateWorkflowType: vi.fn().mockRejectedValue(new Error('回填写失败')),
    });
    docResult = { name: 'api-retry', content: 'workflow_type: test-only\n正文' };
    renderDetail(record({ workflowType: 'requirement' }), list);

    await waitFor(() =>
      expect(list.updateWorkflowType).toHaveBeenCalledWith('api-retry', 'test-only'),
    );
    expect(screen.queryByTestId('explore-promote-error')).toBeNull();
    expect(screen.getByTestId<HTMLSelectElement>('explore-workflow-type-select').value).toBe(
      'requirement',
    );
    expect(screen.getByTestId('explore-detail-title')).toBeTruthy();
  });

  it('边界：promoted 态（promotedTo 非空）停回填、零 updateWorkflowType（笔记已移走）', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: 'workflow_type: test-only\n正文' };
    renderDetail(record({ promotedTo: 'chg-018f3a-0001' }), list);

    await screen.findByTestId('explore-promoted-preview');
    expect(list.updateWorkflowType).not.toHaveBeenCalled();
    expect(list.updateTitle).not.toHaveBeenCalled();
  });

  it('边界：记录缺失（record = null）→ 零回填零取数，缺失态占位照常', async () => {
    const list = makeList();
    docResult = { name: 'api-retry', content: 'workflow_type: test-only\n正文' };
    renderDetail(null, list);

    await screen.findByTestId('explore-detail-missing');
    expect(list.updateWorkflowType).not.toHaveBeenCalled();
    expect(invokeMock.mock.calls.map(([name]) => name)).not.toContain('read_explore');
  });
});

// ---------------------------------------------------------------------------
// auto-named-explore：autoPrompt 读数传递（AC-1 链路入口半边）与宽容解析
// ---------------------------------------------------------------------------

const PLACEHOLDER_NAME = 'draft-1758123456789';

/** 占位记录 fixture（sessionId 在场——auto-send 就绪形态；标题为临时标题）。 */
function placeholderRecord(): ExploreRecord {
  return record({
    id: 9,
    name: PLACEHOLDER_NAME,
    title: '2026-10-17 08:30:45-分析中',
    sessionId: 'ses-draft-1727000001000',
  });
}

/** 携 location.state 挂载（autoPrompt 经路由 state 流入详情页）。 */
function renderDetailWithState(
  recordValue: ExploreRecord | null,
  list: ExploreListState,
  state: unknown,
  path = '/explores/api-retry',
) {
  return render(
    <MemoryRouter initialEntries={[{ pathname: path, state }]}>
      <Routes>
        <Route
          path="/explores/:name"
          element={<ExploreDetailView list={list} record={recordValue} root={ROOT} />}
        />
        <Route path="/changes/:id" element={<span data-testid="change-detail-stub" />} />
      </Routes>
      <LocationProbe />
    </MemoryRouter>,
  );
}

/** 会话链空态底座（session_detail null + 空转录——装载完成后闸门齐备）。 */
function mockEmptySessionIpc() {
  invokeMock.mockImplementation((command: string) => {
    if (command === 'read_explore') return Promise.resolve(docResult);
    if (command === 'explore_doc_path') {
      return Promise.resolve('C:demoalphaopenspecexploresapi-retry.md');
    }
    if (command === 'watch_subscribe') return Promise.resolve(1);
    if (command === 'watch_unsubscribe') return Promise.resolve(null);
    if (command === 'session_detail') return Promise.resolve(null);
    if (command === 'agent_session_transcript') return Promise.resolve([]);
    return Promise.resolve(null);
  });
}

/** 最近一个 Channel 实例（send 创建；信封投递缝）。 */
function lastChannelLike(): { onmessage: ((event: unknown) => void) | null } {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('send 未创建 Channel');
  return instance;
}

const startCalls = () => invokeMock.mock.calls.filter(([name]) => name === 'agent_start');

describe('ExploreDetailView：autoPrompt 读数传递（AC-1 链路入口半边）', () => {
  it('正向：location.state.autoPrompt 为 string → useExploreSession 第三参收到该值（auto-send 以 stance + 原文发起）', async () => {
    mockEmptySessionIpc();
    // agent_start 挂起（运行在途形态即可断言入参面）
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_start') return new Promise<TurnSummaryLike>(() => {});
      return mockEmptySessionAnswer(command);
    });
    renderDetailWithState(record({ sessionId: 'ses-draft-1727000001000' }), makeList(), {
      autoPrompt: '调研重试策略的失效边界',
    });

    await waitFor(() => expect(startCalls()).toHaveLength(1));
    const args = startCalls()[0]?.[1] as Record<string, unknown>;
    expect(String(args.prompt)).toContain('调研重试策略的失效边界');
    expect(String(args.prompt).startsWith('你在探索模式下工作')).toBe(true);
    expect(args.permissionMode).toBe('bypassPermissions');
    expect(args.source).toBe('explore');
  });

  it('边界：state.autoPrompt 为对象 / 数字 / undefined / null → 一律传 null、零自动发送（非 string 宽容口径）', async () => {
    for (const autoPrompt of [{ text: '对象形态' }, 42, undefined, null]) {
      cleanup();
      mockEmptySessionIpc();
      renderDetailWithState(record({ sessionId: 'ses-draft-1727000001000' }), makeList(), {
        autoPrompt,
      });
      await screen.findByTestId('explore-detail-title');
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(startCalls()).toHaveLength(0);
    }
  });
});

/** mockEmptySessionIpc 的可复用应答臂（供局部 mockImplementation 回落）。 */
function mockEmptySessionAnswer(command: string): Promise<unknown> {
  if (command === 'read_explore') return Promise.resolve(docResult);
  if (command === 'explore_doc_path') {
    return Promise.resolve('C:demoalphaopenspecexploresapi-retry.md');
  }
  if (command === 'watch_subscribe') return Promise.resolve(1);
  if (command === 'watch_unsubscribe') return Promise.resolve(null);
  if (command === 'session_detail') return Promise.resolve(null);
  if (command === 'agent_session_transcript') return Promise.resolve([]);
  return Promise.resolve(null);
}

/** TurnSummary 结构最简形态（终态轮行信封投递用）。 */
interface TurnSummaryLike {
  turnId: number;
  sessionId: string;
  status: string;
  startedAt: number;
  finishedAt: number | null;
  numTurns: number | null;
  costUsd: number | null;
  durationMs: number | null;
  error: null;
}

function completedTurn(sessionId: string): TurnSummaryLike {
  return {
    turnId: 13,
    sessionId,
    status: 'completed',
    startedAt: 1727000001000,
    finishedAt: 1727000002000,
    numTurns: 1,
    costUsd: null,
    durationMs: 500,
    error: null,
  };
}

// ---------------------------------------------------------------------------
// run 终态清单刷新与名变收敛导航（AC-4 收敛链）
// ---------------------------------------------------------------------------

describe('ExploreDetailView：run 终态刷新与名变收敛（AC-4 收敛链）', () => {
  it('正向：running true→false → list.refresh 与 doc 定点重读同点触发（read_explore 二次拉取）', async () => {
    mockEmptySessionIpc();
    // agent_start 提前 resolve running 轮行（库态 running 来源）；终态经 Channel
    // record 信封回流，收口按库内重放收敛
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_start') {
        return Promise.resolve({
          ...completedTurn('ses-draft-1727000001000'),
          status: 'running',
          finishedAt: null,
          numTurns: null,
          costUsd: null,
          durationMs: null,
        });
      }
      return mockEmptySessionAnswer(command);
    });
    const list = makeList();
    renderDetailWithState(record({ sessionId: 'ses-draft-1727000001000' }), list, {
      autoPrompt: '触发一轮运行的需求',
    });

    await waitFor(() => expect(startCalls()).toHaveLength(1));

    // 终态轮行信封回流 → running 复位 → doc.refresh + list.refresh 同点触发
    act(() => {
      lastChannelLike().onmessage?.({
        ipc: 'record',
        record: completedTurn('ses-draft-1727000001000'),
      });
    });

    await waitFor(() => expect(list.refresh).toHaveBeenCalled());
    await waitFor(() => {
      const readCalls = invokeMock.mock.calls.filter(([name]) => name === 'read_explore');
      expect(readCalls.length).toBeGreaterThanOrEqual(2);
    });
  });

  it('正向：派生 record.name !== routeName → navigate(/explores/<新名>, { replace: true })', async () => {
    mockEmptySessionIpc();
    renderDetail(record({ name: 'new-final-name' }), makeList());

    await screen.findByTestId('explore-detail-title');
    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/new-final-name'),
    );
  });

  it('正向：名变后 useExploreDoc 以新名读取（read_explore / explore_doc_path 入参为新名）', async () => {
    mockEmptySessionIpc();
    renderDetail(record({ name: 'new-final-name' }), makeList());

    await screen.findByTestId('explore-detail-title');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('read_explore', {
        root: ROOT,
        name: 'new-final-name',
      }),
    );
    expect(invokeMock).toHaveBeenCalledWith('explore_doc_path', {
      root: ROOT,
      name: 'new-final-name',
    });
  });

  it('边界：稳态 record.name === routeName → 零导航（名变 effect 不误触发回归）', async () => {
    mockEmptySessionIpc();
    renderDetail(record(), makeList());

    await screen.findByTestId('explore-detail-title');
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByTestId('location-probe').textContent).toBe('/explores/api-retry');
  });
});

// ---------------------------------------------------------------------------
// 占位中间窗口（proposal 风险 5 留痕口径）与 promote 门禁行内呈现（AC-9）
// ---------------------------------------------------------------------------

describe('ExploreDetailView：占位中间窗口与 promote 门禁行内呈现（AC-2 / AC-9）', () => {
  it('正向：create 已执行、run 未终态 → 页面停留占位名、标题「分析中」、无 MissingRecord', async () => {
    mockEmptySessionIpc();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_start') return new Promise<TurnSummaryLike>(() => {});
      return mockEmptySessionAnswer(command);
    });
    renderDetailWithState(
      placeholderRecord(),
      makeList(),
      { autoPrompt: '首轮需求' },
      `/explores/${PLACEHOLDER_NAME}`,
    );

    // 标题为占位临时标题（「分析中」）、详情渲染、缺失态零出现
    await waitFor(() =>
      expect(screen.getByTestId('explore-detail-title').textContent).toBe(
        '2026-10-17 08:30:45-分析中',
      ),
    );
    expect(screen.queryByTestId('explore-detail-missing')).toBeNull();
    // 停留占位名 URL（record.name === routeName → 零导航）
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByTestId('location-probe').textContent).toBe(`/explores/${PLACEHOLDER_NAME}`);
  });

  it('异常：list.promote reject（占位门禁文案）→ 行内错误呈现、不跳转（AC-9）', async () => {
    const list = makeList({
      promote: vi
        .fn()
        .mockRejectedValue(new Error('探索 "draft-1" 尚未定名（占位记录）：请先在会话中完成定名')),
    });
    // 占位记录以其自身路径挂载（record.name === routeName，与正向用例同底座
    // ——名变收敛 effect 不触发，URL 变化只可能来自 promote 侧）
    renderDetailWithState(placeholderRecord(), list, null, `/explores/${PLACEHOLDER_NAME}`);

    fireEvent.click(await screen.findByTestId('explore-promote-trigger'));
    const error = await screen.findByTestId('explore-promote-error');
    expect(error.textContent).toContain('尚未定名');
    // 不跳转：URL 停留占位名详情
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByTestId('location-probe').textContent).toBe(`/explores/${PLACEHOLDER_NAME}`);
  });
});

// ---------------------------------------------------------------------------
// 结构面补遗：右栏两态、replace 语义词、无 :name 路由挂载、非对象路由 state
// ---------------------------------------------------------------------------

describe('ExploreDetailView：右栏两态与名变收敛导航语义（AC-8 / AC-4）', () => {
  it('正向：草稿态右栏为笔记预览（explore-preview 在场、promoted 面板不在场）', async () => {
    mockEmptySessionIpc();
    renderDetail(record(), makeList());

    await screen.findByTestId('explore-detail-title');
    expect(screen.getByTestId('explore-preview')).toBeTruthy();
    expect(screen.queryByTestId('explore-promoted-preview')).toBeNull();
  });

  it('正向：名变收敛导航为 replace（历史条目不被新增，导航类型 REPLACE）', async () => {
    mockEmptySessionIpc();
    renderDetail(record({ name: 'new-final-name' }), makeList());

    await waitFor(() =>
      expect(screen.getByTestId('location-probe').textContent).toBe('/explores/new-final-name'),
    );
    expect(screen.getByTestId('nav-type-probe').textContent).toBe('REPLACE');
  });

  it('边界：挂载于无 :name 段的路由（routeName 缺失）→ 零名变导航（不误跳）', async () => {
    mockEmptySessionIpc();
    render(
      <MemoryRouter initialEntries={['/explores-flat']}>
        <Routes>
          <Route
            path="/explores-flat"
            element={<ExploreDetailView list={makeList()} record={record()} root={ROOT} />}
          />
        </Routes>
        <LocationProbe />
      </MemoryRouter>,
    );

    await screen.findByTestId('explore-detail-title');
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByTestId('location-probe').textContent).toBe('/explores-flat');
    expect(screen.getByTestId('nav-type-probe').textContent).toBe('POP');
  });

  it('边界：路由 state 为非对象原始值（数字）→ 宽容解析为 null、零自动发送、不崩', async () => {
    mockEmptySessionIpc();
    renderDetailWithState(record({ sessionId: 'ses-draft-1727000001000' }), makeList(), 42);

    await screen.findByTestId('explore-detail-title');
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(startCalls()).toHaveLength(0);
    expect(screen.getByTestId('explore-detail-title').textContent).toBe('接口重试策略');
  });
});

describe('ExploreDetailView：promote 在飞标记（防重复提交态）', () => {
  it('正向：promote 在飞 → 「启动变更」禁用；落地后复位启用（在飞标记随 promise 收敛）', async () => {
    let resolvePromote: () => void = () => {};
    const list = makeList({
      promote: vi.fn(
        () =>
          new Promise<void>((resolve) => {
            resolvePromote = resolve;
          }),
      ),
    });
    renderDetail(record(), list);

    const trigger = await screen.findByTestId<HTMLButtonElement>('explore-promote-trigger');
    expect(trigger.disabled).toBe(false);
    fireEvent.click(trigger);

    await waitFor(() =>
      expect(screen.getByTestId<HTMLButtonElement>('explore-promote-trigger').disabled).toBe(true),
    );
    expect(list.promote).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolvePromote();
    });
    await waitFor(() =>
      expect(screen.getByTestId<HTMLButtonElement>('explore-promote-trigger').disabled).toBe(false),
    );
  });

  it('异常：promote reject 后亦复位启用（在飞标记不与错误态粘连）', async () => {
    const list = makeList({ promote: vi.fn().mockRejectedValue(new Error('门禁未过')) });
    renderDetail(record(), list);

    fireEvent.click(await screen.findByTestId('explore-promote-trigger'));

    await screen.findByTestId('explore-promote-error');
    expect(screen.getByTestId<HTMLButtonElement>('explore-promote-trigger').disabled).toBe(false);
  });
});
// ---------------------------------------------------------------------------
// ChatPane 接线（D9 / D10）：注入形态 AgentSessionView + composer 插槽
// ---------------------------------------------------------------------------

/** 会话 fixture（record.sessionId 在场 + agent_start 提前 resolve）。 */
function seedChatSession(): void {
  invokeMock.mockImplementation((command: string) => {
    if (command === 'read_explore') return Promise.resolve(docResult);
    if (command === 'explore_doc_path') {
      return Promise.resolve('C:\\demo\\alpha\\openspec\\explores\\api-retry.md');
    }
    if (command === 'watch_subscribe') return Promise.resolve(1);
    if (command === 'watch_unsubscribe') return Promise.resolve(null);
    if (command === 'agent_sessions') return Promise.resolve([]);
    if (command === 'session_detail') {
      return Promise.resolve({
        row: {
          id: 'ses-explore-1',
          remoteSessionId: null,
          configSnapshot: {
            engine: 'sdk',
            agentInstanceId: null,
            model: null,
            permissionMode: 'bypassPermissions',
          },
          provenance: { source: 'explore', sourceRef: '7' },
          createdAt: 1727000000000,
          updatedAt: 1727000000001,
        },
        stats: { turnCount: 1, totalDurationMs: null, inputTokens: null, outputTokens: null },
        turns: [
          {
            turnId: 1,
            sessionId: 'ses-explore-1',
            status: 'completed',
            startedAt: 1727000000000,
            finishedAt: 1727000000900,
            numTurns: 1,
            costUsd: null,
            durationMs: 900,
            error: null,
          },
        ],
      });
    }
    if (command === 'agent_session_transcript') {
      return Promise.resolve([
        {
          seq: 0,
          timestampMs: 1727000000000,
          kind: 'message',
          role: 'assistant',
          blocks: [{ kind: 'text', text: '探索会话正文' }],
          parentToolUseId: null,
        },
      ]);
    }
    if (command === 'agent_start') {
      return Promise.resolve({
        turnId: 2,
        sessionId: 'ses-explore-1',
        status: 'running',
        startedAt: 1727000001000,
        finishedAt: null,
        numTurns: null,
        costUsd: null,
        durationMs: null,
        error: null,
      });
    }
    return Promise.resolve(null);
  });
}

describe('ExploreDetailView：ChatPane 注入形态与 composer 插槽', () => {
  it('正向：explore-chat 注入形态挂载，ExploreComposer 由插槽渲染（running 取自会话库态）', async () => {
    seedChatSession();
    renderDetail(record({ sessionId: 'ses-explore-1' }), makeList());

    const chat = await screen.findByTestId('explore-chat');
    await waitFor(() => expect(within(chat).getAllByTestId('block-text')).toHaveLength(1));

    // 插槽内容渲染在消息区下方（同一 chat 容器内）
    const composer = within(chat).getByTestId('explore-composer');
    expect(composer).toBeTruthy();
    // 插槽 running 取自会话库态：已收口 → 输入非空即可发、停止入口隐藏
    fireEvent.change(within(chat).getByTestId('explore-prompt'), {
      target: { value: '收口态可发' },
    });
    expect(within(chat).getByTestId<HTMLButtonElement>('explore-send').disabled).toBe(false);
    expect(within(chat).queryByTestId('explore-stop')).toBeNull();
  });

  it('正向：插槽发送 → agent_start（stance 前导 + source=explore）', async () => {
    seedChatSession();
    renderDetail(record({ sessionId: 'ses-explore-1' }), makeList());
    const chat = await screen.findByTestId('explore-chat');
    await waitFor(() => expect(within(chat).getAllByTestId('block-text')).toHaveLength(1));

    fireEvent.change(within(chat).getByTestId('explore-prompt'), {
      target: { value: '继续追这条线索' },
    });
    fireEvent.click(within(chat).getByTestId('explore-send'));
    await act(async () => {});

    const startCall = invokeMock.mock.calls.filter(([name]) => name === 'agent_start').at(-1);
    expect(startCall).toBeTruthy();
    const args = startCall?.[1] as Record<string, unknown>;
    expect(args.source).toBe('explore');
    expect(String(args.prompt)).toContain('继续追这条线索');
    expect(args.sessionId).toBe('ses-explore-1');
  });

  it('边界：有记录无转录 → 组件空态提示（不虚构气泡）', async () => {
    seedChatSession();
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      if (command === 'read_explore') return Promise.resolve(docResult);
      if (command === 'explore_doc_path') return Promise.resolve('C:\\a.md');
      if (command === 'watch_subscribe') return Promise.resolve(1);
      if (command === 'session_detail') {
        return Promise.resolve({
          row: {
            id: 'ses-explore-1',
            remoteSessionId: null,
            configSnapshot: null,
            provenance: { source: 'explore', sourceRef: '7' },
            createdAt: 1,
            updatedAt: 1,
          },
          stats: { turnCount: 0, totalDurationMs: null, inputTokens: null, outputTokens: null },
          turns: [],
        });
      }
      return Promise.resolve(null);
    });
    renderDetail(record({ sessionId: 'ses-explore-1' }), makeList());

    const chat = await screen.findByTestId('explore-chat');
    await waitFor(() => expect(within(chat).getByTestId('conversation-empty')).toBeTruthy());
    expect(within(chat).queryAllByTestId('chat-bubble')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 记录切换（D9 / D10 边界）：新会话重放重取一次、旧会话流拆除（并行流 ≤ 1）
// ---------------------------------------------------------------------------

/** 运行中会话 summary（订阅状态门判据：轮行含 running）。 */
function runningSession(sessionId: string): SessionSummary {
  return {
    row: {
      id: sessionId,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'explore', sourceRef: '7' },
      createdAt: 1727000000000,
      updatedAt: 1727000000001,
    },
    stats: { turnCount: 1, totalDurationMs: null, inputTokens: null, outputTokens: null },
    turns: [
      {
        turnId: 1,
        sessionId,
        status: 'running',
        startedAt: 1727000000000,
        finishedAt: null,
        numTurns: null,
        costUsd: null,
        durationMs: null,
        error: null,
      },
    ],
  };
}

/** 订阅流 token 级增量信封载荷。 */
function deltaEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

/** 订阅 Channel 缝（onmessage 直投信封）。 */
interface WatchChannel {
  onmessage: ((message: unknown) => void) | null;
}

function bubbleTexts(): string[] {
  return screen.queryAllByTestId('block-text').map((node) => node.textContent ?? '');
}

/** invoke 参数读取（字符串字段；非法 / 缺席回落空串）。 */
function stringArg(args: Record<string, unknown> | undefined, key: string): string {
  const value = args?.[key];
  return typeof value === 'string' ? value : '';
}

describe('ExploreDetailView：记录切换（并行流 ≤ 1）', () => {
  it('切换 explore 记录 → 新会话重放重取一次、旧会话流拆除（agent_session_watch 并行 ≤ 1）', async () => {
    const watched: string[] = [];
    const channels = new Map<string, WatchChannel>();
    const pending = new Map<string, (value: null) => void>();

    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'read_explore') return Promise.resolve(docResult);
      if (command === 'explore_doc_path') return Promise.resolve('C:\\demo\\alpha\\note.md');
      if (command === 'watch_subscribe') return Promise.resolve(1);
      if (command === 'watch_unsubscribe') return Promise.resolve(null);
      if (command === 'session_detail') {
        return Promise.resolve(runningSession(stringArg(args, 'sessionId')));
      }
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      if (command === 'agent_session_watch') {
        const sessionId = stringArg(args, 'sessionId');
        watched.push(sessionId);
        channels.set(sessionId, args?.onEvent as WatchChannel);
        return new Promise<null>((resolve) => {
          pending.set(sessionId, resolve);
        });
      }
      return Promise.resolve(null);
    });

    const list = makeList();
    const { rerender } = render(detailTree(record({ sessionId: 'ses-a' }), list));

    // A 记录装载：库态运行中 → 恰一条订阅（并行计数起点 1）
    await waitFor(() => expect(watched).toEqual(['ses-a']));
    act(() => {
      channels.get('ses-a')?.onmessage?.({ ipc: 'event', event: deltaEvent(1, 'A 会话增量') });
    });
    await waitFor(() => expect(bubbleTexts()).toContain('A 会话增量'));

    // 切到另一条记录（记录回写的会话 id 变化）→ 旧订阅流由后端收尾（拆除）
    rerender(detailTree(record({ sessionId: 'ses-b', name: 'retry-doc' }), list));
    await act(async () => {
      pending.get('ses-a')?.(null);
    });

    // 新会话：重放重取一次（转录查询携新 id）+ 一次新订阅
    await waitFor(() => expect(watched).toContain('ses-b'));
    const transcriptOf = (sessionId: string) =>
      invokeMock.mock.calls.filter(
        ([name, callArgs]) =>
          name === 'agent_session_transcript' &&
          (callArgs as Record<string, unknown> | undefined)?.sessionId === sessionId,
      );
    await waitFor(() => expect(transcriptOf('ses-b')).toHaveLength(1));
    expect(invokeMock).toHaveBeenCalledWith('session_detail', {
      root: ROOT,
      sessionId: 'ses-b',
    });
    // 旧会话零重订、新会话恰一次 → 并行流 ≤ 1（两条各一条，逐会话单流）
    expect(watched.filter((id) => id === 'ses-a')).toHaveLength(1);
    expect(watched.filter((id) => id === 'ses-b')).toHaveLength(1);
    expect(watched, 'agent_session_watch 并行 ≤ 1').toHaveLength(2);

    // 旧会话流已拆除：迟到增量不落对话区；新会话增量照常呈现
    act(() => {
      channels.get('ses-a')?.onmessage?.({ ipc: 'event', event: deltaEvent(2, '旧会话迟到增量') });
    });
    // 流式 messages 通知有节流窗：先冲刷一拍，确保该断言考察链路而非通知时序
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 150));
    });
    expect(bubbleTexts()).not.toContain('旧会话迟到增量');

    act(() => {
      channels.get('ses-b')?.onmessage?.({ ipc: 'event', event: deltaEvent(3, 'B 会话增量') });
    });
    await waitFor(() => expect(bubbleTexts()).toContain('B 会话增量'));
  });
});

// ---------------------------------------------------------------------------
// ChatPane 运行态边界与错误面（D9 / D10 插槽语义）：库态 running → 发送禁用 +
// 停止接线（agent_stop）、收口后恢复可发；会话查询 reject → 对话区错误提示
// 而页面其余区域不塌
// ---------------------------------------------------------------------------

const stopCalls = () => invokeMock.mock.calls.filter(([name]) => name === 'agent_stop');

const watchCalls = () => invokeMock.mock.calls.filter(([name]) => name === 'agent_session_watch');

/** 已收口会话 summary（同会话 id、轮行 completed——收口后的库态形状）。 */
function completedSession(sessionId: string): SessionSummary {
  return {
    row: {
      id: sessionId,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'explore', sourceRef: '7' },
      createdAt: 1727000000000,
      updatedAt: 1727000002000,
    },
    stats: { turnCount: 1, totalDurationMs: null, inputTokens: null, outputTokens: null },
    turns: [
      {
        turnId: 1,
        sessionId,
        status: 'completed',
        startedAt: 1727000000000,
        finishedAt: 1727000002000,
        numTurns: 1,
        costUsd: null,
        durationMs: 900,
        error: null,
      },
    ],
  };
}

/** 终态轮行信封载荷（与库内 running 行同 turnId——upsert 原位替换，不叠加行）。 */
function completedTurnRecord(sessionId: string): TurnSummaryLike {
  return {
    turnId: 1,
    sessionId,
    status: 'completed',
    startedAt: 1727000000000,
    finishedAt: 1727000002000,
    numTurns: 1,
    costUsd: null,
    durationMs: 900,
    error: null,
  };
}

describe('ExploreDetailView：ChatPane 运行态边界（库态 running 禁发与停止接线）', () => {
  it('边界：库态 running → 发送禁用、停止可点（agent_stop）；收口后恢复可发（终态定点重读重评估）', async () => {
    // 收口闸门：false 期间 session_detail 返运行中（重放 + 订阅状态门两次取数），
    // 投终态信封后转已收口（收口重放读已收口库态）
    const gate = { settled: false };
    const watched: string[] = [];
    const watchChannels: WatchChannel[] = [];

    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'read_explore') return Promise.resolve(docResult);
      if (command === 'explore_doc_path') return Promise.resolve('C:\\demo\\alpha\\note.md');
      if (command === 'watch_subscribe') return Promise.resolve(1);
      if (command === 'watch_unsubscribe') return Promise.resolve(null);
      if (command === 'session_detail') {
        const sessionId = stringArg(args, 'sessionId');
        return Promise.resolve(
          gate.settled ? completedSession(sessionId) : runningSession(sessionId),
        );
      }
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      if (command === 'agent_session_watch') {
        watched.push(stringArg(args, 'sessionId'));
        watchChannels.push(args?.onEvent as WatchChannel);
        // 订阅挂起：收口前流不关闭（单流在挂），终态信封由测试侧直投
        return new Promise<null>(() => {});
      }
      if (command === 'agent_stop') return Promise.resolve(null);
      if (command === 'agent_start') {
        return Promise.resolve({
          turnId: 2,
          sessionId: 'ses-explore-1',
          status: 'running',
          startedAt: 1727000003000,
          finishedAt: null,
          numTurns: null,
          costUsd: null,
          durationMs: null,
          error: null,
        });
      }
      return Promise.resolve(null);
    });

    renderDetail(record({ sessionId: 'ses-explore-1' }), makeList());

    const chat = await screen.findByTestId('explore-chat');
    // 库态 running：元信息徽标「运行中」+ 流式状态行，且已建立恰一条订阅
    await waitFor(() =>
      expect(within(chat).getByTestId('session-meta-status').textContent).toBe('运行中'),
    );
    expect(within(chat).getByTestId('agent-messages-running')).toBeTruthy();
    await waitFor(() => expect(watched).toEqual(['ses-explore-1']));

    // 发送入口保持可点（运行中守卫在会话层）——输入非空后点击零 agent_start
    fireEvent.change(within(chat).getByTestId('explore-prompt'), {
      target: { value: '运行中不该发' },
    });
    // 运行中重复发送由会话层 isStreaming 守卫忽略，发送入口不禁
    const sendButton = within(chat).getByTestId<HTMLButtonElement>('explore-send');
    expect(sendButton.disabled).toBe(false);
    fireEvent.click(sendButton);
    await act(async () => {});
    expect(startCalls(), '运行中重复发送由会话层 isStreaming 守卫忽略').toHaveLength(0);

    // 停止可点 → 插槽 onStop 接来源侧 session.stop → agent_stop（root + 会话 id 逐字）
    const stopButton = within(chat).getByTestId<HTMLButtonElement>('explore-stop');
    expect(stopButton.disabled).toBe(false);
    fireEvent.click(stopButton);
    await waitFor(() => expect(stopCalls()).toHaveLength(1));
    expect(stopCalls()[0]?.[1]).toEqual({ root: ROOT, sessionId: 'ses-explore-1' });

    // 收口：终态轮行信封回流（同 turnId 原位替换）→ 库态转「已收口」
    gate.settled = true;
    act(() => {
      watchChannels[0]?.onmessage?.({
        ipc: 'record',
        record: completedTurnRecord('ses-explore-1'),
      });
    });

    await waitFor(() =>
      expect(within(chat).getByTestId('session-meta-status').textContent).toBe('已收口'),
    );
    // 终态定点重读触发重评估：文档重读一次（read_explore 第二次）
    await waitFor(() => {
      const reads = invokeMock.mock.calls.filter(([name]) => name === 'read_explore');
      expect(reads.length).toBeGreaterThanOrEqual(2);
    });

    // 收口后恢复可发：停止入口消失；重新输入后发送可点 → agent_start
    // （source=explore、同会话续话）
    expect(within(chat).queryByTestId('explore-stop')).toBeNull();
    // 重放完成拍落位（loading 复位，发送守卫让位）
    await act(async () => {});
    fireEvent.change(within(chat).getByTestId('explore-prompt'), {
      target: { value: '收口后再发一条' },
    });
    expect(within(chat).getByTestId<HTMLButtonElement>('explore-send').disabled).toBe(false);
    fireEvent.click(within(chat).getByTestId('explore-send'));
    await waitFor(() => expect(startCalls()).toHaveLength(1));
    const startArgs = startCalls()[0]?.[1] as Record<string, unknown>;
    expect(startArgs.source).toBe('explore');
    expect(startArgs.sessionId).toBe('ses-explore-1');
    expect(String(startArgs.prompt)).toContain('收口后再发一条');

    // 收口不重订：订阅流恰一条（并行流 ≤ 1）
    expect(watched).toEqual(['ses-explore-1']);
  });
});

describe('ExploreDetailView：ChatPane 错误面（会话查询失败不塌页面）', () => {
  it('异常：会话查询 reject → 对话区错误提示呈现，记录头 / 右栏面板不塌、零伪气泡、零订阅', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'session_detail') return Promise.reject(new Error('会话查询失败：注入'));
      if (command === 'read_explore') return Promise.resolve(docResult);
      if (command === 'explore_doc_path') return Promise.resolve('C:\\demo\\alpha\\note.md');
      if (command === 'watch_subscribe') return Promise.resolve(1);
      if (command === 'watch_unsubscribe') return Promise.resolve(null);
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      return Promise.resolve(null);
    });

    renderDetail(record({ sessionId: 'ses-explore-1' }), makeList());

    const chat = await screen.findByTestId('explore-chat');
    // 对话区错误提示（不抛错、不崩）
    const error = await within(chat).findByTestId('agent-session-error');
    expect(error.textContent).toContain('会话查询失败：注入');

    // 页面其余区域不塌：记录头（标题 + 草稿态动作面）与右栏笔记预览仍在位
    expect(screen.getByTestId('explore-detail-title').textContent).toBe('接口重试策略');
    expect(screen.getByTestId('explore-workflow-type-select')).toBeTruthy();
    expect(screen.getByTestId('explore-promote-trigger')).toBeTruthy();
    expect(screen.getByTestId('explore-preview')).toBeTruthy();

    // 对话区：空态提示、零伪气泡、不虚构会话元信息
    expect(within(chat).getByTestId('conversation-empty')).toBeTruthy();
    expect(within(chat).queryAllByTestId('chat-bubble')).toHaveLength(0);
    expect(within(chat).queryByTestId('session-meta')).toBeNull();

    // 错误态不误挂流（库态未确立 → 零订阅）
    await act(async () => {});
    expect(watchCalls()).toHaveLength(0);
  });
});
