import { Toaster as Sonner, type ToasterProps } from 'sonner';

/* shadcn Sonner vendored 内部化：去 next-themes 语境残留（本 app 固定 dark，无主题切换）。
   toastOptions.classNames 映射 app 既有 token（bg-card / text-foreground / border-border），
   theme="dark" 覆盖 token 类名够不到的 sonner 内部件（图标/关闭钮/spinner）；
   配色身份不引入 shadcn 默认主题（design D8）；位置/时长保持 sonner 默认。 */
function Toaster(props: ToasterProps): React.JSX.Element {
  return (
    <Sonner
      theme="dark"
      className="toaster group"
      toastOptions={{
        classNames: {
          toast:
            'group-[.toaster]:bg-card group-[.toaster]:text-foreground group-[.toaster]:border-border group-[.toaster]:shadow-lg',
        },
      }}
      {...props}
    />
  );
}

export { Toaster };
