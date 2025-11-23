import { Connection, ConnectionConfig } from '../connectionManager';
import neo4j from 'neo4j-driver';

export class CypherConnection implements Connection {
    public config: ConnectionConfig;
    private driver: neo4j.Driver | null = null;
    private session: neo4j.Session | null = null;

    constructor(config: ConnectionConfig) {
        this.config = config;
    }

    async connect(): Promise<void> {
        const uri = `bolt://${this.config.host}:${this.config.port}`;
        const auth = this.config.username && this.config.password
            ? neo4j.auth.basic(this.config.username, this.config.password)
            : neo4j.auth.none();

        this.driver = neo4j.driver(uri, auth);
        this.session = this.driver.session({
            database: this.config.database || 'neo4j',
        });
    }

    async disconnect(): Promise<void> {
        if (this.session) {
            await this.session.close();
            this.session = null;
        }
        if (this.driver) {
            await this.driver.close();
            this.driver = null;
        }
    }

    isConnected(): boolean {
        return this.driver !== null && this.session !== null;
    }

    async execute(query: string, params?: any): Promise<any> {
        if (!this.session) {
            throw new Error('Not connected');
        }

        const result = await this.session.run(query, params || {});
        return result.records.map(record => {
            const obj: any = {};
            record.keys.forEach(key => {
                obj[key] = record.get(key);
            });
            return obj;
        });
    }
}

