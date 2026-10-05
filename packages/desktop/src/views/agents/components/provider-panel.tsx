import { useState } from 'react';

import { Button } from '@/components/ui/button';

import type { AgentProviderRecord } from '../../../types/generated/bindings';
import type { AgentProvidersState } from '../hooks/use-agent-providers';
import { MaskedApiKey } from './masked-api-key';

interface ProviderFormState {
  id: number | null;
  name: string;
  baseUrl: string;
  apiKey: string;
  high: string;
  medium: string;
  low: string;
  contextLength: string;
}

const EMPTY_FORM: ProviderFormState = {
  id: null,
  name: '',
  baseUrl: '',
  apiKey: '',
  high: '',
  medium: '',
  low: '',
  contextLength: '',
};

/** 表单字段配置（映射驱动渲染；row 圈定换行分组） */
const PROVIDER_FIELDS: {
  row: 0 | 1 | 2;
  id: string;
  label: string;
  key: keyof ProviderFormState;
  placeholder?: string;
}[] = [
  { row: 0, id: 'provider-name', label: '名称', key: 'name' },
  {
    row: 0,
    id: 'provider-base-url',
    label: 'base_url',
    key: 'baseUrl',
    placeholder: 'https://…/v1',
  },
  { row: 1, id: 'provider-api-key', label: 'api_key', key: 'apiKey', placeholder: 'sk-…' },
  { row: 2, id: 'provider-model-high', label: 'model high', key: 'high' },
  { row: 2, id: 'provider-model-medium', label: 'model medium', key: 'medium' },
  { row: 2, id: 'provider-model-low', label: 'model low', key: 'low' },
  {
    row: 2,
    id: 'provider-context-length',
    label: 'context_length',
    key: 'contextLength',
    placeholder: '留空 = 跟随缺省 128K',
  },
];

/** context_length 保存边界 parse（单点）：空串 = 未配置（null）；正整数 =
 * 窗长 token 数；其余形态（非数字 / 零 / 负数 / 小数）undefined = 非法，
 * canSave 拒绝提交 */
function parseContextLength(value: string): number | null | undefined {
  const trimmed = value.trim();
  if (trimmed === '') return null;
  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed <= 0) return undefined;
  return parsed;
}

/** 编辑记录 → 表单初值（null = 新建；api_key 恒空，留空提交由后端回填原值；
 * contextLength 缺列 → 空串未配置语义） */
function toFormState(editing: AgentProviderRecord | null): ProviderFormState {
  return editing === null
    ? EMPTY_FORM
    : {
        id: editing.id,
        name: editing.name,
        baseUrl: editing.baseUrl,
        apiKey: '',
        high: editing.models.high,
        medium: editing.models.medium,
        low: editing.models.low,
        contextLength: editing.contextLength == null ? '' : String(editing.contextLength),
      };
}

/** 单行输入（就近表单控件：原生 input + 轨道同款描边样式） */
function TextField({
  id,
  label,
  value,
  placeholder,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}): React.JSX.Element {
  return (
    <span className="flex items-center gap-1.5 text-sm">
      <label className="whitespace-nowrap text-xs text-muted-foreground" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className="w-44 rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        data-testid={id}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </span>
  );
}

/** 表单动作行（取消 / 保存收口） */
function FormActions({
  busy,
  canSave,
  onCancel,
  onSave,
}: {
  busy: boolean;
  canSave: boolean;
  onCancel: () => void;
  onSave: () => void;
}): React.JSX.Element {
  return (
    <div className="mt-2.5 flex justify-end gap-2">
      <Button
        className="px-2 py-0.5 text-xs"
        data-testid="provider-cancel"
        disabled={busy}
        onClick={onCancel}
      >
        取消
      </Button>
      <Button
        className="px-2 py-0.5 text-xs"
        data-testid="provider-save"
        disabled={!canSave || busy}
        onClick={onSave}
      >
        保存
      </Button>
    </div>
  );
}

/** 表单字段行组（row 分组渲染；字段值 / 回写经 form 与 onField 穿透） */
function ProviderFormRows({
  form,
  onField,
}: {
  form: ProviderFormState;
  onField: (field: keyof ProviderFormState, value: string) => void;
}): React.JSX.Element {
  return (
    <>
      {[0, 1, 2].map((row) => (
        <div
          key={row}
          className={
            row === 0
              ? 'flex flex-wrap items-center gap-2'
              : 'mt-2 flex flex-wrap items-center gap-2'
          }
        >
          {PROVIDER_FIELDS.filter((field) => field.row === row).map((field) => (
            <TextField
              key={field.id}
              id={field.id}
              label={field.label}
              placeholder={field.placeholder}
              value={String(form[field.key])}
              onChange={(value) => onField(field.key, value)}
            />
          ))}
        </div>
      ))}
    </>
  );
}

/** 新建 / 编辑表单（自带表单态，editing null = 新建；保存成功经 onDone 收起，
 * 失败（重名等）由动作轨道 toast 呈现、表单保持；api_key 新建必填——无原值
 * 可保，编辑态留空 = 保持原值不拦） */
function ProviderForm({
  state,
  editing,
  onDone,
}: {
  state: AgentProvidersState;
  editing: AgentProviderRecord | null;
  onDone: () => void;
}): React.JSX.Element {
  const [form, setForm] = useState<ProviderFormState>(() => toFormState(editing));
  const [busy, setBusy] = useState(false);
  const submit = async (): Promise<void> => {
    const contextLength = parseContextLength(form.contextLength);
    if (contextLength === undefined) return; // 非法形态不提交（canSave 已拦，兜底）
    setBusy(true);
    const saved = await state.save({
      id: form.id,
      name: form.name,
      baseUrl: form.baseUrl,
      apiKey: form.apiKey,
      models: { high: form.high, medium: form.medium, low: form.low },
      contextLength,
    });
    setBusy(false);
    if (saved !== null) onDone();
  };
  const canSave =
    form.name.trim() !== '' &&
    form.baseUrl.trim() !== '' &&
    (form.id !== null || form.apiKey.trim() !== '') &&
    parseContextLength(form.contextLength) !== undefined;
  const onField = (field: keyof ProviderFormState, value: string) =>
    setForm((f) => ({ ...f, [field]: value }));
  return (
    <div className="mt-3 rounded-md border border-border px-2.5 py-2" data-testid="provider-form">
      <ProviderFormRows form={form} onField={onField} />
      <FormActions busy={busy} canSave={canSave} onCancel={onDone} onSave={() => void submit()} />
    </div>
  );
}

/** 清单（name / base_url / 三档 model / 遮蔽 key + 行内编辑 / 删除；删除被
 * 引用 reject 经动作轨道 toast 呈现，清单不变） */
function ProviderList({
  state,
  onEdit,
}: {
  state: AgentProvidersState;
  onEdit: (record: AgentProviderRecord) => void;
}): React.JSX.Element {
  return (
    <ul className="m-0 flex list-none flex-col gap-1.5 p-0" data-testid="provider-list">
      {state.providers.map((record) => (
        <li
          className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md border border-border px-2.5 py-1.5 text-sm"
          data-name={record.name}
          data-testid="provider-item"
          key={record.id}
        >
          <span className="font-medium">{record.name}</span>
          <span className="truncate text-xs text-muted-foreground">{record.baseUrl}</span>
          <span className="text-xs text-muted-foreground">
            high {record.models.high} / mid {record.models.medium} / low {record.models.low}
          </span>
          <MaskedApiKey apiKey={record.apiKey} />
          <span className="flex-1" />
          <Button
            className="px-2 py-0.5 text-xs"
            data-testid="provider-edit"
            onClick={() => onEdit(record)}
          >
            编辑
          </Button>
          <Button
            className="px-2 py-0.5 text-xs"
            data-testid="provider-delete"
            onClick={() => void state.remove(record.id)}
          >
            删除
          </Button>
        </li>
      ))}
      {state.providers.length === 0 && (
        <li className="text-muted-foreground" data-testid="provider-empty">
          暂无 provider，点「新建」创建。
        </li>
      )}
    </ul>
  );
}

/**
 * Providers 栏：清单（name / base_url / 三档 model / api_key 遮蔽展示）+
 * 新建 / 编辑表单 + 删除。api_key 遮蔽占位提示「留空保持原值」；保存 / 删除
 * 失败经 hook 动作轨道 toast 呈现（含重名 / 引用阻止 `Err`），清单不变；
 * 清单加载失败 inline error 态。
 */
export function ProviderPanel({ state }: { state: AgentProvidersState }): React.JSX.Element {
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<AgentProviderRecord | null>(null);

  const startEdit = (record: AgentProviderRecord) => {
    setEditing(record);
    setFormOpen(true);
  };
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="provider-panel"
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <h3 className="m-0 text-sm font-medium">Providers</h3>
        <Button
          data-testid="provider-new"
          onClick={() => {
            setEditing(null);
            setFormOpen(true);
          }}
        >
          新建
        </Button>
      </div>
      {state.error !== null && (
        <div
          className="mb-2 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="provider-error"
        >
          加载失败：{state.error}
        </div>
      )}
      {state.loading ? (
        <div className="text-muted-foreground" data-testid="provider-loading">
          加载中…
        </div>
      ) : (
        <ProviderList onEdit={startEdit} state={state} />
      )}
      {formOpen && (
        <ProviderForm
          editing={editing}
          key={editing?.id ?? 'new'}
          onDone={() => setFormOpen(false)}
          state={state}
        />
      )}
    </section>
  );
}
