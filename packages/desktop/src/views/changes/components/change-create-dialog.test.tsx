import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { ChangeCreateDialog } from './change-create-dialog';

// 仅进程边界 mock（@tauri-apps/api/core invoke）：名称 / goal 输入、本地
// kebab-case 校验与提交流转逻辑真实参与；create_change 应答可切换 resolve
//（CreateOutcome fixture，含 worktree 执行锚与警告清单——D14 成功面）/ reject
//（错误串），入参记录用于载荷与调用次数断言；onCreated 以 vi.fn spy 经
// props 注入（入参例外；成功面「进入详情」按钮触发）。
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = '/repo';

/** CreateOutcome fixture（与 Rust 写面 DTO 同形：name / created / worktree /
 * warnings 四字段——worktree 为刻意出线的执行锚）。 */
const OUTCOME = {
  name: 'fix-bug',
  created: '2026-10-02',
  worktree: 'C:\\home\\.dev-team\\worktrees\\repo-ab12\\fix-bug',
  warnings: [] as string[],
};

/** 展开对话框（折叠态仅 toggle 常驻）。 */
function openForm(): void {
  fireEvent.click(screen.getByTestId('change-create-toggle'));
}

/** 填名称与 goal 并提交。 */
function submit(name: string, goal: string): void {
  fireEvent.change(screen.getByTestId('change-create-name'), {
    target: { value: name },
  });
  fireEvent.change(screen.getByTestId('change-create-goal'), {
    target: { value: goal },
  });
  fireEvent.click(screen.getByTestId('change-create-submit'));
}

/** create_change 调用载荷清单（进程边界观察面）。 */
function createCalls(): Array<Record<string, unknown>> {
  return invokeMock.mock.calls
    .filter(([command]) => command === 'create_change')
    .map(([, args]) => args as Record<string, unknown>);
}

describe('ChangeCreateDialog：toggle 展开与成功提交流转', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
  });

  it('点击 toggle 展开名称 / goal 输入与提交按钮，再点收起且输入区不残留', () => {
    render(<ChangeCreateDialog root={ROOT} onCreated={vi.fn()} />);

    expect(screen.queryByTestId('change-create-name')).toBeNull();
    expect(screen.queryByTestId('change-create-goal')).toBeNull();

    openForm();
    expect(screen.getByTestId('change-create-name') !== null).toBe(true);
    expect(screen.getByTestId('change-create-goal') !== null).toBe(true);
    expect(screen.getByTestId('change-create-submit') !== null).toBe(true);

    fireEvent.click(screen.getByTestId('change-create-toggle'));
    expect(screen.queryByTestId('change-create-name')).toBeNull();
    expect(screen.queryByTestId('change-create-goal')).toBeNull();
    expect(screen.queryByTestId('change-create-submit')).toBeNull();
  });

  it('合法输入提交：invoke("create_change") 恰一次且载荷为 {root, name, goal}；成功面呈现 worktree 路径，「进入详情」触发 onCreated(name) 恰一次', async () => {
    const onCreated = vi.fn();
    render(<ChangeCreateDialog root={ROOT} onCreated={onCreated} />);
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    // 成功面替换表单（D14）：worktree 绝对路径行内呈现 + 进入详情按钮
    const success = await screen.findByTestId('change-create-success');
    expect(success.textContent).toContain('fix-bug');
    const worktree = await screen.findByTestId('change-create-worktree');
    expect(worktree.textContent).toContain(OUTCOME.worktree);
    expect(worktree.className).toContain('break-all');
    expect(screen.queryByTestId('change-create-name')).toBeNull();
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'fix-bug', goal: '修复登录重试的竞态问题' },
    ]);
    expect(onCreated).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('change-create-open-detail'));
    expect(onCreated).toHaveBeenCalledTimes(1);
    expect(onCreated).toHaveBeenCalledWith('fix-bug');
  });

  it('成功面警告清单行内逐条呈现（脏仓 / bootstrap 注记持久入 DTO）', async () => {
    invokeMock.mockResolvedValue({
      ...OUTCOME,
      warnings: [
        '主仓有未提交改动（worktree 基线仍取 HEAD）：建议先提交再开新 change',
        '未识别依赖管理器，跳过依赖引导',
      ],
    });
    render(<ChangeCreateDialog root={ROOT} onCreated={vi.fn()} />);
    openForm();

    submit('warned-change', '带警告的目标');

    const warnings = await screen.findByTestId('change-create-warnings');
    const items = warnings.querySelectorAll('li');
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toContain('建议先提交再开新 change');
    expect(items[1].textContent).toContain('未识别依赖管理器');
  });

  it('空警告清单：无警告区块渲染（空清单零占位）；worktree 路径仍呈现', async () => {
    invokeMock.mockResolvedValue({ ...OUTCOME, warnings: [] });
    render(<ChangeCreateDialog root={ROOT} onCreated={vi.fn()} />);
    openForm();

    submit('clean-change', '干净仓目标');

    const worktree = await screen.findByTestId('change-create-worktree');
    expect(worktree.textContent).toContain(OUTCOME.worktree);
    expect(screen.queryByTestId('change-create-warnings')).toBeNull();
  });

  it('toggle 收起重开即重置（成功面随 CreateForm 卸载重建回落表单）', async () => {
    render(<ChangeCreateDialog root={ROOT} onCreated={vi.fn()} />);
    openForm();
    submit('reset-after', '重置验证目标');

    await screen.findByTestId('change-create-success');

    fireEvent.click(screen.getByTestId('change-create-toggle'));
    expect(screen.queryByTestId('change-create-success')).toBeNull();
    fireEvent.click(screen.getByTestId('change-create-toggle'));
    expect(screen.queryByTestId('change-create-success')).toBeNull();
    const name_input = screen.getByTestId<HTMLInputElement>('change-create-name');
    expect(name_input !== null).toBe(true);
    expect(name_input.value).toBe('');
  });
});

describe('ChangeCreateDialog：提交前 trim 与 free-form goal 透传', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
  });

  it('名称与 goal 带首尾空白：invoke 载荷为 trim 后值（D6 校验对象与提交值同为 trim 后串）', async () => {
    const onCreated = vi.fn();
    render(<ChangeCreateDialog root={ROOT} onCreated={onCreated} />);
    openForm();

    submit('  fix-bug  ', '  带空白的目标文本  ');

    await screen.findByTestId('change-create-success');
    expect(createCalls()).toEqual([{ root: ROOT, name: 'fix-bug', goal: '带空白的目标文本' }]);
  });

  it('goal 多行 + emoji + 超 1000 字符原样透传不损（free-form UTF-8 前端半边 D7）', async () => {
    render(<ChangeCreateDialog root={ROOT} onCreated={vi.fn()} />);
    openForm();
    const goal = `第一行\n第二行 🚀 emoji\t制表符\n${'长'.repeat(1200)}`;

    submit('rich-goal', goal);

    await screen.findByTestId('change-create-success');
    expect(createCalls()).toEqual([{ root: ROOT, name: 'rich-goal', goal }]);
  });
});

describe('ChangeCreateDialog：必填与本地 kebab-case 校验（禁提交零 invoke）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
  });

  it('名称空或 goal 空白（"" / "   "）：提交按钮 disabled、点击零 invoke（必填拦截）', () => {
    const onCreated = vi.fn();
    render(<ChangeCreateDialog root={ROOT} onCreated={onCreated} />);
    openForm();

    // 名称空 + goal 空（初始态）
    expect(screen.getByTestId('change-create-submit').hasAttribute('disabled')).toBe(true);
    // 名称空 + goal 有值
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '只有 goal' },
    });
    expect(screen.getByTestId('change-create-submit').hasAttribute('disabled')).toBe(true);
    // 名称合法 + goal 空白（trim 后空）
    fireEvent.change(screen.getByTestId('change-create-name'), {
      target: { value: 'fix-bug' },
    });
    fireEvent.change(screen.getByTestId('change-create-goal'), {
      target: { value: '   ' },
    });
    expect(screen.getByTestId('change-create-submit').hasAttribute('disabled')).toBe(true);

    fireEvent.click(screen.getByTestId('change-create-submit'));
    expect(createCalls()).toHaveLength(0);
    expect(onCreated).not.toHaveBeenCalled();
  });

  it('非法 kebab-case 全族禁提交且零 invoke（本地与写面同口径校验，D3「本地放行 ⇒ 后端必过」）', () => {
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
      const view = render(<ChangeCreateDialog root={ROOT} onCreated={vi.fn()} />);
      openForm();
      submit(name, '合法 goal');

      expect(screen.getByTestId('change-create-submit').hasAttribute('disabled')).toBe(true);
      fireEvent.click(screen.getByTestId('change-create-submit'));
      view.unmount();
    }

    expect(createCalls()).toHaveLength(0);
  });

  it('恰 128 字符名称可提交且 invoke 发起（≤128 含端点，与写面上界同口径）', async () => {
    const onCreated = vi.fn();
    render(<ChangeCreateDialog root={ROOT} onCreated={onCreated} />);
    openForm();
    const name = `a${'b'.repeat(127)}`;
    expect(name.length).toBe(128);

    invokeMock.mockResolvedValue({ ...OUTCOME, name });
    submit(name, '上界内 goal');

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()[0]).toEqual({ root: ROOT, name, goal: '上界内 goal' });
    fireEvent.click(await screen.findByTestId('change-create-open-detail'));
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(name));
  });
});

describe('ChangeCreateDialog：后端错误行内呈现', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockRejectedValue('change "fix-bug" 已存在');
  });

  it('invoke reject：change-create-error 行内块呈现错误文本、onCreated 零调用', async () => {
    const onCreated = vi.fn();
    render(<ChangeCreateDialog root={ROOT} onCreated={onCreated} />);
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    const error = await screen.findByTestId('change-create-error');
    expect(error.textContent).toContain('已存在');
    expect(error.className).toContain('break-all');
    expect(onCreated).not.toHaveBeenCalled();
  });
});
