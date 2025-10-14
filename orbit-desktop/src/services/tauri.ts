import { invoke } from '@tauri-apps/api/tauri';
import { 
  Connection, 
  ConnectionInfo, 
  ConnectionStatus, 
  QueryRequest, 
  QueryResult, 
  ModelInfo, 
  MLFunctionInfo,
  ApiResponse,
  QueryType,
  QueryResultData
} from '@/types';

// Check if running in Tauri environment
const isTauri = (): boolean => {
  try {
    return globalThis.window !== undefined && 
           'window' in globalThis &&
           '__TAURI_IPC__' in globalThis.window && 
           typeof globalThis.window.__TAURI_IPC__ === 'function';
  } catch {
    return false;
  }
};

// Mock data for development/browser environment
const createMockConnections = (): Connection[] => [
  {
    id: 'mock-postgres',
    info: {
      name: 'Mock PostgreSQL',
      connection_type: 'PostgreSQL',
      host: 'localhost',
      port: 5432,
      database: 'demo',
      username: 'postgres',
      password: null,
      ssl_mode: null,
      connection_timeout: null,
      additional_params: {},
    },
    status: 'Connected',
    created_at: new Date().toISOString(),
    last_used: new Date().toISOString(),
    query_count: 0,
  },
  {
    id: 'mock-orbitql',
    info: {
      name: 'Mock OrbitQL',
      connection_type: 'OrbitQL',
      host: 'localhost',
      port: 8080,
      database: null,
      username: null,
      password: null,
      ssl_mode: null,
      connection_timeout: null,
      additional_params: {},
    },
    status: 'Connected',
    created_at: new Date().toISOString(),
    last_used: new Date().toISOString(),
    query_count: 0,
  }
];

const createMockQueryResult = (): QueryResult => ({
  success: true,
  data: {
    columns: [
      { name: 'id', column_type: 'integer' },
      { name: 'name', column_type: 'text' },
      { name: 'value', column_type: 'decimal' }
    ],
    rows: [
      { id: 1, name: 'Sample Data', value: 100.5 },
      { id: 2, name: 'Another Row', value: 250 }
    ]
  } as QueryResultData,
  error: null,
  execution_time: 23.5,
  rows_affected: 2
});

/**
 * Service for communicating with Tauri backend
 */
export class TauriService {
  // Connection Management
  
  static async createConnection(connectionInfo: ConnectionInfo): Promise<string> {
    const response: ApiResponse<string> = await invoke('create_connection', { connectionInfo });
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to create connection');
    }
    return response.data;
  }

  static async testConnection(connectionInfo: ConnectionInfo): Promise<ConnectionStatus> {
    const response: ApiResponse<ConnectionStatus> = await invoke('test_connection', { connectionInfo });
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to test connection');
    }
    return response.data;
  }

  static async getConnections(): Promise<Connection[]> {
    if (!isTauri()) {
      // Return mock data for browser/dev environment
      await new Promise(resolve => setTimeout(resolve, 100)); // Simulate delay
      return createMockConnections();
    }
    
    const response: ApiResponse<Connection[]> = await invoke('get_connections');
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to get connections');
    }
    return response.data;
  }

  static async disconnect(connectionId: string): Promise<void> {
    const response: ApiResponse<boolean> = await invoke('disconnect', { connectionId });
    if (!response.success) {
      throw new Error(response.error || 'Failed to disconnect');
    }
  }

  static async deleteConnection(connectionId: string): Promise<void> {
    const response: ApiResponse<boolean> = await invoke('delete_connection', { connectionId });
    if (!response.success) {
      throw new Error(response.error || 'Failed to delete connection');
    }
  }

  // Query Execution

  static async executeQuery(request: QueryRequest): Promise<QueryResult> {
    if (!isTauri()) {
      // Return mock data for browser/dev environment
      await new Promise(resolve => setTimeout(resolve, 200)); // Simulate execution time
      return createMockQueryResult();
    }
    
    const response: ApiResponse<QueryResult> = await invoke('execute_query', { request });
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to execute query');
    }
    return response.data;
  }

  static async getQueryHistory(connectionId: string, limit?: number): Promise<QueryRequest[]> {
    const response: ApiResponse<QueryRequest[]> = await invoke('get_query_history', { 
      connectionId, 
      limit 
    });
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to get query history');
    }
    return response.data;
  }

  static async explainQuery(request: QueryRequest): Promise<QueryResult> {
    const response: ApiResponse<QueryResult> = await invoke('explain_query', { request });
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to explain query');
    }
    return response.data;
  }

  // ML Model Management

  static async listMlFunctions(): Promise<MLFunctionInfo[]> {
    if (!isTauri()) {
      // Return mock ML functions for browser/dev environment
      return [
        {
          name: 'ML_XGBOOST',
          category: 'Boosting',
          description: 'XGBoost gradient boosting algorithm',
          parameters: [],
          example: 'SELECT ML_XGBOOST(features, target) FROM data;'
        }
      ];
    }
    
    const response: ApiResponse<MLFunctionInfo[]> = await invoke('list_ml_functions');
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to list ML functions');
    }
    return response.data;
  }

  static async listModels(connectionId: string): Promise<ModelInfo[]> {
    const response: ApiResponse<ModelInfo[]> = await invoke('list_models', { connectionId });
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to list models');
    }
    return response.data;
  }

  static async getModelInfo(connectionId: string, modelName: string): Promise<ModelInfo> {
    const response: ApiResponse<ModelInfo> = await invoke('get_model_info', { 
      connectionId, 
      modelName 
    });
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to get model info');
    }
    return response.data;
  }

  static async deleteModel(connectionId: string, modelName: string): Promise<void> {
    const response: ApiResponse<boolean> = await invoke('delete_model', { 
      connectionId, 
      modelName 
    });
    if (!response.success) {
      throw new Error(response.error || 'Failed to delete model');
    }
  }

  // System Operations

  static async getSystemInfo(): Promise<Record<string, any>> {
    const response: ApiResponse<Record<string, any>> = await invoke('get_system_info');
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to get system info');
    }
    return response.data;
  }

  static async saveSettings(settings: Record<string, any>): Promise<void> {
    const response: ApiResponse<boolean> = await invoke('save_settings', { settings });
    if (!response.success) {
      throw new Error(response.error || 'Failed to save settings');
    }
  }

  static async loadSettings(): Promise<Record<string, any>> {
    const response: ApiResponse<Record<string, any>> = await invoke('load_settings');
    if (!response.success || !response.data) {
      throw new Error(response.error || 'Failed to load settings');
    }
    return response.data;
  }

  static async showAboutDialog(): Promise<void> {
    await invoke('show_about_dialog');
  }
}

/**
 * Error handler for Tauri API calls
 */
export const handleTauriError = (error: any): string => {
  if (typeof error === 'string') {
    return error;
  }
  
  if (error?.message) {
    return error.message;
  }
  
  if (error?.error) {
    return error.error;
  }
  
  return 'An unexpected error occurred';
};