// Type definitions for Orbit Desktop.
//
// These mirror the Tauri command payloads in `src-tauri/src`. Keep them in step
// with `connections.rs`, `queries.rs` and `cluster.rs` — there is no codegen
// between the two, so a rename on one side is a silent break on the other.

export interface Connection {
  id: string;
  info: ConnectionInfo;
  status: ConnectionStatus;
  /** Null when the persisted record carried no readable timestamp. */
  created_at: string | null;
  last_used: string | null;
  query_count: number;
}

export interface ConnectionInfo {
  name: string;
  connection_type: ConnectionType;
  host: string;
  port: number;
  database?: string | null;
  username?: string | null;
  password?: string | null;
  ssl_mode?: string | null;
  /** Connect/handshake timeout in milliseconds. */
  connection_timeout?: number | null;
  additional_params: Record<string, string>;
}

export enum ConnectionType {
  PostgreSQL = 'PostgreSQL',
  MySQL = 'MySQL',
  Redis = 'Redis',
  CQL = 'CQL',
  OrbitQL = 'OrbitQL',
  Cypher = 'Cypher',
  AQL = 'AQL',
  FlightSQL = 'FlightSQL',
  OrbitWire = 'OrbitWire',
}

/**
 * Serde's external tagging: unit variants arrive as bare strings, the error
 * variant as `{ Error: "..." }`.
 */
export type ConnectionStatus =
  | 'Connected'
  | 'Disconnected'
  | { Error: string };

export const isConnected = (status: ConnectionStatus): boolean =>
  status === 'Connected';

export const connectionStatusError = (status: ConnectionStatus): string | null =>
  typeof status === 'object' && status !== null && 'Error' in status
    ? status.Error
    : null;

/** A connection type as offered by the backend, with its default port. */
export interface ConnectionTypeInfo {
  id: ConnectionType;
  default_port: number;
  /**
   * False for the HTTP-backed protocols. Their handlers in orbit-server still
   * return fixed example rows, so results are a protocol check, not data.
   */
  native_wire_protocol: boolean;
}

export interface QueryRequest {
  connection_id: string;
  query: string;
  /** Statement timeout in milliseconds; the backend defaults to 30s. */
  timeout_ms?: number | null;
}

/** Which language the editor highlights. Presentation only — execution
 *  dispatches on the connection's protocol, not on this. */
export enum QueryType {
  SQL = 'SQL',
  OrbitQL = 'OrbitQL',
  Redis = 'Redis',
  MySQL = 'MySQL',
  CQL = 'CQL',
  Cypher = 'Cypher',
  AQL = 'AQL',
}

/**
 * What a statement did. `returned` and `affected` are different facts and are
 * kept apart deliberately.
 */
export type StatementOutcome =
  | { kind: 'returned'; rows: number }
  | { kind: 'affected'; rows: number }
  | { kind: 'completed' };

export const describeOutcome = (outcome: StatementOutcome): string => {
  switch (outcome.kind) {
    case 'returned':
      return `${outcome.rows} row${outcome.rows === 1 ? '' : 's'} returned`;
    case 'affected':
      return `${outcome.rows} row${outcome.rows === 1 ? '' : 's'} affected`;
    case 'completed':
      return 'Completed';
  }
};

export interface QueryResult {
  success: boolean;
  data?: QueryResultData | null;
  error?: string | null;
  execution_time_ms: number;
  /** A caveat the grid alone cannot convey, e.g. a mocked server endpoint. */
  notice?: string | null;
}

export interface QueryResultData {
  columns: Column[];
  rows: Row[];
  outcome: StatementOutcome;
}

export interface Column {
  name: string;
  /** The server's own type name. */
  type: string;
}

export type Row = Record<string, any>;

export interface QueryHistoryEntry {
  id: string;
  connection_id: string;
  query: string;
  executed_at: string;
  execution_time_ms: number;
  success: boolean;
  error?: string | null;
  outcome?: StatementOutcome | null;
}

// ---------------------------------------------------------------------------
// Cluster lifecycle
// ---------------------------------------------------------------------------

/**
 * Whether a node's process is alive. `Exited` means a pid file exists but that
 * process is gone — a crash or an unclean stop.
 */
export type ProcessState =
  | { state: 'running'; pid: number; uptime_seconds: number }
  | { state: 'exited'; pid: number };

export interface Endpoint {
  protocol: string;
  port: number;
  /** Whether the port accepted a TCP connection at `checked_at`. */
  reachable: boolean;
}

export interface ClusterNode {
  node_id: string;
  process: ProcessState;
  /** Read from the live process's command line. Empty when it is not running:
   *  the ports it would use are a guess, not an observation. */
  endpoints: Endpoint[];
  log_file: string;
}

export interface ClusterStatus {
  root: string;
  /** False when no cluster has ever been started in this checkout. */
  initialized: boolean;
  nodes: ClusterNode[];
  running_nodes: number;
  /** Nodes that are both alive and answering on at least one port. */
  serving_nodes: number;
  checked_at: string;
}

// ML types — these mirror `models.rs`.

export interface ModelInfo {
  id: string;
  name: string;
  model_type: string;
  status: ModelStatus;
  accuracy?: number | null;
  created_at: string;
  last_trained?: string | null;
  features: string[];
  target?: string | null;
  metadata: Record<string, any>;
}

export enum ModelStatus {
  Training = 'Training',
  Ready = 'Ready',
  Error = 'Error',
  Deleted = 'Deleted',
}

export interface MLFunctionInfo {
  name: string;
  category: string;
  description: string;
  parameters: MLParameter[];
  example: string;
}

export interface MLParameter {
  name: string;
  param_type: string;
  required: boolean;
  description: string;
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
  /** A single column. Multi-series charts are not built here, and the union
   *  that allowed for them only ever produced invalid row-index expressions. */
  y_axis: string;
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

