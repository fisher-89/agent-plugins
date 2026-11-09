import { useEffect, useRef } from 'react';

import { CodeBlockCopyButton } from '@/components/ui/code-block';

import type { ChangeStepStatus } from '../../../types/dto';
import { useExecOutput } from '../hooks/use-exec-output';
import { isStuckToBottom, normalizeCarriageReturns } from './stdout-text';

interface StdoutPanelProps {
  /** workspace root（输出记录目录树解析锚） */
  root: string | null;
  /** change name（磁盘面寻址键——与 `record_exec_output` 同口径，非 id） */
  changeName: string;
  /** 输出记录引用（节点 `outputRef` = exec_id） */
  execId: string;
  /** 节点步状态（运行中 → watch 增量跟随；转终态 → 整读一次） */
  status: ChangeStepStatus | null;
}

/**
 * 脚本真实输出
 */
export function StdoutPanel({
  root,
  changeName,
  execId,
  status,
}: StdoutPanelProps): React.JSX.Element {
  const { command, text, error } = useExecOutput({ root, changeName, execId, status });
  const rendered = text === null ? null : normalizeCarriageReturns(text);
  const running = status === 'running';
  const empty = (text ?? '').trim() === '';
  const scrollRef = useRef<HTMLPreElement | null>(null);
  /** 粘底标志：初值 true（打开即跟随）；上滑即 false，回底恢复 true */
  const stickRef = useRef(true);

  useEffect(() => {
    const element = scrollRef.current;
    if (element !== null && stickRef.current) element.scrollTop = element.scrollHeight;
  }, [rendered]);

  return (
    <section className="flex h-full min-h-0 flex-col" data-testid="stdout-panel">
      <h3 className="m-0 mb-2 shrink-0 text-[13px] text-muted-foreground">执行输出</h3>
      <div className="mb-2 flex shrink-0 items-start gap-2">
        <pre
          className="m-0 min-w-0 flex-1 overflow-x-auto whitespace-pre-wrap break-all rounded-md bg-muted/40 p-2 text-[12px]"
          data-testid="stdout-command"
        >
          {command ?? '（命令未记录）'}
        </pre>
        {command !== null && (
          <CodeBlockCopyButton value={command} data-testid="stdout-command-copy" />
        )}
      </div>
      {error !== null ? (
        <div className="mb-2 shrink-0 break-all text-[13px] text-fail" data-testid="stdout-error">
          {error}
        </div>
      ) : running && empty ? (
        <div className="shrink-0 text-[13px] text-muted-foreground" data-testid="stdout-running">
          运行中，等待输出…
        </div>
      ) : !running && empty ? (
        <div className="shrink-0 text-[13px] text-muted-foreground" data-testid="stdout-empty">
          （无输出记录）
        </div>
      ) : (
        <pre
          ref={scrollRef}
          className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted/40 p-2 text-[12px]"
          data-testid="stdout-text"
          onScroll={(event) => {
            stickRef.current = isStuckToBottom(event.currentTarget);
          }}
        >
          {rendered}
        </pre>
      )}
    </section>
  );
}
