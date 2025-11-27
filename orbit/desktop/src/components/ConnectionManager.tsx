import React, { useState } from 'react';
import styled from 'styled-components';
import { Connection, ConnectionInfo } from '@/types';
import { TauriService } from '@/services/tauri';
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

const StatusIndicator = styled.span<{ status: string }>`
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${props => 
    props.status === 'Connected' ? '#107c10' :
    props.status === 'Connecting' ? '#ff8c00' :
    props.status === 'Error' ? '#d13438' :
    '#666666'
  };
  margin-right: 6px;
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
          {connections.map(conn => (
            <ConnectionItem key={conn.id}>
              <ConnectionHeader>
                <div style={{ display: 'flex', alignItems: 'center', flex: 1 }}>
                  <StatusIndicator status={conn.status} />
                  <ConnectionName>{conn.info.name}</ConnectionName>
                </div>
                <ConnectionType>{conn.info.connection_type}</ConnectionType>
                <ConnectionActions>
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
            </ConnectionItem>
          ))}
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

