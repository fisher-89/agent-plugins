// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ModelInfo, RecordEnvelope } from '../types/dto';
import { DbInspectorView } from '../views/db/DbInspectorView';

// ---------------------------------------------------------------------------
// 集成关系：DbInspectorView → useDbInspector → db_models/db_records IPC（AC-5/AC-7）。
// 真 hook + 真 view 挂载，仅 mock IPC 进程边界：invoke 按命令名分发多页信封
// fixture 并记录调用序列（翻页 offset 步进与 hasMore 判定的拼接不重不漏只有
// 组合才可验证）。sonner 以 spy mock 注入：「查询失败 inline 持久」场景断言
// 无 toast 调用。
// ---------------------------------------------------------------------------

const { invokeMock, toastErrorMock, toastInfoMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastErrorMock: vi.fn(),
  toastInfoMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('sonner', () => ({
  toast: { error: toastErrorMock, success: toastInfoMock, info: toastInfoMock },
}));

// ---------------------------------------------------------------------------
// fixture：agent_run 载荷 120 条（页长 50 → 三页：50 / 50 / 20，末页不足整页）
// ---------------------------------------------------------------------------

const PAGE_SIZE = 50;
const MODELS: ModelInfo[] = [
  { name: 'agent_run', count: 120 },
  { name: 'workspace', count: 0 },
];
const INVENTORY: RecordEnvelope[] = Array.from({ length: 120 }, (_, index) => ({
  key: index,
  value: { id: index, prompt: `run-${index}`, status: 'completed' },
}));

let recordsReject: string | null = null;

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

function dbRecordsCalls(): Array<{ model?: string; offset?: number; limit?: number }> {
  return invokeMock.mock.calls
    .filter(([name]) => name === 'db_records')
    .map(([, params]) => params as { model?: string; offset?: number; limit?: number });
}

function mockIpc() {
  recordsReject = null;
  invokeMock.mockImplementation(
    (command: string, params?: { model?: string; offset?: number; limit?: number }) => {
      if (command === 'db_models') {
        return Promise.resolve(MODELS.map((model) => ({ ...model })));
      }
      if (command === 'db_records') {
        if (recordsReject !== null) return Promise.reject(new Error(recordsReject));
        const offset = params?.offset ?? 0;
        const limit = params?.limit ?? PAGE_SIZE;
        return Promise.resolve(INVENTORY.slice(offset, offset + limit).map((e) => ({ ...e })));
      }
      return Promise.resolve(null);
    },
  );
}

/** 从渲染行提取 key（数值 key 的 JSON 文本即数字）。 */
function renderedKeys(): number[] {
  return screen
    .getAllByTestId('db-record-item')
    .map((row) => row.textContent?.match(/key: (-?\d+)/)?.[1])
    .map((text) => {
      if (text === undefined) throw new Error('记录行 key 文本无法解析');
      return Number(text);
    });
}

async function mountAndSelect() {
  render(<DbInspectorView />);
  await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
  fireEvent.click(screen.getAllByTestId('db-model-item')[0]); // agent_run
  await waitFor(() => expect(screen.getAllByTestId('db-record-item')).toHaveLength(PAGE_SIZE));
}

beforeEach(() => {
  invokeMock.mockReset();
  toastErrorMock.mockReset();
  toastInfoMock.mockReset();
  mockIpc();
});

afterEach(() => {
  cleanup();
});

describe('db_inspector_pipeline：清单到翻页取数链不重不漏', () => {
  it('进页取清单一次 → 选中模型取第一页 → 两次翻页：invoke offset 严格步进、并集不重不漏', async () => {
    await mountAndSelect();

    expect(countOf('db_models')).toBe(1);
    // 记录区标题以所选模型名（清单首项）呈现；有记录时无错误区与空态
    expect(screen.getByRole('heading', { name: 'agent_run' }) !== null).toBe(true);
    expect(screen.queryByText(/记录扫描失败/)).toBeNull();
    expect(screen.queryByTestId('db-records-empty')).toBeNull();
    const page1 = renderedKeys();

    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(renderedKeys()[0]).toBe(50));
    const page2 = renderedKeys();

    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(renderedKeys()).toHaveLength(20));
    const page3 = renderedKeys();

    // invoke 参数契约：offset 严格步进、limit 恒页长
    expect(dbRecordsCalls()).toEqual([
      { model: 'agent_run', offset: 0, limit: PAGE_SIZE },
      { model: 'agent_run', offset: 50, limit: PAGE_SIZE },
      { model: 'agent_run', offset: 100, limit: PAGE_SIZE },
    ]);

    // 三页并集 = fixture 全集：无重复、无遗漏（120 = 50 + 50 + 20）
    const union = [...page1, ...page2, ...page3];
    expect(new Set(union).size).toBe(120);
    expect(union).toHaveLength(120);
    expect(union.every((key, index) => key === index)).toBe(true);
  });

  it('点开单条记录：呈现的 pretty JSON 与该信封 key/value 一致（人可读、可复制语义）', async () => {
    await mountAndSelect();

    fireEvent.click(screen.getAllByTestId('db-record-item')[0]);

    const detail = screen.getByTestId('db-record-json');
    expect(detail.textContent).toBe(JSON.stringify(INVENTORY[0], null, 2));
    expect(detail.textContent).toContain('"prompt"');
    expect(detail.textContent).toContain('run-0');
  });

  it('静置后 invoke 调用数不增长（无轮询、无事件订阅，AC-7 只读取数纪律）', async () => {
    render(<DbInspectorView />);
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));
    const baseline = invokeMock.mock.calls.length;

    await new Promise((resolve) => setTimeout(resolve, 30));

    expect(invokeMock.mock.calls.length).toBe(baseline);
    expect(countOf('db_records')).toBe(0); // 未选中模型不取数
  });

  it('末页 hasMore=false 时翻页控件禁用、点击不产生多余 invoke', async () => {
    await mountAndSelect();

    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(renderedKeys()[0]).toBe(50));
    fireEvent.click(screen.getByTestId('db-page-next'));
    await waitFor(() => expect(renderedKeys()).toHaveLength(20)); // 末页不足整页
    const callsAtLastPage = countOf('db_records');

    expect(screen.getByTestId('db-page-next').hasAttribute('disabled')).toBe(true);
    fireEvent.click(screen.getByTestId('db-page-next')); // disabled → 无效点击
    expect(countOf('db_records')).toBe(callsAtLastPage); // 无多余 invoke
    expect(screen.getByTestId('db-page-prev').hasAttribute('disabled')).toBe(false);
  });

  it('db_records reject：inline 持久错误区呈现且不消失、无 toast 调用', async () => {
    recordsReject = 'db: 未知模型: nope';
    render(<DbInspectorView />);
    await waitFor(() => expect(screen.getByTestId('db-model-list') !== null).toBe(true));

    fireEvent.click(screen.getAllByTestId('db-model-item')[0]);
    // hook 以 String(err) 呈现（带 Error: 前缀），错误串原文不丢失
    const errorNote = await screen.findByText(/记录扫描失败：.*未知模型: nope/);
    expect(errorNote !== null).toBe(true);

    // inline 持久：静置后错误区仍在（后续渲染不顶替），清单不丢失
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(screen.getByText(/记录扫描失败：.*未知模型: nope/) !== null).toBe(true);
    expect(screen.getAllByTestId('db-model-item')).toHaveLength(2);
    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(toastInfoMock).not.toHaveBeenCalled();
  });
});
