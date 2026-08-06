import { invoke } from '@tauri-apps/api/tauri';
import {
  ApiResponse,
  ClusterStatus,
  Connection,
  ConnectionInfo,
  ConnectionStatus,
  ConnectionTypeInfo,
  MLFunctionInfo,
  ModelInfo,
  QueryHistoryEntry,
  QueryRequest,
  QueryResult,
} from '@/types';

/**
 * Whether the Tauri IPC bridge is present.
 *
 * Outside it (a plain `vite dev` in a browser) no command can run. Rather than
 * substituting sample connections and rows — which look exactly like real ones
 * and have historically been mistaken for them — every call fails with a clear
 * message saying the desktop shell is required.
 */
export const isTauri = (): boolean => {
  try {
    return (
      typeof globalThis.window !== 'undefined' &&
      typeof (globalThis.window as any).__TAURI_IPC__ === 'function'
    );
  } catch {
    return false;
  }
};

const BROWSER_MODE_MESSAGE =
  'This action needs the Orbit Desktop application. The page is running in a ' +
  'plain browser, which has no connection to the database. Run `npm run dev` ' +
  '(which starts Tauri) or launch the built app.';

/** Invoke a Tauri command, unwrapping the `ApiResponse` envelope. */
const call = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
  if (!isTauri()) {
    throw new Error(BROWSER_MODE_MESSAGE);
  }

  const response = await invoke<ApiResponse<T>>(command, args);
  if (!response.success) {
    throw new Error(response.error || `${command} failed`);
  }
  // `data` is absent only for commands whose payload is genuinely empty.
  return response.data as T;
};

/** Service for communicating with the Tauri backend. */
export class TauriService {
  // ----------------------------- Connections -----------------------------

  static createConnection(connectionInfo: ConnectionInfo): Promise<string> {
    return call<string>('create_connection', { connectionInfo });
  }

  static testConnection(connectionInfo: ConnectionInfo): Promise<ConnectionStatus> {
    return call<ConnectionStatus>('test_connection', { connectionInfo });
  }

  static getConnections(): Promise<Connection[]> {
    return call<Connection[]>('get_connections');
  }

  /** Open a session now, so the UI can report whether a saved connection works. */
  static connect(connectionId: string): Promise<ConnectionStatus> {
    return call<ConnectionStatus>('connect', { connectionId });
  }

  static disconnect(connectionId: string): Promise<void> {
    return call<boolean>('disconnect', { connectionId }).then(() => undefined);
  }

  static deleteConnection(connectionId: string): Promise<void> {
    return call<boolean>('delete_connection', { connectionId }).then(() => undefined);
  }

  static listConnectionTypes(): Promise<ConnectionTypeInfo[]> {
    return call<ConnectionTypeInfo[]>('list_connection_types');
  }

  // ------------------------------- Queries -------------------------------

  static executeQuery(request: QueryRequest): Promise<QueryResult> {
    return call<QueryResult>('execute_query', { request });
  }

  /**
   * Ask for a plan. `analyze` runs the statement to collect real timings, so it
   * is off unless the caller asks: EXPLAIN ANALYZE on a DELETE deletes.
   */
  static explainQuery(request: QueryRequest, analyze = false): Promise<QueryResult> {
    return call<QueryResult>('explain_query', { request, analyze });
  }

  static getQueryHistory(connectionId: string, limit?: number): Promise<QueryHistoryEntry[]> {
    return call<QueryHistoryEntry[]>('get_query_history', { connectionId, limit });
  }

  // ------------------------------- Cluster -------------------------------

  static getClusterStatus(): Promise<ClusterStatus> {
    return call<ClusterStatus>('get_cluster_status');
  }

  static setClusterRoot(path: string): Promise<ClusterStatus> {
    return call<ClusterStatus>('set_cluster_root', { path });
  }

  /** Starts the cluster script and returns immediately; poll status for progress. */
  static startCluster(size: number): Promise<void> {
    return call<boolean>('start_cluster', { size }).then(() => undefined);
  }

  static stopCluster(): Promise<void> {
    return call<boolean>('stop_cluster').then(() => undefined);
  }

  /** Tail a node's log, or the start/stop control log when `nodeId` is omitted. */
  static getClusterLog(nodeId?: string, lines?: number): Promise<string> {
    return call<string>('get_cluster_log', { nodeId, lines });
  }

  // ------------------------------ ML models ------------------------------

  static listMlFunctions(): Promise<MLFunctionInfo[]> {
    return call<MLFunctionInfo[]>('list_ml_functions');
  }

  static listModels(connectionId: string): Promise<ModelInfo[]> {
    return call<ModelInfo[]>('list_models', { connectionId });
  }

  static getModelInfo(connectionId: string, modelName: string): Promise<ModelInfo> {
    return call<ModelInfo>('get_model_info', { connectionId, modelName });
  }

  static deleteModel(connectionId: string, modelName: string): Promise<void> {
    return call<boolean>('delete_model', { connectionId, modelName }).then(() => undefined);
  }

  // ------------------------------- System --------------------------------

  static getSystemInfo(): Promise<Record<string, any>> {
    return call<Record<string, any>>('get_system_info');
  }

  static saveSettings(settings: Record<string, any>): Promise<void> {
    return call<boolean>('save_settings', { settings }).then(() => undefined);
  }

  static loadSettings(): Promise<Record<string, any>> {
    return call<Record<string, any>>('load_settings');
  }

  static async showAboutDialog(): Promise<void> {
    if (!isTauri()) return;
    await invoke('show_about_dialog');
  }
}

/** Reduce anything thrown by a command to a message worth displaying. */
export const handleTauriError = (error: unknown): string => {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === 'object') {
    const candidate = error as { message?: unknown; error?: unknown };
    if (typeof candidate.message === 'string') return candidate.message;
    if (typeof candidate.error === 'string') return candidate.error;
  }
  return 'An unexpected error occurred';
};
