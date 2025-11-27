import React, { useState, useEffect } from 'react';
import styled from 'styled-components';
import { QueryRequest, QueryType } from '@/types';
import { TauriService } from '@/services/tauri';

interface QueryHistoryPanelProps {
  connectionId?: string;
  onSelectQuery?: (query: string, queryType: QueryType) => void;
}

const Container = styled.div`
  padding: 16px;
  height: 100%;
  overflow-y: auto;
`;

const Header = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
`;

const Title = styled.h3`
  color: #ffffff;
  font-size: 16px;
  font-weight: 600;
  margin: 0;
`;

const ClearButton = styled.button`
  padding: 6px 12px;
  background: #3c3c3c;
  color: #cccccc;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: #484848;
    color: #ffffff;
  }
`;

const HistoryList = styled.div`
  display: flex;
  flex-direction: column;
  gap: 8px;
`;

const HistoryItem = styled.div`
  background: #2d2d2d;
  border: 1px solid #3c3c3c;
  border-radius: 4px;
  padding: 12px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: #3c3c3c;
    border-color: #5a5a5a;
  }
`;

const QueryPreview = styled.div`
  color: #cccccc;
  font-size: 12px;
  font-family: 'Courier New', monospace;
  margin-bottom: 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
`;

const QueryMeta = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
  color: #999999;
`;

const QueryTypeBadge = styled.span<{ type: QueryType }>`
  padding: 2px 6px;
  border-radius: 10px;
  font-size: 10px;
  font-weight: 600;
  background: ${props => {
    switch (props.type) {
      case QueryType.SQL: return '#0078d4';
      case QueryType.MySQL: return '#00758f';
      case QueryType.OrbitQL: return '#107c10';
      case QueryType.Redis: return '#d83b01';
      case QueryType.CQL: return '#1287b1';
      case QueryType.Cypher: return '#008cc1';
      case QueryType.AQL: return '#dd5324';
      default: return '#5a5a5a';
    }
  }};
  color: white;
`;

const EmptyState = styled.div`
  text-align: center;
  padding: 40px 20px;
  color: #999999;
  font-size: 14px;
`;

const LoadingState = styled.div`
  text-align: center;
  padding: 40px 20px;
  color: #999999;
  font-size: 14px;
`;

export const QueryHistoryPanel: React.FC<QueryHistoryPanelProps> = ({
  connectionId,
  onSelectQuery
}) => {
  const [history, setHistory] = useState<QueryRequest[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (connectionId) {
      loadHistory();
    } else {
      setHistory([]);
    }
  }, [connectionId]);

  const loadHistory = async () => {
    if (!connectionId) return;
    
    setLoading(true);
    try {
      const queryHistory = await TauriService.getQueryHistory(connectionId, 50);
      setHistory(queryHistory);
    } catch (err) {
      console.error('Failed to load query history:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleQueryClick = (query: QueryRequest) => {
    if (onSelectQuery) {
      onSelectQuery(query.query, query.query_type);
    }
  };

  const formatTime = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    return date.toLocaleDateString();
  };

  if (!connectionId) {
    return (
      <Container>
        <EmptyState>
          <div>Select a connection to view query history</div>
        </EmptyState>
      </Container>
    );
  }

  if (loading) {
    return (
      <Container>
        <LoadingState>Loading history...</LoadingState>
      </Container>
    );
  }

  return (
    <Container>
      <Header>
        <Title>Query History</Title>
        {history.length > 0 && (
          <ClearButton onClick={loadHistory}>Refresh</ClearButton>
        )}
      </Header>

      {history.length === 0 ? (
        <EmptyState>
          <div>No query history yet</div>
          <div style={{ marginTop: '8px', fontSize: '12px' }}>
            Execute queries to see them here
          </div>
        </EmptyState>
      ) : (
        <HistoryList>
          {history.map((query, index) => (
            <HistoryItem key={index} onClick={() => handleQueryClick(query)}>
              <QueryPreview>{query.query}</QueryPreview>
              <QueryMeta>
                <QueryTypeBadge type={query.query_type}>
                  {query.query_type}
                </QueryTypeBadge>
                <span>{formatTime(new Date().toISOString())}</span>
              </QueryMeta>
            </HistoryItem>
          ))}
        </HistoryList>
      )}
    </Container>
  );
};

