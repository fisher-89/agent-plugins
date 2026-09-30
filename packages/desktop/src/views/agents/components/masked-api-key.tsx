/**
 * api_key 遮蔽展示（`sk-***abc` 形态：末 3 字符；空 / 过短恒 `sk-***`）。
 *
 * 机密面有意放宽:api_key 后端全链路明文（IPC body / 全局库文件 / 进程内存），
 * 遮蔽只在本展示层（值本身仍随信封抵达前端），边界表见 specs/desktop-agent-management
 */
export function MaskedApiKey({ apiKey }: { apiKey: string }): React.JSX.Element {
  const masked = apiKey.length > 3 ? `sk-***${apiKey.slice(-3)}` : 'sk-***';
  return (
    <span className="font-mono text-xs text-muted-foreground" data-testid="masked-api-key">
      {masked}
    </span>
  );
}
