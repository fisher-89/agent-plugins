import { ArrowUpIcon, SquareIcon } from 'lucide-react';
import { type KeyboardEvent, useMemo, useState } from 'react';

import { StandardSelect } from '@/components/standard/select';
import { Button } from '@/components/ui/button';
import { Field, FieldLabel } from '@/components/ui/field';
import { Kbd, KbdGroup } from '@/components/ui/kbd';
import { Textarea } from '@/components/ui/textarea';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import type { AgentPermissionMode } from '@/types/dto';

export interface AgentInputProps {
  /** id 与 data-testid 前缀（如 explore → explore-prompt / explore-send） */
  idPrefix?: string;
  /** 输入框 label（缺省不呈现） */
  label?: string;
  /** 输入框 placeholder */
  placeholder?: string;
  /** 运行中：发送禁用 + 停止入口呈现 */
  running: boolean;
  /** 整面板禁用态（业务判据，如归档只读 / 流程运行中）：输入框与发送同时禁用，
   * 与 `running` 语义正交（缺省 false） */
  disabled?: boolean;
  /** 禁用理由文案（`disabled` 时呈于工具行上方；缺省不呈现） */
  disabledReason?: string;
  /** 隐藏 permission-mode 档位行（缺省 false）：`true` 时工具行不渲档位，
   * `onSend` 的 `permissionMode` 参数位保留但下游 MUST NOT 消费 */
  hidePermissionMode?: boolean;
  /** 在途提交态（缺省 false）：`true` 时发送禁用并呈「排队中…」可见文案
   * （防连点）；输入框与停止入口不受影响 */
  pending?: boolean;
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
  disabled,
  onChange,
}: {
  id: string;
  value: AgentPermissionMode;
  disabled: boolean;
  onChange: (value: AgentPermissionMode) => void;
}): React.JSX.Element {
  return (
    <Field orientation="horizontal">
      <FieldLabel>permission-mode</FieldLabel>
      <StandardSelect
        id={id}
        data-testid={id}
        value={value}
        disabled={disabled}
        onChange={(value) => {
          const next = PERMISSION_OPTIONS.find((option) => option.value === value);
          if (next) onChange(next.value);
        }}
        items={PERMISSION_OPTIONS}
      />
    </Field>
  );
}

/** 工具行：permission-mode 档位（可隐藏）+ 停止入口（运行中且未整面板禁用）+
 * 发送按钮 */
function ComposerToolbar({
  running,
  disabled,
  canSend,
  permissionMode,
  hidePermissionMode,
  sendId,
  stopId,
  modeId,
  onPermissionModeChange,
  onStop,
  onSend,
}: {
  running: boolean;
  disabled: boolean;
  canSend: boolean;
  permissionMode: AgentPermissionMode;
  hidePermissionMode: boolean;
  sendId: string;
  stopId: string;
  modeId: string;
  onPermissionModeChange: (value: AgentPermissionMode) => void;
  onStop: () => void;
  onSend: () => void;
}): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-center gap-4">
      {!hidePermissionMode && (
        <ModeSelect
          id={modeId}
          value={permissionMode}
          onChange={onPermissionModeChange}
          disabled={disabled}
        />
      )}
      <span className="flex-1" />

      {running && (
        <Button variant="default" size="icon" title="停止" data-testid={stopId} onClick={onStop}>
          <SquareIcon fill="currentColor" />
        </Button>
      )}
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              variant="default"
              size="icon"
              data-testid={sendId}
              disabled={!canSend}
              onClick={onSend}
            >
              <ArrowUpIcon />
            </Button>
          }
        />
        <TooltipContent>
          发送
          <KbdGroup>
            <Kbd>Ctrl</Kbd>
            <span>+</span>
            <Kbd>Enter</Kbd>
          </KbdGroup>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}

/**
 * composer：prompt 输入（原生 styled textarea）+ permission-mode 档位（默认
 * bypassPermissions）+ 发送（运行中禁发）+ 停止入口（运行中呈现）。发送后
 * 输入清空；档位状态本地持有。整面板禁用态（`disabled`）下 textarea 与发送
 * 同时禁用并呈理由文案——用户不会「打了字发不出去」。`hidePermissionMode` 与
 * `pending` 两位缺省即默认变体（渲染与交互逐字不变）。
 */
export function AgentInput({
  idPrefix = 'common',
  label,
  placeholder = '向Agent描述问题',
  running,
  disabled = false,
  disabledReason,
  hidePermissionMode = false,
  pending = false,
  onSend,
  onStop,
}: AgentInputProps): React.JSX.Element {
  const [prompt, setPrompt] = useState('');
  const [permissionMode, setPermissionMode] = useState<AgentPermissionMode>('bypassPermissions');

  const canSend = useMemo(
    () => !disabled && !pending && prompt.length > 0,
    [disabled, pending, prompt],
  );
  /** 发送收口：`text` 由调用方显式给出（快捷键路径取 textarea 原生值，避免读到
   * 尚未提交的闭包 state），点击发送与快捷键发送走同一条路径。 */
  const send = (text: string) => {
    onSend({ text, permissionMode });
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
      <InputArea
        idPrefix={idPrefix}
        placeholder={placeholder}
        disabled={disabled}
        canSend={canSend}
        value={prompt}
        setValue={setPrompt}
        onSend={send}
      />
      {disabled && disabledReason !== undefined && (
        <div
          className="mb-2 text-xs text-muted-foreground"
          data-testid={`${idPrefix}-disabled-reason`}
        >
          {disabledReason}
        </div>
      )}
      {pending && (
        <div className="mb-2 text-xs text-muted-foreground" data-testid={`${idPrefix}-pending`}>
          排队中…
        </div>
      )}
      <ComposerToolbar
        running={running}
        disabled={disabled}
        canSend={canSend}
        hidePermissionMode={hidePermissionMode}
        permissionMode={permissionMode}
        sendId={`${idPrefix}-send`}
        stopId={`${idPrefix}-stop`}
        modeId={`${idPrefix}-permission-mode`}
        onPermissionModeChange={setPermissionMode}
        onStop={onStop}
        onSend={() => send(prompt)}
      />
    </section>
  );
}

function InputArea({
  idPrefix,
  placeholder,
  disabled,
  canSend,
  value,
  setValue,
  onSend,
}: {
  idPrefix: string;
  placeholder: string;
  disabled: boolean;
  canSend: boolean;
  value: string;
  setValue: (value: string) => void;
  onSend: (text: string) => void;
}): React.JSX.Element {
  /** Ctrl+Enter 发送：文本取 textarea 的原生值。受控组件的 `value` 是上一次渲染
   * 的快照，同一事件回调内 `setValue` 尚未生效，读它只会拿到旧值。 */
  const submitByHotKey = ({ currentTarget, ctrlKey, key }: KeyboardEvent<HTMLTextAreaElement>) => {
    if (!canSend) {
      return;
    }
    if (key === 'Enter' && ctrlKey === true) {
      onSend(currentTarget.value);
    }
  };
  return (
    <Textarea
      id={`${idPrefix}-prompt`}
      data-testid={`${idPrefix}-prompt`}
      className="mb-2"
      placeholder={placeholder}
      value={value}
      disabled={disabled}
      onChange={(e) => setValue(e.target.value)}
      onKeyUp={submitByHotKey}
    />
  );
}
