import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ExploreRecord } from '../../types/dto';
import { ExploreView } from './explore-view';

// 进程边界 Mock：仅 mock IPC（@tauri-apps/api/core 的 invoke / Channel），被测
// 页面 import 的内部 hooks（useExploreList / useExploreDoc / useExploreSession）
// 真实组合，fixture 经 mock IPC 流入后断言渲染输出（最小 mock 纪律）。
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

/** 清单 fixture（title ≠ name：展示面必须取 title）。 */
function record(id: number, name: string, title: string): ExploreRecord {
  return {
    id,
    root: ROOT,
    name,
    title,
    promotedTo: null,
    createdAt: 1727000000000 + id,
    updatedAt: 1727000000000 + id,
  };
}

let records: ExploreRecord[] = [];

/** 进程边界应答面：清单 + 详情页 hooks 所需命令的空态应答。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'list_explore_records') return Promise.resolve(records);
  if (command === 'read_explore') return Promise.resolve(null);
  if (command === 'explore_doc_path')
    return Promise.resolve('C:\\demo\\alpha\\openspec\\explores\\x.md');
  if (command === 'watch_subscribe') return Promise.resolve(1);
  if (command === 'watch_unsubscribe') return Promise.resolve(null);
  if (command === 'agent_sessions') return Promise.resolve([]);
  if (command === 'agent_session_transcript') return Promise.resolve([]);
  if (command === 'promote_explore') {
    return Promise.resolve({ changeId: 'chg-1', changeName: 'api-retry' });
  }
  return Promise.resolve(null);
}

/** 路由装置：/explores（清单）与 /explores/:name（详情）两形态。 */
function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/explores" element={<ExploreView root={ROOT} />} />
        <Route path="/explores/:name" element={<ExploreView root={ROOT} />} />
        <Route path="/changes/:id" element={<span data-testid="change-detail-stub" />} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => ipc(command));
  records = [record(1, 'api-retry', '接口重试策略'), record(2, 'layout-design', '布局设计')];
});

afterEach(() => {
  cleanup();
});

describe('ExploreView 清单页：条目渲染 record.title（AC-8 清单半边）', () => {
  it('正向：条目标题显示 title（title ≠ name 时显示 title 而非 name）', async () => {
    renderAt('/explores');

    await screen.findByTestId('explore-items');
    const titles = (await screen.findAllByTestId('explore-item-title')).map(
      (node) => node.textContent,
    );
    expect(titles).toEqual(['接口重试策略', '布局设计']);
    // name 仅作 data-name 保留（零标题语义）
    const items = screen.getAllByTestId('explore-item');
    expect(items.map((node) => node.getAttribute('data-name'))).toEqual([
      'api-retry',
      'layout-design',
    ]);
    expect(screen.queryByText('api-retry')).toBeNull();
  });

  it('边界：title 两态（title = name 缺省样本与显式标题样本）各渲染正确，恒非空零回退分支', async () => {
    records = [record(1, 'same-name', 'same-name'), record(2, 'explicit', '显式标题')];
    renderAt('/explores');

    await screen.findByTestId('explore-items');
    const titles = (await screen.findAllByTestId('explore-item-title')).map(
      (node) => node.textContent,
    );
    expect(titles).toEqual(['same-name', '显式标题']);
    expect(titles.every((title) => title !== null && title.length > 0)).toBe(true);
  });
});

describe('ExploreView 详情页：list 透传与缺记录态（AC-8 / AC-5 动作可达）', () => {
  it('正向：选中记录 → ExploreDetailView 收到 list（详情头 title + promote 动作可达）', async () => {
    renderAt('/explores/api-retry');

    const title = await screen.findByTestId('explore-detail-title');
    expect(title.textContent).toBe('接口重试策略');

    // list.promote 可达：点击「启动变更」→ invoke promote_explore
    fireEvent.click(screen.getByTestId('explore-promote-trigger'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('promote_explore', {
        root: ROOT,
        name: 'api-retry',
      }),
    );
  });

  it('边界：未选中 / 缺失记录（清单无该 name）→ 详情缺失态占位（既有语义零改动）', async () => {
    renderAt('/explores/ghost-topic');

    expect(await screen.findByTestId('explore-detail-missing')).toBeTruthy();
    expect(screen.queryByTestId('explore-detail-title')).toBeNull();
  });
});
