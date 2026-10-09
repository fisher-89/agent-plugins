import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ChangeCreateDialog } from './change-create-dialog';

// 仅进程边界 mock
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = '/repo';

/** CreateOutcome fixture（与 Rust 写面 DTO 同形：name / created / worktree /
 * warnings 四字段）。 */
const OUTCOME = {
  name: 'fix-bug',
  created: '2026-10-02',
  worktree: 'C:\\home\\.dev-team\\worktrees\\repo-ab12\\fix-bug',
  warnings: [] as string[],
};

/** 挂载对话框（触发器 = children 文本按钮）。 */
function renderDialog(onCreated: (name: string) => void = vi.fn()): void {
  render(
    <ChangeCreateDialog root={ROOT} onCreated={onCreated}>
      <button type="button">打开新建</button>
    </ChangeCreateDialog>,
  );
}

/** 点击触发器打开弹窗。 */
function openForm(): void {
  fireEvent.click(screen.getByRole('button', { name: '打开新建' }));
}

/** 填名称与 goal 并点页脚「提交」。 */
function submit(name: string, goal: string): void {
  fireEvent.change(screen.getByTestId('change-create-name'), {
    target: { value: name },
  });
  fireEvent.change(screen.getByTestId('change-create-goal'), {
    target: { value: goal },
  });
  fireEvent.click(screen.getByRole('button', { name: '提交' }));
}

/** create_change 调用载荷清单（进程边界观察面）。 */
function createCalls(): Array<Record<string, unknown>> {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'create_change')
    .map(([, args]) => args as Record<string, unknown>);
}

describe('ChangeCreateDialog：弹窗开合与成功提交流转', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
  });

  it('关闭态仅触发器在场；点开呈现标题 / 名称 / goal 输入与页脚按钮，取消关闭后输入区不残留', async () => {
    renderDialog();

    expect(screen.queryByTestId('change-create-name')).toBeNull();
    expect(screen.queryByTestId('change-create-goal')).toBeNull();

    openForm();
    expect(screen.getByText('新建变更') !== null).toBe(true);
    expect(screen.getByTestId('change-create-name') !== null).toBe(true);
    expect(screen.getByTestId('change-create-goal') !== null).toBe(true);
    expect(screen.getByRole('button', { name: '提交' }) !== null).toBe(true);
    expect(screen.getByRole('button', { name: '取消' }) !== null).toBe(true);

    fireEvent.click(screen.getByRole('button', { name: '取消' }));
    await waitFor(() => {
      expect(screen.queryByTestId('change-create-name')).toBeNull();
      expect(screen.queryByTestId('change-create-goal')).toBeNull();
    });
  });

  it('合法输入提交：invoke("create_change") 恰一次且载荷为 {root, name, goal}；成功即关窗并触发 onCreated(name) 恰一次（成功面退役，直连导航）', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(onCreated).toHaveBeenCalledWith('fix-bug');
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'fix-bug', goal: '修复登录重试的竞态问题' },
    ]);
    // 成功关窗：输入区退场
    await waitFor(() => expect(screen.queryByTestId('change-create-name')).toBeNull());
  });

  it('成功后再开即新表单（成功路径重置名称与 goal，无残留）', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();
    submit('reset-after', '重置验证目标');
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByTestId('change-create-name')).toBeNull());

    openForm();
    const name_input = screen.getByTestId<HTMLInputElement>('change-create-name');
    const goal_input = screen.getByTestId<HTMLTextAreaElement>('change-create-goal');
    expect(name_input.value).toBe('');
    expect(goal_input.value).toBe('');
    expect(screen.queryByTestId('change-create-error')).toBeNull();
  });
});

describe('ChangeCreateDialog：提交前 trim 与 free-form goal 透传', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
  });

  it('名称与 goal 带首尾空白：invoke 载荷为 trim 后值（校验对象与提交值同为 trim 后串）', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    submit('  fix-bug  ', '  带空白的目标文本  ');

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()).toEqual([{ root: ROOT, name: 'fix-bug', goal: '带空白的目标文本' }]);
  });

  it('goal 多行 + emoji + 超 1000 字符原样透传不损（free-form UTF-8 前端半边）', async () => {
    renderDialog();
    openForm();
    const goal = `第一行\n第二行 🚀 emoji\t制表符\n${'长'.repeat(1200)}`;

    submit('rich-goal', goal);

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()).toEqual([{ root: ROOT, name: 'rich-goal', goal }]);
  });
});

describe('ChangeCreateDialog：必填与本地 kebab-case 校验（失败留窗零 invoke）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
  });

  it('名称空或 goal 空白（"" / "   "）：点「提交」零 invoke、onCreated 零调用、弹窗不关且行内呈现校验错误', () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    // 名称空 + goal 空（初始态）
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    expect(createCalls()).toHaveLength(0);
    expect(onCreated).not.toHaveBeenCalled();
    expect(screen.getByTestId('change-create-error').textContent).toContain('校验不通过');
    expect(screen.getByTestId('change-create-name') !== null).toBe(true);

    // 名称空 + goal 有值
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '只有 goal' },
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    expect(createCalls()).toHaveLength(0);

    // 名称合法 + goal 空白（trim 后空）
    fireEvent.change(screen.getByTestId('change-create-name'), {
      target: { value: 'fix-bug' },
    });
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '   ' },
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    expect(createCalls()).toHaveLength(0);
    expect(onCreated).not.toHaveBeenCalled();
    expect(screen.getByTestId('change-create-error') !== null).toBe(true);
  });

  it('非法 kebab-case 全族零 invoke（本地与写面同口径校验，「本地放行 ⇒ 后端必过」）', () => {
    const invalidNames = [
      'Fix-Bug', // 大写
      'fix_bug', // 下划线
      'fix bug', // 空格
      '1fix', // 前导数字
      'fix-', // 尾连字符
      '-fix', // 前导连字符
      'fix--bug', // 连号连字符
      `a${'b'.repeat(128)}`, // 超 128 字符
    ];

    for (const name of invalidNames) {
      const view = render(
        <ChangeCreateDialog root={ROOT} onCreated={vi.fn()}>
          <button type="button">打开新建</button>
        </ChangeCreateDialog>,
      );
      openForm();
      submit(name, '合法 goal');

      expect(screen.getByTestId('change-create-error').textContent).toContain('校验不通过');
      view.unmount();
    }

    expect(createCalls()).toHaveLength(0);
  });

  it('恰 128 字符名称可提交且 invoke 发起（≤128 含端点，与写面上界同口径）', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();
    const name = `a${'b'.repeat(127)}`;
    expect(name.length).toBe(128);

    invokeMock.mockResolvedValue({ ...OUTCOME, name });
    submit(name, '上界内 goal');

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()[0]).toEqual({ root: ROOT, name, goal: '上界内 goal' });
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(name));
  });
});

describe('ChangeCreateDialog：后端错误行内呈现（失败留窗）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockRejectedValue('change "fix-bug" 已存在');
  });

  it('invoke reject：change-create-error 行内块呈现错误文本、弹窗不关、onCreated 零调用', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    const error = await screen.findByTestId('change-create-error');
    expect(error.textContent).toContain('已存在');
    expect(error.className).toContain('break-all');
    // 失败留窗：输入区仍在场（修正后可直接重提）
    expect(screen.getByTestId('change-create-name') !== null).toBe(true);
    expect(onCreated).not.toHaveBeenCalled();
  });
});
