import { type DrawerTriggerState, type UseRenderRenderProp } from '@base-ui/react';
import { useEffect, useState } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import { Alert } from '@/components/ui/alert';
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from '@/components/ui/empty';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Spinner } from '@/components/ui/spinner';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { commands } from '@/types/generated/bindings';

import { StandardDrawer } from '../standard/drawer';

interface File {
  path: string;
  /** 文件类型（md/js/tsx/rs），非预制的类型显示文本；缺省按路径后缀推断 */
  type?: string;
}

type InnerFile = Required<File> & {
  name: string;
};

export interface FileViewerProps {
  root: string;
  files: File[];
  title?: string;
  trigger?: UseRenderRenderProp<DrawerTriggerState>;
}

const REMARK_PLUGINS = [remarkGfm];

/** 路径末段（`/` 与 `\` 两种分隔符都认）。 */
function fileName(path: string): string {
  return path.split(/[\\/]/).at(-1) ?? path;
}

/** 后缀（小写；无后缀或点开头的隐藏文件名 → 空串）。 */
function fileType(path: string): string {
  const name = fileName(path);
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}

/** 显式 `type` 优先，缺省取路径后缀；`name` 恒取路径末段。 */
function toInnerFiles(files: File[]): InnerFile[] {
  return files.map((file) => ({
    path: file.path,
    type: file.type?.toLowerCase() ?? fileType(file.path),
    name: fileName(file.path),
  }));
}

interface FileContentProps {
  root: string;
  path: string;
  type: string;
}

function FileContent({ root, path, type }: FileContentProps): React.JSX.Element {
  const [loading, setLoading] = useState(true);
  const [content, setContent] = useState<string | null>(null);
  const [binary, setBinary] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setContent(null);
    setBinary(false);
    setError(null);
    // 通用读面：`root` 与关联集校验都已退役，路径由调用方给出（可绝对 / 相对
    // root），服务端按文件系统直读
    const absolute =
      path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path)
        ? path
        : `${root.replace(/[\\/]+$/, '')}/${path.replace(/^[\\/]+/, '')}`;
    commands
      .readFile(absolute, null, null)
      .then((slice) => {
        if (cancelled) return;
        if (slice === null) {
          setError(`文件不存在：${path}`);
          setLoading(false);
          return;
        }
        setContent(slice.content);
        setBinary(slice.encoding === 'binary');
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setError(String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root, path]);

  if (loading) {
    return (
      <span
        className="flex items-center gap-2 text-muted-foreground"
        data-testid="file-content-loading"
      >
        加载中
        <Spinner />
      </span>
    );
  }
  if (error !== null) {
    return (
      <Alert variant="destructive" data-testid="file-content-error">
        {error}
      </Alert>
    );
  }
  if (binary) {
    return <Alert data-testid="file-content-binary">二进制文件，无法以文本呈现。</Alert>;
  }
  if (type === 'md') {
    return (
      <div
        className="prose prose-sm dark:prose-invert max-w-full"
        data-testid="file-content-markdown"
      >
        <Markdown remarkPlugins={REMARK_PLUGINS}>{content ?? ''}</Markdown>
      </div>
    );
  }
  return (
    <pre
      className="m-0 max-h-[60vh] overflow-auto rounded-md bg-muted/40 p-2 text-xs whitespace-pre-wrap wrap-break-word"
      data-testid="file-content-text"
    >
      {content ?? ''}
    </pre>
  );
}

export function FileViewer({
  root,
  files,
  title = '文档预览',
  trigger,
}: FileViewerProps): React.JSX.Element {
  const innerFiles = toInnerFiles(files);
  if (innerFiles.length === 0) {
    return (
      <Empty data-testid="file-viewer-empty">
        <EmptyHeader>
          <EmptyTitle>无文件可预览</EmptyTitle>
          <EmptyDescription>当前条目没有关联的文档。</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }
  // 首个文件默认激活；面板按需挂载（base-ui Tabs 缺省 keepMounted = false），
  // 故非激活页签的正文到切换时才取数
  const [first] = innerFiles;
  return (
    <StandardDrawer
      containerClassName="w-[960px] max-w-[85vw]"
      title={title}
      trigger={trigger}
      content={
        <Tabs defaultValue={first.path} orientation="vertical" className="h-full">
          <TabsList>
            {innerFiles.map(({ path, name }) => (
              <TabsTrigger key={path} value={path}>
                {name}
              </TabsTrigger>
            ))}
          </TabsList>
          <ScrollArea className="max-h-full mask-linear-to-b mask-[linear-gradient(to_bottom,black_calc(100%-var(--spacing)*16),transparent_100%)]">
            {innerFiles.map(({ path, type }) => (
              <TabsContent key={path} value={path}>
                <FileContent root={root} path={path} type={type} />
                <div className="w-full h-16"></div>
              </TabsContent>
            ))}
          </ScrollArea>
        </Tabs>
      }
    />
  );
}
