// Tiny char-by-char typewriter used by the hero intro.

export interface TypewriterOptions {
  text: string;
  el: HTMLElement;
  /** Characters per second. Defaults to 30. */
  cps?: number;
  /** Aborting the signal jumps the element to the full text and resolves. */
  signal?: AbortSignal;
}

export function typewrite(opts: TypewriterOptions): Promise<void> {
  const { text, el, cps = 30, signal } = opts;
  const delayMs = 1000 / cps;
  el.textContent = '';

  return new Promise<void>((resolve) => {
    if (signal?.aborted) {
      el.textContent = text;
      resolve();
      return;
    }

    let i = 0;
    let timer = 0;

    const onAbort = (): void => {
      window.clearTimeout(timer);
      el.textContent = text;
      resolve();
    };
    signal?.addEventListener('abort', onAbort, { once: true });

    const tick = (): void => {
      i += 1;
      el.textContent = text.slice(0, i);
      if (i >= text.length) {
        signal?.removeEventListener('abort', onAbort);
        resolve();
        return;
      }
      timer = window.setTimeout(tick, delayMs);
    };
    timer = window.setTimeout(tick, delayMs);
  });
}
