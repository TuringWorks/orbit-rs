/**
 * ReDoS safety tests for the query formatter.
 *
 * These import the formatter the editor actually calls. An earlier version of
 * this file re-declared the logic inline, so it could not fail when the real
 * implementation changed.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  MAX_QUERY_SIZE,
  collapseWhitespace,
  safeFormatQuery,
} from './queryFormatter';

describe('safeFormatQuery', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('input size limits', () => {
    it('rejects queries larger than the cap', () => {
      const large = `SELECT * FROM t WHERE ${'a'.repeat(MAX_QUERY_SIZE + 1)}`;
      expect(() => safeFormatQuery(large)).toThrow(/Query too large for formatting/);
    });

    it('accepts queries within the cap', () => {
      expect(() => safeFormatQuery('SELECT * FROM t WHERE col = 1')).not.toThrow();
    });
  });

  describe('pathological inputs complete in linear time', () => {
    it('handles long whitespace runs', () => {
      const input = `SELECT${' '.repeat(10000)}FROM${'\t'.repeat(10000)}WHERE`;

      const start = Date.now();
      const result = safeFormatQuery(input);
      expect(Date.now() - start).toBeLessThan(1000);

      expect(result).toContain('SELECT');
      expect(result).toContain('FROM');
      expect(result).toContain('WHERE');
    });

    it('handles repeated mixed whitespace without blowing up', () => {
      const input = `SELECT${'  \t  \n  '.repeat(1000)}FROM table`;

      const start = Date.now();
      const result = safeFormatQuery(input);
      expect(Date.now() - start).toBeLessThan(500);

      expect(result).toMatch(/SELECT\s+FROM/);
    });

    it('handles a very long comment body', () => {
      const input = `SELECT /* ${'a'.repeat(50000)} */ FROM table`;

      const start = Date.now();
      expect(() => safeFormatQuery(input)).not.toThrow();
      expect(Date.now() - start).toBeLessThan(1000);
    });
  });

  describe('timeout guard', () => {
    it('aborts once the clock passes the budget mid-format', () => {
      // The guard is what stops a pathological pattern running unbounded, so
      // drive the clock rather than trusting a real format to be slow.
      const real = Date.now();
      let call = 0;
      vi.spyOn(Date, 'now').mockImplementation(() => {
        call += 1;
        // First call sets the start; later calls appear far in the future.
        return call === 1 ? real : real + 60_000;
      });

      expect(() => safeFormatQuery('SELECT a, b FROM t')).toThrow(
        /Query formatting timeout/
      );
    });

    it('does not abort a normal query under the default budget', () => {
      expect(() => safeFormatQuery('SELECT a, b FROM t')).not.toThrow();
    });
  });

  describe('formatting behaviour', () => {
    it('puts clause keywords on their own lines and normalises their case', () => {
      const result = safeFormatQuery('select a from t where a = 1 order by a');
      const lines = result.split('\n').map(line => line.trim());

      // Keyword replacement substitutes the canonical spelling, so lowercase
      // input comes back uppercased.
      expect(lines).toContain('SELECT a');
      expect(lines.some(line => line.startsWith('FROM'))).toBe(true);
      expect(lines.some(line => line.startsWith('WHERE'))).toBe(true);
      expect(lines.some(line => line.startsWith('ORDER BY'))).toBe(true);
    });

    it('breaks select lists on commas', () => {
      expect(safeFormatQuery('SELECT a,b FROM t')).toContain(',\n');
    });

    it('is idempotent: formatting twice matches formatting once', () => {
      const once = safeFormatQuery('SELECT a, b FROM t WHERE a = 1');
      expect(safeFormatQuery(once)).toBe(once);
    });

    it('leaves an empty input empty', () => {
      expect(safeFormatQuery('')).toBe('');
      expect(safeFormatQuery('   \n\t ')).toBe('');
    });
  });
});

describe('collapseWhitespace', () => {
  it('reduces every whitespace run to a single space', () => {
    expect(collapseWhitespace('SELECT \t\n  a   FROM  t ')).toBe('SELECT a FROM t');
  });

  it('handles input that is only whitespace', () => {
    expect(collapseWhitespace('  \t\n ')).toBe('');
  });

  it('accepts input far larger than the formatter cap', () => {
    const huge = `SELECT ${'a '.repeat(MAX_QUERY_SIZE)}`;
    expect(() => collapseWhitespace(huge)).not.toThrow();
  });
});
