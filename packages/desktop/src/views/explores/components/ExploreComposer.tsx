import { useState } from 'react';

import type { AgentEnvMode, AgentPermissionMode } from '../../../types/dto';
import type { ExploreSendInput } from '../hooks/useExploreSession';

export interface ExploreComposerProps {
  /** 运行中禁用发送（防重复发送） */
  disabled: boolean;
  /** 发送上抛（stance 拼接与链续话由 session hook 承担） */
  onSend: (input: ExploreSendInput) => void;
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

/** 档位下拉：option 清单驱动，仅接受清单内的值（沿调试页 ModeSelect 语义） */
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

/** 工具行：env / permission-mode 档位 + 发送按钮 */
function ComposerToolbar({
  disabled,
  prompt,
  env,
  permissionMode,
  onEnvChange,
  onPermissionModeChange,
  onSend,
}: {
  disabled: boolean;
  prompt: string;
  env: AgentEnvMode;
  permissionMode: AgentPermissionMode;
  onEnvChange: (value: AgentEnvMode) => void;
  onPermissionModeChange: (value: AgentPermissionMode) => void;
  onSend: () => void;
}): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-center gap-4">
      <ModeSelect
        id="explore-env"
        label="环境"
        options={ENV_OPTIONS}
        value={env}
        onChange={onEnvChange}
      />
      <ModeSelect
        id="explore-permission-mode"
        label="permission-mode"
        options={PERMISSION_OPTIONS}
        value={permissionMode}
        onChange={onPermissionModeChange}
      />
      <span className="flex-1" />
      <button
        type="button"
        className="cursor-pointer rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground outline-none transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
        data-testid="explore-send"
        disabled={disabled || prompt.trim().length === 0}
        onClick={onSend}
      >
        发送
      </button>
    </div>
  );
}

/**
 * composer：prompt 输入（原生 styled textarea）+ env / permission-mode 档位
 * （默认 default + bypassPermissions，沿调试页档位语义与默认值）；bare 档
 * 认证前提固定提示。发送经 onSend 上抛。
 */
export function ExploreComposer({ disabled, onSend }: ExploreComposerProps): React.JSX.Element {
  const [prompt, setPrompt] = useState('');
  const [env, setEnv] = useState<AgentEnvMode>('default');
  const [permissionMode, setPermissionMode] = useState<AgentPermissionMode>('bypassPermissions');

  const send = () => {
    onSend({ prompt, env, permissionMode });
    setPrompt('');
  };

  return (
    <section
      className="rounded-lg border border-border bg-card px-3 py-2.5"
      data-testid="explore-composer"
    >
      <label className="mb-1 block text-xs text-muted-foreground" htmlFor="explore-prompt">
        探索输入
      </label>
      <textarea
        id="explore-prompt"
        className="mb-2 block min-h-16 w-full rounded-md border border-border bg-transparent px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        data-testid="explore-prompt"
        placeholder="想聊清楚什么？调研问题、思路、约束…"
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
      />
      <ComposerToolbar
        disabled={disabled}
        env={env}
        onEnvChange={setEnv}
        onPermissionModeChange={setPermissionMode}
        onSend={send}
        permissionMode={permissionMode}
        prompt={prompt}
      />
      {env === 'bare' && (
        <div className="mt-1.5 text-xs text-muted-foreground" data-testid="explore-bare-auth-note">
          bare 档不读取 OAuth 凭据与系统 keychain：需设置 ANTHROPIC_API_KEY（或经 --settings 配
          apiKeyHelper），否则运行将认证失败。
        </div>
      )}
    </section>
  );
}
