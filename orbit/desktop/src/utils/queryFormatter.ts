/**
 * Query formatting, hardened against regex denial-of-service.
 *
 * Extracted from `QueryEditor` so the safety tests exercise the code the editor
 * actually runs. Previously the test file carried its own copy of this logic,
 * which meant a change here could not fail a test.
 */

/** Inputs above this are rejected rather than formatted. */
export const MAX_QUERY_SIZE = 1024 * 100;

/** Wall-clock budget for one format call. */
export const DEFAULT_FORMAT_TIMEOUT_MS = 5000;

/** Keywords placed on their own line, applied one at a time so the pattern
 *  never contains alternation that could backtrack. */
const LINE_BREAK_KEYWORDS = [
  'SELECT',
  'FROM',
  'WHERE',
  'JOIN',
  'GROUP BY',
  'HAVING',
  'ORDER BY',
  'LIMIT',
] as const;

/**
 * Format a SQL-like statement.
 *
 * Every pattern here is linear-time: no nested quantifiers and no alternation
 * over user input. The size cap and the elapsed-time check between passes are
 * belt-and-braces in case a future edit introduces one.
 *
 * Note that clause keywords are matched case-insensitively and replaced with
 * their canonical spelling, so `select` comes back as `SELECT`.
 *
 * @throws If `input` exceeds {@link MAX_QUERY_SIZE}, or formatting runs past
 *         `timeoutMs`.
 */
export const safeFormatQuery = (
  input: string,
  timeoutMs: number = DEFAULT_FORMAT_TIMEOUT_MS
): string => {
  if (input.length > MAX_QUERY_SIZE) {
    throw new Error(
      `Query too large for formatting (${input.length} chars, max: ${MAX_QUERY_SIZE})`
    );
  }

  const start = Date.now();
  const checkTimeout = () => {
    if (Date.now() - start > timeoutMs) {
      throw new Error('Query formatting timeout - potential ReDoS detected');
    }
  };

  let result = input;

  checkTimeout();
  // Collapse runs of whitespace. Single character class, no backtracking.
  result = result.replace(/[ \t\r\n]+/g, ' ');

  checkTimeout();
  result = result.replace(/[ \t]*,[ \t]*/g, ',\n  ');

  for (const keyword of LINE_BREAK_KEYWORDS) {
    checkTimeout();
    result = result.replace(new RegExp(`\\b${keyword}\\b`, 'gi'), `\n${keyword}`);
  }

  checkTimeout();
  result = result.replace(/^[ \t]+/gm, '  ');

  return result.trim();
};

/**
 * Collapse whitespace without regex backtracking risk.
 *
 * Used as the fallback when {@link safeFormatQuery} refuses an input, so the
 * button still does something predictable on a very large query.
 */
export const collapseWhitespace = (input: string): string =>
  input
    .split(/\s+/)
    .filter(word => word.length > 0)
    .join(' ')
    .trim();
