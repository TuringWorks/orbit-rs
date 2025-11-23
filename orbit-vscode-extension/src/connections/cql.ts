import { Connection, ConnectionConfig } from '../connectionManager';
import { Client } from 'cassandra-driver';

export class CQLConnection implements Connection {
    public config: ConnectionConfig;
    private client: Client | null = null;

    constructor(config: ConnectionConfig) {
        this.config = config;
    }

    async connect(): Promise<void> {
        const authProvider = this.config.username && this.config.password
            ? { username: this.config.username, password: this.config.password }
            : undefined;

        this.client = new Client({
            contactPoints: [this.config.host],
            port: this.config.port,
            localDataCenter: 'datacenter1',
            authProvider,
            socketOptions: {
                connectTimeout: (this.config.timeout || 30) * 1000,
            },
        });

        await this.client.connect();
        
        if (this.config.database) {
            await this.client.execute(`USE ${this.config.database}`);
        }
    }

    async disconnect(): Promise<void> {
        if (this.client) {
            await this.client.shutdown();
            this.client = null;
        }
    }

    isConnected(): boolean {
        return this.client !== null;
    }

    async execute(query: string, params?: any): Promise<any> {
        if (!this.client) {
            throw new Error('Not connected');
        }

        const result = await this.client.execute(query, params, { prepare: true });
        return result.rows.map(row => row.toObject());
    }
}

