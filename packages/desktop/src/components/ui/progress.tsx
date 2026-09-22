import * as ProgressPrimitive from '@radix-ui/react-progress';

import { cn } from '@/lib/utils';

/* shadcn Progress vendored 内部化（radix Root/Track/Indicator 封装）：
   value 承载百分比（0-100，null 按无进度处理），取代原 .progress-track/.progress-fill
   的内联 width 样式。轨道刻度对齐原 .progress-track（h-3 / rounded-md / bg-background）。 */
function Progress({
  className,
  value,
  ...props
}: React.ComponentProps<typeof ProgressPrimitive.Root>): React.JSX.Element {
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      className={cn('relative h-3 w-full overflow-hidden rounded-md bg-background', className)}
      value={value}
      {...props}
    >
      <ProgressPrimitive.Indicator
        data-slot="progress-indicator"
        className="h-full w-full flex-1 bg-primary transition-all"
        style={{ transform: `translateX(-${100 - (value ?? 0)}%)` }}
      />
    </ProgressPrimitive.Root>
  );
}

export { Progress };
