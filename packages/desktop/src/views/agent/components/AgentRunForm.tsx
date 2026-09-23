import { useState } from 'react';

import { Button } from '@/components/ui/button';

import type { AgentEnvMode, AgentPermissionMode } from '../../../types/dto';

/** 发起一次运行的入参（hooks 与表单共用；cwd 隐含当前 workspace root、model 不进 MVP，均无输入） */
export interface AgentStartInput {
  prompt: string;
  env: AgentEnvMode;
  permissionMode: AgentPermissionMode;
}

export interface AgentRunFormProps {
  /** 已有运行进行中时禁用启动 */
  disabled: boolean;
  onStart: (input: AgentStartInput) => void;
}

const ENV_OPTIONS: { value: AgentEnvMode; label: string }[] = [
  { value: 'default', label: 'default（完整环境）' },
  { value: 'bare', label: 'bare（纯净）' },
];

const PERMISSION_OPTIONS: { value: AgentPermissionMode; label: string }[] = [
  { value: 'bypassPermissions', label: 'bypassPermissions' },
  { value: 'acceptEdits', label: 'acceptEdits' },
  { value: 'default', label: 'default' },
];

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

/** 启动工具行：env / permission-mode 档位 + 发起按钮 */
function StartToolbar({
  disabled,
  prompt,
  env,
  permissionMode,
  onEnvChange,
  onPermissionModeChange,
  onStart,
}: {
  disabled: boolean;
  prompt: string;
  env: AgentEnvMode;
  permissionMode: AgentPermissionMode;
  onEnvChange: (value: AgentEnvMode) => void;
  onPermissionModeChange: (value: AgentPermissionMode) => void;
  onStart: (input: AgentStartInput) => void;
}): React.JSX.Element {
  return (
    <div className="mb-3 flex flex-wrap items-center gap-4">
      <ModeSelect
        id="agent-env"
        label="环境"
        options={ENV_OPTIONS}
        value={env}
        onChange={onEnvChange}
      />
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
        onClick={() => onStart({ prompt, env, permissionMode })}
      >
        发起运行
      </Button>
    </div>
  );
}

/**
 * 参数面（最小集）：prompt 必填（空则禁用启动）、env 双档默认 default、
 * permission-mode 三档下拉默认 bypassPermissions（无头 default 档下需审批
 * 工具直接被拒，调试页以完整循环为默认）。bare 档不读 OAuth 凭据与系统
 * keychain，须 ANTHROPIC_API_KEY 等外部认证前提——开关旁固定提示。
 */
export function AgentRunForm({ disabled, onStart }: AgentRunFormProps): React.JSX.Element {
  const [prompt, setPrompt] = useState('');
  const [env, setEnv] = useState<AgentEnvMode>('default');
  const [permissionMode, setPermissionMode] = useState<AgentPermissionMode>('bypassPermissions');

  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="agent-run-form"
    >
      <label className="mb-1 block text-xs text-muted-foreground" htmlFor="agent-prompt">
        提示词
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
        env={env}
        permissionMode={permissionMode}
        onEnvChange={setEnv}
        onPermissionModeChange={setPermissionMode}
        onStart={onStart}
      />
      {env === 'bare' && (
        <div className="text-xs text-muted-foreground" data-testid="bare-auth-note">
          bare 档不读取 OAuth 凭据与系统 keychain：需设置 ANTHROPIC_API_KEY（或经 --settings 配
          apiKeyHelper），否则运行将认证失败。
        </div>
      )}
    </section>
  );
}
