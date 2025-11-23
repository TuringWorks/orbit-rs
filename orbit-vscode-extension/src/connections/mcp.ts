import { Connection, ConnectionConfig } from '../connectionManager';
import axios from 'axios';

export class MCPConnection implements Connection {
    public config: ConnectionConfig;
    private baseUrl: string;

    constructor(config: ConnectionConfig) {
        this.config = config;
        this.baseUrl = (config as any).base_url || `http://${config.host}:${config.port}`;
    }

    async connect(): Promise<void> {
        // MCP doesn't require persistent connection
        // Just verify the server is reachable
        try {
            const response = await axios.get(`${this.baseUrl}/health`, {
                timeout: (this.config.timeout || 30) * 1000,
            });
            if (response.status !== 200) {
                throw new Error('MCP server health check failed');
            }
        } catch (error: any) {
            // If health endpoint doesn't exist, that's okay
            // We'll discover during actual query execution
        }
    }

    async disconnect(): Promise<void> {
        // MCP doesn't require persistent connection
    }

    isConnected(): boolean {
        return true; // MCP is stateless
    }

    async execute(query: string, params?: any): Promise<any> {
        try {
            const response = await axios.post(
                `${this.baseUrl}/mcp`,
                {
                    jsonrpc: '2.0',
                    id: 1,
                    method: query,
                    params: params || {},
                },
                {
                    timeout: (this.config.timeout || 30) * 1000,
                }
            );

            if (response.data.error) {
                throw new Error(`MCP error: ${response.data.error.message}`);
            }

            return response.data.result;
        } catch (error: any) {
            if (error.response) {
                throw new Error(`MCP request failed: ${error.response.data.message || error.message}`);
            }
            throw new Error(`MCP request failed: ${error.message}`);
        }
    }
}

