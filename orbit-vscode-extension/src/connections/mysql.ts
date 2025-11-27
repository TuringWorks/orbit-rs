import { Connection, ConnectionConfig } from '../connectionManager';
import * as mysql from 'mysql2/promise';

export class MySQLConnection implements Connection {
    public config: ConnectionConfig;
    private connection: mysql.Connection | null = null;

    constructor(config: ConnectionConfig) {
        this.config = config;
    }

    async connect(): Promise<void> {
        this.connection = await mysql.createConnection({
            host: this.config.host,
            port: this.config.port,
            user: this.config.username || 'orbit',
            password: this.config.password || '',
            database: this.config.database || 'orbit',
            connectTimeout: (this.config.timeout || 30) * 1000,
        });
    }

    async disconnect(): Promise<void> {
        if (this.connection) {
            await this.connection.end();
            this.connection = null;
        }
    }

    isConnected(): boolean {
        return this.connection !== null;
    }

    async execute(query: string, params?: any): Promise<any> {
        if (!this.connection) {
            throw new Error('Not connected');
        }

        const [rows] = await this.connection.execute(query, params);
        return rows as any[];
    }
}

