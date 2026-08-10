export interface SettledRequests<T> {
  values: T[];
  errors: unknown[];
}

export async function settleIndependentRequests<T>(
  requests: readonly Promise<T>[]
): Promise<SettledRequests<T>> {
  const outcomes = await Promise.allSettled(requests);
  const values: T[] = [];
  const errors: unknown[] = [];

  for (const outcome of outcomes) {
    if (outcome.status === 'fulfilled') values.push(outcome.value);
    else errors.push(outcome.reason);
  }

  return { values, errors };
}
