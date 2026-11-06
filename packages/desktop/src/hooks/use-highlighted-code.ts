import * as React from 'react';

import { loadHighlighter } from '@/lib/highlighter';

type Highlighter = Awaited<ReturnType<typeof loadHighlighter>>;

interface Highlighted {
  highlighter: Highlighter;
  text: string;
  lines: React.ReactNode[];
  state?: Parameters<NonNullable<Highlighter>['highlight']>[1];
}

/** 按语言装载语法；未就绪或未登记的语言为 null */
function useLoadedHighlighter(language: string): Highlighter {
  const [loaded, setLoaded] = React.useState<{
    language: string;
    highlighter: Highlighter;
  } | null>(null);

  React.useEffect(() => {
    let cancelled = false;

    loadHighlighter(language)
      .then((next) => {
        if (!cancelled) {
          setLoaded({ language, highlighter: next });
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [language]);

  return loaded?.language === language ? loaded.highlighter : null;
}

export function useHighlightedCode(value: string, language: string) {
  // A trailing newline would render as an extra empty line.
  const code = value.endsWith('\n') ? value.slice(0, -1) : value;
  const highlighter = useLoadedHighlighter(language);

  // The complete lines, highlighted. While the code streams, only the new
  // lines are tokenized, starting from the grammar state of the last one.
  const [done, setDone] = React.useState<Highlighted>({ highlighter: null, text: '', lines: [] });

  let next = done;

  if (next.highlighter !== highlighter || !code.startsWith(next.text)) {
    next = { highlighter, text: '', lines: [] };
  }

  const end = code.lastIndexOf('\n') + 1;

  if (highlighter && end > next.text.length) {
    const result = highlighter.highlight(code.slice(next.text.length, end - 1), next.state);

    next = {
      highlighter,
      text: code.slice(0, end),
      lines: [...next.lines, ...result.lines],
      state: result.state,
    };
  }

  if (next !== done) {
    setDone(next);
  }

  // The last line is still being written, so it is tokenized on every render.
  const tail = code.slice(end);

  return React.useMemo(() => {
    if (!highlighter) {
      return code.split('\n');
    }

    return [...next.lines, highlighter.highlight(tail, next.state).lines[0]];
  }, [code, highlighter, next, tail]);
}
