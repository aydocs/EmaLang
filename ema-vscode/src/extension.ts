import * as vscode from 'vscode';
import { spawn, ChildProcessWithoutNullStreams } from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

export function activate(context: vscode.ExtensionContext) {
    const diagnosticCollection = vscode.languages.createDiagnosticCollection('ema');
    const pendingTimers = new Map<string, NodeJS.Timeout>();
    const running = new Map<string, ChildProcessWithoutNullStreams>();
    const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    status.text = 'EMA: ready';
    status.show();

    const formatEma = (input: string): string => {
        const lines = input.replace(/\r\n/g, '\n').split('\n');
        let indent = 0;
        const out: string[] = [];

        let inHeredoc = false;
        let heredocDelim = '';

        const startsHeredoc = (line: string): string | null => {
            // Matches: html <<<TAG   or  css<<<TAG (allow whitespace)
            const m = line.match(/\b(html|css|js|php)\b\s*<<<\s*([A-Za-z_][A-Za-z0-9_]*)\s*$/);
            return m ? m[2] : null;
        };

        for (let i = 0; i < lines.length; i++) {
            const rawLine = lines[i];
            const line = rawLine.replace(/\s+$/g, ''); // trim right

            if (inHeredoc) {
                out.push(line);
                if (line.trim() === heredocDelim) {
                    inHeredoc = false;
                    heredocDelim = '';
                }
                continue;
            }

            const delim = startsHeredoc(line);
            if (delim) {
                out.push(' '.repeat(Math.max(0, indent) * 4) + line.trim());
                inHeredoc = true;
                heredocDelim = delim;
                continue;
            }

            const trimmed = line.trim();
            if (trimmed.length === 0) {
                out.push('');
                continue;
            }

            // De-indent on closing brace at line start.
            const startsWithClosing = trimmed.startsWith('}');
            const effectiveIndent = startsWithClosing ? Math.max(0, indent - 1) : indent;
            out.push(' '.repeat(effectiveIndent * 4) + trimmed);

            // Update indent after writing line.
            // Heuristic: count { and } on the line (outside heredoc only).
            const opens = (trimmed.match(/\{/g) || []).length;
            const closes = (trimmed.match(/\}/g) || []).length;
            indent = Math.max(0, indent + opens - closes);
        }

        return out.join('\n');
    };

    const resolveCommand = (workspaceFolder: string, filePath: string): { cmd: string, args: string[], cwd: string } => {
        const isWin = process.platform === 'win32';
        const compiled = path.join(workspaceFolder, 'target', 'debug', `ema_compiler${isWin ? '.exe' : ''}`);
        const configured = vscode.workspace.getConfiguration('ema').get<string>('compiler.path', '').trim();
        const useCargoFallback = vscode.workspace.getConfiguration('ema').get<boolean>('diagnostics.useCargoFallback', true);
        const strictEmbedded = vscode.workspace.getConfiguration('ema').get<boolean>('diagnostics.strictEmbeddedParsing', false);
        const strictArgs = strictEmbedded ? ['--strict-embedded'] : [];

        if (configured.length > 0) {
            return { cmd: configured, args: [filePath, '--check', '--json', ...strictArgs], cwd: workspaceFolder };
        }
        if (fs.existsSync(compiled)) {
            return { cmd: compiled, args: [filePath, '--check', '--json', ...strictArgs], cwd: workspaceFolder };
        }
        if (useCargoFallback) {
            return { cmd: 'cargo', args: ['run', '--quiet', '--bin', 'ema_compiler', '--', filePath, '--check', '--json', ...strictArgs], cwd: workspaceFolder };
        }
        return { cmd: compiled, args: [filePath, '--check', '--json', ...strictArgs], cwd: workspaceFolder };
    };

    const updateDiagnostics = (document: vscode.TextDocument) => {
        if (document.languageId !== 'ema') return;
        if (!vscode.workspace.getConfiguration('ema').get<boolean>('diagnostics.enabled', true)) return;

        const workspaceFolder = vscode.workspace.workspaceFolders?.[0].uri.fsPath || '';
        if (!workspaceFolder) return;

        const key = document.uri.toString();
        const prev = running.get(key);
        if (prev) {
            try { prev.kill(); } catch {}
            running.delete(key);
        }

        const { cmd, args, cwd } = resolveCommand(workspaceFolder, document.uri.fsPath);
        status.text = 'EMA: checking…';
        const p = spawn(cmd, args, { cwd, shell: false });
        running.set(key, p);

        let stdout = '';
        let stderr = '';
        p.stdout.on('data', (d) => { stdout += d.toString(); });
        p.stderr.on('data', (d) => { stderr += d.toString(); });

        p.on('close', (_code) => {
            if (running.get(key) === p) running.delete(key);
            diagnosticCollection.delete(document.uri);
            const diagnostics: vscode.Diagnostic[] = [];
            let parsedOk = false;

            if (stdout && stdout.trim().length > 0) {
                try {
                    const errors: any[] = JSON.parse(stdout);
                    const includeEmbeddedParse = vscode.workspace.getConfiguration('ema').get<boolean>('diagnostics.embeddedParseDiagnostics', true);
                    errors.forEach(err => {
                        if (!includeEmbeddedParse && typeof err.message === 'string' && err.message.startsWith('Error parsing ')) {
                            return;
                        }
                        const range = new vscode.Range(
                            new vscode.Position(err.line - 1, err.col - 1),
                            new vscode.Position(err.line - 1, err.col + 10) // Approx width
                        );
                        let severity = vscode.DiagnosticSeverity.Error;
                        if (typeof err.severity === 'string') {
                            severity = (err.severity === 'warning')
                                ? vscode.DiagnosticSeverity.Warning
                                : vscode.DiagnosticSeverity.Error;
                        } else {
                            // Backward-compatible heuristic (old compiler output)
                            const strictEmbedded = vscode.workspace.getConfiguration('ema').get<boolean>('diagnostics.strictEmbeddedParsing', false);
                            const isEmbeddedParse = typeof err.message === 'string' && err.message.startsWith('Error parsing ');
                            severity = (isEmbeddedParse && !strictEmbedded)
                                ? vscode.DiagnosticSeverity.Warning
                                : vscode.DiagnosticSeverity.Error;
                        }
                        diagnostics.push(new vscode.Diagnostic(range, err.message, severity));
                    });
                    parsedOk = true;
                } catch (e) {
                    // Not JSON or empty
                }
            }

            diagnosticCollection.set(document.uri, diagnostics);
            status.text = diagnostics.length === 0 ? 'EMA: OK' : `EMA: ${diagnostics.length} error(s)`;
            if (!parsedOk && stderr.trim().length > 0) {
                // Keep status helpful even when no JSON was produced
                status.text = 'EMA: check output (stderr)';
            }
        });
    };

    const scheduleDiagnostics = (document: vscode.TextDocument) => {
        const key = document.uri.toString();
        const existing = pendingTimers.get(key);
        if (existing) clearTimeout(existing);
        const debounceMs = vscode.workspace.getConfiguration('ema').get<number>('diagnostics.debounceMs', 500);
        pendingTimers.set(
            key,
            setTimeout(() => {
                pendingTimers.delete(key);
                updateDiagnostics(document);
            }, Math.max(0, debounceMs))
        );
    };

    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument(updateDiagnostics),
        vscode.workspace.onDidOpenTextDocument(updateDiagnostics),
        vscode.workspace.onDidChangeTextDocument(e => scheduleDiagnostics(e.document)),
        vscode.languages.registerDocumentFormattingEditProvider('ema', {
            provideDocumentFormattingEdits(document: vscode.TextDocument): vscode.TextEdit[] {
                const fullRange = new vscode.Range(
                    document.positionAt(0),
                    document.positionAt(document.getText().length)
                );
                const formatted = formatEma(document.getText());
                return [vscode.TextEdit.replace(fullRange, formatted)];
            }
        }),
        vscode.commands.registerCommand('ema.checkCurrentFile', () => {
            const doc = vscode.window.activeTextEditor?.document;
            if (doc) updateDiagnostics(doc);
        }),
        status,
        diagnosticCollection
    );

    // Initial check
    if (vscode.window.activeTextEditor) {
        updateDiagnostics(vscode.window.activeTextEditor.document);
    }
}

export function deactivate() {}
