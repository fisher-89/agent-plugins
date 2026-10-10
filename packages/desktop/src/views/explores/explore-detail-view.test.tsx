import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ExploreDoc, ExploreRecord } from '../../types/dto';
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
    promotedTo: null,
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

/** URL 探针：把当前 pathname 投影到 DOM 供 navigate 断言。 */
function LocationProbe() {
  const { pathname } = useLocation();
  return <span data-testid="location-probe">{pathname}</span>;
}

function renderDetail(recordValue: ExploreRecord | null, list: ExploreListState) {
  return render(
    <MemoryRouter initialEntries={['/explores/api-retry']}>
      <Routes>
        <Route
          path="/explores/:name"
          element={<ExploreDetailView list={list} record={recordValue} root={ROOT} />}
        />
        <Route path="/changes/:id" element={<LocationProbe />} />
      </Routes>
    </MemoryRouter>,
  );
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

  it('正向：草稿态（promotedTo 为 null）→ 「启动变更」入口；点击调 list.promote 恰一次', async () => {
    const list = makeList();
    renderDetail(record(), list);

    const trigger = await screen.findByTestId('explore-promote-trigger');
    expect(screen.queryByTestId('explore-promoted-badge')).toBeNull();
    fireEvent.click(trigger);

    await waitFor(() => expect(list.promote).toHaveBeenCalledTimes(1));
    expect(list.promote).toHaveBeenCalledWith('api-retry');
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
