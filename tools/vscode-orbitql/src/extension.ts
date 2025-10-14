import {
    ExtensionContext,
    workspace,
    window,
    commands
} from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export async function activate(context: ExtensionContext) {
    // Get configuration
    const config = workspace.getConfiguration('orbitql');
    const serverPath = config.get<string>('server.path', 'orbitql-lsp');
    const serverArgs = config.get<string[]>('server.args', []);
    const traceLevel = config.get<string>('trace.server', 'off');

    // Server options - how to start the language server
    const serverOptions: ServerOptions = {
        command: serverPath,
        args: serverArgs,
        transport: TransportKind.stdio,
        options: {
            env: {
                ...process.env,
                RUST_LOG: traceLevel === 'verbose' ? 'debug' : 'info'
            }
        }
    };

    // Client options - what features to enable
    const clientOptions: LanguageClientOptions = {
        // Register the server for OrbitQL documents
        documentSelector: [
            { scheme: 'file', language: 'orbitql' },
            { scheme: 'untitled', language: 'orbitql' }
        ],
        synchronize: {
            // Notify the server about file changes to OrbitQL files
            fileEvents: workspace.createFileSystemWatcher('**/*.{oql,orbitql}')
        },
        // Pass configuration to the server
        initializationOptions: {
            enableCompletion: config.get<boolean>('completion.enabled', true),
            enableDiagnostics: config.get<boolean>('diagnostics.enabled', true),
            enableHover: config.get<boolean>('hover.enabled', true),
            enableFormatting: config.get<boolean>('formatting.enabled', true)
        }
    };

    // Create the language client
    client = new LanguageClient(
        'orbitql-lsp',
        'OrbitQL Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client and server
    try {
        await client.start();
        
        window.showInformationMessage('OrbitQL Language Server started successfully');

        // Register additional commands
        registerCommands(context);

    } catch (error) {
        window.showErrorMessage(`Failed to start OrbitQL Language Server: ${error}`);
        console.error('Failed to start LSP client:', error);
    }
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

function registerCommands(context: ExtensionContext) {
    // Command to restart the language server
    const restartCommand = commands.registerCommand('orbitql.restart', async () => {
        if (client) {
            await client.stop();
            await client.start();
            window.showInformationMessage('OrbitQL Language Server restarted');
        }
    });

    // Command to show server status
    const statusCommand = commands.registerCommand('orbitql.status', () => {
        if (client) {
            const state = client.state;
            let statusText = 'Unknown';
            
            switch (state) {
                case 0: statusText = 'Stopped'; break;
                case 1: statusText = 'Starting'; break;
                case 2: statusText = 'Running'; break;
                case 3: statusText = 'Stopping'; break;
            }
            
            window.showInformationMessage(`OrbitQL Language Server Status: ${statusText}`);
        } else {
            window.showWarningMessage('OrbitQL Language Server is not initialized');
        }
    });

    // Command to format the current OrbitQL document
    const formatCommand = commands.registerCommand('orbitql.format', async () => {
        const editor = window.activeTextEditor;
        if (editor && editor.document.languageId === 'orbitql') {
            await commands.executeCommand('editor.action.formatDocument');
        } else {
            window.showWarningMessage('No active OrbitQL document to format');
        }
    });

    // Command to validate the current OrbitQL document
    const validateCommand = commands.registerCommand('orbitql.validate', async () => {
        const editor = window.activeTextEditor;
        if (editor && editor.document.languageId === 'orbitql') {
            // Trigger diagnostics refresh
            await commands.executeCommand('editor.action.marker.next');
            window.showInformationMessage('OrbitQL document validated');
        } else {
            window.showWarningMessage('No active OrbitQL document to validate');
        }
    });

    context.subscriptions.push(restartCommand, statusCommand, formatCommand, validateCommand);
}

// Handle configuration changes
workspace.onDidChangeConfiguration(event => {
    if (event.affectsConfiguration('orbitql')) {
        window.showInformationMessage(
            'OrbitQL configuration changed. Restart the language server to apply changes.',
            'Restart'
        ).then(selection => {
            if (selection === 'Restart') {
                commands.executeCommand('orbitql.restart');
            }
        });
    }
});
