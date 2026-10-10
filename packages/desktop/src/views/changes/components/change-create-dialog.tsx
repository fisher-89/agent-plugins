import { useCallback, useState } from 'react';

import { StandardDialog } from '@/components/standard/dailog';
import { Field, FieldGroup, FieldLabel, FieldDescription } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';

import { commands } from '../../../types/generated/bindings';

export interface ChangeCreateDialogProps {
  children: React.JSX.Element;
  /** 当前 workspace root（建档入参） */
  root: string;
  /** 成功创建后回调（父层 refresh 清单并按铸出 id 导航进详情——`CreateOutcome.id`） */
  onCreated: (id: string) => void;
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
    <Field>
      <FieldLabel htmlFor="change-create-name-input">变更名</FieldLabel>
      <FieldDescription>kebab-case：小写字母 / 数字，以 `-` 连接，最长 128 字符</FieldDescription>
      <Input
        id="change-create-name-input"
        data-testid="change-create-name"
        placeholder="例如 fix-login-retry"
        autoComplete="false"
        required
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </Field>
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
    <Field>
      <FieldLabel htmlFor="change-create-goal-input">最初目标</FieldLabel>
      <FieldDescription>写入 explore.md，作为提案阶段的探索上下文</FieldDescription>
      <Textarea
        id="change-create-goal-input"
        data-testid="change-create-goal"
        rows={4}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="描述这次变更要解决的问题与预期结果"
        required
      />
    </Field>
  );
}

type CreateFormData = {
  name: string;
  goal: string;
};

function CreateForm({
  data,
  onChange,
  error,
}: {
  data: CreateFormData;
  onChange: (data: CreateFormData) => void;
  error: string | null;
}): React.JSX.Element {
  const setProp = <T extends keyof CreateFormData>(key: T) => {
    return (value: CreateFormData[T]) => onChange({ ...data, [key]: value });
  };

  return (
    <FieldGroup>
      <NameField onChange={setProp('name')} value={data.name} />
      <GoalField onChange={setProp('goal')} value={data.goal} />
      {error !== null && (
        <div
          className="mb-2 break-all rounded-md bg-fail-bg px-2 py-1.5 text-xs text-fail"
          data-testid="change-create-error"
        >
          {error}
        </div>
      )}
    </FieldGroup>
  );
}

export function ChangeCreateDialog({
  children,
  root,
  onCreated,
}: ChangeCreateDialogProps): React.JSX.Element {
  const [formData, setFormData] = useState<CreateFormData>({ name: '', goal: '' });
  const [error, setError] = useState<string | null>(null);

  const onSubmit = useCallback(
    (close: () => void) => {
      const { name, goal } = formData;
      if (!canSubmit(name, goal)) {
        setError('校验不通过');
        return;
      }
      const trimmedName = name.trim();
      const trimmedGoal = goal.trim();
      setError(null);
      commands
        .createChange(root, trimmedName, trimmedGoal)
        .then((result) => {
          onCreated(result.id);
          setFormData({ name: '', goal: '' });
          close();
        })
        .catch((err: unknown) => setError(String(err)));
    },
    [formData, onCreated, root],
  );
  return (
    <StandardDialog
      title="新建变更"
      trigger={children}
      content={<CreateForm data={formData} onChange={setFormData} error={error} />}
      onSubmit={onSubmit}
    ></StandardDialog>
  );
}
