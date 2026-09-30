import type { ConfigDiagnostic, DiagnosticKind } from '../../../types/generated/bindings';

/** kind 视觉分级：invalidValue 强调（违例需注意），defaultApplied 弱化（吃默认为常规提示） */
function itemClass(kind: DiagnosticKind): string {
  return kind === 'invalidValue' ? 'text-fail' : 'text-muted-foreground';
}

/**
 * diagnostics 警示区（单列置顶）：逐条呈现 path + message（中文文案，含
 * 违例原值与所落默认值），kind 视觉分级可分；diagnostics 为空整区不渲染。
 */
export function DiagnosticsSection({
  diagnostics,
}: {
  diagnostics: ConfigDiagnostic[];
}): React.JSX.Element | null {
  if (diagnostics.length === 0) {
    return null;
  }
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="config-diagnostics"
    >
      <h2 className="m-0 mb-3 text-[15px]">配置诊断</h2>
      <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
        {diagnostics.map((diagnostic, index) => (
          <li
            className={itemClass(diagnostic.kind)}
            data-diagnostic-kind={diagnostic.kind}
            data-testid="config-diagnostic-item"
            key={index}
          >
            <span className="font-mono text-xs">{diagnostic.path}</span>
            <span>{` — ${diagnostic.message}`}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
