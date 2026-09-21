import type { ArtifactEnvelope } from '../types/dto';

/**
 * 未注册 kind 的兜底组件（硬要求，永不白屏）：
 * 以 fallback_text 渲染并附 kind 徽标。
 */
export function Fallback({ envelope }: { envelope: ArtifactEnvelope }) {
  return (
    <div>
      <pre className="fallback-text">
        {envelope.fallbackText ?? '（该产物类型暂无渲染器，且无保底文本）'}
      </pre>
    </div>
  );
}
