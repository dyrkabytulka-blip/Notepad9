using System.IO;

namespace Notepad9.Services;

public enum RunKind
{
    /// <summary>Run a command line in the internal console.</summary>
    Console,
    /// <summary>Open with the program Windows associates with the file (browser, viewer ...).</summary>
    Shell,
    /// <summary>Markdown: toggle the preview instead.</summary>
    Preview,
}

/// <summary>Picks how to run a file from its extension.</summary>
public static class Runner
{
    public static (RunKind kind, string command) For(string path)
    {
        string f = Q(path);
        string ext = Path.GetExtension(path).ToLowerInvariant();
        string tmpExe = Q(Path.Combine(Path.GetTempPath(), "notepad9_run_" + Path.GetFileNameWithoutExtension(path) + ".exe"));
        string cmd = ext switch
        {
            ".py" or ".pyw" => $"python -u {f}",
            ".js" or ".mjs" or ".cjs" => $"node {f}",
            ".ts" or ".mts" => $"npx --yes tsx {f}",
            ".ps1" => $"powershell -NoProfile -ExecutionPolicy Bypass -File {f}",
            ".bat" or ".cmd" => $"call {f}",
            ".cs" => $"dotnet run {f}",
            ".csx" => $"dotnet script {f}",
            ".fsx" => $"dotnet fsi {f}",
            ".java" => $"java {f}",
            ".kt" or ".kts" => $"kotlinc -script {f}",
            ".go" => $"go run {f}",
            ".rb" => $"ruby {f}",
            ".php" => $"php {f}",
            ".lua" => $"lua {f}",
            ".pl" => $"perl {f}",
            ".r" => $"Rscript {f}",
            ".jl" => $"julia {f}",
            ".dart" => $"dart run {f}",
            ".swift" => $"swift {f}",
            ".sh" or ".bash" => $"bash {f}",
            ".vbs" => $"cscript //nologo {f}",
            ".c" => $"gcc {f} -o {tmpExe} && {tmpExe}",
            ".cpp" or ".cc" or ".cxx" => $"g++ -std=c++20 {f} -o {tmpExe} && {tmpExe}",
            ".rs" => $"rustc {f} -o {tmpExe} && {tmpExe}",
            ".exe" or ".com" => f,
            _ => null,
        };
        if (cmd != null) return (RunKind.Console, cmd);
        if (ext is ".md" or ".markdown") return (RunKind.Preview, null);
        return (RunKind.Shell, null);
    }

    static string Q(string s) => "\"" + s + "\"";
}
