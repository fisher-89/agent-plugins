import { type DialogTriggerProps, type UseRenderRenderProp } from '@base-ui/react';
import { useCallback, useState } from 'react';

import { StandardDialog } from '@/components/standard/dailog';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Textarea } from '@/components/ui/textarea';
import { ipc } from '@/lib/ipc';

export interface ExploreCreateDialogProps {
  trigger: UseRenderRenderProp<DialogTriggerProps>;
  /** 当前 workspace root（扫描与建档入参） */
  root: string;
  /** 成功建档后回调（父层 refresh 清单并导航进详情）：`autoPrompt` = 新话题
   * 需求原文（导入路径恒 null） */
  onCreated: (name: string, autoPrompt: string | null) => void;
}

export function ExploreCreateDialog({
  trigger,
  root,
  onCreated,
}: ExploreCreateDialogProps): React.JSX.Element {
  const [requirement, setRequirement] = useState('');
  const [error, setError] = useState<string | null>(null);

  const onSubmit = useCallback(
    (close: () => void) => {
      const trimmed = requirement.trim();
      if (trimmed.length === 0) {
        setError('请输入探索需求');
        return;
      }
      setError(null);
      ipc
        .createExploreRecord(root, null)
        .then((record) => {
          setRequirement('');
          close();
          onCreated(record.name, trimmed);
        })
        .catch((err: unknown) => setError(String(err)));
    },
    [onCreated, requirement, root],
  );

  return (
    <StandardDialog
      title="新建探索"
      trigger={trigger}
      content={
        <Field>
          <FieldLabel>描述需求或问题</FieldLabel>
          <Textarea
            data-testid="explore-topic-requirement"
            value={requirement}
            onChange={(e) => setRequirement(e.target.value)}
            rows={6}
            placeholder="例如：给列表增加默认排序规则"
          />
          {error !== null && <FieldError data-testid="explore-create-error">{error}</FieldError>}
        </Field>
      }
      onSubmit={onSubmit}
    ></StandardDialog>
  );
}
