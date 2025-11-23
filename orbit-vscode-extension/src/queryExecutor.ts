import * as vscode from 'vscode';
import { ConnectionManager } from './connectionManager';
import { ResultsView } from './resultsView';

export class QueryExecutor {
    private currentLanguage: string = 'sql';
    private resultsView: ResultsView;

    constructor(
        private connectionManager: ConnectionManager,
        resultsView?: ResultsView
    ) {
        this.resultsView = resultsView || new ResultsView(vscode.workspace.workspaceFolders?.[0]?.uri || vscode.Uri.file(''));
    }

    setLanguage(languageId: string): void {
        this.currentLanguage = languageId;
    }

    async executeQuery(): Promise<void> {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showWarningMessage('No active editor');
            return;
        }

        const document = editor.document;
        const query = document.getText();
        
        if (!query.trim()) {
            vscode.window.showWarningMessage('No query to execute');
            return;
        }

        await this.execute(query, document.languageId);
    }

    async executeSelection(): Promise<void> {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showWarningMessage('No active editor');
            return;
        }

        const selection = editor.selection;
        const document = editor.document;
        const query = document.getText(selection.isEmpty ? undefined : selection);

        if (!query.trim()) {
            vscode.window.showWarningMessage('No query selected');
            return;
        }

        await this.execute(query, document.languageId);
    }

    private async execute(query: string, languageId: string): Promise<void> {
        if (!this.connectionManager.isConnected()) {
            const action = await vscode.window.showWarningMessage(
                'Not connected to Orbit-RS. Connect now?',
                'Connect'
            );
            if (action === 'Connect') {
                await this.connectionManager.connect();
            } else {
                return;
            }
        }

        const connection = this.connectionManager.getCurrentConnection();
        if (!connection) {
            vscode.window.showErrorMessage('No active connection');
            return;
        }

        // Show progress
        await vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: "Executing query",
            cancellable: false
        }, async (progress) => {
            try {
                progress.report({ increment: 0, message: "Executing..." });
                
                const startTime = Date.now();
                const results = await connection.execute(query);
                const executionTime = Date.now() - startTime;

                progress.report({ increment: 100, message: "Complete" });

                // Display results
                await this.resultsView.showResults({
                    query,
                    results,
                    executionTime,
                    protocol: connection.config.protocol,
                    language: languageId,
                });

                vscode.window.showInformationMessage(
                    `Query executed successfully in ${executionTime}ms`
                );
            } catch (error: any) {
                vscode.window.showErrorMessage(`Query execution failed: ${error.message}`);
                throw error;
            }
        });
    }
}

