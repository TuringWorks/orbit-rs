import * as vscode from 'vscode';

export interface QueryResult {
    query: string;
    results: any;
    executionTime: number;
    protocol: string;
    language: string;
}

export class ResultsView {
    private panel: vscode.WebviewPanel | undefined;

    constructor(private context: vscode.Uri) {}

    async showResults(result: QueryResult): Promise<void> {
        if (!this.panel) {
            this.panel = vscode.window.createWebviewPanel(
                'orbitResults',
                'Query Results',
                vscode.ViewColumn.Beside,
                {
                    enableScripts: true,
                    retainContextWhenHidden: true
                }
            );

            this.panel.onDidDispose(() => {
                this.panel = undefined;
            });
        }

        this.panel.webview.html = this.getWebviewContent(result);
        this.panel.reveal();
    }

    show(): void {
        if (this.panel) {
            this.panel.reveal();
        }
    }

    private getWebviewContent(result: QueryResult): string {
        const results = Array.isArray(result.results) ? result.results : [result.results];
        const maxResults = vscode.workspace.getConfiguration('orbit').get<number>('maxResults', 1000);
        const displayResults = results.slice(0, maxResults);
        const hasMore = results.length > maxResults;

        // Generate table HTML
        let tableHtml = '';
        if (displayResults.length > 0) {
            const columns = Object.keys(displayResults[0]);
            tableHtml = `
                <table style="border-collapse: collapse; width: 100%; font-family: monospace;">
                    <thead>
                        <tr style="background-color: var(--vscode-editor-background); border-bottom: 2px solid var(--vscode-panel-border);">
                            ${columns.map(col => `<th style="padding: 8px; text-align: left; border: 1px solid var(--vscode-panel-border);">${this.escapeHtml(col)}</th>`).join('')}
                        </tr>
                    </thead>
                    <tbody>
                        ${displayResults.map(row => `
                            <tr style="border-bottom: 1px solid var(--vscode-panel-border);">
                                ${columns.map(col => `<td style="padding: 8px; border: 1px solid var(--vscode-panel-border);">${this.formatValue(row[col])}</td>`).join('')}
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            `;
        } else {
            tableHtml = '<p>Query executed successfully. No results returned.</p>';
        }

        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Query Results</title>
    <style>
        body {
            font-family: var(--vscode-font-family);
            padding: 20px;
            color: var(--vscode-foreground);
            background-color: var(--vscode-editor-background);
        }
        .header {
            margin-bottom: 20px;
            padding: 10px;
            background-color: var(--vscode-editor-background);
            border: 1px solid var(--vscode-panel-border);
            border-radius: 4px;
        }
        .info {
            display: flex;
            gap: 20px;
            margin-bottom: 10px;
        }
        .info-item {
            display: flex;
            flex-direction: column;
        }
        .info-label {
            font-size: 12px;
            color: var(--vscode-descriptionForeground);
        }
        .info-value {
            font-weight: bold;
        }
        .query {
            margin-top: 10px;
            padding: 10px;
            background-color: var(--vscode-textCodeBlock-background);
            border-radius: 4px;
            font-family: monospace;
            font-size: 12px;
            white-space: pre-wrap;
            word-break: break-all;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 20px;
        }
        th, td {
            padding: 8px;
            text-align: left;
            border: 1px solid var(--vscode-panel-border);
        }
        th {
            background-color: var(--vscode-editor-background);
            font-weight: bold;
        }
        tr:nth-child(even) {
            background-color: var(--vscode-textCodeBlock-background);
        }
        .warning {
            margin-top: 10px;
            padding: 10px;
            background-color: var(--vscode-inputValidation-warningBackground);
            border: 1px solid var(--vscode-inputValidation-warningBorder);
            border-radius: 4px;
            color: var(--vscode-inputValidation-warningForeground);
        }
    </style>
</head>
<body>
    <div class="header">
        <div class="info">
            <div class="info-item">
                <span class="info-label">Protocol</span>
                <span class="info-value">${this.escapeHtml(result.protocol.toUpperCase())}</span>
            </div>
            <div class="info-item">
                <span class="info-label">Language</span>
                <span class="info-value">${this.escapeHtml(result.language.toUpperCase())}</span>
            </div>
            <div class="info-item">
                <span class="info-label">Execution Time</span>
                <span class="info-value">${result.executionTime}ms</span>
            </div>
            <div class="info-item">
                <span class="info-label">Rows</span>
                <span class="info-value">${results.length}${hasMore ? ` (showing first ${maxResults})` : ''}</span>
            </div>
        </div>
        <div class="query">${this.escapeHtml(result.query)}</div>
    </div>
    ${hasMore ? `<div class="warning">⚠️ Results truncated. Showing first ${maxResults} of ${results.length} rows.</div>` : ''}
    ${tableHtml}
</body>
</html>`;
    }

    private escapeHtml(text: string): string {
        const div = { innerHTML: '' };
        return String(text)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#039;');
    }

    private formatValue(value: any): string {
        if (value === null || value === undefined) {
            return '<em>NULL</em>';
        }
        if (typeof value === 'object') {
            return '<pre>' + this.escapeHtml(JSON.stringify(value, null, 2)) + '</pre>';
        }
        return this.escapeHtml(String(value));
    }
}

