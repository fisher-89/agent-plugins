import type { ConfigExtraField } from '../../../types/generated/bindings';

/**
 * 未知字段分区（passthrough 可见性即需求本体）：逐条 key + JSON 预览
 * （等宽块，对象 / 数组 / 标量 / null 多形态无损）；extra 为空整区不渲染。
 */
export function ExtraFieldsSection({
  extra,
}: {
  extra: ConfigExtraField[];
}): React.JSX.Element | null {
  if (extra.length === 0) {
    return null;
  }
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="config-extra"
    >
      <h2 className="m-0 mb-3 text-[15px]">未知字段</h2>
      <div className="flex flex-col gap-3">
        {extra.map((field, index) => (
          <div key={index}>
            <span className="font-mono text-xs text-muted-foreground">{field.key}</span>
            <pre className="m-0 mt-1 overflow-x-auto rounded-md bg-muted px-3 py-2 font-mono text-xs">
              {JSON.stringify(field.value, null, 2)}
            </pre>
          </div>
        ))}
      </div>
    </section>
  );
}
