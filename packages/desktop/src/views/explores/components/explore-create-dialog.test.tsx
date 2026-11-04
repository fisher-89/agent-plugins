import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, resetDrift } from '../../../lib/ipc-drift';
import { ExploreCreateDialog } from './explore-create-dialog';

// 进程边界 Mock：仅 mock IPC（invoke）。组件与内部状态真实组合。
// 建档入口自「内联折叠区 + 页签」改为 `StandardDialog` 弹窗（弹窗本体与
// trigger / footer 契约由 `components/standard/dailog.test.tsx` 锁定），本文件
// 只断言 explore 侧的载荷与回调语义。
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = 'C:\\demo\\alpha';
/** 后端占位建档返回的占位名（draft-<毫秒> 形态基设） */
const PLACEHOLDER_NAME = 'draft-1758123456789';

type OnCreated = (name: string, autoPrompt: string | null) => void;

let onCreated: OnCreated;
/** onCreated 捕获面（回调载荷录制，供载荷形状断言） */
let createdCalls: Array<[string, string | null]>;

/** 合法 `ExploreRecord`（九字段全在场——strict 面零漂移的干净基线）。 */
function exploreRecord(name: string): Record<string, unknown> {
  return {
    id: 1,
    root: ROOT,
    name,
    title: '2026-10-17 08:30:45-分析中',
    workflowType: 'requirement',
    promotedTo: null,
    sessionId: null,
    createdAt: 1,
    updatedAt: 1,
  };
}

/** 进程边界应答面：建档命令可编程（其余命令零参与）。 */
function ipc(command: string): Promise<unknown> {
  if (command === 'create_explore_record') {
    return Promise.resolve(exploreRecord(PLACEHOLDER_NAME));
  }
  return Promise.resolve(null);
}

/** 挂载对话框（触发器 = 消费方渲染的图标按钮）。 */
function renderDialog(): ReturnType<typeof render> {
  return render(
    <ExploreCreateDialog
      root={ROOT}
      onCreated={onCreated}
      trigger={({ onClick }) => (
        <button type="button" onClick={onClick}>
          创建探索
        </button>
      )}
    />,
  );
}

/** 点击触发器打开弹窗。 */
function openDialog(): void {
  fireEvent.click(screen.getByRole('button', { name: '创建探索' }));
}

/** 在需求输入框写入文本。 */
function typeRequirement(value: string): void {
  fireEvent.change(screen.getByTestId('explore-topic-requirement'), { target: { value } });
}

/** 点击页脚「提交」。 */
function submit(): void {
  fireEvent.click(screen.getByRole('button', { name: '提交' }));
}

/** 建档命令调用载荷清单（进程边界观察面）。 */
function createCalls(): Array<Record<string, unknown>> {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'create_explore_record')
    .map(([, args]) => args as Record<string, unknown>);
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
// 弹窗形态：关闭态零输入面，打开后呈标题 / 需求输入 / 页脚
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：弹窗开合与结构', () => {
  it('正向：关闭态仅触发器在场；打开后呈「新建探索」标题、需求 textarea 与「取消 / 提交」页脚', async () => {
    renderDialog();

    expect(screen.queryByTestId('explore-topic-requirement')).toBeNull();

    openDialog();

    expect(await screen.findByText('新建探索')).toBeTruthy();
    expect(screen.getByTestId('explore-topic-requirement').tagName).toBe('TEXTAREA');
    expect(screen.getByRole('button', { name: '取消' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '提交' })).toBeTruthy();
  });

  it('正向：「取消」关窗且零 invoke、零 onCreated（输入不残留到下次打开）', async () => {
    renderDialog();
    openDialog();
    typeRequirement('取消前的草稿');

    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    await waitFor(() => expect(screen.queryByTestId('explore-topic-requirement')).toBeNull());
    expect(invokeMock).not.toHaveBeenCalled();
    expect(onCreated).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 新话题入口：需求输入建档
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：需求输入建档', () => {
  it('正向：自然语言需求（中文/空格/换行）→ invoke name=null 恰一次、onCreated(占位名, 需求原文 trim)', async () => {
    renderDialog();
    openDialog();
    const requirement = '调研 API 重试策略\n现有超时与重试逻辑在哪些边界会失效？  ';
    typeRequirement(requirement);

    submit();

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(createCalls()).toEqual([{ root: ROOT, name: null }]);
    // 占位名取返回记录 name、需求原文 trim 后上抛（auto-send 链路的载荷半边）
    expect(onCreated).toHaveBeenCalledWith(PLACEHOLDER_NAME, requirement.trim());
  });

  it('正向：成功后关窗且需求输入清空（再次打开即新表单）', async () => {
    const view = renderDialog();
    openDialog();
    typeRequirement('调研需求');

    submit();

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByTestId('explore-topic-requirement')).toBeNull());

    view.unmount();
    cleanup();
    renderDialog();
    openDialog();
    expect(screen.getByTestId<HTMLTextAreaElement>('explore-topic-requirement').value).toBe('');
    expect(screen.queryByTestId('explore-create-error')).toBeNull();
  });

  it('边界：空串与全空白需求（空格/换行）→ 零 invoke、onCreated 零调用、行内校验错误且留窗', () => {
    renderDialog();
    openDialog();

    for (const blank of ['', '   ', '\n\t\n']) {
      typeRequirement(blank);
      submit();

      expect(screen.getByTestId('explore-create-error').textContent).toContain('请输入探索需求');
      expect(screen.getByTestId('explore-topic-requirement')).toBeTruthy();
    }

    expect(invokeMock).not.toHaveBeenCalled();
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('边界：超长需求（>1000 字符）→ 正常提交、trim 后原样上抛不被截断', async () => {
    renderDialog();
    openDialog();
    const requirement = `超长需求开头${'长'.repeat(1200)}   `;
    typeRequirement(requirement);

    submit();

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(onCreated).toHaveBeenCalledWith(PLACEHOLDER_NAME, requirement.trim());
    expect(createdCalls[0]?.[1]?.length).toBe(requirement.trim().length);
  });

  it('边界：kebab 校验退役——大写/下划线/前导数字文本不触发 explore-topic-invalid、提交照常建档', async () => {
    renderDialog();
    openDialog();

    for (const text of ['Api-Retry', 'api_retry', '1api', 'api retry']) {
      typeRequirement(text);
      expect(screen.queryByTestId('explore-topic-invalid')).toBeNull();
      expect(screen.getByRole('button', { name: '提交' }).hasAttribute('disabled')).toBe(false);
    }

    // 输入态零 invoke（尚未点击提交）
    expect(invokeMock).not.toHaveBeenCalled();

    // 任意非 kebab 文本仍可提交建档（需求原文上抛）
    typeRequirement('1api');
    submit();
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(onCreated).toHaveBeenCalledWith(PLACEHOLDER_NAME, '1api');
  });
});

// ---------------------------------------------------------------------------
// 失败面：后端 reject 行内呈现、错误不跨提交残留
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：后端错误行内呈现（失败留窗）', () => {
  it('异常：invoke reject → explore-create-error 呈现后端原文、弹窗不关、onCreated 零调用', async () => {
    invokeMock.mockImplementation(() => Promise.reject('非法 root'));
    renderDialog();
    openDialog();
    typeRequirement('调研需求');

    submit();

    const error = await screen.findByTestId('explore-create-error');
    expect(error.textContent).toContain('非法 root');
    expect(screen.getByTestId('explore-topic-requirement')).toBeTruthy();
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('异常：再次提交清空上轮错误（错误不跨提交残留）', async () => {
    invokeMock
      .mockImplementationOnce(() => Promise.reject('第一次失败'))
      .mockImplementationOnce(() => Promise.resolve(exploreRecord(PLACEHOLDER_NAME)));
    renderDialog();
    openDialog();
    typeRequirement('调研需求');

    submit();
    await screen.findByTestId('explore-create-error');

    submit();
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId('explore-create-error')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 经 ipc 的参数面 / 结果面（参数面 strict / 结果面 soft-report）：
// 组件只换调用面（`commands.` → `ipc.`），结果面契约外只上报（对话框状态不崩）。
// 漂移断言用真实 collector。
// ---------------------------------------------------------------------------

describe('ExploreCreateDialog：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    resetDrift();
  });

  it('正向：提交 → create_explore_record 恰 1 次、参数面放行、零漂移', async () => {
    renderDialog();
    openDialog();
    typeRequirement('调研需求');

    submit();

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(createCalls()).toEqual([{ root: ROOT, name: null }]);
    expect(driftCount()).toBe(0);
  });

  it('异常：结果面契约外（返回记录缺 name）→ 漂移恰 1 条、对话框状态不崩', async () => {
    invokeMock.mockImplementation(() => {
      const dirty = exploreRecord(PLACEHOLDER_NAME);
      delete dirty.name;
      return Promise.resolve(dirty);
    });
    renderDialog();
    openDialog();
    typeRequirement('调研需求');

    submit();

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(driftCount()).toBe(1);
    // 软失败：既有回调照旧被调用（占位名取不到即为 undefined），不呈现错误块
    expect(screen.queryByTestId('explore-create-error')).toBeNull();
  });

  it('边界：空串 / 全空白需求 → 既有前置校验拦截、零 invoke、零漂移', () => {
    renderDialog();
    openDialog();

    for (const blank of ['', '   ', '\n\t\n']) {
      typeRequirement(blank);
      submit();
      expect(screen.getByTestId('explore-create-error').textContent).toContain('请输入探索需求');
    }

    expect(createCalls()).toHaveLength(0);
    expect(driftCount()).toBe(0);
  });
});
