import { useCallback, useState } from 'react';

import { commands } from '../../../types/generated/bindings';

export interface ChangeCreateDialogProps {
  /** 当前 workspace root（建档入参） */
  root: string;
  /** 成功创建后回调（父层 refresh 清单并导航进详情） */
  onCreated: (name: string) => void;
}

/** 本地同口径 kebab-case 校验（与写面同一正则字面量，spec 权威） */
const KEBAB_CASE_PATTERN = /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/;

/** 名称长度上限（与写面同宽） */
const MAX_NAME_LENGTH = 128;

/** 名称合法 + goal 非空白才放行提交（校验对象与提交值同为 trim 后串，
 * 本地放行 ⇒ 写面必过） */
function canSubmit(name: string, goal: string): boolean {
  const trimmedName = name.trim();
  const trimmedGoal = goal.trim();
  return (
    KEBAB_CASE_PATTERN.test(trimmedName) &&
    trimmedName.length <= MAX_NAME_LENGTH &&
    trimmedGoal.length > 0
  );
}

/** 名称输入行：kebab-case 提示标签 + 单行输入 */
function NameField({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}): React.JSX.Element {
  return (
    <>
      <label
        className="mb-1 mt-2 block text-xs text-muted-foreground"
        htmlFor="change-create-name-input"
      >
        变更名（kebab-case：小写字母 / 数字，以 `-` 连接，最长 128 字符）
      </label>
      <input
        id="change-create-name-input"
        className="mb-2 block w-full rounded-md border border-border bg-transparent px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        data-testid="change-create-name"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="例如 fix-login-retry"
      />
    </>
  );
}

/** goal 输入行：必填提示标签 + 多行输入 */
function GoalField({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}): React.JSX.Element {
  return (
    <>
      <label
        className="mb-1 block text-xs text-muted-foreground"
        htmlFor="change-create-goal-input"
      >
        最初目标（必填；写入 explore.md，作为提案阶段的探索上下文）
      </label>
      <textarea
        id="change-create-goal-input"
        className="mb-2 block w-full rounded-md border border-border bg-transparent px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        data-testid="change-create-goal"
        rows={4}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="描述这次变更要解决的问题与预期结果"
      />
    </>
  );
}

/** 展开态表单：名称与 goal 提交前 trim；本地同口径校验不合法或 goal 空白
 * 时禁提交且不发起 invoke；成功回调 onCreated，后端错误 break-all 行内块 */
function CreateForm({
  root,
  onCreated,
}: Pick<ChangeCreateDialogProps, 'root' | 'onCreated'>): React.JSX.Element {
  const [name, setName] = useState('');
  const [goal, setGoal] = useState('');
  const [error, setError] = useState<string | null>(null);

  const create = () => {
    const trimmedName = name.trim();
    const trimmedGoal = goal.trim();
    setError(null);
    commands
      .createChange(root, trimmedName, trimmedGoal)
      .then(() => onCreated(trimmedName))
      .catch((err: unknown) => setError(String(err)));
  };

  return (
    <>
      <NameField onChange={setName} value={name} />
      <GoalField onChange={setGoal} value={goal} />
      {error !== null && (
        <div
          className="mb-2 break-all rounded-md bg-fail-bg px-2 py-1.5 text-xs text-fail"
          data-testid="change-create-error"
        >
          {error}
        </div>
      )}
      <button
        type="button"
        className="cursor-pointer rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground outline-none transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
        data-testid="change-create-submit"
        disabled={!canSubmit(name, goal)}
        onClick={create}
      >
        创建
      </button>
    </>
  );
}

/** 新建对话框：toggle 展开（沿 explore-create-dialog 折叠卡片先例），折叠态
 * toggle 常驻；展开态表单见 CreateForm（名称 / goal 必填 + 本地校验） */
export function ChangeCreateDialog({
  root,
  onCreated,
}: ChangeCreateDialogProps): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const toggle = useCallback(() => setOpen((prev) => !prev), []);

  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="change-create-dialog"
    >
      <div className="flex items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">新建变更</h2>
        <button
          type="button"
          className="cursor-pointer rounded-md border border-border bg-transparent px-3 py-1.5 text-sm hover:bg-muted"
          data-testid="change-create-toggle"
          onClick={toggle}
        >
          {open ? '收起' : '新建'}
        </button>
      </div>
      {open && <CreateForm onCreated={onCreated} root={root} />}
    </section>
  );
}
