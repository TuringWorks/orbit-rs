import * as vscode from 'vscode';
import { ConnectionManager } from './connectionManager';
import { QueryExecutor } from './queryExecutor';
import { SchemaBrowser } from './schemaBrowser';
import { ResultsView } from './resultsView';
import { ConnectionsView } from './connectionsView';

let connectionManager: ConnectionManager;
let queryExecutor: QueryExecutor;
let schemaBrowser: SchemaBrowser;
let resultsView: ResultsView;
let connectionsView: ConnectionsView;

export function activate(context: vscode.ExtensionContext) {
    console.log('Orbit-RS extension is now active!');

    // Initialize components
    connectionManager = new ConnectionManager(context);
    queryExecutor = new QueryExecutor(connectionManager);
    schemaBrowser = new SchemaBrowser(connectionManager);
    resultsView = new ResultsView(context);
    connectionsView = new ConnectionsView(context, connectionManager);

    // Register commands
    const commands = [
        vscode.commands.registerCommand('orbit.connect', () => connectionManager.connect()),
        vscode.commands.registerCommand('orbit.disconnect', () => connectionManager.disconnect()),
        vscode.commands.registerCommand('orbit.executeQuery', () => queryExecutor.executeQuery()),
        vscode.commands.registerCommand('orbit.executeSelection', () => queryExecutor.executeSelection()),
        vscode.commands.registerCommand('orbit.addConnection', () => connectionsView.addConnection()),
        vscode.commands.registerCommand('orbit.browseSchema', () => schemaBrowser.browse()),
        vscode.commands.registerCommand('orbit.showResults', () => resultsView.show()),
    ];

    commands.forEach(cmd => context.subscriptions.push(cmd));

    // Register views
    context.subscriptions.push(
        vscode.window.createTreeView('orbit.connectionsView', {
            treeDataProvider: connectionsView,
            showCollapseAll: true
        })
    );

    context.subscriptions.push(
        vscode.window.createTreeView('orbit.schemaView', {
            treeDataProvider: schemaBrowser,
            showCollapseAll: true
        })
    );

    // Status bar item
    const statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBarItem.command = 'orbit.showConnections';
    statusBarItem.text = "$(database) Orbit-RS";
    statusBarItem.tooltip = "Orbit-RS: Click to manage connections";
    statusBarItem.show();
    context.subscriptions.push(statusBarItem);

    // Update status bar on connection changes
    connectionManager.onConnectionChange((connected) => {
        if (connected) {
            statusBarItem.text = "$(database) Orbit-RS: Connected";
            statusBarItem.backgroundColor = undefined;
        } else {
            statusBarItem.text = "$(database) Orbit-RS: Disconnected";
            statusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
        }
    });

    // Language detection for query execution
    context.subscriptions.push(
        vscode.workspace.onDidChangeTextDocument((e) => {
            // Auto-detect protocol based on file language
            const languageId = e.document.languageId;
            if (['orbitql', 'sql', 'cypher', 'aql'].includes(languageId)) {
                // Update query executor with current language
                queryExecutor.setLanguage(languageId);
            }
        })
    );
}

export function deactivate() {
    if (connectionManager) {
        connectionManager.disconnect();
    }
}

