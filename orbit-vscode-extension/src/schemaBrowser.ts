import * as vscode from 'vscode';
import { ConnectionManager } from './connectionManager';

export class SchemaBrowser implements vscode.TreeDataProvider<SchemaItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<SchemaItem | undefined | null | void> = new vscode.EventEmitter<SchemaItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<SchemaItem | undefined | null | void> = this._onDidChangeTreeData.event;

    constructor(private connectionManager: ConnectionManager) {
        connectionManager.onConnectionChange(() => {
            this.refresh();
        });
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: SchemaItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: SchemaItem): Promise<SchemaItem[]> {
        if (!this.connectionManager.isConnected()) {
            return [new SchemaItem('Not connected', vscode.TreeItemCollapsibleState.None)];
        }

        if (!element) {
            // Root level - show databases/schemas
            return await this.getDatabases();
        }

        if (element.type === 'database') {
            return await this.getTables(element.name);
        }

        if (element.type === 'table') {
            return await this.getColumns(element.parent?.name || '', element.name);
        }

        return [];
    }

    private async getDatabases(): Promise<SchemaItem[]> {
        const connection = this.connectionManager.getCurrentConnection();
        if (!connection) {
            return [];
        }

        try {
            const protocol = connection.config.protocol;
            let query = '';

            switch (protocol) {
                case 'postgres':
                    query = "SELECT datname FROM pg_database WHERE datistemplate = false";
                    break;
                case 'mysql':
                    query = "SHOW DATABASES";
                    break;
                case 'cql':
                    query = "SELECT keyspace_name FROM system_schema.keyspaces";
                    break;
                default:
                    return [new SchemaItem('Schema browsing not supported for this protocol', vscode.TreeItemCollapsibleState.None)];
            }

            const results = await connection.execute(query);
            return results.map((row: any) => {
                const dbName = row.datname || row.Database || row.keyspace_name || Object.values(row)[0];
                return new SchemaItem(
                    dbName,
                    vscode.TreeItemCollapsibleState.Collapsed,
                    'database',
                    dbName
                );
            });
        } catch (error: any) {
            return [new SchemaItem(`Error: ${error.message}`, vscode.TreeItemCollapsibleState.None)];
        }
    }

    private async getTables(database: string): Promise<SchemaItem[]> {
        const connection = this.connectionManager.getCurrentConnection();
        if (!connection) {
            return [];
        }

        try {
            const protocol = connection.config.protocol;
            let query = '';

            switch (protocol) {
                case 'postgres':
                    query = `SELECT table_name FROM information_schema.tables WHERE table_schema = 'public'`;
                    break;
                case 'mysql':
                    query = `SHOW TABLES FROM ${database}`;
                    break;
                case 'cql':
                    query = `SELECT table_name FROM system_schema.tables WHERE keyspace_name = '${database}'`;
                    break;
                default:
                    return [];
            }

            const results = await connection.execute(query);
            return results.map((row: any) => {
                const tableName = row.table_name || row[`Tables_in_${database}`] || Object.values(row)[0];
                return new SchemaItem(
                    tableName,
                    vscode.TreeItemCollapsibleState.Collapsed,
                    'table',
                    tableName,
                    new SchemaItem(database, vscode.TreeItemCollapsibleState.None, 'database', database)
                );
            });
        } catch (error: any) {
            return [new SchemaItem(`Error: ${error.message}`, vscode.TreeItemCollapsibleState.None)];
        }
    }

    private async getColumns(database: string, table: string): Promise<SchemaItem[]> {
        const connection = this.connectionManager.getCurrentConnection();
        if (!connection) {
            return [];
        }

        try {
            const protocol = connection.config.protocol;
            let query = '';

            switch (protocol) {
                case 'postgres':
                    query = `SELECT column_name, data_type FROM information_schema.columns WHERE table_name = '${table}'`;
                    break;
                case 'mysql':
                    query = `SHOW COLUMNS FROM ${database}.${table}`;
                    break;
                case 'cql':
                    query = `SELECT column_name, type FROM system_schema.columns WHERE keyspace_name = '${database}' AND table_name = '${table}'`;
                    break;
                default:
                    return [];
            }

            const results = await connection.execute(query);
            return results.map((row: any) => {
                const colName = row.column_name || row.Field || Object.values(row)[0];
                const colType = row.data_type || row.Type || row.type || '';
                return new SchemaItem(
                    `${colName}: ${colType}`,
                    vscode.TreeItemCollapsibleState.None,
                    'column',
                    colName,
                    new SchemaItem(table, vscode.TreeItemCollapsibleState.None, 'table', table)
                );
            });
        } catch (error: any) {
            return [new SchemaItem(`Error: ${error.message}`, vscode.TreeItemCollapsibleState.None)];
        }
    }

    async browse(): Promise<void> {
        this.refresh();
    }
}

class SchemaItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly type: 'database' | 'table' | 'column' | 'none' = 'none',
        public readonly name: string = label,
        public readonly parent?: SchemaItem
    ) {
        super(label, collapsibleState);

        this.tooltip = `${this.type}: ${this.label}`;
        this.contextValue = this.type;

        // Set icons
        switch (this.type) {
            case 'database':
                this.iconPath = new vscode.ThemeIcon('database');
                break;
            case 'table':
                this.iconPath = new vscode.ThemeIcon('table');
                break;
            case 'column':
                this.iconPath = new vscode.ThemeIcon('symbol-field');
                break;
        }
    }
}

