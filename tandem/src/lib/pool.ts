// Bounded-concurrency task runner: at most `limit` tasks in flight at
// once, reporting progress as each settles. `shouldContinue` is polled
// before every task starts so a caller can stop enqueueing new work
// (e.g. the user picked a different repo) without waiting for
// in-flight tasks to unwind.

export async function runPool<T, R>(
  items: T[],
  limit: number,
  task: (item: T) => Promise<R>,
  onProgress?: (done: number, total: number) => void,
  shouldContinue?: () => boolean,
): Promise<R[]> {
  const results: R[] = new Array<R>(items.length);
  let next = 0;
  let done = 0;

  const worker = async () => {
    for (;;) {
      if (shouldContinue && !shouldContinue()) return;
      const i = next++;
      if (i >= items.length) return;
      // Safe: i < items.length was just checked.
      results[i] = await task(items[i] as T);
      done++;
      onProgress?.(done, items.length);
    }
  };

  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}
