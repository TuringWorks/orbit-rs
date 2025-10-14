import React, { useState, useEffect } from 'react';
import styled, { ThemeProvider, createGlobalStyle } from 'styled-components';
import Split from 'react-split';
import { Tabs, TabList, Tab, TabPanel } from 'react-tabs';
import 'react-tabs/style/react-tabs.css';

import { QueryEditor } from '@/components/QueryEditor';
import { MLModelManager } from '@/components/MLModelManager';
import { DataVisualization } from '@/components/DataVisualization';
import { SampleQueries } from '@/components/SampleQueries';
import QueryResultsTable from '@/components/QueryResultsTable';
import { TauriService, handleTauriError } from '@/services/tauri';
import { useQueryTabs } from '@/hooks/useQueryTabs';
import { 
  Connection, 
  QueryType, 
  QueryRequest,
  QueryTab,
  Theme
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

const App: React.FC = () => {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [currentConnection, setCurrentConnection] = useState<Connection | null>(null);
  const [resultsView, setResultsView] = useState<'table' | 'chart' | 'models'>('table');
  const [rightPanelView, setRightPanelView] = useState<'models' | 'samples'>('samples');
  const [error, setError] = useState<string | null>(null);
  
  const {
    queryTabs,
    activeTabIndex,
    setActiveTabIndex,
    createNewTab,
    closeTab,
    updateTabQuery,
    updateTabState,
    getCurrentTab
  } = useQueryTabs();

  useEffect(() => {
    loadConnections();
    
    // Check if running in browser mode and show notification
    if (globalThis.window !== undefined && (!globalThis.window.__TAURI_IPC__ || typeof globalThis.window.__TAURI_IPC__ !== 'function')) {
      console.log('🌐 Running in browser mode with mock data. For full functionality, run as Tauri desktop app.');
    }
  }, []);

  const loadConnections = async () => {
    try {
      const connectionList = await TauriService.getConnections();
      setConnections(connectionList);
      
      // Auto-select first connected connection
      const connected = connectionList.find(c => c.status === 'Connected');
      if (connected && !currentConnection) {
        setCurrentConnection(connected);
      }
    } catch (err) {
      console.error('Failed to load connections:', err);
    }
  };


  const executeQuery = async (query: string) => {
    if (!currentConnection) {
      setError('Please select a connection first');
      return;
    }

    const currentTab = getCurrentTab();
    if (!currentTab) return;

    // Update tab state to executing
    updateTabState(activeTabIndex, {
      is_executing: true,
      unsaved_changes: false,
    });
    setError(null);

    try {
      const request: QueryRequest = {
        connection_id: currentConnection.id,
        query,
        query_type: currentTab.query_type,
        timeout: 30000,
      };

      const result = await TauriService.executeQuery(request);
      
      // Update tab with result
      updateTabState(activeTabIndex, {
        result,
        is_executing: false,
      });

      // Switch to appropriate results view
      if (result.data && result.data.rows.length > 0) {
        const numericColumns = result.data.columns.filter(col => 
          col.type.includes('int') || col.type.includes('float') || col.type.includes('decimal')
        );
        
        if (numericColumns.length > 0 && result.data.rows.length > 1) {
          setResultsView('chart');
        } else {
          setResultsView('table');
        }
      }

    } catch (err) {
      const errorMessage = handleTauriError(err);
      setError(errorMessage);
      
      // Clear executing state
      updateTabState(activeTabIndex, { is_executing: false });
    }
  };

  const explainQuery = async (query: string) => {
    if (!currentConnection) {
      setError('Please select a connection first');
      return;
    }

    try {
      const request: QueryRequest = {
        connection_id: currentConnection.id,
        query: `EXPLAIN ANALYZE ${query}`,
        query_type: QueryType.SQL,
      };

      const result = await TauriService.explainQuery(request);
      
      // Update tab with explain result
      updateTabState(activeTabIndex, { result });
      setResultsView('table');

    } catch (err) {
      setError(handleTauriError(err));
    }
  };

  const handleConnectionChange = (connectionId: string) => {
    const connection = connections.find(c => c.id === connectionId);
    setCurrentConnection(connection || null);
  };

  const handleSampleQuerySelect = (query: string, queryType: QueryType) => {
    const newTab: QueryTab = {
      id: Date.now().toString(),
      name: `Sample ${queryTabs.length + 1}`,
      query: query,
      query_type: queryType,
      unsaved_changes: false,
      is_executing: false,
    };

    setQueryTabs([...queryTabs, newTab]);
    setActiveTabIndex(queryTabs.length);
  };

  const currentTab = getCurrentTab();
  const hasResults = currentTab?.result?.success && currentTab.result.data;

  return (
    <ThemeProvider theme={theme}>
      <GlobalStyle />
      <AppContainer>
        <Header>
          <Logo>
            <div className="icon">🌌</div>
            Orbit Desktop
          </Logo>
          
          <ConnectionStatus>
            <StatusDot connected={!!currentConnection} />
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
            
            <Button onClick={() => createNewTab()}>+ New Query</Button>
            <Button onClick={() => createNewTab(QueryType.Redis)}>+ Redis</Button>
          </ConnectionStatus>
        </Header>

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
                              borderRadius: '4px'
                            }}>
                              {error}
                            </div>
                          )}
                          
                          {resultsView === 'table' && currentTab?.result && (
                            <QueryResultsTable result={currentTab.result} />
                          )}
                          
                          {resultsView === 'chart' && hasResults && (
                            <DataVisualization data={currentTab.result.data!} />
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
              <div style={{ display: 'flex', borderBottom: '1px solid #3c3c3c', background: '#2d2d2d' }}>
                <button
                  style={{
                    padding: '8px 16px',
                    background: rightPanelView === 'samples' ? '#0078d4' : 'transparent',
                    border: 'none',
                    color: rightPanelView === 'samples' ? 'white' : '#cccccc',
                    cursor: 'pointer',
                    fontSize: '13px',
                    borderBottom: rightPanelView === 'samples' ? '2px solid #0078d4' : '2px solid transparent'
                  }}
                  onClick={() => setRightPanelView('samples')}
                >
                  📚 Sample Queries
                </button>
                <button
                  style={{
                    padding: '8px 16px',
                    background: rightPanelView === 'models' ? '#0078d4' : 'transparent',
                    border: 'none',
                    color: rightPanelView === 'models' ? 'white' : '#cccccc',
                    cursor: 'pointer',
                    fontSize: '13px',
                    borderBottom: rightPanelView === 'models' ? '2px solid #0078d4' : '2px solid transparent'
                  }}
                  onClick={() => setRightPanelView('models')}
                >
                  🤖 ML Models
                </button>
              </div>
              <div style={{ flex: 1, overflow: 'hidden' }}>
                {rightPanelView === 'samples' && (
                  <SampleQueries onSelectQuery={handleSampleQuerySelect} />
                )}
                {rightPanelView === 'models' && (
                  <MLModelManager connection={currentConnection} />
                )}
              </div>
            </div>
          </Split>
        </MainContent>
      </AppContainer>
    </ThemeProvider>
  );
};

export default App;