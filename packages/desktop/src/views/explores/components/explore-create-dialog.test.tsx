import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ExploreCreateDialog } from './explore-create-dialog';

// 进程边界 Mock：仅 mock IPC（invoke）。组件与内部状态真实组合。
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = 'C:\\demo\\alpha';
/** 后端占位建档返回的占位名（draft-<毫秒> 形态基设） */
const PLACEHOLDER_NAME = 'draft-1758123456789';

type OnCreated = (name: string, autoPrompt: string | null) => void;

let onCreated: OnCreated;
/** onCreated 捕获面（回调载荷录制，供载荷形状断言） */
let createdCalls: Array<[string, string | null]>;

/** 进程边界应答面：scan_explores 可编程（命令层已过滤结果）、create 占位建档可编程。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'scan_explores') return Promise.resolve([]);
  if (command === 'create_explore_record') {
    return Promise.resolve({
      id: 1,
      root: ROOT,
      name: PLACEHOLDER_NAME,
      title: '2026-10-17 08:30:45-分析中',
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

/** 在需求 textarea 输入（新话题入口的唯一输入面）。 */
function typeRequirement(value: string) {
  fireEvent.change(screen.getByTestId('explore-topic-requirement'), { target: { value } });
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => ipc(command));
  createdCalls = [];
  onCreated = vi.fn<OnCreated>((name, autoPrompt) => {
    createdCalls.push([name, autoPrompt]);
  });
});

afterEach(() => {
  cleanup();
});

// ---------------------------------------------------------------------------
// 新话题入口：需求输入建档（AC-1）
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：新话题需求输入建档（AC-1）', () => {
  it('正向：自然语言需求（中文/空格/换行）→ 提交启用、invoke name=null 恰一次、onCreated(占位名, 需求原文)', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();
    const requirement = '调研 API 重试策略\n现有超时与重试逻辑在哪些边界会失效？';
    typeRequirement(requirement);

    const create = screen.getByTestId<HTMLButtonElement>('explore-topic-create');
    expect(create.disabled).toBe(false);
    fireEvent.click(create);

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('create_explore_record', {
      root: ROOT,
      name: null,
    });
    // 占位名取返回记录 name、需求原文 trim 后上抛（auto-send 链路的载荷半边）
    expect(onCreated).toHaveBeenCalledWith(PLACEHOLDER_NAME, requirement.trim());
  });

  it('边界：空串与全空白需求（空格/换行）→ 提交禁用且零 invoke', () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();

    for (const blank of ['', '   ', '\n\t\n']) {
      typeRequirement(blank);
      const create = screen.getByTestId<HTMLButtonElement>('explore-topic-create');
      expect(create.disabled).toBe(true);
      fireEvent.click(create);
    }

    expect(invokeMock).not.toHaveBeenCalled();
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('边界：超长需求（>1000 字符）→ 正常提交、trim 后原样上抛不被截断', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();
    const requirement = `超长需求开头${'长'.repeat(1200)}   `;
    typeRequirement(requirement);

    const create = screen.getByTestId<HTMLButtonElement>('explore-topic-create');
    expect(create.disabled).toBe(false);
    fireEvent.click(create);

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(onCreated).toHaveBeenCalledWith(PLACEHOLDER_NAME, requirement.trim());
    expect(createdCalls[0]?.[1]?.length).toBe(requirement.trim().length);
  });

  it('边界：kebab 校验退役——大写/下划线/前导数字文本不触发 explore-topic-invalid、提交仍可用', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();

    for (const text of ['Api-Retry', 'api_retry', '1api', 'api retry']) {
      typeRequirement(text);
      expect(screen.queryByTestId('explore-topic-invalid')).toBeNull();
      const create = screen.getByTestId<HTMLButtonElement>('explore-topic-create');
      expect(create.disabled).toBe(false);
    }

    // 输入态零 invoke（尚未点击提交）
    expect(invokeMock).not.toHaveBeenCalled();

    // 任意非 kebab 文本仍可提交建档（需求原文上抛）
    typeRequirement('1api');
    fireEvent.click(screen.getByTestId('explore-topic-create'));
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(onCreated).toHaveBeenCalledWith(PLACEHOLDER_NAME, '1api');
  });

  it('异常：invoke reject → 错误行呈现、onCreated 不调用', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'create_explore_record') return Promise.reject('非法 root');
      return Promise.resolve([]);
    });
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();
    typeRequirement('调研需求');
    fireEvent.click(screen.getByTestId('explore-topic-create'));

    const error = await screen.findByTestId('explore-create-error');
    expect(error.textContent).toContain('非法 root');
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('结构迁移：explore-topic-requirement 在场、explore-topic-name 退役不在场', () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();

    expect(screen.getByTestId('explore-topic-requirement').tagName).toBe('TEXTAREA');
    expect(screen.queryByTestId('explore-topic-name')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 导入入口：显式名建档（AC-8 回归——onCreated(name, null) 零语义变化）
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：导入入口显式名回归（AC-8）', () => {
  it('正向：导入成功 → onCreated(entry.name, null)（autoPrompt 恒 null）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'scan_explores') {
        return Promise.resolve([
          { name: 'valid-topic', modifiedAt: 1727000000000 },
          { name: 'another-topic', modifiedAt: null },
        ]);
      }
      if (command === 'create_explore_record') {
        return Promise.resolve({
          id: 2,
          root: ROOT,
          name: 'valid-topic',
          title: 'valid-topic',
          promotedTo: null,
          createdAt: 1,
          updatedAt: 1,
        });
      }
      return Promise.resolve(null);
    });
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    const items = await screen.findAllByTestId('explore-import-item');
    expect(items).toHaveLength(2);
    expect(invokeMock).toHaveBeenCalledWith('scan_explores', { root: ROOT });

    fireEvent.click(screen.getByText('valid-topic'));
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(onCreated).toHaveBeenCalledWith('valid-topic', null);
  });

  it('边界：scan_explores 返回空 → 空态提示（命令层已过滤的另一形态）', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    expect(await screen.findByTestId('explore-import-empty')).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// 折叠开关与页签外观（结构面：新建/收起文案、活动页签样式、页签可回切）
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：折叠开关与入口页签（结构面）', () => {
  it('正向：折叠态文案「新建」→ 展开后「收起」→ 再点回「新建」，展开/收起切换入口区在场性', () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    const toggle = screen.getByTestId('explore-create-toggle');
    expect(toggle.textContent).toBe('新建');
    expect(screen.queryByTestId('explore-create-tabs')).toBeNull();

    fireEvent.click(toggle);
    expect(toggle.textContent).toBe('收起');
    expect(screen.getByTestId('explore-create-tabs')).toBeTruthy();

    fireEvent.click(toggle);
    expect(toggle.textContent).toBe('新建');
    expect(screen.queryByTestId('explore-create-tabs')).toBeNull();
  });

  it('正向：活动页签呈 bg-muted font-medium、非活动页签呈 bg-transparent（两态各验一次）', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));

    // 默认页签 = 新话题
    const topicTab = screen.getByTestId('explore-tab-topic');
    const importTab = screen.getByTestId('explore-tab-import');
    expect(topicTab.className).toContain('bg-muted font-medium');
    expect(topicTab.className).not.toContain('bg-transparent');
    expect(importTab.className).toContain('bg-transparent');
    expect(importTab.className).not.toContain('bg-muted');

    fireEvent.click(importTab);
    expect(importTab.className).toContain('bg-muted font-medium');
    expect(importTab.className).not.toContain('bg-transparent');
    expect(topicTab.className).toContain('bg-transparent');
    expect(topicTab.className).not.toContain('bg-muted');
    // 排空导入页扫描应答（状态更新收口）
    await screen.findByTestId('explore-import-empty');
  });

  it('正向：切到导入页再点回「新话题」→ 新话题入口恢复在场、导入入口卸载', async () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));
    await screen.findByTestId('explore-import-entry');

    fireEvent.click(screen.getByTestId('explore-tab-topic'));

    expect(screen.getByTestId('explore-topic-entry')).toBeTruthy();
    expect(screen.queryByTestId('explore-import-entry')).toBeNull();
  });

  it('初值：新话题需求为空串 → 提交禁用、零错误块、零 invoke', () => {
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    openTopicTab();

    expect(screen.getByTestId<HTMLTextAreaElement>('explore-topic-requirement').value).toBe('');
    expect(screen.getByTestId<HTMLButtonElement>('explore-topic-create').disabled).toBe(true);
    expect(screen.queryByTestId('explore-create-error')).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 导入入口：扫描时间戳两态、建档案失败呈现、扫描失败呈现（边界面）
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：导入入口时间戳与失败呈现（AC-8 边界）', () => {
  it('正向：modifiedAt 非空 → 条目标注修改时间；modifiedAt 为空 → 仅呈现 stem（无时间文本）', async () => {
    const modifiedAt = 1727000000000;
    invokeMock.mockImplementation((command: string) => {
      if (command === 'scan_explores') {
        return Promise.resolve([{ name: 'with-time', modifiedAt }]);
      }
      return Promise.resolve(null);
    });
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    const withTime = await screen.findByTestId('explore-import-item');
    expect(withTime.textContent).toContain('with-time');
    expect(withTime.textContent).toContain(new Date(modifiedAt).toLocaleString());
    // 零错误基线：正常扫描不呈现错误块
    expect(screen.queryByTestId('explore-create-error')).toBeNull();
    cleanup();

    // modifiedAt 为空：仅 stem（零时间文本，无空 Span 残留）
    invokeMock.mockImplementation((command: string) => {
      if (command === 'scan_explores') {
        return Promise.resolve([{ name: 'no-time', modifiedAt: null }]);
      }
      return Promise.resolve(null);
    });
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    const noTime = await screen.findByTestId('explore-import-item');
    expect(noTime.textContent).toBe('no-time');
    expect(noTime.querySelector('span')).toBeNull();
    expect(screen.queryByTestId('explore-create-error')).toBeNull();
  });

  it('异常：选中导入项后建档 reject → 错误块呈现后端原文、onCreated 不调用', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'scan_explores') {
        return Promise.resolve([{ name: 'valid-topic', modifiedAt: null }]);
      }
      if (command === 'create_explore_record') return Promise.reject('笔记已被占用');
      return Promise.resolve(null);
    });
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    fireEvent.click(await screen.findByTestId('explore-import-item'));

    const error = await screen.findByTestId('explore-create-error');
    expect(error.textContent).toContain('笔记已被占用');
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('异常：扫描 reject → 错误块呈现扫描失败原文（导入清单保持扫描中态，不崩）', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'scan_explores') return Promise.reject('目录不可读');
      return Promise.resolve(null);
    });
    render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    const error = await screen.findByTestId('explore-create-error');
    expect(error.textContent).toContain('目录不可读');
    expect(screen.getByText('扫描中…')).toBeTruthy();
  });

  it('边界：root 切换 → 以新 root 重扫，旧 root 迟到应答不覆盖新清单（取消抑制）', async () => {
    const ROOT2 = 'C:demo\beta';
    let resolveStale: (value: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
      if (command === 'scan_explores') {
        if (params?.root === ROOT) {
          return new Promise((resolve) => {
            resolveStale = resolve;
          });
        }
        return Promise.resolve([{ name: 'new-topic', modifiedAt: null }]);
      }
      return Promise.resolve(null);
    });
    const { rerender } = render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    rerender(<ExploreCreateDialog onCreated={onCreated} root={ROOT2} />);

    // 新 root 重扫发生
    await screen.findByText('new-topic');
    expect(invokeMock).toHaveBeenCalledWith('scan_explores', { root: ROOT2 });

    // 旧 root 迟到应答被取消抑制吞掉（不覆盖新清单）
    await act(async () => {
      resolveStale([{ name: 'stale-topic', modifiedAt: null }]);
    });
    expect(screen.queryByText('stale-topic')).toBeNull();
    expect(screen.getByText('new-topic')).toBeTruthy();
  });
});

describe('ExploreCreateDialog：root 切换后旧扫描失败应答的取消抑制', () => {
  it('边界：旧 root 扫描迟到 reject → 不呈现错误块（已卸载形态的应答被吞掉）', async () => {
    const ROOT2 = 'C:demo\beta';
    let rejectStale: (err: unknown) => void = () => {};
    invokeMock.mockImplementation((command: string, params?: { root?: string }) => {
      if (command === 'scan_explores') {
        if (params?.root === ROOT) {
          return new Promise((_resolve, reject) => {
            rejectStale = reject;
          });
        }
        return Promise.resolve([{ name: 'new-topic', modifiedAt: null }]);
      }
      return Promise.resolve(null);
    });
    const { rerender } = render(<ExploreCreateDialog onCreated={onCreated} root={ROOT} />);
    fireEvent.click(screen.getByTestId('explore-create-toggle'));
    fireEvent.click(screen.getByTestId('explore-tab-import'));

    rerender(<ExploreCreateDialog onCreated={onCreated} root={ROOT2} />);
    await screen.findByText('new-topic');

    await act(async () => {
      rejectStale('旧 root 目录不可读');
    });
    expect(screen.queryByTestId('explore-create-error')).toBeNull();
    expect(screen.getByText('new-topic')).toBeTruthy();
  });
});
