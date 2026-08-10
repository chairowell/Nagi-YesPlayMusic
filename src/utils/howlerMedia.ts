/**
 * Isolate Howler's private HTMLMediaElement access for native seek and output.
 */
export interface HowlerMediaNode extends EventTarget {
  currentTime: number;
  seeking: boolean;
  error: MediaError | null;
  setSinkId?: (deviceId: string) => Promise<void>;
}

interface HowlerInternals {
  _sounds?: Array<{ _node?: unknown }>;
}

export function getHowlerMediaNode(howler: unknown): HowlerMediaNode | null {
  const node = (howler as HowlerInternals | null | undefined)?._sounds?.[0]
    ?._node;
  if (
    typeof node !== 'object' ||
    node === null ||
    !('addEventListener' in node) ||
    typeof node.addEventListener !== 'function'
  ) {
    return null;
  }
  return node as HowlerMediaNode;
}
