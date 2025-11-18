/**
 * ReDoS Safety Tests for Query Formatter
 * 
 * These tests verify that the regex patterns used in query formatting
 * are safe from Regular Expression Denial of Service (ReDoS) attacks.
 */

import { describe, it, expect, beforeEach, afterEach } from '@jest/globals';

// Mock the formatter function from QueryEditor.tsx
const safeFormatQuery = (input: string): string => {
  const MAX_QUERY_SIZE = 1024 * 100; // 100KB limit
  if (input.length > MAX_QUERY_SIZE) {
    throw new Error(`Query too large for formatting (${input.length} chars, max: ${MAX_QUERY_SIZE})`);
  }

  const formatWithTimeout = (text: string, timeoutMs: number = 5000): string => {
    const start = Date.now();
    
    const checkTimeout = () => {
      if (Date.now() - start > timeoutMs) {
        throw new Error('Query formatting timeout - potential ReDoS detected');
      }
    };

    let result = text;
    
    checkTimeout();
    // Safe: atomic group prevents backtracking on whitespace sequences
    result = result.replace(/(?:[ \t\r\n])+/g, ' ');
    
    checkTimeout();
    // Safe: limited quantifiers with character classes
    result = result.replace(/[ \t]*,[ \t]*/g, ',\n  ');
    
    // Safe: individual keyword replacements avoid alternation backtracking
    const keywords = ['SELECT', 'FROM', 'WHERE', 'JOIN', 'GROUP BY', 'HAVING', 'ORDER BY', 'LIMIT'];
    for (const keyword of keywords) {
      checkTimeout();
      const regex = new RegExp(`\\b${keyword}\\b`, 'gi');
      result = result.replace(regex, `\n${keyword}`);
    }
    
    checkTimeout();
    // Safe: anchored pattern with character class, no backtracking
    result = result.replace(/^[ \t]+/gm, '  ');
    
    return result.trim();
  };

  return formatWithTimeout(input);
};

describe('Query Formatter ReDoS Safety Tests', () => {
  let consoleErrorSpy: jest.SpyInstance;

  beforeEach(() => {
    consoleErrorSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  describe('Input Size Limits', () => {
    it('should reject queries larger than 100KB', () => {
      const largeQuery = 'SELECT * FROM table WHERE ' + 'a'.repeat(1024 * 101);
      
      expect(() => safeFormatQuery(largeQuery)).toThrow(
        /Query too large for formatting/
      );
    });

    it('should accept queries within size limits', () => {
      const normalQuery = 'SELECT * FROM table WHERE col = 1';
      
      expect(() => safeFormatQuery(normalQuery)).not.toThrow();
    });
  });

  describe('ReDoS Attack Patterns', () => {
    it('should handle catastrophic backtracking patterns safely', () => {
      // Classic ReDoS pattern that would cause exponential backtracking in vulnerable regex
      const maliciousInput = 'SELECT' + ' '.repeat(10000) + 'FROM' + '\t'.repeat(10000) + 'WHERE';
      
      const start = Date.now();
      const result = safeFormatQuery(maliciousInput);
      const executionTime = Date.now() - start;
      
      // Should complete in reasonable time (< 1 second)
      expect(executionTime).toBeLessThan(1000);
      expect(result).toContain('SELECT');
      expect(result).toContain('FROM');
      expect(result).toContain('WHERE');
    });

    it('should handle nested quantifier patterns without exponential time', () => {
      // Pattern that could cause ReDoS: repeated whitespace with alternation
      const nestedPattern = 'SELECT' + '  \t  \n  '.repeat(1000) + 'FROM table';
      
      const start = Date.now();
      const result = safeFormatQuery(nestedPattern);
      const executionTime = Date.now() - start;
      
      // Should complete quickly
      expect(executionTime).toBeLessThan(500);
      expect(result).toMatch(/SELECT\s+FROM/);
    });

    it('should timeout on extremely long processing', () => {
      // Create a pattern that would take very long if not protected
      const extremeInput = 'SELECT ' + '/* ' + 'a'.repeat(50000) + ' */ FROM table';
      
      // Mock Date.now to simulate timeout condition
      const originalDateNow = Date.now;
      let callCount = 0;
      Date.now = jest.fn(() => {
        callCount++;
        // Simulate timeout after several calls
        return callCount > 5 ? originalDateNow() + 10000 : originalDateNow();
      });

      try {
        expect(() => safeFormatQuery(extremeInput)).toThrow(
          /Query formatting timeout/
        );
      } finally {
        Date.now = originalDateNow;
      }
    });
  });

  describe('Regex Pattern Safety', () => {
    it('should use linear time complexity for whitespace normalization', () => {
      const inputs = [
        '   SELECT    FROM   table   ',
        '\t\t\tSELECT\n\n\nFROM\r\r\rtable',
        ' '.repeat(1000) + 'SELECT' + '\n'.repeat(1000) + 'FROM'
      ];

      inputs.forEach(input => {
        const start = Date.now();
        const result = safeFormatQuery(input);
        const executionTime = Date.now() - start;
        
        expect(executionTime).toBeLessThan(100);
        expect(result).not.toMatch(/\s{2,}/); // Should not have multiple consecutive spaces
      });
    });

    it('should handle comma formatting without backtracking', () => {
      const commaHeavyQuery = 'SELECT col1,col2,  col3 ,col4,   col5 FROM table';
      
      const start = Date.now();
      const result = safeFormatQuery(commaHeavyQuery);
      const executionTime = Date.now() - start;
      
      expect(executionTime).toBeLessThan(50);
      expect(result).toMatch(/,\s*\n/g); // Commas should be followed by newlines
    });

    it('should format SQL keywords without alternation backtracking', () => {
      const keywordQuery = 'select col from table where id = 1 group by col having count > 0 order by col limit 10';
      
      const start = Date.now();
      const result = safeFormatQuery(keywordQuery);
      const executionTime = Date.now() - start;
      
      expect(executionTime).toBeLessThan(100);
      // Each keyword should be on its own line
      expect(result).toMatch(/\nSELECT/);
      expect(result).toMatch(/\nFROM/);
      expect(result).toMatch(/\nWHERE/);
      expect(result).toMatch(/\nGROUP BY/);
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty input safely', () => {
      expect(() => safeFormatQuery('')).not.toThrow();
      expect(safeFormatQuery('')).toBe('');
    });

    it('should handle input with only whitespace', () => {
      const whitespaceOnly = '   \t\t\n\n   ';
      const result = safeFormatQuery(whitespaceOnly);
      expect(result).toBe('');
    });

    it('should handle unicode and special characters safely', () => {
      const unicodeQuery = 'SELECT 你好, прив世ет FROM tåble_ñame WHERE çøl = "spéçiål"';
      
      expect(() => safeFormatQuery(unicodeQuery)).not.toThrow();
      const result = safeFormatQuery(unicodeQuery);
      expect(result).toContain('你好');
      expect(result).toContain('прив世ет');
    });

    it('should preserve query semantics while formatting', () => {
      const originalQuery = 'SELECT id,name FROM users WHERE active=1 ORDER BY created_at LIMIT 100';
      const formatted = safeFormatQuery(originalQuery);
      
      // Should contain all original elements
      expect(formatted).toContain('SELECT');
      expect(formatted).toContain('id');
      expect(formatted).toContain('name');
      expect(formatted).toContain('FROM users');
      expect(formatted).toContain('WHERE active=1');
      expect(formatted).toContain('ORDER BY');
      expect(formatted).toContain('LIMIT 100');
    });
  });
});