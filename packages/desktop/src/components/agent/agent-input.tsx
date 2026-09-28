/**
 * composer（agent 会话统一输入面）：prompt 输入 / permission-mode 三档下拉
 * （默认 bypassPermissions，沿调试页 ModeSelect 语义）/ 发送（运行中禁发）/
 * 停止入口（运行中呈现）。id 与 data-testid 经 `idPrefix` 前缀化，消费方
 * 保留各自的 testid 命名空间（如 explore-*）。
 */

import { useState } from 'react';

import type { AgentPermissionMode } from '../../types/dto';

export interface AgentInputProps {
  /** id 与 data-testid 前缀（如 explore → explore-prompt / explore-send） */
  idPrefix: string;
  /** 输入框 label（缺省不呈现） */
  label?: string;
  /** 输入框 placeholder */
  placeholder?: string;
  /** 运行中：发送禁用 + 停止入口呈现 */
  running: boolean;
  /** 发送上抛（链参数与 stance 组装由上游承担） */
  onSend: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  /** 停止上抛（触发 agent_stop） */
  onStop: () => void;
}

const PERMISSION_OPTIONS: { value: AgentPermissionMode; label: string }[] = [
  { value: 'bypassPermissions', label: 'bypassPermissions' },
  { value: 'acceptEdits', label: 'acceptEdits' },
  { value: 'default', label: 'default' },
];

/** 档位下拉：option 清单驱动，仅接受清单内的值（沿调试页 ModeSelect 语义） */
function ModeSelect({
  id,
  value,
  onChange,
}: {
  id: string;
  value: AgentPermissionMode;
  onChange: (value: AgentPermissionMode) => void;
}): React.JSX.Element {
  return (
    <span className="flex items-center gap-1.5 text-sm">
      <label className="text-xs text-muted-foreground" htmlFor={id}>
        permission-mode
      </label>
      <select
        id={id}
        className="rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm"
        data-testid={id}
        value={value}
        onChange={(e) => {
          const next = PERMISSION_OPTIONS.find((option) => option.value === e.target.value);
          if (next) onChange(next.value);
        }}
      >
        {PERMISSION_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </span>
  );
}

/** 工具行：permission-mode 档位 + 停止入口（运行中）+ 发送按钮 */
function ComposerToolbar({
  running,
  prompt,
  permissionMode,
  sendId,
  stopId,
  modeId,
  onPermissionModeChange,
  onStop,
  onSend,
}: {
  running: boolean;
  prompt: string;
  permissionMode: AgentPermissionMode;
  sendId: string;
  stopId: string;
  modeId: string;
  onPermissionModeChange: (value: AgentPermissionMode) => void;
  onStop: () => void;
  onSend: () => void;
}): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-center gap-4">
      <ModeSelect id={modeId} value={permissionMode} onChange={onPermissionModeChange} />
      <span className="flex-1" />
      {running && (
        <button
          type="button"
          className="cursor-pointer rounded-md border border-border bg-background px-4 py-2 text-sm font-medium text-foreground outline-none transition-colors hover:bg-muted"
          data-testid={stopId}
          onClick={onStop}
        >
          停止
        </button>
      )}
      <button
        type="button"
        className="cursor-pointer rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground outline-none transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
        data-testid={sendId}
        disabled={running || prompt.trim().length === 0}
        onClick={onSend}
      >
        发送
      </button>
    </div>
  );
}

/**
 * composer：prompt 输入（原生 styled textarea）+ permission-mode 档位（默认
 * bypassPermissions）+ 发送（运行中禁发）+ 停止入口（运行中呈现）。发送后
 * 输入清空；档位状态本地持有。
 */
export function AgentInput({
  idPrefix,
  label,
  placeholder,
  running,
  onSend,
  onStop,
}: AgentInputProps): React.JSX.Element {
  const [prompt, setPrompt] = useState('');
  const [permissionMode, setPermissionMode] = useState<AgentPermissionMode>('bypassPermissions');

  const send = () => {
    onSend({ text: prompt, permissionMode });
    setPrompt('');
  };

  return (
    <section
      className="rounded-lg border border-border bg-card px-3 py-2.5"
      data-testid={`${idPrefix}-composer`}
    >
      {label !== undefined && (
        <label className="mb-1 block text-xs text-muted-foreground" htmlFor={`${idPrefix}-prompt`}>
          {label}
        </label>
      )}
      <textarea
        id={`${idPrefix}-prompt`}
        className="mb-2 block min-h-16 w-full rounded-md border border-border bg-transparent px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
        data-testid={`${idPrefix}-prompt`}
        placeholder={placeholder}
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
      />
      <ComposerToolbar
        running={running}
        prompt={prompt}
        permissionMode={permissionMode}
        sendId={`${idPrefix}-send`}
        stopId={`${idPrefix}-stop`}
        modeId={`${idPrefix}-permission-mode`}
        onPermissionModeChange={setPermissionMode}
        onStop={onStop}
        onSend={send}
      />
    </section>
  );
}
