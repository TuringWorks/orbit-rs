import React, { useCallback, useState, useEffect } from 'react';
import styled, { ThemeProvider, createGlobalStyle } from 'styled-components';
import Split from 'react-split';
import { Tabs, TabList, Tab, TabPanel } from 'react-tabs';
import 'react-tabs/style/react-tabs.css';

import { QueryEditor } from '@/components/QueryEditor';
import { MLModelManager } from '@/components/MLModelManager';
import { DataVisualization } from '@/components/DataVisualization';
import { SampleQueries } from '@/components/SampleQueries';
import QueryResultsTable from '@/components/QueryResultsTable';
import { ConnectionManager } from '@/components/ConnectionManager';
import { ClusterPanel } from '@/components/ClusterPanel';
import { QueryHistoryPanel } from '@/components/QueryHistoryPanel';
import { KeyboardShortcuts } from '@/components/KeyboardShortcuts';
import { TauriService, handleTauriError, isTauri } from '@/services/tauri';
import { useQueryTabs } from '@/hooks/useQueryTabs';
import { useHotkeys } from 'react-hotkeys-hook';
import {
  Connection,
  QueryType,
  QueryRequest,
  Theme,
  isConnected,
} from '@/types';

// Global styles
const GlobalStyle = createGlobalStyle`
  * {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }
  
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
    background: #1e1e1e;
    color: #ffffff;
    overflow: hidden;
    height: 100vh;
  }
  
  #root {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
  }

  /* React Tabs Styling */
  .react-tabs {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .react-tabs__tab-list {
    margin: 0;
    padding: 0;
    border-bottom: 1px solid #3c3c3c;
    background: #2d2d2d;
    display: flex;
  }

  .react-tabs__tab {
    display: flex;
    align-items: center;
    padding: 8px 12px;
    background: none;
    border: none;
    color: #cccccc;
    cursor: pointer;
    font-size: 13px;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
    gap: 6px;
  }

  .react-tabs__tab:hover {
    color: #ffffff;
    background: #3c3c3c;
  }

  .react-tabs__tab--selected {
    color: #0078d4;
    border-bottom-color: #0078d4;
    background: #2d2d2d;
  }

  .react-tabs__tab-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .react-tabs__tab-panel--selected {
    display: flex;
  }

  /* Split Pane Styling */
  .split {
    display: flex;
    height: 100%;
  }

  .split.split-horizontal {
    flex-direction: row;
  }

  .split.split-vertical {
    flex-direction: column;
  }

  .gutter {
    background: #3c3c3c;
    background-repeat: no-repeat;
    background-position: 50%;
  }

  .gutter.gutter-horizontal {
    cursor: ew-resize;
    width: 4px;
  }

  .gutter.gutter-vertical {
    cursor: ns-resize;
    height: 4px;
  }

  /* Scrollbar styling */
  ::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  ::-webkit-scrollbar-track {
    background: #2d2d2d;
  }

  ::-webkit-scrollbar-thumb {
    background: #5a5a5a;
    border-radius: 4px;
  }

  ::-webkit-scrollbar-thumb:hover {
    background: #6a6a6a;
  }
`;

const theme: Theme = {
  name: 'dark',
  primary: '#0078d4',
  secondary: '#107c10',
  background: '#1e1e1e',
  surface: '#2d2d2d',
  text: '#ffffff',
  textSecondary: '#cccccc',
  border: '#3c3c3c',
  error: '#d13438',
  warning: '#ff8c00',
  success: '#107c10',
  info: '#0078d4',
};

const AppContainer = styled.div`
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: ${(props: any) => props.theme.background};
  color: ${(props: any) => props.theme.text};
`;

const Header = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  background: ${(props: any) => props.theme.surface};
  border-bottom: 1px solid ${(props: any) => props.theme.border};
  min-height: 48px;
`;

const Logo = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 16px;
  
  .icon {
    width: 24px;
    height: 24px;
    background: linear-gradient(45deg, #0078d4, #107c10);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-size: 12px;
  }
`;

const ConnectionStatus = styled.div`
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 13px;
`;

const StatusDot = styled.div<{ connected: boolean }>`
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${props => props.connected ? '#107c10' : '#d13438'};
`;

const ConnectionSelect = styled.select`
  padding: 4px 8px;
  background: #3c3c3c;
  border: 1px solid #5a5a5a;
  border-radius: 4px;
  color: #ffffff;
  font-size: 13px;
  outline: none;
  min-width: 200px;

  &:focus {
    border-color: #0078d4;
  }

  option {
    background: #3c3c3c;
    color: #ffffff;
  }
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
    background: ${(props: any) => props.theme.primary};
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
`;

const MainContent = styled.div`
  flex: 1;
  display: flex;
  overflow: hidden;
`;

const TabCloseButton = styled.button`
  background: none;
  border: none;
  color: #888888;
  cursor: pointer;
  padding: 2px;
  margin-left: 4px;
  border-radius: 2px;
  font-size: 12px;
  transition: all 0.2s;

  &:hover {
    background: #d13438;
    color: white;
  }
`;

const ResultsContainer = styled.div`
  display: flex;
  flex-direction: column;
  height: 100%;
`;

const ResultsTabs = styled.div`
  display: flex;
  border-bottom: 1px solid #3c3c3c;
  background: #2d2d2d;
`;

const ResultsTab = styled.button<{ active: boolean }>`
  padding: 8px 16px;
  background: none;
  border: none;
  color: ${props => props.active ? '#0078d4' : '#cccccc'};
  cursor: pointer;
  font-size: 13px;
  border-bottom: ${props => props.active ? '2px solid #0078d4' : '2px solid transparent'};
  transition: all 0.2s;

  &:hover {
    color: ${props => props.active ? '#0078d4' : '#ffffff'};
  }
`;

const ResultsContent = styled.div`
  flex: 1;
  overflow: auto;
`;

type RightPanelView = 'samples' | 'connections' | 'cluster' | 'history' | 'models';

const RIGHT_PANEL_TABS: ReadonlyArray<{ id: RightPanelView; label: string }> = [
  { id: 'samples', label: '📚 Samples' },
  { id: 'connections', label: '🔌 Connections' },
  { id: 'cluster', label: '🖥️ Cluster' },
  { id: 'history', label: '📜 History' },
  { id: 'models', label: '🤖 Models' },
];

const App: React.FC = () => {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [currentConnection, setCurrentConnection] = useState<Connection | null>(null);
  const [resultsView, setResultsView] = useState<'table' | 'chart' | 'models'>('table');
  const [rightPanelView, setRightPanelView] = useState<RightPanelView>('samples');
  const [error, setError] = useState<string | null>(null);
  const [showShortcuts, setShowShortcuts] = useState(false);

  const {
    queryTabs,
    activeTabIndex,
    setActiveTabIndex,
    createNewTab,
    openTab,
    closeTab,
    updateTabQuery,
    updateTabState,
    getCurrentTab
  } = useQueryTabs();

  // Keyboard shortcuts
  useHotkeys('ctrl+/,cmd+/', () => {
    setShowShortcuts(true);
  });

  const loadConnections = useCallback(async () => {
    try {
      const connectionList = await TauriService.getConnections();
      setConnections(connectionList);
      setError(null);

      // Prefer a connection with a live session; fall back to the first saved
      // one so a restored connection is selectable before it has been opened.
      setCurrentConnection(previous => {
        if (previous && connectionList.some(c => c.id === previous.id)) {
          return connectionList.find(c => c.id === previous.id) ?? previous;
        }
        return connectionList.find(c => isConnected(c.status)) ?? connectionList[0] ?? null;
      });
    } catch (err) {
      setError(handleTauriError(err));
    }
  }, []);

  useEffect(() => {
    void loadConnections();
  }, [loadConnections]);

  const executeQuery = async (query: string) => {
    if (!currentConnection) {
      setError('Please select a connection first');
      return;
    }

    updateTabState(activeTabIndex, {
      is_executing: true,
      unsaved_changes: false,
    });
    setError(null);

    try {
      const request: QueryRequest = {
        connection_id: currentConnection.id,
        query,
        timeout_ms: 30000,
      };

      const result = await TauriService.executeQuery(request);

      updateTabState(activeTabIndex, {
        result,
        is_executing: false,
      });

      // A failed statement's message lives in the results grid, but surface it
      // in the banner too so it is visible without switching views.
      if (!result.success && result.error) {
        setError(result.error);
      }

      // Only offer the chart view when there is something plottable; never
      // switch away from the grid on the user's behalf otherwise.
      const rows = result.data?.rows ?? [];
      const numericColumns = (result.data?.columns ?? []).filter(col =>
        /int|float|double|decimal|numeric|real|serial/i.test(col.type)
      );
      setResultsView(numericColumns.length > 0 && rows.length > 1 ? 'chart' : 'table');

      // Usage counters and session state changed; refresh the connection list.
      void loadConnections();
    } catch (err) {
      setError(handleTauriError(err));
      updateTabState(activeTabIndex, { is_executing: false });
    }
  };

  const explainQuery = async (query: string) => {
    if (!currentConnection) {
      setError('Please select a connection first');
      return;
    }

    setError(null);
    try {
      // The backend prefixes EXPLAIN itself and defaults to the non-executing
      // form, so a plan request cannot modify data.
      const result = await TauriService.explainQuery({
        connection_id: currentConnection.id,
        query,
        timeout_ms: 30000,
      });

      updateTabState(activeTabIndex, { result });
      setResultsView('table');
      if (!result.success && result.error) {
        setError(result.error);
      }
    } catch (err) {
      setError(handleTauriError(err));
    }
  };

  const handleConnectionChange = (connectionId: string) => {
    setCurrentConnection(connections.find(c => c.id === connectionId) ?? null);
    setError(null);
  };

  const handleSampleQuerySelect = (query: string, queryType: QueryType) => {
    openTab(query, queryType, `Sample ${queryTabs.length + 1}`);
  };

  const currentTab = getCurrentTab();
  const currentResult = currentTab?.result;
  const hasResults = Boolean(currentResult?.success && currentResult.data);

  return (
    <ThemeProvider theme={theme}>
      <GlobalStyle />
      <AppContainer>
        <Header>
          <Logo>
            <div className="icon">🌌</div>
            Orbit Desktop
            <button
              onClick={() => setShowShortcuts(true)}
              style={{
                marginLeft: '12px',
                background: 'none',
                border: 'none',
                color: '#999999',
                cursor: 'pointer',
                fontSize: '12px',
                padding: '4px 8px',
                borderRadius: '4px',
                transition: 'all 0.2s'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = '#3c3c3c';
                e.currentTarget.style.color = '#ffffff';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'none';
                e.currentTarget.style.color = '#999999';
              }}
              title="Keyboard Shortcuts (Ctrl+/)"
            >
              ⌨️ Shortcuts
            </button>
          </Logo>
          
          <ConnectionStatus>
            <StatusDot
              connected={Boolean(currentConnection && isConnected(currentConnection.status))}
              title={
                currentConnection
                  ? isConnected(currentConnection.status)
                    ? 'Session open'
                    : 'Saved, but no session open yet'
                  : 'No connection selected'
              }
            />
            <ConnectionSelect
              value={currentConnection?.id || ''}
              onChange={(e) => handleConnectionChange(e.target.value)}
            >
              <option value="">Select Connection...</option>
              {connections.map(conn => (
                <option key={conn.id} value={conn.id}>
                  {conn.info.name} ({conn.info.connection_type})
                </option>
              ))}
            </ConnectionSelect>

            <Button
              onClick={() => setRightPanelView('connections')}
              title="Manage Connections"
            >
              ⚙️ Manage
            </Button>

            <Button onClick={() => createNewTab()}>+ New Query</Button>
          </ConnectionStatus>
        </Header>

        {!isTauri() && (
          <div style={{
            padding: '8px 16px',
            background: 'rgba(255, 140, 0, 0.12)',
            borderBottom: '1px solid rgba(255, 140, 0, 0.3)',
            color: '#ffb454',
            fontSize: '12px',
          }}>
            Running in a plain browser: there is no IPC bridge to the database, so every
            action will report an error. Launch the desktop app for a working session.
          </div>
        )}

        <MainContent>
          <Split
            sizes={[60, 40]}
            direction="horizontal"
            className="split"
          >
            {/* Left Panel - Query Editor */}
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              <Tabs selectedIndex={activeTabIndex} onSelect={setActiveTabIndex}>
                <TabList>
                  {queryTabs.map((tab, index) => (
                    <Tab key={tab.id}>
                      <span>{tab.name}</span>
                      {tab.unsaved_changes && <span style={{ color: '#ff8c00' }}>●</span>}
                      {queryTabs.length > 1 && (
                        <TabCloseButton
                          onClick={(e) => {
                            e.stopPropagation();
                            closeTab(index);
                          }}
                        >
                          ×
                        </TabCloseButton>
                      )}
                    </Tab>
                  ))}
                </TabList>

                {queryTabs.map((tab, index) => (
                  <TabPanel key={tab.id}>
                    <Split
                      sizes={[50, 50]}
                      direction="vertical"
                      className="split"
                    >
                      {/* Query Editor */}
                      <QueryEditor
                        value={tab.query}
                        onChange={(query) => updateTabQuery(index, query)}
                        queryType={tab.query_type}
                        onExecute={executeQuery}
                        onExplain={explainQuery}
                        isExecuting={tab.is_executing}
                        connection={currentConnection}
                      />

                      {/* Results */}
                      <ResultsContainer>
                        <ResultsTabs>
                          <ResultsTab 
                            active={resultsView === 'table'}
                            onClick={() => setResultsView('table')}
                          >
                            📋 Results
                          </ResultsTab>
                          {hasResults && (
                            <ResultsTab 
                              active={resultsView === 'chart'}
                              onClick={() => setResultsView('chart')}
                            >
                              📊 Chart
                            </ResultsTab>
                          )}
                          <ResultsTab 
                            active={resultsView === 'models'}
                            onClick={() => setResultsView('models')}
                          >
                            🤖 Models
                          </ResultsTab>
                        </ResultsTabs>
                        
                        <ResultsContent>
                          {error && (
                            <div style={{
                              padding: '16px',
                              background: 'rgba(209, 52, 56, 0.1)',
                              color: '#d13438',
                              border: '1px solid rgba(209, 52, 56, 0.3)',
                              margin: '16px',
                              borderRadius: '4px',
                              whiteSpace: 'pre-wrap',
                            }}>
                              {error}
                            </div>
                          )}

                          {currentResult?.notice && (
                            <div style={{
                              padding: '12px 16px',
                              background: 'rgba(255, 140, 0, 0.1)',
                              color: '#ffb454',
                              border: '1px solid rgba(255, 140, 0, 0.3)',
                              margin: '16px',
                              borderRadius: '4px',
                              fontSize: '12px',
                              lineHeight: 1.5,
                            }}>
                              ⚠️ {currentResult.notice}
                            </div>
                          )}

                          {resultsView === 'table' && currentResult && (
                            <QueryResultsTable result={currentResult} />
                          )}

                          {resultsView === 'chart' && currentResult?.data && (
                            <DataVisualization data={currentResult.data} />
                          )}

                          {resultsView === 'models' && (
                            <MLModelManager connection={currentConnection} />
                          )}
                        </ResultsContent>
                      </ResultsContainer>
                    </Split>
                  </TabPanel>
                ))}
              </Tabs>
            </div>

            {/* Right Panel - Additional Tools */}
            <div style={{ background: '#1e1e1e', borderLeft: '1px solid #3c3c3c', display: 'flex', flexDirection: 'column' }}>
              <div style={{ display: 'flex', borderBottom: '1px solid #3c3c3c', background: '#2d2d2d', flexWrap: 'wrap' }}>
                {RIGHT_PANEL_TABS.map(tab => (
                  <button
                    key={tab.id}
                    style={{
                      padding: '8px 12px',
                      background: rightPanelView === tab.id ? '#0078d4' : 'transparent',
                      border: 'none',
                      color: rightPanelView === tab.id ? 'white' : '#cccccc',
                      cursor: 'pointer',
                      fontSize: '12px',
                      borderBottom:
                        rightPanelView === tab.id ? '2px solid #0078d4' : '2px solid transparent',
                    }}
                    onClick={() => setRightPanelView(tab.id)}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>
              <div style={{ flex: 1, overflow: 'hidden' }}>
                {rightPanelView === 'samples' && (
                  <SampleQueries onSelectQuery={handleSampleQuerySelect} />
                )}
                {rightPanelView === 'connections' && (
                  <ConnectionManager
                    connections={connections}
                    onConnectionsChange={loadConnections}
                  />
                )}
                {rightPanelView === 'cluster' && <ClusterPanel />}
                {rightPanelView === 'history' && (
                  <QueryHistoryPanel
                    connectionId={currentConnection?.id}
                    onSelectQuery={handleSampleQuerySelect}
                  />
                )}
                {rightPanelView === 'models' && (
                  <MLModelManager connection={currentConnection} />
                )}
              </div>
            </div>
          </Split>
        </MainContent>
      </AppContainer>
      
      <KeyboardShortcuts isOpen={showShortcuts} onClose={() => setShowShortcuts(false)} />
    </ThemeProvider>
  );
};

export default App;