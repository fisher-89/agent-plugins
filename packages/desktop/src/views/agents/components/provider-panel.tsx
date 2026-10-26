import { Checkbox } from '@base-ui/react/checkbox';
import { type HTMLInputTypeAttribute, useState } from 'react';

import { StandardSelect } from '@/components/standard/select';
import { Button } from '@/components/ui/button';
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';

import type { AgentProviderProtocol, AgentProviderRecord } from '../../../types/generated/bindings';
import type { AgentProvidersState } from '../hooks/use-agent-providers';
import { MaskedApiKey } from './masked-api-key';

interface ProviderFormState {
  id: number | null;
  /** 模型协议（编辑态可改，与改 name / base_url 同权） */
  protocol: AgentProviderProtocol;
  name: string;
  baseUrl: string;
  apiKey: string;
  high: string;
  medium: string;
  low: string;
  contextLength: string;
  /** 输出上限输入态（空串 = 未配置 = 存 null；数字串须 >= 4096） */
  maxTokens: string;
  /** 云端 `web_search` 开关（布尔控件——MUST NOT 混入文本字段受控输入路径；
   * 仅 anthropic 协议渲染，切离即清零） */
  webSearch: boolean;
}

const EMPTY_FORM: ProviderFormState = {
  id: null,
  // 新建缺省协议 openai（后端缺省同源）
  protocol: 'openai',
  name: '',
  baseUrl: '',
  apiKey: '',
  high: '',
  medium: '',
  low: '',
  contextLength: '',
  maxTokens: '',
  webSearch: false,
};

/** `max_tokens` 下限（与引擎侧 `MIN_OUTPUT_TOKENS` 同值；后端为唯一权威，
 * 前端只拦非法提交） */
const MIN_OUTPUT_TOKENS = 4096;

/** 协议选项（线值与 bindings 枚举同源；复用标准选择器，不新增标准组件） */
const PROTOCOL_ITEMS: { value: AgentProviderProtocol; label: string }[] = [
  { value: 'openai', label: 'openai' },
  { value: 'anthropic', label: 'anthropic' },
];

/** `base_url` 提示随协议切换：协议决定地址语义（openai 需含版本段；anthropic
 * 的 `/v1` / `/messages` 后缀由引擎剥离后拼 `/v1/messages`） */
const BASE_URL_HINTS: Record<AgentProviderProtocol, { placeholder: string; hint: string }> = {
  openai: {
    placeholder: 'https://…/v1',
    hint: 'openai 协议：请填含版本段的端点（如 https://…/v1），引擎原样拼 /chat/completions',
  },
  anthropic: {
    placeholder: 'https://api.anthropic.com',
    hint: 'anthropic 协议：填 https://api.anthropic.com 即可，/v1 等后缀由引擎自动剥离后拼 /v1/messages',
  },
};

/** 文本输入字段键（协议走选择器、web_search 走勾选框，均不经文本行渲染） */
type ProviderTextField = Exclude<keyof ProviderFormState, 'id' | 'protocol' | 'webSearch'>;

/** 表单字段配置（映射驱动渲染；row 圈定换行分组） */
const PROVIDER_FIELDS: {
  id: string;
  label: string;
  key: ProviderTextField;
  type?: HTMLInputTypeAttribute;
  placeholder?: string;
}[] = [
  { id: 'provider-name', label: '名称', key: 'name' },
  {
    id: 'provider-base-url',
    label: 'base_url',
    key: 'baseUrl',
  },
  {
    id: 'provider-api-key',
    label: 'api_key',
    key: 'apiKey',
    placeholder: 'sk-…',
    type: 'password',
  },
  {
    id: 'provider-model-high',
    label: 'model high',
    key: 'high',
    placeholder: '用于设计、检视、决策等深度思考步骤',
  },
  {
    id: 'provider-model-medium',
    label: 'model medium',
    key: 'medium',
    placeholder: '暂未启用',
  },
  {
    id: 'provider-model-low',
    label: 'model low',
    key: 'low',
    placeholder: '用于编码、执行测试等快速实现步骤',
  },
  {
    id: 'provider-context-length',
    label: 'context_length',
    key: 'contextLength',
    placeholder: '留空 = 跟随缺省 128K',
  },
  {
    id: 'provider-max-tokens',
    label: 'max_tokens',
    key: 'maxTokens',
    placeholder: '留空 = 按协议缺省',
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

/** max_tokens 保存边界 parse（单点）：空串 = 未配置（null，存库同 null）；
 * 十进制整数串且 >= 4096 = 输出上限 token 数；其余形态（非数字 / 小数 /
 * 科学计数 / 负数 / 精度外超长串 / 低于下限）undefined = 非法，canSave 拒绝
 * 提交（MUST NOT 静默截断到边界，也不静默按浮点精度取整） */
function parseMaxTokens(value: string): number | null | undefined {
  const trimmed = value.trim();
  if (trimmed === '') return null;
  // 仅接受十进制整数串：`1.5` / `8e3` / `8192abc` / `-1` 一律非法
  if (!/^\d+$/.test(trimmed)) return undefined;
  const parsed = Number(trimmed);
  // 精度外（> Number.MAX_SAFE_INTEGER）非法，不静默截断
  if (!Number.isSafeInteger(parsed) || parsed < MIN_OUTPUT_TOKENS) return undefined;
  return parsed;
}

/** `[1m]` 后缀形态（大小写不敏感；解析规则权威在引擎侧 `parse_model_spec`，
 * 此处仅用于提示「窗长已被模型名覆盖」） */
const SUFFIX_1M = /\[1m\]$/i;

/** 模型名后缀提示：带后缀的档位与 `context_length` 同时在场时声明「已被覆盖」
 * （清除「两者叠加」的歧义） */
function suffixHint(form: ProviderFormState): string {
  const base =
    form.protocol === 'anthropic'
      ? '模型名可带 [1m] 后缀（只影响上下文窗长与 anthropic beta，不改输出上限）'
      : '';
  const suffixed = [
    { tier: 'high', value: form.high },
    { tier: 'medium', value: form.medium },
    { tier: 'low', value: form.low },
  ]
    .filter((entry) => SUFFIX_1M.test(entry.value.trim()))
    .map((entry) => entry.tier);
  if (suffixed.length === 0 || form.contextLength.trim() === '') return base;
  return `${base}；${suffixed.join(' / ')} 档窗长已被模型名覆盖（后缀优先于 context_length）`;
}

/** 编辑记录 → 表单初值（null = 新建；api_key 恒空，留空提交由后端回填原值；
 * contextLength / maxTokens 缺列 → 空串未配置语义；protocol 缺列 → 缺省
 * openai 档） */
function toFormState(editing: AgentProviderRecord | null): ProviderFormState {
  return editing === null
    ? EMPTY_FORM
    : {
        id: editing.id,
        protocol: editing.protocol ?? 'openai',
        name: editing.name,
        baseUrl: editing.baseUrl,
        apiKey: '',
        high: editing.models.high,
        medium: editing.models.medium,
        low: editing.models.low,
        contextLength: editing.contextLength == null ? '' : String(editing.contextLength),
        maxTokens: editing.maxTokens == null ? '' : String(editing.maxTokens),
        // 缺列 / 旧记录 → 缺省关（存量行行为零变化）
        webSearch: editing.webSearch ?? false,
      };
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

/** 协议选择器（标准组件复用；`null` 回调保持现值——选择器不允许清空） */
function ProtocolSelect({
  value,
  onChange,
}: {
  value: AgentProviderProtocol;
  onChange: (protocol: AgentProviderProtocol) => void;
}): React.JSX.Element {
  return (
    <Field>
      <FieldLabel htmlFor="provider-protocol">协议</FieldLabel>
      <StandardSelect
        id="provider-protocol"
        data-testid="provider-protocol"
        value={value}
        items={PROTOCOL_ITEMS}
        onChange={(next) => {
          if (next !== null) onChange(next);
        }}
      />
    </Field>
  );
}

/** 云端 web_search 勾选项（仅 anthropic 协议渲染）：布尔控件自带提示——按次
 * 计费语义在这里显式声明，不与文本字段的受控输入路径混用 */
function WebSearchField({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <Field className="w-auto">
      <FieldLabel htmlFor="provider-web-search">web_search</FieldLabel>
      <div className="flex items-center gap-2">
        <Checkbox.Root
          checked={checked}
          className="flex h-4 w-4 shrink-0 items-center justify-center rounded border border-border data-[checked]:bg-primary data-[checked]:text-primary-foreground"
          data-testid="provider-web-search"
          id="provider-web-search"
          onCheckedChange={(next) => onChange(next)}
        >
          <Checkbox.Indicator className="text-[10px] leading-none">✓</Checkbox.Indicator>
        </Checkbox.Root>
        <span className="text-xs text-muted-foreground">Anthropic 服务端网页搜索（按次计费）</span>
      </div>
    </Field>
  );
}

/** 表单字段行组（row 分组渲染；字段值 / 回写经 form 与 onField 穿透；row 0
 * 前置协议选择器——协议决定 base_url 语义；`web_search` 勾选项仅 anthropic
 * 协议在场） */
function ProviderFormRows({
  form,
  onField,
  onProtocol,
  onWebSearch,
}: {
  form: ProviderFormState;
  onField: (field: ProviderTextField, value: string) => void;
  onProtocol: (protocol: AgentProviderProtocol) => void;
  onWebSearch: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <FieldGroup>
      <ProtocolSelect onChange={onProtocol} value={form.protocol} />
      {form.protocol === 'anthropic' && (
        <WebSearchField checked={form.webSearch} onChange={onWebSearch} />
      )}
      {PROVIDER_FIELDS.map((field) => (
        <Field key={field.id} className="w-auto">
          <FieldLabel htmlFor={field.id}>{field.label}</FieldLabel>
          <Input
            id={field.id}
            data-testid={field.id}
            type={field.type}
            autoComplete={field.type === 'password' ? 'off' : 'on'}
            placeholder={
              field.key === 'baseUrl'
                ? BASE_URL_HINTS[form.protocol].placeholder
                : field.placeholder
            }
            value={String(form[field.key])}
            onChange={(e) => onField(field.key, e.target.value)}
          ></Input>
        </Field>
      ))}
    </FieldGroup>
  );
}

/** 字段提示区（三锚：base_url 随协议切换 / max_tokens 语义与下限 / 模型名后缀
 * 与「已被覆盖」判定） */
function ProviderHints({ form }: { form: ProviderFormState }): React.JSX.Element {
  return (
    <div className="mt-2 flex flex-col gap-0.5 text-xs text-muted-foreground">
      <span data-testid="provider-base-url-hint">{BASE_URL_HINTS[form.protocol].hint}</span>
      <span data-testid="provider-max-tokens-hint">
        max_tokens：输出上限，留空 = 按协议缺省；仅 anthropic 协议生效（openai
        协议请求保持省略上限）；下限 4096
      </span>
      <span data-testid="provider-suffix-hint">{suffixHint(form)}</span>
    </div>
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
    const maxTokens = parseMaxTokens(form.maxTokens);
    // 非法形态不提交（canSave 已拦，兜底）
    if (contextLength === undefined || maxTokens === undefined) return;
    setBusy(true);
    const saved = await state.save({
      id: form.id,
      name: form.name,
      protocol: form.protocol,
      baseUrl: form.baseUrl,
      apiKey: form.apiKey,
      models: { high: form.high, medium: form.medium, low: form.low },
      contextLength,
      maxTokens,
      // 恒携（openai 下为 false，不省略——省略会与「保持原值」混淆）
      webSearch: form.webSearch,
    });
    setBusy(false);
    if (saved !== null) onDone();
  };
  const canSave =
    form.name.trim() !== '' &&
    form.baseUrl.trim() !== '' &&
    (form.id !== null || form.apiKey.trim() !== '') &&
    parseContextLength(form.contextLength) !== undefined &&
    parseMaxTokens(form.maxTokens) !== undefined;
  const onField = (field: ProviderTextField, value: string) =>
    setForm((f) => ({ ...f, [field]: value }));
  // 切离 anthropic 就地清零（切回不自动恢复：开关是 provider 属性，需用户
  // 重新勾选——避免协议来回切换时静默恢复计费项）
  const onProtocol = (protocol: AgentProviderProtocol) =>
    setForm((f) => ({
      ...f,
      protocol,
      webSearch: protocol === 'anthropic' ? f.webSearch : false,
    }));
  const onWebSearch = (webSearch: boolean) => setForm((f) => ({ ...f, webSearch }));
  return (
    <div className="mt-3 rounded-md border border-border px-2.5 py-2" data-testid="provider-form">
      <ProviderFormRows
        form={form}
        onField={onField}
        onProtocol={onProtocol}
        onWebSearch={onWebSearch}
      />
      <ProviderHints form={form} />
      <FormActions busy={busy} canSave={canSave} onCancel={onDone} onSave={() => void submit()} />
    </div>
  );
}

/** 清单（name / 协议徽标 / base_url / 三档 model / 遮蔽 key + 行内编辑 /
 * 删除；删除被引用 reject 经动作轨道 toast 呈现，清单不变） */
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
          <span
            className="rounded border border-border px-1 text-xs text-muted-foreground"
            data-testid="provider-protocol-badge"
          >
            {record.protocol ?? 'openai'}
          </span>
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
 * Providers 栏：清单（name / 协议徽标 / base_url / 三档 model / api_key 遮蔽
 * 展示）+ 新建 / 编辑表单 + 删除。表单含协议选择器（`base_url` 提示与
 * placeholder 随协议切换）、可空 `context_length` / `max_tokens` 字段（留空 =
 * 未配置；`max_tokens` < 4096 拒绝提交）、模型名 `[1m]` 后缀提示（含「该档
 * 窗长已被覆盖」判定）。api_key 遮蔽占位提示「留空保持原值」；保存 / 删除
 * 失败经 hook 动作轨道 toast 呈现（含重名 / 引用阻止 `Err` / 非法后缀），清单
 * 不变；清单加载失败 inline error 态。
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
