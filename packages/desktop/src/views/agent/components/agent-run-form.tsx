import { useEffect, useState } from 'react';

import { StandardSelect } from '@/components/standard/select';
import { Button } from '@/components/ui/button';
import { Field, FieldLabel } from '@/components/ui/field';

import type { AgentInstanceRecord, AgentPermissionMode } from '../../../types/generated/bindings';

/** 发起一次运行的入参（hooks 与表单共用；cwd 隐含当前 workspace root、model 不进本期，均无输入） */
export interface AgentStartInput {
  prompt: string;
  permissionMode: AgentPermissionMode | null;
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

/** agent 选择器 */
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
    <Field orientation="horizontal" className="w-fit">
      <FieldLabel>agent</FieldLabel>
      <StandardSelect
        id="agent-select"
        data-testid="agent-select"
        value={value}
        onChange={onChange}
        items={agents.map((record) => ({
          value: record.id,
          label: `${record.name}（${record.engine}）`,
        }))}
      />
    </Field>
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
  value: T | null;
  onChange: (value: T | null) => void;
}): React.JSX.Element {
  return (
    <Field orientation="horizontal" className="w-fit">
      <FieldLabel>{label}</FieldLabel>
      <StandardSelect
        id="agent-select"
        data-testid={id}
        value={value}
        onChange={onChange}
        items={options}
      />
    </Field>
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
  permissionMode: AgentPermissionMode | null;
  onAgentChange: (value: number | null) => void;
  onPermissionModeChange: (value: AgentPermissionMode | null) => void;
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

export function AgentRunForm({ disabled, agents, onStart }: AgentRunFormProps): React.JSX.Element {
  const [prompt, setPrompt] = useState('');
  const [agentId, setAgentId] = useState<number | null>(null);
  const [permissionMode, setPermissionMode] = useState<AgentPermissionMode | null>(
    'bypassPermissions',
  );

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
