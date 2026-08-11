export declare const COMMIT_TYPES: Record<string, string>;

export declare const MAX_SUBJECT_LENGTH: number;

export interface CommitMessageParts {
  skipped: boolean;
  emoji?: string;
  type?: string;
  scope?: string | null;
  breaking?: boolean;
  description?: string;
}

export function verifyCommitMessage(rawText: string): CommitMessageParts;
