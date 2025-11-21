import React, { useState, useEffect } from 'react';
import styled from 'styled-components';
import { ConnectionInfo, ConnectionType, ConnectionStatus } from '@/types';
import { TauriService } from '@/services/tauri';

interface ConnectionDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: () => void;
  connection?: ConnectionInfo | null;
  mode: 'create' | 'edit';
}

const Overlay = styled.div<{ isOpen: boolean }>`
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: ${props => props.isOpen ? 'flex' : 'none'};
  align-items: center;
  justify-content: center;
  z-index: 1000;
`;

const Dialog = styled.div`
  background: #2d2d2d;
  border-radius: 8px;
  padding: 24px;
  width: 600px;
  max-width: 90vw;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
`;

const DialogHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid #3c3c3c;
`;

const DialogTitle = styled.h2`
  color: #ffffff;
  font-size: 20px;
  font-weight: 600;
  margin: 0;
`;

const CloseButton = styled.button`
  background: none;
  border: none;
  color: #cccccc;
  font-size: 24px;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: all 0.2s;

  &:hover {
    background: #3c3c3c;
    color: #ffffff;
  }
`;

const FormGroup = styled.div`
  margin-bottom: 20px;
`;

const Label = styled.label`
  display: block;
  color: #cccccc;
  font-size: 13px;
  font-weight: 500;
  margin-bottom: 8px;
`;

const Input = styled.input`
  width: 100%;
  padding: 10px 12px;
  background: #1e1e1e;
  border: 1px solid #3c3c3c;
  border-radius: 4px;
  color: #ffffff;
  font-size: 14px;
  outline: none;
  transition: all 0.2s;

  &:focus {
    border-color: #0078d4;
    box-shadow: 0 0 0 2px rgba(0, 120, 212, 0.2);
  }

  &::placeholder {
    color: #666666;
  }
`;

const Select = styled.select`
  width: 100%;
  padding: 10px 12px;
  background: #1e1e1e;
  border: 1px solid #3c3c3c;
  border-radius: 4px;
  color: #ffffff;
  font-size: 14px;
  outline: none;
  cursor: pointer;
  transition: all 0.2s;

  &:focus {
    border-color: #0078d4;
    box-shadow: 0 0 0 2px rgba(0, 120, 212, 0.2);
  }

  option {
    background: #1e1e1e;
    color: #ffffff;
  }
`;

const ButtonGroup = styled.div`
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  margin-top: 24px;
  padding-top: 16px;
  border-top: 1px solid #3c3c3c;
`;

const Button = styled.button<{ variant?: 'primary' | 'secondary' | 'danger' }>`
  padding: 10px 20px;
  border: none;
  border-radius: 4px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  ${props => {
    if (props.variant === 'primary') {
      return `
        background: #0078d4;
        color: white;
        &:hover:not(:disabled) {
          background: #106ebe;
        }
      `;
    } else if (props.variant === 'danger') {
      return `
        background: #d13438;
        color: white;
        &:hover:not(:disabled) {
          background: #a4262c;
        }
      `;
    } else {
      return `
        background: #3c3c3c;
        color: #ffffff;
        &:hover:not(:disabled) {
          background: #484848;
        }
      `;
    }
  }}

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
`;

const TestButton = styled(Button)`
  background: #107c10;
  color: white;

  &:hover:not(:disabled) {
    background: #0e6e0e;
  }
`;

const ErrorMessage = styled.div`
  background: rgba(209, 52, 56, 0.1);
  border: 1px solid rgba(209, 52, 56, 0.3);
  color: #d13438;
  padding: 12px;
  border-radius: 4px;
  margin-bottom: 16px;
  font-size: 13px;
`;

const SuccessMessage = styled.div`
  background: rgba(16, 124, 16, 0.1);
  border: 1px solid rgba(16, 124, 16, 0.3);
  color: #107c10;
  padding: 12px;
  border-radius: 4px;
  margin-bottom: 16px;
  font-size: 13px;
`;

const LoadingSpinner = styled.div`
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: #ffffff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
  margin-right: 8px;

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
`;

export const ConnectionDialog: React.FC<ConnectionDialogProps> = ({
  isOpen,
  onClose,
  onSave,
  connection,
  mode
}) => {
  const [formData, setFormData] = useState<ConnectionInfo>({
    name: '',
    connection_type: ConnectionType.PostgreSQL,
    host: 'localhost',
    port: 5432,
    database: '',
    username: '',
    password: '',
    ssl_mode: undefined,
    connection_timeout: 30,
    additional_params: {},
  });

  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      if (mode === 'edit' && connection) {
        setFormData(connection);
      } else {
        // Reset form for create mode
        setFormData({
          name: '',
          connection_type: ConnectionType.PostgreSQL,
          host: 'localhost',
          port: 5432,
          database: '',
          username: '',
          password: '',
          ssl_mode: undefined,
          connection_timeout: 30,
          additional_params: {},
        });
      }
      setTestResult(null);
      setError(null);
    }
  }, [isOpen, mode, connection]);

  const updateFormData = (field: keyof ConnectionInfo, value: any) => {
    setFormData(prev => ({ ...prev, [field]: value }));
    setTestResult(null);
    setError(null);
  };

  const getDefaultPort = (type: ConnectionType): number => {
    switch (type) {
      case ConnectionType.PostgreSQL: return 5432;
      case ConnectionType.MySQL: return 3306;
      case ConnectionType.OrbitQL: return 8081;
      case ConnectionType.Redis: return 6379;
      case ConnectionType.CQL: return 9042;
      case ConnectionType.Cypher: return 7687;
      case ConnectionType.AQL: return 8529;
      default: return 5432;
    }
  };

  const handleConnectionTypeChange = (type: ConnectionType) => {
    updateFormData('connection_type', type);
    updateFormData('port', getDefaultPort(type));
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    setError(null);

    try {
      const status = await TauriService.testConnection(formData);
      if (status === ConnectionStatus.Connected) {
        setTestResult({ success: true, message: 'Connection successful!' });
      } else if (status === ConnectionStatus.Error) {
        setTestResult({ success: false, message: 'Connection failed. Please check your settings.' });
      } else {
        setTestResult({ success: false, message: 'Connection test returned unexpected status.' });
      }
    } catch (err: any) {
      setTestResult({ success: false, message: err.message || 'Connection test failed' });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    if (!formData.name.trim()) {
      setError('Connection name is required');
      return;
    }

    if (!formData.host.trim()) {
      setError('Host is required');
      return;
    }

    setSaving(true);
    setError(null);

    try {
      await TauriService.createConnection(formData);
      onSave();
      onClose();
    } catch (err: any) {
      setError(err.message || 'Failed to save connection');
    } finally {
      setSaving(false);
    }
  };

  const requiresDatabase = (type: ConnectionType): boolean => {
    return [ConnectionType.PostgreSQL, ConnectionType.MySQL, ConnectionType.AQL].includes(type);
  };

  const requiresAuth = (type: ConnectionType): boolean => {
    return type !== ConnectionType.Redis && type !== ConnectionType.OrbitQL;
  };

  if (!isOpen) return null;

  return (
    <Overlay isOpen={isOpen} onClick={onClose}>
      <Dialog onClick={(e) => e.stopPropagation()}>
        <DialogHeader>
          <DialogTitle>
            {mode === 'create' ? 'New Connection' : 'Edit Connection'}
          </DialogTitle>
          <CloseButton onClick={onClose}>×</CloseButton>
        </DialogHeader>

        {error && <ErrorMessage>{error}</ErrorMessage>}
        {testResult && (
          testResult.success ? (
            <SuccessMessage>{testResult.message}</SuccessMessage>
          ) : (
            <ErrorMessage>{testResult.message}</ErrorMessage>
          )
        )}

        <FormGroup>
          <Label>Connection Name *</Label>
          <Input
            type="text"
            value={formData.name}
            onChange={(e) => updateFormData('name', e.target.value)}
            placeholder="My Database Connection"
          />
        </FormGroup>

        <FormGroup>
          <Label>Connection Type *</Label>
          <Select
            value={formData.connection_type}
            onChange={(e) => handleConnectionTypeChange(e.target.value as ConnectionType)}
          >
            <option value={ConnectionType.PostgreSQL}>PostgreSQL</option>
            <option value={ConnectionType.MySQL}>MySQL</option>
            <option value={ConnectionType.OrbitQL}>OrbitQL</option>
            <option value={ConnectionType.Redis}>Redis</option>
            <option value={ConnectionType.CQL}>CQL (Cassandra)</option>
            <option value={ConnectionType.Cypher}>Cypher (Neo4j)</option>
            <option value={ConnectionType.AQL}>AQL (ArangoDB)</option>
          </Select>
        </FormGroup>

        <FormGroup>
          <Label>Host *</Label>
          <Input
            type="text"
            value={formData.host}
            onChange={(e) => updateFormData('host', e.target.value)}
            placeholder="localhost"
          />
        </FormGroup>

        <FormGroup>
          <Label>Port *</Label>
          <Input
            type="number"
            value={formData.port}
            onChange={(e) => updateFormData('port', parseInt(e.target.value) || getDefaultPort(formData.connection_type))}
            min="1"
            max="65535"
          />
        </FormGroup>

        {requiresDatabase(formData.connection_type) && (
          <FormGroup>
            <Label>Database</Label>
            <Input
              type="text"
              value={formData.database || ''}
              onChange={(e) => updateFormData('database', e.target.value)}
              placeholder="database_name"
            />
          </FormGroup>
        )}

        {requiresAuth(formData.connection_type) && (
          <>
            <FormGroup>
              <Label>Username</Label>
              <Input
                type="text"
                value={formData.username || ''}
                onChange={(e) => updateFormData('username', e.target.value)}
                placeholder="username"
              />
            </FormGroup>

            <FormGroup>
              <Label>Password</Label>
              <Input
                type="password"
                value={formData.password || ''}
                onChange={(e) => updateFormData('password', e.target.value)}
                placeholder="password"
              />
            </FormGroup>
          </>
        )}

        <ButtonGroup>
          <TestButton
            onClick={handleTest}
            disabled={testing || !formData.name.trim() || !formData.host.trim()}
          >
            {testing && <LoadingSpinner />}
            {testing ? 'Testing...' : 'Test Connection'}
          </TestButton>
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={handleSave}
            disabled={saving || !formData.name.trim() || !formData.host.trim()}
          >
            {saving ? 'Saving...' : mode === 'create' ? 'Create' : 'Save'}
          </Button>
        </ButtonGroup>
      </Dialog>
    </Overlay>
  );
};

