const vscode = require("vscode");
const { exec } = require("child_process");
const path = require("path");

let diagnosticCollection;

function activate(context) {
  diagnosticCollection =
    vscode.languages.createDiagnosticCollection("llts");
  context.subscriptions.push(diagnosticCollection);

  const buildCmd = vscode.commands.registerCommand("llts.build", () => {
    runCompiler(false);
  });

  const runCmd = vscode.commands.registerCommand("llts.run", () => {
    runCompiler(true);
  });

  context.subscriptions.push(buildCmd, runCmd);
}

function runCompiler(shouldRun) {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showErrorMessage("No active file");
    return;
  }

  const filePath = editor.document.fileName;
  if (!filePath.endsWith(".ts")) {
    vscode.window.showErrorMessage("Active file is not a .ts file");
    return;
  }

  const workspaceFolder =
    vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || path.dirname(filePath);
  const outPath = path.join(workspaceFolder, "a.out");
  const cmd = `llts "${filePath}" -o "${outPath}"`;

  diagnosticCollection.clear();

  const terminal = vscode.window.createTerminal({ name: "LLTS" });
  terminal.show();

  if (shouldRun) {
    terminal.sendText(`${cmd} && "${outPath}"`);
  } else {
    terminal.sendText(cmd);
  }
}

function deactivate() {
  if (diagnosticCollection) {
    diagnosticCollection.dispose();
  }
}

module.exports = { activate, deactivate };
