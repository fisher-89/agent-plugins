import { useExecOutput } from '../hooks/use-exec-output';

interface StdoutPanelProps {
  /** workspace root（输出记录目录树解析锚） */
  root: string | null;
  /** change name（磁盘面寻址键——与 `record_exec_output` 同口径，非 id） */
  changeName: string;
  /** 输出记录引用（节点 `outputRef` = exec_id） */
  execId: string;
}

/**
 * stdout 单列面板：脚本真实输出（static-check 的 `2>&1` 交错流 / test-execution
 * 的 stdout + stderr 合并流）单列 `<pre>` 呈现，完整还原终端文本。miss（未命中 /
 * 未取数）呈空态、invoke 拒绝呈错误态——不炸图、不阻断抽屉其余内容。
 */
export function StdoutPanel({ root, changeName, execId }: StdoutPanelProps): React.JSX.Element {
  const { text, loading, error } = useExecOutput({ root, changeName, execId });
  return (
    <section className="flex h-full min-h-0 flex-col" data-testid="stdout-panel">
      <h3 className="m-0 mb-2 shrink-0 text-[13px] text-muted-foreground">执行输出</h3>
      {loading && (
        <div
          className="mb-2 shrink-0 text-[13px] text-muted-foreground"
          data-testid="stdout-loading"
        >
          加载中…
        </div>
      )}
      {error !== null ? (
        <div className="mb-2 shrink-0 break-all text-[13px] text-fail" data-testid="stdout-error">
          {error}
        </div>
      ) : text === null ? (
        <div className="shrink-0 text-[13px] text-muted-foreground" data-testid="stdout-empty">
          （无输出记录）
        </div>
      ) : (
        <pre
          className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted/40 p-2 text-[12px]"
          data-testid="stdout-text"
        >
          {text}
        </pre>
      )}
    </section>
  );
}
