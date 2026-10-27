import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, resetDrift } from '../../../lib/ipc-drift';
import { ChangeCreateDialog } from './change-create-dialog';

// 仅进程边界 mock
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = '/repo';

/** CreateOutcome fixture（与 Rust 写面 DTO 同形：id / name / created / worktree /
 * warnings 五字段；id 为 uuid v7 形态身份锚——成功回调携该值而非 name）。 */
const OUTCOME = {
  id: '0199a2f0-1c3d-7e45-8a9b-0d1e2f3a4b5c',
  name: 'fix-bug',
  created: '2026-10-02',
  worktree: 'C:\\home\\.dev-team\\worktrees\\repo-ab12\\fix-bug',
  warnings: [] as string[],
};

/** 挂载对话框（触发器 = children 文本按钮）。 */
function renderDialog(onCreated: (id: string) => void = vi.fn()): void {
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

  it('合法输入提交：invoke("create_change") 恰一次且载荷为 {root, name, goal, workflowType}；成功即关窗并触发 onCreated(outcome.id) 恰一次（id 身份锚直连导航）', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    // 计费面 / 导航面身份锚 = 铸出 id（name 恒展示面，不作寻址）
    expect(onCreated).toHaveBeenCalledWith(OUTCOME.id);
    expect(onCreated).not.toHaveBeenCalledWith(OUTCOME.name);
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'fix-bug', goal: '修复登录重试的竞态问题', workflowType: 'requirement' },
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
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'fix-bug', goal: '带空白的目标文本', workflowType: 'requirement' },
    ]);
  });

  it('goal 多行 + emoji + 超 1000 字符原样透传不损（free-form UTF-8 前端半边）', async () => {
    renderDialog();
    openForm();
    const goal = `第一行\n第二行 🚀 emoji\t制表符\n${'长'.repeat(1200)}`;

    submit('rich-goal', goal);

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'rich-goal', goal, workflowType: 'requirement' },
    ]);
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
      'nul', // Windows 保留设备名
      'con', // Windows 保留设备名
      'lpt0', // Windows 保留设备名（git 拒绝 LPT0）
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

  it('保留名近邻可提交（COM0 / nul-safe 不被误拦）', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();
    invokeMock.mockResolvedValue({ ...OUTCOME, name: 'nul-safe' });
    submit('nul-safe', '合法 goal');

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()[0]?.name).toBe('nul-safe');
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
    expect(createCalls()[0]).toEqual({
      root: ROOT,
      name,
      goal: '上界内 goal',
      workflowType: 'requirement',
    });
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(OUTCOME.id));
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

describe('ChangeCreateDialog：工作流类型选择器（AC-7）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
  });

  it('正向：弹窗内联类型选择器（change-create-workflow-type），两字面选项，默认 requirement', () => {
    renderDialog();
    openForm();

    const select = screen.getByTestId<HTMLSelectElement>('change-create-workflow-type');
    expect(select.value).toBe('requirement');
    expect(Array.from(select.querySelectorAll('option')).map((option) => option.value)).toEqual([
      'requirement',
      'test-only',
    ]);
    expect(screen.queryByTestId('change-create-error')).toBeNull();
  });

  it('正向：改选 test-only 后提交 → 四参载荷 workflowType 为 test-only', async () => {
    renderDialog();
    openForm();

    fireEvent.change(screen.getByTestId('change-create-workflow-type'), {
      target: { value: 'test-only' },
    });
    submit('only-tests', '给既有模块补测试用例');

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'only-tests', goal: '给既有模块补测试用例', workflowType: 'test-only' },
    ]);
  });

  it('正向：不改选时提交 → 四参载荷 workflowType 为默认 requirement', async () => {
    renderDialog();
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    await waitFor(() => expect(createCalls()).toHaveLength(1));
    expect(createCalls()[0].workflowType).toBe('requirement');
  });

  it('正向：成功后重置把类型归回默认 requirement（再开即新表单）', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    fireEvent.change(screen.getByTestId('change-create-workflow-type'), {
      target: { value: 'test-only' },
    });
    submit('only-tests', '补测试目标');
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByTestId('change-create-name')).toBeNull());

    openForm();
    expect(screen.getByTestId<HTMLSelectElement>('change-create-workflow-type').value).toBe(
      'requirement',
    );
  });

  it('边界：类型选择不影响本地必填 / kebab 校验语义（校验失败零 invoke）', () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    fireEvent.change(screen.getByTestId('change-create-workflow-type'), {
      target: { value: 'test-only' },
    });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));

    expect(createCalls()).toHaveLength(0);
    expect(onCreated).not.toHaveBeenCalled();
    expect(screen.getByTestId('change-create-error').textContent).toContain('校验不通过');

    // 非法 kebab 名称 + test-only 选择 → 仍零 invoke
    fireEvent.change(screen.getByTestId('change-create-name'), { target: { value: 'Fix_Bug' } });
    fireEvent.change(screen.getByTestId('change-create-goal'), { target: { value: '合法 goal' } });
    fireEvent.click(screen.getByRole('button', { name: '提交' }));
    expect(createCalls()).toHaveLength(0);
    expect(screen.getByTestId<HTMLSelectElement>('change-create-workflow-type').value).toBe(
      'test-only',
    );
  });

  it('异常：invoke reject（后端类型拒绝文案）→ 行内呈现、留窗、onCreated 零调用、选择器保留用户选择', async () => {
    invokeMock.mockRejectedValue(
      'workflow_type "bug-fix" 不受支持（支持 requirement / test-only）',
    );
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    const error = await screen.findByTestId('change-create-error');
    expect(error.textContent).toContain('不受支持');
    expect(screen.getByTestId('change-create-name')).toBeTruthy();
    expect(onCreated).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// 经 ipc 的参数面 / 结果面（AC-1 参数面 strict / AC-2 结果面 soft-report）：
// 对话框只换调用面（`commands.` → `ipc.`），参数面错本地拒、结果面脏只上报。
// 漂移断言用真实 collector（`driftCount()`），不以替身次数测上报。
// ---------------------------------------------------------------------------

describe('ChangeCreateDialog：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(OUTCOME);
    resetDrift();
  });

  it('正向：合法表单提交 → create_change 恰 1 次、参数面放行、成功后关窗回调照旧、零漂移', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    submit('fix-bug', '修复登录重试的竞态问题');

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(OUTCOME.id));
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'fix-bug', goal: '修复登录重试的竞态问题', workflowType: 'requirement' },
    ]);
    await waitFor(() => expect(screen.queryByTestId('change-create-name')).toBeNull());
    expect(driftCount()).toBe(0);
  });

  it('正向：workflowType 全部枚举值逐值提交 → 均放行、零 IpcArgsError、零漂移', async () => {
    for (const workflowType of ['requirement', 'test-only']) {
      invokeMock.mockClear();
      const onCreated = vi.fn();
      const view = render(
        <ChangeCreateDialog root={ROOT} onCreated={onCreated}>
          <button type="button">打开新建</button>
        </ChangeCreateDialog>,
      );
      openForm();
      fireEvent.change(screen.getByTestId('change-create-workflow-type'), {
        target: { value: workflowType },
      });
      submit('enum-value', '枚举取值验证');

      await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
      expect(createCalls()).toEqual([
        { root: ROOT, name: 'enum-value', goal: '枚举取值验证', workflowType },
      ]);
      expect(screen.queryByTestId('change-create-error')).toBeNull();
      view.unmount();
    }

    expect(driftCount()).toBe(0);
  });

  it('异常：后端 reject（重名 / 非法名）→ 既有 String(err) 行内呈现、弹窗不关、零漂移', async () => {
    for (const rejection of ['change "fix-bug" 已存在', '非法变更名']) {
      invokeMock.mockClear();
      invokeMock.mockRejectedValue(rejection);
      const onCreated = vi.fn();
      const view = render(
        <ChangeCreateDialog root={ROOT} onCreated={onCreated}>
          <button type="button">打开新建</button>
        </ChangeCreateDialog>,
      );
      openForm();
      submit('fix-bug', '合法 goal');

      const error = await screen.findByTestId('change-create-error');
      // 后端 reject 串语义不变：String(err) 逐字呈现（非 IpcArgsError 形态）
      expect(error.textContent).toBe(rejection);
      expect(screen.getByTestId('change-create-name') !== null).toBe(true);
      expect(onCreated).not.toHaveBeenCalled();
      view.unmount();
    }

    expect(driftCount()).toBe(0);
  });

  it('异常：结果面契约外（created 给数字）→ 漂移恰 1 条、既有成功回调照旧（漂移不阻断流程）', async () => {
    invokeMock.mockResolvedValue({ ...OUTCOME, created: 42 });
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    submit('drift-case', '结果面漂移验证');

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(OUTCOME.id));
    expect(driftCount()).toBe(1);
    // 软失败：错误块不出现、弹窗照旧关闭（漂移只上报）
    expect(screen.queryByTestId('change-create-error')).toBeNull();
    await waitFor(() => expect(screen.queryByTestId('change-create-name')).toBeNull());
  });

  it('边界：name 全空白 / goal 空串 → 既有表单前置校验拦截、零 invoke、零 IpcArgsError', () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();

    for (const [name, goal] of [
      ['   ', '合法 goal'],
      ['fix-bug', ''],
    ] as const) {
      fireEvent.change(screen.getByTestId('change-create-name'), { target: { value: name } });
      fireEvent.change(screen.getByTestId('change-create-goal'), { target: { value: goal } });
      fireEvent.click(screen.getByRole('button', { name: '提交' }));

      const error = screen.getByTestId('change-create-error');
      expect(error.textContent).toContain('校验不通过');
      // 前置校验先于 ipc：不出现参数面错误文案（IpcArgsError）
      expect(error.textContent).not.toContain('参数校验失败');
    }

    expect(createCalls()).toHaveLength(0);
    expect(onCreated).not.toHaveBeenCalled();
    expect(driftCount()).toBe(0);
  });

  it('边界：goal 超长（>1000 字符）→ 参数面放行（schema 只表达接受面）、invoke 恰 1 次', async () => {
    const onCreated = vi.fn();
    renderDialog(onCreated);
    openForm();
    const goal = `${'长'.repeat(1200)}\nemoji 🚀 结尾`;

    submit('long-goal', goal);

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(OUTCOME.id));
    expect(createCalls()).toEqual([
      { root: ROOT, name: 'long-goal', goal, workflowType: 'requirement' },
    ]);
    expect(driftCount()).toBe(0);
  });
});
