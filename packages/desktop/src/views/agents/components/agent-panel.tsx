import { useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type {
  AgentEngineKind,
  AgentInstanceRecord,
  AgentProviderRecord,
} from '../../../types/generated/bindings';
import type { AgentInstancesState } from '../hooks/use-agent-instances';

/** 表单态（默认标记不由表单写：新建恒非默认、更新保留存量标记，见 store） */
interface AgentFormState {
  id: number | null;
  name: string;
  engine: AgentEngineKind;
  providerId: number | null;
}

const EMPTY_FORM: AgentFormState = { id: null, name: '', engine: 'sdk', providerId: null };

const ENGINE_OPTIONS: { value: AgentEngineKind; label: string }[] = [
  { value: 'sdk', label: 'sdk' },
  { value: 'cli', label: 'cli' },
];

/** 编辑记录 → 表单初值（null = 新建，engine 初始 sdk） */
function toFormState(editing: AgentInstanceRecord | null): AgentFormState {
  return editing === null
    ? EMPTY_FORM
    : {
        id: editing.id,
        name: editing.name,
        engine: editing.engine,
        providerId: editing.providerId,
      };
}

/** 引擎二值下拉（就地表单控件：原生 select + 轨道同款描边样式） */
function EngineSelect({
  value,
  onChange,
}: {
  value: AgentEngineKind;
  onChange: (value: AgentEngineKind) => void;
}): React.JSX.Element {
  return (
    <span className="flex items-center gap-1.5 text-sm">
      <label className="text-xs text-muted-foreground" htmlFor="agent-engine-kind">
        engine
      </label>
      <select
        id="agent-engine-kind"
        className="rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm"
        data-testid="agent-engine-kind"
        value={value}
        onChange={(e) => {
          const next = ENGINE_OPTIONS.find((option) => option.value === e.target.value);
          if (next) onChange(next.value);
        }}
      >
        {ENGINE_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </span>
  );
}

/** provider 选择（cli 档禁用——provider 可空；sdk 档必选，未选前端禁提交） */
function ProviderSelect({
  providers,
  engine,
  value,
  onChange,
}: {
  providers: AgentProviderRecord[];
  engine: AgentEngineKind;
  value: number | null;
  onChange: (value: number | null) => void;
}): React.JSX.Element {
  return (
    <span className="flex items-center gap-1.5 text-sm">
      <label className="text-xs text-muted-foreground" htmlFor="agent-provider-select">
        provider
      </label>
      <select
        id="agent-provider-select"
        className="rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm disabled:opacity-50"
        data-testid="agent-provider-select"
        disabled={engine === 'cli'}
        value={value === null ? '' : String(value)}
        onChange={(e) => onChange(e.target.value === '' ? null : Number(e.target.value))}
      >
        <option value="">（未选择）</option>
        {providers.map((provider) => (
          <option key={provider.id} value={String(provider.id)}>
            {provider.name}
          </option>
        ))}
      </select>
    </span>
  );
}

/** 表单字段行（名称 + engine 二值 + provider 选择；回写经 onPatch 合并） */
function AgentFormFields({
  form,
  providers,
  onPatch,
}: {
  form: AgentFormState;
  providers: AgentProviderRecord[];
  onPatch: (patch: Partial<AgentFormState>) => void;
}): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="flex items-center gap-1.5 text-sm">
        <label className="text-xs text-muted-foreground" htmlFor="agent-name">
          名称
        </label>
        <input
          id="agent-name"
          className="w-44 rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm outline-none focus:ring-1 focus:ring-ring"
          data-testid="agent-name"
          value={form.name}
          onChange={(e) => onPatch({ name: e.target.value })}
        />
      </span>
      <EngineSelect onChange={(engine) => onPatch({ engine })} value={form.engine} />
      <ProviderSelect
        engine={form.engine}
        onChange={(providerId) => onPatch({ providerId })}
        providers={providers}
        value={form.providerId}
      />
    </div>
  );
}

/** 表单动作行（取消 / 保存收口；sdk 未选 provider 前端禁提交——canSave 承载） */
function AgentFormActions({
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
        data-testid="agent-cancel"
        disabled={busy}
        onClick={onCancel}
      >
        取消
      </Button>
      <Button
        className="px-2 py-0.5 text-xs"
        data-testid="agent-save"
        disabled={!canSave || busy}
        onClick={onSave}
      >
        保存
      </Button>
    </div>
  );
}

/** 新建 / 编辑表单（自带表单态；保存失败经动作轨道 toast 呈现、表单保持；
 * sdk 缺 provider / 悬空引用 / 重名由后端 store 单点校验 reject） */
function AgentForm({
  state,
  providers,
  editing,
  onDone,
}: {
  state: AgentInstancesState;
  providers: AgentProviderRecord[];
  editing: AgentInstanceRecord | null;
  onDone: () => void;
}): React.JSX.Element {
  const [form, setForm] = useState<AgentFormState>(() => toFormState(editing));
  const [busy, setBusy] = useState(false);
  const submit = async (): Promise<void> => {
    setBusy(true);
    const saved = await state.save({
      id: form.id,
      name: form.name,
      engine: form.engine,
      providerId: form.providerId,
    });
    setBusy(false);
    if (saved !== null) onDone();
  };
  const canSave = form.name.trim() !== '' && (form.engine === 'cli' || form.providerId !== null);
  return (
    <div className="mt-3 rounded-md border border-border px-2.5 py-2" data-testid="agent-form">
      <AgentFormFields
        form={form}
        onPatch={(patch) => setForm((f) => ({ ...f, ...patch }))}
        providers={providers}
      />
      <AgentFormActions
        busy={busy}
        canSave={canSave}
        onCancel={onDone}
        onSave={() => void submit()}
      />
    </div>
  );
}

/** 引用 provider 展示名（悬空引用兜底 #id——删除阻止下不应出现） */
function providerName(providers: AgentProviderRecord[], providerId: number | null): string {
  if (providerId === null) return '—';
  return providers.find((provider) => provider.id === providerId)?.name ?? `#${providerId}`;
}

/** 默认标记单元：已默认呈徽标，未默认呈「设为默认」切换入口（标记即切换，
 * 旧默认自动清除） */
function DefaultMark({ record }: { record: AgentInstanceRecord }): React.JSX.Element | null {
  if (record.isDefault) {
    return (
      <Badge variant="default" data-testid="agent-default-badge">
        默认
      </Badge>
    );
  }
  return null;
}

/** 清单行（name / engine / 引用 provider 名 / 默认标记 + 行内编辑 / 删除） */
function AgentListItem({
  record,
  providers,
  onEdit,
  onSetDefault,
  onRemove,
}: {
  record: AgentInstanceRecord;
  providers: AgentProviderRecord[];
  onEdit: (record: AgentInstanceRecord) => void;
  onSetDefault: (id: number) => void;
  onRemove: (id: number) => void;
}): React.JSX.Element {
  return (
    <li
      className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md border border-border px-2.5 py-1.5 text-sm"
      data-name={record.name}
      data-testid="agent-item"
    >
      <span className="font-medium">{record.name}</span>
      <span className="text-xs text-muted-foreground">{record.engine}</span>
      <span className="text-xs text-muted-foreground">
        provider: {providerName(providers, record.providerId)}
      </span>
      <DefaultMark record={record} />
      <span className="flex-1" />
      <Button
        size="sm"
        variant="secondary"
        data-testid="agent-set-default"
        disabled={record.isDefault}
        onClick={() => onSetDefault(record.id)}
      >
        设为默认
      </Button>
      <Button size="sm" variant="secondary" data-testid="agent-edit" onClick={() => onEdit(record)}>
        编辑
      </Button>
      <Button
        size="sm"
        variant="secondary"
        data-testid="agent-delete"
        onClick={() => onRemove(record.id)}
      >
        删除
      </Button>
    </li>
  );
}

/** 清单（默认 agent 删除后全局无默认——无顺延；删除失败 toast 呈现清单不变） */
function AgentList({
  state,
  providers,
  onEdit,
}: {
  state: AgentInstancesState;
  providers: AgentProviderRecord[];
  onEdit: (record: AgentInstanceRecord) => void;
}): React.JSX.Element {
  return (
    <ul className="m-0 flex list-none flex-col gap-1.5 p-0" data-testid="agent-list">
      {state.instances.map((record) => (
        <AgentListItem
          key={record.id}
          onEdit={onEdit}
          onRemove={(id) => void state.remove(id)}
          onSetDefault={(id) => void state.setDefault(id)}
          providers={providers}
          record={record}
        />
      ))}
      {state.instances.length === 0 && (
        <li className="text-muted-foreground" data-testid="agent-empty">
          暂无 agent，点「新建」创建。
        </li>
      )}
    </ul>
  );
}

/** 状态面：清单加载失败 inline error / loading 行（动作轨道失败走 toast，
 * 不置 error 态，不经本面） */
function AgentStatusFaces({ state }: { state: AgentInstancesState }): React.JSX.Element | null {
  if (state.error !== null) {
    return (
      <div
        className="mb-2 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
        data-testid="agent-error"
      >
        加载失败：{state.error}
      </div>
    );
  }
  if (!state.loading) return null;
  return (
    <div className="text-muted-foreground" data-testid="agent-loading">
      加载中…
    </div>
  );
}

/**
 * Agents 栏：清单（name / engine / 引用 provider 名 / 默认标记）+ 新建 /
 * 编辑表单（engine 二值下拉 + provider 选择，sdk 必选）+ 默认标记切换
 * （标记即切换，旧默认自动清除）+ 删除（默认 agent 删除后全局无默认）。
 * 保存 / 删除 / 设默认失败经 hook 动作轨道 toast 呈现（含 sdk 缺 provider /
 * 悬空引用 / 重名 `Err`），清单不变；清单加载失败 inline error 态。
 */
export function AgentPanel({
  state,
  providers,
}: {
  state: AgentInstancesState;
  providers: AgentProviderRecord[];
}): React.JSX.Element {
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<AgentInstanceRecord | null>(null);

  const startCreate = () => {
    setEditing(null);
    setFormOpen(true);
  };
  const startEdit = (record: AgentInstanceRecord) => {
    setEditing(record);
    setFormOpen(true);
  };
  const faces = <AgentStatusFaces state={state} />;
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="agent-panel"
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <h3 className="m-0 text-sm font-medium">Agents</h3>
        <Button data-testid="agent-new" onClick={startCreate}>
          新建
        </Button>
      </div>
      {faces}
      {state.loading ? null : <AgentList onEdit={startEdit} providers={providers} state={state} />}
      {formOpen && (
        <AgentForm
          editing={editing}
          key={editing?.id ?? 'new'}
          onDone={() => setFormOpen(false)}
          providers={providers}
          state={state}
        />
      )}
    </section>
  );
}
