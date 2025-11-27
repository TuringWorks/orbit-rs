import * as vscode from 'vscode';
import { ConnectionManager, ConnectionConfig } from './connectionManager';

export class ConnectionsView implements vscode.TreeDataProvider<ConnectionTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<ConnectionTreeItem | undefined | null | void> = new vscode.EventEmitter<ConnectionTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<ConnectionTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;

    constructor(
        private context: vscode.ExtensionContext,
        private connectionManager: ConnectionManager
    ) {
        connectionManager.onConnectionChange(() => {
            this.refresh();
        });
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: ConnectionTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: ConnectionTreeItem): Promise<ConnectionTreeItem[]> {
        if (!element) {
            // Root level - show all connections
            const configs = this.connectionManager.getConnections();
            const currentConnection = this.connectionManager.getCurrentConnection();

            if (configs.length === 0) {
                return [new ConnectionTreeItem(
                    'No connections configured',
                    vscode.TreeItemCollapsibleState.None,
                    'empty'
                )];
            }

            return configs.map(config => {
                const isConnected = currentConnection?.config.name === config.name;
                return new ConnectionTreeItem(
                    config.name,
                    vscode.TreeItemCollapsibleState.Collapsed,
                    'connection',
                    config,
                    isConnected
                );
            });
        }

        if (element.type === 'connection') {
            // Show connection details
            const config = element.config!;
            return [
                new ConnectionTreeItem(`Protocol: ${config.protocol.toUpperCase()}`, vscode.TreeItemCollapsibleState.None, 'detail'),
                new ConnectionTreeItem(`Host: ${config.host}:${config.port}`, vscode.TreeItemCollapsibleState.None, 'detail'),
                new ConnectionTreeItem(`Database: ${config.database || 'N/A'}`, vscode.TreeItemCollapsibleState.None, 'detail'),
                new ConnectionTreeItem(`Status: ${element.isConnected ? 'Connected' : 'Disconnected'}`, vscode.TreeItemCollapsibleState.None, 'detail'),
            ];
        }

        return [];
    }

    async addConnection(): Promise<void> {
        const config = await this.connectionManager.addConnection();
        if (config) {
            this.refresh();
            vscode.window.showInformationMessage(`Connection "${config.name}" added`);
        }
    }
}

class ConnectionTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
        public readonly type: 'connection' | 'detail' | 'empty',
        public readonly config?: ConnectionConfig,
        public readonly isConnected: boolean = false
    ) {
        super(label, collapsibleState);

        this.tooltip = this.label;
        this.contextValue = this.type;

        if (this.type === 'connection') {
            this.iconPath = new vscode.ThemeIcon(this.isConnected ? 'database' : 'circle-outline');
            this.description = this.isConnected ? 'Connected' : 'Disconnected';
        } else if (this.type === 'empty') {
            this.iconPath = new vscode.ThemeIcon('info');
        }
    }
}

