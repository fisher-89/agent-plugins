import { useEffect, useState } from 'react';

import { Button } from '@/components/ui/button';

import type { AgentInstanceRecord, AgentPermissionMode } from '../../../types/generated/bindings';

/** 发起一次运行的入参（hooks 与表单共用；cwd 隐含当前 workspace root、model 不进本期，均无输入） */
export interface AgentStartInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
  /** agent 实例 id（null = 缺省——由后端解析默认 agent；explore 链恒 null） */
  agent: number | null;
}

export interface AgentRunFormProps {
  /** 已有运行进行中时禁用启动 */
  disabled: boolean;
  /** agent 实例清单（选择器选项数据面，来自 useAgentOptions） */
  agents: AgentInstanceRecord[];
  onStart: (input: AgentStartInput) => void;
}

const PERMISSION_OPTIONS: { value: AgentPermissionMode; label: string }[] = [
  { value: 'bypassPermissions', label: 'bypassPermissions' },
  { value: 'acceptEdits', label: 'acceptEdits' },
  { value: 'default', label: 'default' },
];

/** agent 选择器缺省选项值（select 值域为 string：'' = 缺省（后端解析默认
 * agent），数字串 = 显式实例 id） */
const DEFAULT_OPTION_VALUE = '';

/** agent 选择器：选项 = agent 实例清单（名称 + engine 标注），首项为缺省
 * （清单为空时仅存缺省项——发起走后端缺省解析，无默认经错误横幅显式报错）。
 * 清单守卫：仅接受已渲染 option 值，清单外程序值不回填不触发 onChange */
function AgentSelect({
  agents,
  value,
  onChange,
}: {
  agents: AgentInstanceRecord[];
  value: number | null;
  onChange: (value: number | null) => void;
}): React.JSX.Element {
  return (
    <span className="flex items-center gap-1.5 text-sm">
      <label className="text-xs text-muted-foreground" htmlFor="agent-select">
        agent
      </label>
      <select
        id="agent-select"
        className="rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm"
        data-testid="agent-select"
        value={value === null ? DEFAULT_OPTION_VALUE : String(value)}
        onChange={(e) => {
          const raw = e.target.value;
          if (raw !== DEFAULT_OPTION_VALUE && !agents.some((record) => String(record.id) === raw)) {
            return;
          }
          onChange(raw === DEFAULT_OPTION_VALUE ? null : Number(raw));
        }}
      >
        <option value={DEFAULT_OPTION_VALUE}>（默认 agent）</option>
        {agents.map((record) => (
          <option key={record.id} value={String(record.id)}>
            {`${record.name}（${record.engine}）`}
          </option>
        ))}
      </select>
    </span>
  );
}

/** 档位下拉：option 清单驱动，仅接受清单内的值（无类型断言） */
function ModeSelect<T extends string>({
  id,
  label,
  options,
  value,
  onChange,
}: {
  id: string;
  label: string;
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}): React.JSX.Element {
  return (
    <span className="flex items-center gap-1.5 text-sm">
      <label className="text-xs text-muted-foreground" htmlFor={id}>
        {label}
      </label>
      <select
        id={id}
        className="rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm"
        data-testid={id}
        value={value}
        onChange={(e) => {
          const next = options.find((option) => option.value === e.target.value);
          if (next) onChange(next.value);
        }}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </span>
  );
}

/** 启动工具行：agent 选择器 + permission-mode 档位 + 发起按钮 */
function StartToolbar({
  disabled,
  prompt,
  agents,
  agentId,
  permissionMode,
  onAgentChange,
  onPermissionModeChange,
  onStart,
}: {
  disabled: boolean;
  prompt: string;
  agents: AgentInstanceRecord[];
  agentId: number | null;
  permissionMode: AgentPermissionMode;
  onAgentChange: (value: number | null) => void;
  onPermissionModeChange: (value: AgentPermissionMode) => void;
  onStart: (input: AgentStartInput) => void;
}): React.JSX.Element {
  return (
    <div className="mb-3 flex flex-wrap items-center gap-4">
      <AgentSelect agents={agents} value={agentId} onChange={onAgentChange} />
      <ModeSelect
        id="agent-permission-mode"
        label="permission-mode"
        options={PERMISSION_OPTIONS}
        value={permissionMode}
        onChange={onPermissionModeChange}
      />
      <span className="flex-1" />
      <Button
        disabled={disabled}
        data-testid="agent-start"
        onClick={() => onStart({ prompt, permissionMode, agent: agentId })}
      >
        发起运行
      </Button>
    </div>
  );
}

/**
 * 参数面（最小集）：prompt 必填（空则禁用启动）、agent 选择器（选项 =
 * agent 实例清单，默认选中默认 agent；缺省项与空清单均可发起——走后端
 * 缺省解析，无默认 agent 经错误横幅显式报错）、permission-mode 三档下拉
 * 默认 bypassPermissions（无头 default 档下需审批工具直接被拒，调试页以
 * 完整循环为默认）。bare 档不读 OAuth 凭据与系统 keychain，须
 * ANTHROPIC_API_KEY 等外部认证前提——开关旁固定提示。
 */
export function AgentRunForm({ disabled, agents, onStart }: AgentRunFormProps): React.JSX.Element {
  const [prompt, setPrompt] = useState('');
  const [agentId, setAgentId] = useState<number | null>(null);
  const [permissionMode, setPermissionMode] = useState<AgentPermissionMode>('bypassPermissions');

  // 默认选中默认 agent（清单装载后自动落位；用户显式改动不被回写）
  const defaultId = agents.find((record) => record.isDefault)?.id ?? null;
  useEffect(() => {
    setAgentId((current) => (current === null && defaultId !== null ? defaultId : current));
  }, [defaultId]);

  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="agent-run-form"
    >
      <label className="mb-1 block text-xs text-muted-foreground" htmlFor="agent-prompt">
        用户输入
      </label>
      <textarea
        id="agent-prompt"
        className="mb-3 block min-h-16 w-full rounded-md border border-border bg-transparent px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        data-testid="agent-prompt"
        placeholder="要交给 agent 的任务描述…"
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
      />
      <StartToolbar
        disabled={disabled || prompt.trim().length === 0}
        prompt={prompt}
        agents={agents}
        agentId={agentId}
        permissionMode={permissionMode}
        onAgentChange={setAgentId}
        onPermissionModeChange={setPermissionMode}
        onStart={onStart}
      />
    </section>
  );
}
