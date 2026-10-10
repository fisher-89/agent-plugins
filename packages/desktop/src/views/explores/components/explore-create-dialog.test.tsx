import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ExploreCreateDialog } from './explore-create-dialog';

// 进程边界 Mock：仅 mock IPC（invoke）。组件与内部状态真实组合。
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = 'C:\\demo\\alpha';

let onCreated = vi.fn<(name: string) => void>();

/** 进程边界应答面：scan_explores 可编程（命令层已过滤结果）、create 可编程。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'scan_explores') return Promise.resolve([]);
  if (command === 'create_explore_record') {
    return Promise.resolve({
      id: 1,
      root: ROOT,
      name: 'api-retry',
      title: 'api-retry',
      promotedTo: null,
      createdAt: 1,
      updatedAt: 1,
    });
  }
  return Promise.resolve(null);
}

/** 打开对话框并切到「新话题」页。 */
function openTopicTab() {
  fireEvent.click(screen.getByTestId('explore-create-toggle'));
  fireEvent.click(screen.getByTestId('explore-tab-topic'));
}

function typeTopic(value: string) {
  fireEvent.change(screen.getByTestId('explore-topic-name'), { target: { value } });
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => ipc(command));
  onCreated = vi.fn<(name: string) => void>();
});

afterEach(() => {
  cleanup();
});

describe('ExploreCreateDialog：新话题本地 kebab 校验（AC-3 本地校验半边）', () => {
  it('正向：合法 kebab → 建档按钮可点，invoke create_explore_record 恰一次并回调 onCreated', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();
    typeTopic('api-retry');

    const create = screen.getByTestId<HTMLButtonElement>('explore-topic-create');
    expect(create.disabled).toBe(false);
    fireEvent.click(create);

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(invokeMock).toHaveBeenCalledWith('create_explore_record', {
      root: ROOT,
      name: 'api-retry',
    });
    expect(onCreated).toHaveBeenCalledWith('api-retry');
  });

  it('边界：非法 kebab（大写 / 下划线 / 空格 / 前导数字）→ 提交禁用 + 本地错误提示（零 invoke）', () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();

    for (const illegal of ['Api-Retry', 'api_retry', 'api retry', '1api', 'api-']) {
      typeTopic(illegal);
      const create = screen.getByTestId<HTMLButtonElement>('explore-topic-create');
      expect(create.disabled).toBe(true);
      expect(screen.getByTestId('explore-topic-invalid')).toBeTruthy();
      fireEvent.click(create);
    }

    expect(invokeMock.mock.calls.filter(([name]) => name === 'create_explore_record')).toHaveLength(
      0,
    );
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('边界：本地错误块呈现、合法化后错误清除', () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();

    typeTopic('Bad_Name');
    expect(screen.getByTestId('explore-topic-invalid').textContent).toContain('kebab-case');

    typeTopic('good-name');
    expect(screen.queryByTestId('explore-topic-invalid')).toBeNull();
    expect(screen.getByTestId<HTMLButtonElement>('explore-topic-create').disabled).toBe(false);
  });
});

describe('ExploreCreateDialog：导入列表消费命令层已过滤结果（AC-3 导入半边）', () => {
  it('边界：scan_explores 返回结果原样渲染（本地零重复 kebab 校验——守线在命令层）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'scan_explores') {
        // 命令层过滤后的结果面：组件侧零二次校验（非 kebab 亦原样渲染——
        // 守线在命令层 scan_explores 的 kebab 过滤，本地不重复判定）
        return Promise.resolve([
          { name: 'valid-topic', modifiedAt: 1727000000000 },
          { name: 'another-topic', modifiedAt: null },
          { name: 'Not-Kebab', modifiedAt: null },
        ]);
      }
      return Promise.resolve(null);
    });
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    await waitFor(() => expect(screen.getAllByTestId('explore-import-item')).toHaveLength(3));
    expect(invokeMock).toHaveBeenCalledWith('scan_explores', { root: ROOT });
    expect(screen.getByText('valid-topic')).toBeTruthy();
    expect(screen.getByText('another-topic')).toBeTruthy();
  });

  it('边界：scan_explores 返回空 → 空态提示（命令层已过滤的另一形态）', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    expect(await screen.findByTestId('explore-import-empty')).toBeTruthy();
  });
});
