/**
 * 停摆态面板（全或无）：模型有**任何**结构错时，画布与元素树一起停摆，此处逐条
 * 呈现 `kind` + `message` + `file:line` 并提示「修好后再看」。
 *
 * 零图数据、零降级样式、零占位节点——残缺图的读法本身歧义（环画成什么样？悬空
 * 边画在哪？），而结构错都有精确行号，读文本比读畸形图快。
 */
import type { ArchiDiagnostic } from '../../../types/generated/bindings';

/** 位置串：行号未知（`line === 0`）时只渲文件。 */
function position(diagnostic: ArchiDiagnostic): string {
  if (diagnostic.line === 0) return diagnostic.file;
  return `${diagnostic.file}:${diagnostic.line}`;
}

export function StructureErrorPanel({
  diagnostics,
}: {
  diagnostics: ArchiDiagnostic[];
}): React.JSX.Element {
  return (
    <section
      className="rounded-lg border border-fail bg-fail-bg px-4 py-3"
      data-testid="archi-error-panel"
    >
      <h2 className="m-0 mb-2 text-[15px] text-fail">模型结构错（{diagnostics.length} 条）</h2>
      <div className="flex flex-col gap-2">
        {diagnostics.map((diagnostic, index) => (
          <div
            className="break-all text-[13px] text-fail"
            data-testid="archi-error-row"
            key={`${diagnostic.kind}-${position(diagnostic)}-${index}`}
          >
            <div className="font-medium">
              {diagnostic.kind} · {position(diagnostic)}
            </div>
            <pre className="m-0 whitespace-pre-wrap break-words font-mono text-xs">
              {diagnostic.message}
            </pre>
          </div>
        ))}
      </div>
      <div className="mt-2 text-xs text-muted-foreground">
        修好后再看：结构错未消除前不渲染任何图数据（零节点、零边、零降级样式）。
      </div>
    </section>
  );
}
