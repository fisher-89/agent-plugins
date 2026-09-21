import type { ArtifactEnvelope } from '../types/dto';
import { resolveRenderer } from './registry';

/** 信封路由组件：按 resolveRenderer(kind) 渲染单个 ArtifactEnvelope */
export function ArtifactView({ envelope }: { envelope: ArtifactEnvelope }) {
  const Renderer = resolveRenderer(envelope.kind);
  return (
    <section className="artifact-card">
      <header>
        <span className="badge badge-kind">{envelope.kind}</span>
        <h3>{envelope.title}</h3>
        <span className="artifact-version">v{envelope.version}</span>
      </header>
      <Renderer envelope={envelope} />
    </section>
  );
}
