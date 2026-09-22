import { Badge } from '@/components/ui/badge';

import type { ArtifactEnvelope } from '../types/dto';
import { resolveRenderer } from './registry';

/** 信封路由组件：按 resolveRenderer(kind) 渲染单个 ArtifactEnvelope */
export function ArtifactView({ envelope }: { envelope: ArtifactEnvelope }) {
  const Renderer = resolveRenderer(envelope.kind);
  return (
    <section
      className="mb-3 rounded-lg border border-border bg-card px-3.5 py-3"
      data-testid="artifact-card"
    >
      <header className="mb-2 flex items-center gap-2">
        <Badge variant="kind" data-testid="artifact-kind">
          {envelope.kind}
        </Badge>
        <h3 className="m-0 text-sm">{envelope.title}</h3>
        <span className="text-xs text-muted-foreground" data-testid="artifact-version">
          v{envelope.version}
        </span>
      </header>
      <Renderer envelope={envelope} />
    </section>
  );
}
