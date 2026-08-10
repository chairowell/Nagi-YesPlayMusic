export interface MemoryStorage extends Storage {
  readonly values: Map<string, string>;
  has(key: string): boolean;
}

export function createMemoryStorage(
  initial: Readonly<Record<string, string>> = {}
): MemoryStorage {
  const values = new Map<string, string>(Object.entries(initial));
  return {
    get length() {
      return values.size;
    },
    values,
    clear() {
      values.clear();
    },
    getItem(key: string) {
      return values.get(key) ?? null;
    },
    key(index: number) {
      return [...values.keys()][index] ?? null;
    },
    removeItem(key: string) {
      values.delete(key);
    },
    setItem(key: string, value: string) {
      values.set(key, String(value));
    },
    has(key: string) {
      return values.has(key);
    },
  };
}

export function requireStoredItem(storage: Storage, key: string): string {
  const value = storage.getItem(key);
  if (value === null) throw new Error(`存储项不存在: ${key}`);
  return value;
}
