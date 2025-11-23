import { Connection, ConnectionConfig } from '../connectionManager';
import { Database } from 'arangojs';

export class AQLConnection implements Connection {
    public config: ConnectionConfig;
    private db: Database | null = null;

    constructor(config: ConnectionConfig) {
        this.config = config;
    }

    async connect(): Promise<void> {
        const url = `http://${this.config.host}:${this.config.port}`;
        this.db = new Database({
            url,
            auth: this.config.username && this.config.password
                ? { username: this.config.username, password: this.config.password }
                : undefined,
        });

        if (this.config.database) {
            this.db.useDatabase(this.config.database);
        }
    }

    async disconnect(): Promise<void> {
        // ArangoDB client doesn't have explicit disconnect
        this.db = null;
    }

    isConnected(): boolean {
        return this.db !== null;
    }

    async execute(query: string, params?: any): Promise<any> {
        if (!this.db) {
            throw new Error('Not connected');
        }

        const cursor = await this.db.query(query, { bindVars: params || {} });
        const results = await cursor.all();
        return results;
    }
}

