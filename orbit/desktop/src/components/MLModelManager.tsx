import React, { useState, useEffect } from 'react';
import styled from 'styled-components';
import {
  ModelInfo,
  MLFunctionInfo,
  Connection,
  ModelStatus,
} from '@/types';
import { TauriService } from '@/services/tauri';

interface MLModelManagerProps {
  connection: Connection | null;
  className?: string;
}

const Container = styled.div`
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #1e1e1e;
`;

const Header = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid #3c3c3c;
`;

const Title = styled.h2`
  margin: 0;
  color: #ffffff;
  font-size: 18px;
  font-weight: 600;
`;

const RefreshButton = styled.button`
  padding: 6px 12px;
  background: #0078d4;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  
  &:hover {
    background: #106ebe;
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
`;

const TabContainer = styled.div`
  display: flex;
  border-bottom: 1px solid #3c3c3c;
`;

const Tab = styled.button<{ active: boolean }>`
  padding: 12px 20px;
  background: none;
  border: none;
  color: ${props => props.active ? '#0078d4' : '#cccccc'};
  cursor: pointer;
  font-size: 14px;
  border-bottom: ${props => props.active ? '2px solid #0078d4' : '2px solid transparent'};
  transition: all 0.2s;

  &:hover {
    color: ${props => props.active ? '#0078d4' : '#ffffff'};
  }
`;

const Content = styled.div`
  flex: 1;
  overflow: auto;
  padding: 20px;
`;

// Models Tab Components
const ModelsGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 16px;
  margin-bottom: 20px;
`;

const ModelCard = styled.div`
  background: #2d2d2d;
  border: 1px solid #3c3c3c;
  border-radius: 8px;
  padding: 16px;
  transition: all 0.2s;

  &:hover {
    border-color: #0078d4;
    box-shadow: 0 2px 8px rgba(0, 120, 212, 0.1);
  }
`;

const ModelName = styled.h3`
  margin: 0 0 8px 0;
  color: #ffffff;
  font-size: 16px;
  font-weight: 600;
`;

const ModelMeta = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
`;

const ModelAlgorithm = styled.div`
  background: #107c10;
  color: white;
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
`;

const StatusIndicator = styled.div<{ status: ModelStatus }>`
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${props => {
    switch (props.status) {
      case ModelStatus.Ready: return '#107c10';
      case ModelStatus.Training: return '#ff8c00';
      case ModelStatus.Error: return '#d13438';
      case ModelStatus.Deleted: return '#5a5a5a';
      default: return '#5a5a5a';
    }
  }};
`;

const ModelStats = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-bottom: 12px;
`;

const StatItem = styled.div`
  color: #cccccc;
  font-size: 12px;
  
  .label {
    color: #888888;
    display: block;
  }
  
  .value {
    color: #ffffff;
    font-weight: 600;
    font-size: 14px;
  }
`;

const ModelActions = styled.div`
  display: flex;
  gap: 8px;
  margin-top: 12px;
`;

const ActionButton = styled.button<{ variant?: 'danger' }>`
  flex: 1;
  padding: 6px 12px;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;

  ${props => props.variant === 'danger' ? `
    background: #d13438;
    color: white;
    
    &:hover {
      background: #b71c1c;
    }
  ` : `
    background: #3c3c3c;
    color: #ffffff;
    
    &:hover {
      background: #484848;
    }
  `}

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
`;

// ML Functions Tab Components
const FunctionCategories = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 20px;
`;

const CategoryCard = styled.div`
  background: #2d2d2d;
  border: 1px solid #3c3c3c;
  border-radius: 8px;
  overflow: hidden;
`;

/**
 * Colours for the categories the backend currently emits. The category is a
 * free-form string there, so anything unrecognised falls back to grey rather
 * than being dropped.
 */
const CATEGORY_COLORS: Record<string, string> = {
  Boosting: '#0078d4',
  ModelManagement: '#107c10',
  Statistical: '#d83b01',
  FeatureEngineering: '#5c2d91',
  VectorOperations: '#e81123',
};

const CategoryHeader = styled.div<{ category: string }>`
  padding: 12px 16px;
  background: ${props => CATEGORY_COLORS[props.category] ?? '#5a5a5a'};
  color: white;
  font-weight: 600;
  font-size: 14px;
`;

const FunctionList = styled.div`
  padding: 16px;
`;

const FunctionItem = styled.div`
  margin-bottom: 12px;
  padding: 8px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.2s;

  &:hover {
    background: #3c3c3c;
  }

  .name {
    color: #0078d4;
    font-weight: 600;
    font-size: 13px;
    margin-bottom: 4px;
  }

  .description {
    color: #cccccc;
    font-size: 12px;
    line-height: 1.4;
  }
`;

const EmptyState = styled.div`
  text-align: center;
  padding: 40px 20px;
  color: #888888;
  
  .icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.5;
  }
  
  .message {
    font-size: 16px;
    margin-bottom: 8px;
  }
  
  .submessage {
    font-size: 14px;
    opacity: 0.7;
  }
`;

export const MLModelManager: React.FC<MLModelManagerProps> = ({
  connection,
  className
}) => {
  const [activeTab, setActiveTab] = useState<'models' | 'functions'>('models');
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [mlFunctions, setMlFunctions] = useState<MLFunctionInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (activeTab === 'models' && connection) {
      loadModels();
    } else if (activeTab === 'functions') {
      loadMlFunctions();
    }
  }, [activeTab, connection]);

  const loadModels = async () => {
    if (!connection) return;
    
    setLoading(true);
    setError(null);
    
    try {
      const modelList = await TauriService.listModels(connection.id);
      setModels(modelList);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load models');
    } finally {
      setLoading(false);
    }
  };

  const loadMlFunctions = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const functions = await TauriService.listMlFunctions();
      setMlFunctions(functions);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load ML functions');
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteModel = async (modelName: string) => {
    if (!connection) return;
    
    if (!confirm(`Are you sure you want to delete the model "${modelName}"?`)) {
      return;
    }

    try {
      await TauriService.deleteModel(connection.id, modelName);
      await loadModels(); // Refresh the list
    } catch (err) {
      alert(`Failed to delete model: ${err instanceof Error ? err.message : 'Unknown error'}`);
    }
  };

  const formatAccuracy = (accuracy: number) => `${(accuracy * 100).toFixed(1)}%`;

  /**
   * Bucket functions by whatever category the backend reported.
   *
   * Built from the data rather than from a fixed list: a hardcoded set of
   * categories silently dropped every function whose category was not in it.
   */
  const groupFunctionsByCategory = (): Record<string, MLFunctionInfo[]> =>
    mlFunctions.reduce<Record<string, MLFunctionInfo[]>>((groups, func) => {
      (groups[func.category] ??= []).push(func);
      return groups;
    }, {});

  /** Prettier labels for the categories we know; others show as sent. */
  const CATEGORY_LABELS: Record<string, string> = {
    Boosting: 'Boosting Algorithms',
    ModelManagement: 'Model Management',
    Statistical: 'Statistical Functions',
    FeatureEngineering: 'Feature Engineering',
    VectorOperations: 'Vector Operations',
  };

  const getCategoryDisplayName = (category: string) =>
    CATEGORY_LABELS[category] ?? category;

  return (
    <Container className={className}>
      <Header>
        <Title>ML Models & Functions</Title>
        <RefreshButton 
          onClick={() => activeTab === 'models' ? loadModels() : loadMlFunctions()}
          disabled={loading}
        >
          {loading ? '⟳ Loading...' : '🔄 Refresh'}
        </RefreshButton>
      </Header>

      <TabContainer>
        <Tab 
          active={activeTab === 'models'} 
          onClick={() => setActiveTab('models')}
        >
          📊 Models ({models.length})
        </Tab>
        <Tab 
          active={activeTab === 'functions'} 
          onClick={() => setActiveTab('functions')}
        >
          🧠 ML Functions ({mlFunctions.length})
        </Tab>
      </TabContainer>

      <Content>
        {error && (
          <div style={{ 
            color: '#d13438', 
            background: 'rgba(209, 52, 56, 0.1)', 
            padding: '12px', 
            borderRadius: '4px', 
            marginBottom: '16px' 
          }}>
            {error}
          </div>
        )}

        {activeTab === 'models' && (
          <>
            {!connection ? (
              <EmptyState>
                <div className="icon">🔌</div>
                <div className="message">No Connection Selected</div>
                <div className="submessage">Please connect to a database to view ML models</div>
              </EmptyState>
            ) : models.length === 0 && !loading ? (
              <EmptyState>
                <div className="icon">🤖</div>
                <div className="message">No ML Models Found</div>
                <div className="submessage">
                  Train your first model using OrbitQL:
                  <br />
                  <code>SELECT ML_TRAIN_MODEL('my_model', 'XGBOOST', features, target) FROM data</code>
                </div>
              </EmptyState>
            ) : (
              <ModelsGrid>
                {models.map((model) => (
                  <ModelCard key={model.id || model.name}>
                    <ModelName>{model.name}</ModelName>

                    <ModelMeta>
                      <ModelAlgorithm>{model.model_type}</ModelAlgorithm>
                      <StatusIndicator status={model.status} title={model.status} />
                    </ModelMeta>

                    <ModelStats>
                      <StatItem>
                        <span className="label">Accuracy</span>
                        {/* An untrained model has no accuracy; "unknown" beats 0%. */}
                        <span className="value">
                          {model.accuracy === null || model.accuracy === undefined
                            ? '—'
                            : formatAccuracy(model.accuracy)}
                        </span>
                      </StatItem>
                      <StatItem>
                        <span className="label">Features</span>
                        <span className="value">{model.features.length}</span>
                      </StatItem>
                      <StatItem>
                        <span className="label">Target</span>
                        <span className="value">{model.target ?? '—'}</span>
                      </StatItem>
                      <StatItem>
                        <span className="label">Last trained</span>
                        <span className="value">
                          {model.last_trained
                            ? new Date(model.last_trained).toLocaleDateString()
                            : 'never'}
                        </span>
                      </StatItem>
                    </ModelStats>

                    <ModelActions>
                      <ActionButton>📊 View Details</ActionButton>
                      <ActionButton variant="danger" onClick={() => handleDeleteModel(model.name)}>
                        🗑️ Delete
                      </ActionButton>
                    </ModelActions>
                  </ModelCard>
                ))}
              </ModelsGrid>
            )}
          </>
        )}

        {activeTab === 'functions' && (
          <>
            {mlFunctions.length === 0 && !loading ? (
              <EmptyState>
                <div className="icon">🔍</div>
                <div className="message">No ML Functions Available</div>
                <div className="submessage">Check your connection to load available ML functions</div>
              </EmptyState>
            ) : (
              <FunctionCategories>
                {Object.entries(groupFunctionsByCategory()).map(([category, functions]) => {
                  if (functions.length === 0) return null;
                  
                  return (
                    <CategoryCard key={category}>
                      <CategoryHeader category={category}>
                        {getCategoryDisplayName(category)} ({functions.length})
                      </CategoryHeader>
                      <FunctionList>
                        {functions.map((func) => (
                          <FunctionItem key={func.name}>
                            <div className="name">{func.name}</div>
                            <div className="description">{func.description}</div>
                          </FunctionItem>
                        ))}
                      </FunctionList>
                    </CategoryCard>
                  );
                })}
              </FunctionCategories>
            )}
          </>
        )}
      </Content>
    </Container>
  );
};