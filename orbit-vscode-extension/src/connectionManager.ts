import * as vscode from 'vscode';
import { PostgresConnection } from './connections/postgres';
import { MySQLConnection } from './connections/mysql';
import { CQLConnection } from './connections/cql';
import { RedisConnection } from './connections/redis';
import { CypherConnection } from './connections/cypher';
import { AQLConnection } from './connections/aql';
import { MCPConnection } from './connections/mcp';

export type Protocol = 'postgres' | 'mysql' | 'cql' | 'redis' | 'cypher' | 'aql' | 'mcp';

export interface ConnectionConfig {
    name: string;
    protocol: Protocol;
    host: string;
    port: number;
    username?: string;
    password?: string;
    database?: string;
    ssl?: boolean;
    timeout?: number;
}

export interface Connection {
    config: ConnectionConfig;
    connect(): Promise<void>;
    disconnect(): Promise<void>;
    isConnected(): boolean;
    execute(query: string, params?: any): Promise<any>;
}

export class ConnectionManager {
    private currentConnection: Connection | null = null;
    private connectionChangeEmitter = new vscode.EventEmitter<boolean>();
    public readonly onConnectionChange = this.connectionChangeEmitter.event;

    constructor(private context: vscode.ExtensionContext) {}

    async connect(config?: ConnectionConfig): Promise<void> {
        if (!config) {
            config = await this.selectConnection();
            if (!config) {
                return;
            }
        }

        try {
            // Disconnect existing connection
            if (this.currentConnection) {
                await this.disconnect();
            }

            // Create new connection based on protocol
            this.currentConnection = this.createConnection(config);
            await this.currentConnection.connect();

            vscode.window.showInformationMessage(`Connected to Orbit-RS via ${config.protocol.toUpperCase()}`);
            this.connectionChangeEmitter.fire(true);
        } catch (error: any) {
            vscode.window.showErrorMessage(`Failed to connect: ${error.message}`);
            throw error;
        }
    }

    async disconnect(): Promise<void> {
        if (this.currentConnection) {
            await this.currentConnection.disconnect();
            this.currentConnection = null;
            this.connectionChangeEmitter.fire(false);
            vscode.window.showInformationMessage('Disconnected from Orbit-RS');
        }
    }

    isConnected(): boolean {
        return this.currentConnection?.isConnected() ?? false;
    }

    getCurrentConnection(): Connection | null {
        return this.currentConnection;
    }

    getCurrentProtocol(): Protocol | null {
        return this.currentConnection?.config.protocol ?? null;
    }

    private createConnection(config: ConnectionConfig): Connection {
        switch (config.protocol) {
            case 'postgres':
                return new PostgresConnection(config);
            case 'mysql':
                return new MySQLConnection(config);
            case 'cql':
                return new CQLConnection(config);
            case 'redis':
                return new RedisConnection(config);
            case 'cypher':
                return new CypherConnection(config);
            case 'aql':
                return new AQLConnection(config);
            case 'mcp':
                return new MCPConnection(config);
            default:
                throw new Error(`Unsupported protocol: ${config.protocol}`);
        }
    }

    private async selectConnection(): Promise<ConnectionConfig | undefined> {
        const configs = this.getConnections();
        
        if (configs.length === 0) {
            const action = await vscode.window.showInformationMessage(
                'No connections configured. Add a connection?',
                'Add Connection'
            );
            if (action === 'Add Connection') {
                return await this.addConnection();
            }
            return undefined;
        }

        const items = configs.map(config => ({
            label: config.name,
            description: `${config.protocol.toUpperCase()} @ ${config.host}:${config.port}`,
            config
        }));

        const selected = await vscode.window.showQuickPick(items, {
            placeHolder: 'Select a connection'
        });

        return selected?.config;
    }

    async addConnection(): Promise<ConnectionConfig | undefined> {
        const name = await vscode.window.showInputBox({
            prompt: 'Connection name',
            placeHolder: 'My Orbit Connection'
        });
        if (!name) return undefined;

        const protocol = await vscode.window.showQuickPick<vscode.QuickPickItem & { protocol: Protocol }>([
            { label: 'PostgreSQL', protocol: 'postgres' },
            { label: 'MySQL', protocol: 'mysql' },
            { label: 'CQL/Cassandra', protocol: 'cql' },
            { label: 'Redis/RESP', protocol: 'redis' },
            { label: 'Cypher/Bolt', protocol: 'cypher' },
            { label: 'AQL/ArangoDB', protocol: 'aql' },
            { label: 'MCP', protocol: 'mcp' },
        ], { placeHolder: 'Select protocol' });
        if (!protocol) return undefined;

        const host = await vscode.window.showInputBox({
            prompt: 'Host',
            value: '127.0.0.1'
        });
        if (!host) return undefined;

        const portStr = await vscode.window.showInputBox({
            prompt: 'Port',
            value: this.getDefaultPort(protocol.protocol).toString()
        });
        if (!portStr) return undefined;

        const username = await vscode.window.showInputBox({
            prompt: 'Username (optional)',
            ignoreFocusOut: true
        });

        const password = await vscode.window.showInputBox({
            prompt: 'Password (optional)',
            password: true,
            ignoreFocusOut: true
        });

        const database = await vscode.window.showInputBox({
            prompt: 'Database (optional)',
            ignoreFocusOut: true
        });

        const config: ConnectionConfig = {
            name,
            protocol: protocol.protocol,
            host,
            port: parseInt(portStr, 10),
            username,
            password,
            database,
        };

        // Save connection
        const configs = this.getConnections();
        configs.push(config);
        await this.saveConnections(configs);

        return config;
    }

    private getDefaultPort(protocol: Protocol): number {
        const ports: Record<Protocol, number> = {
            postgres: 5432,
            mysql: 3306,
            cql: 9042,
            redis: 6379,
            cypher: 7687,
            aql: 8529,
            mcp: 8080,
        };
        return ports[protocol];
    }

    getConnections(): ConnectionConfig[] {
        const config = vscode.workspace.getConfiguration('orbit');
        return config.get<ConnectionConfig[]>('connections', []);
    }

    private async saveConnections(configs: ConnectionConfig[]): Promise<void> {
        const config = vscode.workspace.getConfiguration('orbit');
        await config.update('connections', configs, vscode.ConfigurationTarget.Global);
    }
}

