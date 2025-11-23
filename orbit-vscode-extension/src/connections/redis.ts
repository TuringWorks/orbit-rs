import { Connection, ConnectionConfig } from '../connectionManager';
import { createClient } from 'redis';

export class RedisConnection implements Connection {
    public config: ConnectionConfig;
    private client: ReturnType<typeof createClient> | null = null;

    constructor(config: ConnectionConfig) {
        this.config = config;
    }

    async connect(): Promise<void> {
        this.client = createClient({
            socket: {
                host: this.config.host,
                port: this.config.port,
                connectTimeout: (this.config.timeout || 30) * 1000,
            },
            password: this.config.password,
            database: (this.config as any).db || 0,
        });

        await this.client.connect();
    }

    async disconnect(): Promise<void> {
        if (this.client) {
            await this.client.quit();
            this.client = null;
        }
    }

    isConnected(): boolean {
        return this.client !== null && this.client.isOpen;
    }

    async execute(query: string, params?: any): Promise<any> {
        if (!this.client) {
            throw new Error('Not connected');
        }

        // Parse Redis command
        const parts = query.trim().split(/\s+/);
        const command = parts[0].toUpperCase();
        const args = parts.slice(1);

        // Execute command
        const result = await (this.client as any)[command.toLowerCase()](...args);
        return result;
    }
}

