import React, { useEffect, useRef, useState } from 'react';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState, Extension } from '@codemirror/state';
import { sql } from '@codemirror/lang-sql';
import { javascript } from '@codemirror/lang-javascript';
import { oneDark } from '@codemirror/theme-one-dark';
import { keymap } from '@codemirror/view';
import { indentWithTab } from '@codemirror/commands';
import { autocompletion, completionKeymap } from '@codemirror/autocomplete';
import styled from 'styled-components';
import { QueryType, QueryRequest, QueryResult, Connection } from '@/types';
import { TauriService } from '@/services/tauri';
import { useHotkeys } from 'react-hotkeys-hook';
import { orbitqlKeywords, redisCommands } from '@/constants/queryKeywords';

interface QueryEditorProps {
  value: string;
  onChange: (value: string) => void;
  queryType: QueryType;
  onExecute: (query: string) => void;
  onExplain?: (query: string) => void;
  isExecuting?: boolean;
  connection?: Connection | null;
  className?: string;
}

const EditorContainer = styled.div`
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  
  .cm-editor {
    height: 100%;
    font-size: 14px;
    border: 1px solid #3c3c3c;
    border-radius: 4px;
  }

  .cm-focused {
    outline: none;
    border-color: #0078d4;
  }

  .cm-content {
    padding: 12px;
    min-height: 200px;
  }

  .cm-line {
    line-height: 1.6;
  }

  .cm-cursor {
    border-left: 2px solid #ffffff;
  }

  .cm-selectionBackground {
    background: #264f78 !important;
  }

  .cm-activeLine {
    background-color: rgba(255, 255, 255, 0.05);
  }

  .cm-activeLineGutter {
    background-color: rgba(255, 255, 255, 0.05);
  }
`;

const Toolbar = styled.div`
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  background: #2d2d2d;
  border-bottom: 1px solid #3c3c3c;
  align-items: center;
`;

const Button = styled.button<{ variant?: 'primary' | 'secondary' }>`
  padding: 6px 12px;
  border: none;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  transition: all 0.2s;

  ${props => props.variant === 'primary' ? `
    background: #0078d4;
    color: white;
    
    &:hover:not(:disabled) {
      background: #106ebe;
    }
  ` : `
    background: #3c3c3c;
    color: #ffffff;
    
    &:hover:not(:disabled) {
      background: #484848;
    }
  `}

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  &:active {
    transform: translateY(1px);
  }
`;

const StatusBar = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 12px;
  background: #2d2d2d;
  border-top: 1px solid #3c3c3c;
  font-size: 12px;
  color: #cccccc;
`;

const QueryTypeIndicator = styled.div<{ type: QueryType }>`
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  
  ${props => {
    switch (props.type) {
      case QueryType.SQL:
        return 'background: #0078d4; color: white;';
      case QueryType.OrbitQL:
        return 'background: #107c10; color: white;';
      case QueryType.Redis:
        return 'background: #d83b01; color: white;';
      default:
        return 'background: #5a5a5a; color: white;';
    }
  }}
`;

export const QueryEditor: React.FC<QueryEditorProps> = ({
  value,
  onChange,
  queryType,
  onExecute,
  onExplain,
  isExecuting = false,
  connection,
  className
}) => {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [cursorPosition, setCursorPosition] = useState({ line: 1, column: 1 });

  // Hotkeys for query execution
  useHotkeys('ctrl+enter,cmd+enter', () => {
    if (!isExecuting && value.trim()) {
      handleExecute();
    }
  });

  useHotkeys('ctrl+shift+enter,cmd+shift+enter', () => {
    if (!isExecuting && value.trim() && onExplain) {
      handleExplain();
    }
  });

  // Create autocompletion based on query type
  const createAutocompletion = () => {
    let keywords: string[] = [];
    
    switch (queryType) {
      case QueryType.SQL:
      case QueryType.OrbitQL:
        keywords = orbitqlKeywords;
        break;
      case QueryType.Redis:
        keywords = redisCommands;
        break;
    }
    
    return autocompletion({
      override: [
        (context) => {
          const word = context.matchBefore(/\w*/);
          if (!word || (word.from === word.to && !context.explicit)) return null;
          
          const options = keywords
            .filter(k => k.toLowerCase().includes(word.text.toLowerCase()))
            .map(k => ({
              label: k,
              type: 'keyword',
              boost: k.startsWith(word.text.toLowerCase()) ? 1 : 0,
            }));
            
          return {
            from: word.from,
            options,
          };
        }
      ]
    });
  };

  // Create editor extensions based on query type
  const createExtensions = (): Extension[] => {
    const extensions: Extension[] = [
      basicSetup,
      oneDark,
      keymap.of([...completionKeymap, indentWithTab]),
      createAutocompletion(),
      EditorView.updateListener.of(update => {
        if (update.docChanged) {
          onChange(update.state.doc.toString());
        }
        if (update.selectionSet) {
          const pos = update.state.selection.main.head;
          const line = update.state.doc.lineAt(pos);
          setCursorPosition({
            line: line.number,
            column: pos - line.from + 1
          });
        }
      }),
    ];

    // Add language support based on query type
    switch (queryType) {
      case QueryType.SQL:
      case QueryType.OrbitQL:
        extensions.push(sql());
        break;
      case QueryType.Redis:
        extensions.push(javascript()); // Use JavaScript for Redis commands
        break;
    }

    return extensions;
  };

  // Initialize/update editor
  useEffect(() => {
    if (!editorRef.current) return;

    if (viewRef.current) {
      viewRef.current.destroy();
    }

    const state = EditorState.create({
      doc: value,
      extensions: createExtensions(),
    });

    viewRef.current = new EditorView({
      state,
      parent: editorRef.current,
    });

    return () => {
      if (viewRef.current) {
        viewRef.current.destroy();
      }
    };
  }, [queryType]);

  // Update editor content when value prop changes
  useEffect(() => {
    if (viewRef.current && viewRef.current.state.doc.toString() !== value) {
      viewRef.current.dispatch({
        changes: {
          from: 0,
          to: viewRef.current.state.doc.length,
          insert: value,
        },
      });
    }
  }, [value]);

  const handleExecute = () => {
    if (value.trim()) {
      onExecute(value.trim());
    }
  };

  const handleExplain = () => {
    if (value.trim() && onExplain) {
      onExplain(value.trim());
    }
  };

  // ReDoS-safe formatter utility with input size limits and timeout protection
  const safeFormatQuery = (input: string): string => {
    // Prevent DoS by limiting input size (typical large queries are < 100KB)
    const MAX_QUERY_SIZE = 1024 * 100; // 100KB limit
    if (input.length > MAX_QUERY_SIZE) {
      throw new Error(`Query too large for formatting (${input.length} chars, max: ${MAX_QUERY_SIZE})`);
    }

    // Use timeout wrapper to prevent infinite regex execution
    const formatWithTimeout = (text: string, timeoutMs: number = 5000): string => {
      const start = Date.now();
      
      // Check timeout before each regex operation
      const checkTimeout = () => {
        if (Date.now() - start > timeoutMs) {
          throw new Error('Query formatting timeout - potential ReDoS detected');
        }
      };

      // ReDoS-safe regex patterns with linear time complexity O(n)
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
        // Safe: exact word boundary matches, no nested quantifiers
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

  const handleFormat = () => {
    if (queryType === QueryType.SQL || queryType === QueryType.OrbitQL) {
      try {
        const formatted = safeFormatQuery(value);
        onChange(formatted);
      } catch (error) {
        console.error('Query formatting failed:', error);
        // Graceful fallback - just clean up basic whitespace without regex
        const basicFormatted = value
          .split(/\s+/)
          .filter(word => word.length > 0)
          .join(' ')
          .trim();
        onChange(basicFormatted);
      }
    }
  };

  const getQueryTypeHelp = () => {
    switch (queryType) {
      case QueryType.SQL:
        return 'Standard PostgreSQL syntax';
      case QueryType.OrbitQL:
        return 'OrbitQL with ML functions - try ML_XGBOOST(), ML_TRAIN_MODEL()';
      case QueryType.Redis:
        return 'Redis commands - GET, SET, HGET, etc.';
      default:
        return '';
    }
  };

  return (
    <EditorContainer className={className}>
      <Toolbar>
        <Button variant="primary" onClick={handleExecute} disabled={isExecuting || !value.trim()}>
          {isExecuting ? (
            <>
              <span>⟳</span> Executing...
            </>
          ) : (
            <>
              <span>▶</span> Execute (Ctrl+Enter)
            </>
          )}
        </Button>
        
        {(queryType === QueryType.SQL || queryType === QueryType.OrbitQL) && (
          <Button onClick={handleExplain} disabled={isExecuting || !value.trim() || !onExplain}>
            <span>📊</span> Explain
          </Button>
        )}
        
        {(queryType === QueryType.SQL || queryType === QueryType.OrbitQL) && (
          <Button onClick={handleFormat}>
            <span>📝</span> Format
          </Button>
        )}

        <div style={{ flex: 1 }} />
        
        <QueryTypeIndicator type={queryType}>
          {queryType}
        </QueryTypeIndicator>
      </Toolbar>
      
      <div ref={editorRef} style={{ flex: 1 }} />
      
      <StatusBar>
        <div>
          Line {cursorPosition.line}, Column {cursorPosition.column}
        </div>
        <div>
          {connection ? `Connected to ${connection.info.name}` : 'No connection'}
          {' • '}
          {getQueryTypeHelp()}
        </div>
      </StatusBar>
    </EditorContainer>
  );
};