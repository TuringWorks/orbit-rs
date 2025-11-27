// Type definitions for Orbit Desktop

export interface Connection {
  id: string;
  info: ConnectionInfo;
  status: ConnectionStatus;
  created_at: string;
  last_used: string | null;
  query_count: number;
}

export interface ConnectionInfo {
  name: string;
  connection_type: ConnectionType;
  host: string;
  port: number;
  database?: string;
  username?: string;
  password?: string;
  ssl_mode?: string;
  connection_timeout?: number;
  additional_params: Record<string, string>;
}

export enum ConnectionType {
  PostgreSQL = 'PostgreSQL',
  OrbitQL = 'OrbitQL',
  Redis = 'Redis',
  MySQL = 'MySQL',
  CQL = 'CQL',
  Cypher = 'Cypher',
  AQL = 'AQL',
}

export enum ConnectionStatus {
  Connected = 'Connected',
  Disconnected = 'Disconnected',
  Connecting = 'Connecting',
  Error = 'Error',
}

export interface QueryRequest {
  connection_id: string;
  query: string;
  query_type: QueryType;
  parameters?: Record<string, any>;
  timeout?: number;
}

export enum QueryType {
  SQL = 'SQL',
  OrbitQL = 'OrbitQL',
  Redis = 'Redis',
  MySQL = 'MySQL',
  CQL = 'CQL',
  Cypher = 'Cypher',
  AQL = 'AQL',
}

export interface QueryResult {
  success: boolean;
  data?: QueryResultData;
  error?: string;
  execution_time: number;
  rows_affected?: number;
  timestamp?: string;
}

export interface QueryResultData {
  columns: Column[];
  rows: Row[];
  metadata?: Record<string, any>;
}

export interface Column {
  name: string;
  type: string;
  nullable: boolean;
}

export type Row = Record<string, any>;

// ML Model types
export interface ModelInfo {
  name: string;
  algorithm: string;
  accuracy: number;
  training_samples: number;
  feature_count: number;
  size_bytes: number;
  status: ModelStatus;
  created_at: string;
  updated_at: string;
  version: string;
  description?: string;
}

export enum ModelStatus {
  Training = 'Training',
  Ready = 'Ready',
  Error = 'Error',
  Deprecated = 'Deprecated',
}

export interface MLFunctionInfo {
  name: string;
  description: string;
  category: MLFunctionCategory;
  parameters: MLParameter[];
  return_type: string;
  examples: string[];
}

export enum MLFunctionCategory {
  ModelManagement = 'ModelManagement',
  Statistical = 'Statistical',
  SupervisedLearning = 'SupervisedLearning',
  UnsupervisedLearning = 'UnsupervisedLearning',
  BoostingAlgorithms = 'BoostingAlgorithms',
  FeatureEngineering = 'FeatureEngineering',
  VectorOperations = 'VectorOperations',
  TimeSeries = 'TimeSeries',
  NLP = 'NLP',
}

export interface MLParameter {
  name: string;
  param_type: string;
  required: boolean;
  description: string;
  default_value?: any;
}

// API Response wrapper
export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
  timestamp: string;
}

// UI State types
export interface AppState {
  currentConnection: Connection | null;
  connections: Connection[];
  queryHistory: QueryRequest[];
  models: ModelInfo[];
  mlFunctions: MLFunctionInfo[];
  settings: AppSettings;
  ui: UIState;
}

export interface AppSettings {
  theme: 'dark' | 'light';
  auto_save: boolean;
  query_timeout: number;
  editor_font_size: number;
  editor_theme: string;
  show_line_numbers: boolean;
  word_wrap: boolean;
}

export interface UIState {
  sidebarCollapsed: boolean;
  activeTab: string;
  loading: boolean;
  error: string | null;
  modal: {
    isOpen: boolean;
    type: string;
    data?: any;
  };
}

// Query Editor types
export interface QueryTab {
  id: string;
  name: string;
  query: string;
  connection_id?: string;
  query_type: QueryType;
  unsaved_changes: boolean;
  result?: QueryResult;
  is_executing: boolean;
}

// Chart/Visualization types
export interface ChartConfig {
  type: 'line' | 'bar' | 'pie' | 'scatter' | 'area';
  title: string;
  x_axis: string;
  y_axis: string | string[];
  color_scheme: string[];
  show_legend: boolean;
  show_grid: boolean;
}

export interface VisualizationData {
  config: ChartConfig;
  data: Record<string, any>[];
}

// Redis specific types
export interface RedisCommand {
  name: string;
  description: string;
  syntax: string;
  examples: string[];
  category: string;
  complexity: string;
  since_version: string;
}

export interface RedisKeyInfo {
  key: string;
  type: string;
  ttl: number;
  size: number;
  encoding?: string;
}

// File/Export types
export interface ExportOptions {
  format: 'csv' | 'json' | 'xlsx' | 'sql';
  include_headers: boolean;
  delimiter?: string;
  encoding?: string;
}

// Theme types
export interface Theme {
  name: string;
  primary: string;
  secondary: string;
  background: string;
  surface: string;
  text: string;
  textSecondary: string;
  border: string;
  error: string;
  warning: string;
  success: string;
  info: string;
}

