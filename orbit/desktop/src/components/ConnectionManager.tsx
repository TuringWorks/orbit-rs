import React, { useState } from 'react';
import styled from 'styled-components';
import { Connection, connectionStatusError, isConnected } from '@/types';
import { TauriService, handleTauriError } from '@/services/tauri';
import { ConnectionDialog } from './ConnectionDialog';

interface ConnectionManagerProps {
  connections: Connection[];
  onConnectionsChange: () => void;
}

const Container = styled.div`
  padding: 16px;
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

const Button = styled.button`
  padding: 8px 16px;
  background: #0078d4;
  color: white;
  border: none;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: #106ebe;
  }
`;

const ConnectionList = styled.div`
  display: flex;
  flex-direction: column;
  gap: 8px;
`;

const ConnectionItem = styled.div<{ active?: boolean }>`
  background: ${props => props.active ? '#3c3c3c' : '#2d2d2d'};
  border: 1px solid ${props => props.active ? '#0078d4' : '#3c3c3c'};
  border-radius: 4px;
  padding: 12px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: #3c3c3c;
    border-color: #5a5a5a;
  }
`;

const ConnectionHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
`;

const ConnectionName = styled.div`
  color: #ffffff;
  font-size: 14px;
  font-weight: 500;
`;

const ConnectionType = styled.span`
  color: #cccccc;
  font-size: 12px;
  background: #3c3c3c;
  padding: 2px 8px;
  border-radius: 12px;
`;

const ConnectionDetails = styled.div`
  color: #999999;
  font-size: 12px;
  margin-top: 4px;
`;

const ConnectionActions = styled.div`
  display: flex;
  gap: 8px;
`;

const ActionButton = styled.button<{ variant?: 'danger' }>`
  padding: 4px 8px;
  background: ${props => props.variant === 'danger' ? '#d13438' : '#3c3c3c'};
  color: ${props => props.variant === 'danger' ? '#ffffff' : '#cccccc'};
  border: none;
  border-radius: 4px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: ${props => props.variant === 'danger' ? '#a4262c' : '#484848'};
    color: #ffffff;
  }
`;

type StatusTone = 'connected' | 'error' | 'idle';

const StatusIndicator = styled.span<{ tone: StatusTone }>`
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${props =>
    props.tone === 'connected' ? '#107c10' : props.tone === 'error' ? '#d13438' : '#666666'};
  margin-right: 6px;
`;

const ErrorNote = styled.div`
  color: #f2a3a5;
  font-size: 12px;
  margin-top: 4px;
  line-height: 1.4;
`;

const EmptyState = styled.div`
  text-align: center;
  padding: 40px 20px;
  color: #999999;
  font-size: 14px;
`;

export const ConnectionManager: React.FC<ConnectionManagerProps> = ({
  connections,
  onConnectionsChange
}) => {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingConnection, setEditingConnection] = useState<Connection | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  /** Per-connection failure from the most recent connect attempt. */
  const [attemptErrors, setAttemptErrors] = useState<Record<string, string>>({});

  const setAttemptError = (id: string, message: string | null) =>
    setAttemptErrors(prev => {
      const next = { ...prev };
      if (message === null) {
        delete next[id];
      } else {
        next[id] = message;
      }
      return next;
    });

  /** Open a session now so the user finds out here, not on their first query. */
  const handleConnect = async (connectionId: string) => {
    setBusyId(connectionId);
    setAttemptError(connectionId, null);
    try {
      await TauriService.connect(connectionId);
      onConnectionsChange();
    } catch (err) {
      setAttemptError(connectionId, handleTauriError(err));
    } finally {
      setBusyId(null);
    }
  };

  const handleDisconnect = async (connectionId: string) => {
    setBusyId(connectionId);
    try {
      await TauriService.disconnect(connectionId);
      onConnectionsChange();
    } catch (err) {
      setAttemptError(connectionId, handleTauriError(err));
    } finally {
      setBusyId(null);
    }
  };

  const handleCreate = () => {
    setEditingConnection(null);
    setDialogOpen(true);
  };

  const handleEdit = (connection: Connection) => {
    setEditingConnection(connection);
    setDialogOpen(true);
  };

  const handleDelete = async (connectionId: string) => {
    if (!confirm('Are you sure you want to delete this connection?')) {
      return;
    }

    setDeleting(connectionId);
    try {
      await TauriService.deleteConnection(connectionId);
      onConnectionsChange();
    } catch (err) {
      console.error('Failed to delete connection:', err);
      alert('Failed to delete connection');
    } finally {
      setDeleting(null);
    }
  };

  const handleDialogClose = () => {
    setDialogOpen(false);
    setEditingConnection(null);
  };

  const handleDialogSave = () => {
    onConnectionsChange();
  };

  return (
    <Container>
      <Header>
        <Title>Connections</Title>
        <Button onClick={handleCreate}>+ New Connection</Button>
      </Header>

      {connections.length === 0 ? (
        <EmptyState>
          <div>No connections yet</div>
          <div style={{ marginTop: '8px', fontSize: '12px' }}>
            Click "New Connection" to add one
          </div>
        </EmptyState>
      ) : (
        <ConnectionList>
          {connections.map(conn => {
            const connected = isConnected(conn.status);
            // A stored status error and a failed click are different events;
            // show whichever is more recent, preferring the click.
            const failure = attemptErrors[conn.id] ?? connectionStatusError(conn.status);
            const tone: StatusTone = connected ? 'connected' : failure ? 'error' : 'idle';

            return (
              <ConnectionItem key={conn.id}>
                <ConnectionHeader>
                  <div style={{ display: 'flex', alignItems: 'center', flex: 1 }}>
                    <StatusIndicator
                      tone={tone}
                      title={connected ? 'Session open' : failure ?? 'No session open'}
                    />
                    <ConnectionName>{conn.info.name}</ConnectionName>
                  </div>
                  <ConnectionType>{conn.info.connection_type}</ConnectionType>
                  <ConnectionActions>
                    {connected ? (
                      <ActionButton
                        onClick={() => handleDisconnect(conn.id)}
                        disabled={busyId === conn.id}
                      >
                        Disconnect
                      </ActionButton>
                    ) : (
                      <ActionButton
                        onClick={() => handleConnect(conn.id)}
                        disabled={busyId === conn.id}
                      >
                        {busyId === conn.id ? 'Connecting…' : 'Connect'}
                      </ActionButton>
                    )}
                    <ActionButton onClick={() => handleEdit(conn)}>
                      Edit
                    </ActionButton>
                    <ActionButton
                      variant="danger"
                      onClick={() => handleDelete(conn.id)}
                      disabled={deleting === conn.id}
                    >
                      {deleting === conn.id ? 'Deleting...' : 'Delete'}
                    </ActionButton>
                  </ConnectionActions>
                </ConnectionHeader>
                <ConnectionDetails>
                  {conn.info.host}:{conn.info.port}
                  {conn.info.database && ` • ${conn.info.database}`}
                  {conn.query_count > 0 && ` • ${conn.query_count} queries`}
                </ConnectionDetails>
                {!connected && failure && <ErrorNote>{failure}</ErrorNote>}
              </ConnectionItem>
            );
          })}
        </ConnectionList>
      )}

      <ConnectionDialog
        isOpen={dialogOpen}
        onClose={handleDialogClose}
        onSave={handleDialogSave}
        connection={editingConnection?.info || null}
        mode={editingConnection ? 'edit' : 'create'}
      />
    </Container>
  );
};

