import { useEffect, useRef } from 'react';

import {
  Alert,
  CodeBlock,
  CodeBlockActions,
  CodeBlockContent,
  CodeBlockCopyButton,
  CodeBlockHeader,
  CodeBlockLine,
  CodeBlockTitle,
  Empty,
  EmptyContent,
  useHighlightedCode,
} from '@/components/ui';

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
  const lines = useHighlightedCode(rendered ?? '', 'ansi');

  useEffect(() => {
    const element = scrollRef.current;
    if (element !== null && stickRef.current) element.scrollTop = element.scrollHeight;
  }, [rendered]);

  return (
    <section className="flex h-full min-h-0 flex-col" data-testid="stdout-panel">
      {command !== null ? (
        <CodeBlock className="mb-2 shrink-0">
          <CodeBlockActions>
            <CodeBlockCopyButton value={command} data-testid="stdout-command-copy" />
          </CodeBlockActions>
          <CodeBlockContent className="max-h-40 scroll-fade-y" data-testid="stdout-command" wrap>
            {command}
          </CodeBlockContent>
        </CodeBlock>
      ) : (
        <Empty>
          <EmptyContent data-testid="stdout-command">（命令未记录）</EmptyContent>
        </Empty>
      )}

      {error !== null ? (
        <Alert variant="destructive" className="mb-2 break-all" data-testid="stdout-error">
          {error}
        </Alert>
      ) : running && empty ? (
        <div className="shrink-0 text-[13px] text-muted-foreground" data-testid="stdout-running">
          运行中，等待输出…
        </div>
      ) : !running && empty ? (
        <div className="shrink-0 text-[13px] text-muted-foreground" data-testid="stdout-empty">
          （无输出记录）
        </div>
      ) : (
        <CodeBlock className="min-h-0 flex-1">
          <CodeBlockHeader>
            <CodeBlockTitle>日志</CodeBlockTitle>
            <CodeBlockActions>
              <CodeBlockCopyButton value={rendered ?? ''} />
            </CodeBlockActions>
          </CodeBlockHeader>
          <CodeBlockContent
            ref={scrollRef}
            className="h-full"
            data-testid="stdout-text"
            onScroll={(event) => {
              stickRef.current = isStuckToBottom(event.currentTarget);
            }}
            wrap
          >
            {lines.map((line, index) => (
              <CodeBlockLine key={index}>{line}</CodeBlockLine>
            ))}
          </CodeBlockContent>
        </CodeBlock>
      )}
    </section>
  );
}
