import { useState } from 'react';

import { Button } from '@/components/ui/button';

import type { AgentPermissionMode, EngineKind } from '../../../types/dto';

/** 发起一次运行的入参（hooks 与表单共用；cwd 隐含当前 workspace root、model 不进 MVP，均无输入） */
export interface AgentStartInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
  /** 引擎选择（镜像生成绑定 `EngineKind`）：sdk 初始（与后端默认一致）/
   * cli 显式可选项；引擎选择仅调试页暴露，正式场景无选择入口 */
  engine: EngineKind;
}

export interface AgentRunFormProps {
  /** 已有运行进行中时禁用启动 */
  disabled: boolean;
  onStart: (input: AgentStartInput) => void;
}

const PERMISSION_OPTIONS: { value: AgentPermissionMode; label: string }[] = [
  { value: 'bypassPermissions', label: 'bypassPermissions' },
  { value: 'acceptEdits', label: 'acceptEdits' },
  { value: 'default', label: 'default' },
];

/** 引擎二值下拉清单（引擎选择仅调试页暴露——正式场景无选择入口、不传
 * engine 走后端默认 agent；cli 为显式可选项。sdk 引擎配置为硬编码预留位，
 * 未手填时启动以错误横幅显式失败） */
const ENGINE_OPTIONS: { value: EngineKind; label: string }[] = [
  { value: 'cli', label: 'cli' },
  { value: 'sdk', label: 'sdk' },
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

/** 启动工具行：引擎二值下拉 + permission-mode 档位 + 发起按钮 */
function StartToolbar({
  disabled,
  prompt,
  engine,
  permissionMode,
  onEngineChange,
  onPermissionModeChange,
  onStart,
}: {
  disabled: boolean;
  prompt: string;
  engine: EngineKind;
  permissionMode: AgentPermissionMode;
  onEngineChange: (value: EngineKind) => void;
  onPermissionModeChange: (value: AgentPermissionMode) => void;
  onStart: (input: AgentStartInput) => void;
}): React.JSX.Element {
  return (
    <div className="mb-3 flex flex-wrap items-center gap-4">
      <ModeSelect
        id="agent-engine"
        label="engine"
        options={ENGINE_OPTIONS}
        value={engine}
        onChange={onEngineChange}
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
        onClick={() => onStart({ prompt, permissionMode, engine })}
      >
        发起运行
      </Button>
    </div>
  );
}

/**
 * 参数面（最小集）：prompt 必填（空则禁用启动）、engine 引擎二值下拉初始
 * sdk（与后端硬编码默认一致；cli 为显式可选项——引擎选择仅调试页暴露，
 * 正式场景无选择入口；sdk 引擎直连 openai 兼容端点，配置为硬编码
 * 预留位——未手填时启动显式失败）、permission-mode 三档下拉默认
 * bypassPermissions（无头 default 档下需审批工具直接被拒，调试页以完整
 * 循环为默认）。bare 档不读 OAuth 凭据与系统 keychain，须
 * ANTHROPIC_API_KEY 等外部认证前提——开关旁固定提示。
 */
export function AgentRunForm({ disabled, onStart }: AgentRunFormProps): React.JSX.Element {
  const [prompt, setPrompt] = useState('');
  const [engine, setEngine] = useState<EngineKind>('sdk');
  const [permissionMode, setPermissionMode] = useState<AgentPermissionMode>('bypassPermissions');

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
        engine={engine}
        permissionMode={permissionMode}
        onEngineChange={setEngine}
        onPermissionModeChange={setPermissionMode}
        onStart={onStart}
      />
    </section>
  );
}
