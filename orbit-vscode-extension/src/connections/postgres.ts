import { Connection, ConnectionConfig } from '../connectionManager';
import * as pg from 'pg';

export class PostgresConnection implements Connection {
    public config: ConnectionConfig;
    private client: pg.Client | null = null;

    constructor(config: ConnectionConfig) {
        this.config = config;
    }

    async connect(): Promise<void> {
        this.client = new pg.Client({
            host: this.config.host,
            port: this.config.port,
            user: this.config.username || 'orbit',
            password: this.config.password || '',
            database: this.config.database || 'postgres',
            connectionTimeoutMillis: (this.config.timeout || 30) * 1000,
        });

        await this.client.connect();
    }

    async disconnect(): Promise<void> {
        if (this.client) {
            await this.client.end();
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

        const result = await this.client.query(query, params);
        return result.rows;
    }
}

