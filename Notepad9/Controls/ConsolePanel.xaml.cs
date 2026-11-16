using System.Diagnostics;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using ICSharpCode.AvalonEdit.Document;
using ICSharpCode.AvalonEdit.Rendering;

namespace Notepad9.Controls;

/// <summary>
/// Console docked inside the main window. Runs cmd.exe with redirected input/output:
/// the output is shown in a read-only editor, commands are typed into the input line.
/// </summary>
public partial class ConsolePanel
{
    Process _shell;
    readonly StringBuilder _pending = new();
    readonly DispatcherTimer _flush;
    readonly List<string> _history = new();
    int _historyPos = -1;
    const int MaxChars = 600_000;
    static readonly Regex Ansi = new(@"\x1B(\[[0-9;?]*[ -/]*[@-~]|\][^\x07]*\x07|[@-Z\\-_])", RegexOptions.Compiled);

    /// <summary>Asked to run the current file (header Run button).</summary>
    public event EventHandler RunRequested;
    public event EventHandler CloseRequested;
    /// <summary>Returns the folder of the current file (or null).</summary>
    public Func<string> FileFolder { get; set; }

    public bool IsShellAlive => _shell is { HasExited: false };

    public ConsolePanel()
    {
        InitializeComponent();
        Output.Options.EnableHyperlinks = true;
        Output.Options.RequireControlModifierForHyperlinkClick = false;
        Output.TextArea.TextView.LinkTextForegroundBrush = new SolidColorBrush(Color.FromRgb(0x4F, 0xA3, 0xF7));
        Output.TextArea.SelectionBrush = new SolidColorBrush(Color.FromRgb(0x26, 0x4F, 0x78));
        Output.TextArea.SelectionForeground = null;
        Output.TextArea.SelectionBorder = null;
        Output.TextArea.Caret.CaretBrush = Brushes.Transparent;
        Output.TextArea.TextView.LineTransformers.Add(new ConsoleColorizer());
        Output.PreviewKeyDown += (_, e) =>
        {
            // typing while the output has focus goes to the input line
            if (e.Key is >= Key.A and <= Key.Z or >= Key.D0 and <= Key.D9 or Key.Space or Key.Enter
                && Keyboard.Modifiers is ModifierKeys.None or ModifierKeys.Shift)
                Input.Focus();
        };
        _flush = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromMilliseconds(30) };
        _flush.Tick += (_, _) => Flush();
    }

    // ================================================================ shell
    static ProcessStartInfo ShellInfo(string args, string dir)
    {
        var psi = new ProcessStartInfo("cmd.exe")
        {
            Arguments = args,
            UseShellExecute = false,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            WorkingDirectory = Directory.Exists(dir) ? dir : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
            StandardOutputEncoding = new UTF8Encoding(false),
            StandardErrorEncoding = new UTF8Encoding(false),
            StandardInputEncoding = new UTF8Encoding(false),
        };
        psi.Environment["PYTHONIOENCODING"] = "utf-8";
        psi.Environment["PYTHONUNBUFFERED"] = "1";
        return psi;
    }

    /// <summary>Starts an interactive shell if none is running.</summary>
    public void EnsureShell(string dir)
    {
        if (IsShellAlive) return;
        StartProcess(ShellInfo("/K chcp 65001>nul", dir));
    }

    void StartProcess(ProcessStartInfo psi)
    {
        KillShell();
        try
        {
            var p = new Process { StartInfo = psi, EnableRaisingEvents = true };
            p.Exited += (_, _) => Dispatcher.BeginInvoke(() =>
            {
                if (_shell == p)
                {
                    Append("\n[shell exited]\n");
                    SetState("not running");
                }
            });
            p.Start();
            p.StandardInput.AutoFlush = true;
            _shell = p;
            _ = Pump(p, p.StandardOutput);
            _ = Pump(p, p.StandardError);
            SetState("cmd · " + psi.WorkingDirectory);
        }
        catch (Exception ex)
        {
            Append($"\nCould not start cmd.exe: {ex.Message}\n");
        }
    }

    async Task Pump(Process p, StreamReader reader)
    {
        var buf = new char[4096];
        try
        {
            int n;
            while ((n = await reader.ReadAsync(buf, 0, buf.Length).ConfigureAwait(false)) > 0)
            {
                if (p != _shell) return;
                lock (_pending) _pending.Append(buf, 0, n);
                _ = Dispatcher.BeginInvoke(() => { if (!_flush.IsEnabled) _flush.Start(); });
            }
        }
        catch { /* process killed */ }
    }

    void KillShell()
    {
        var p = _shell;
        _shell = null;
        if (p == null) return;
        try { if (!p.HasExited) p.Kill(entireProcessTree: true); } catch { }
        p.Dispose();
    }

    /// <summary>Runs a command line in a fresh shell started in <paramref name="dir"/>; the shell stays open afterwards.</summary>
    public void RunCommand(string commandLine, string dir)
    {
        Append((Output.Document.TextLength > 0 ? "\n" : "") + "▶ " + commandLine + "\n");
        bool delayed = !commandLine.Contains('!') && !(dir ?? "").Contains('!');
        string tail = delayed ? " & echo. & echo [exit code !errorlevel!]" : "";
        string inner = $"chcp 65001>nul & cd /d \"{dir}\" & {commandLine}{tail}";
        StartProcess(ShellInfo((delayed ? "/V:ON " : "") + "/S /K \"" + inner + "\"", dir));
        FocusInput();
    }

    /// <summary>Type a line into the running shell.</summary>
    public void Send(string line)
    {
        if (!IsShellAlive) EnsureShell(FileFolder?.Invoke());
        try { _shell.StandardInput.WriteLine(line); }
        catch (Exception ex) { Append($"\n[{ex.Message}]\n"); }
    }

    public void Stop()
    {
        bool was = IsShellAlive;
        KillShell();
        if (was) Append("\n■ stopped\n");
        EnsureShell(FileFolder?.Invoke());
    }

    public void Shutdown() => KillShell();

    public void Clear()
    {
        lock (_pending) _pending.Clear();
        Output.Document.Text = "";
    }

    public void FocusInput() => Dispatcher.BeginInvoke(DispatcherPriority.Input, () => Input.Focus());

    void SetState(string text) => StateText.Text = text;

    // ================================================================ output
    public void Append(string text)
    {
        lock (_pending) _pending.Append(text);
        if (!_flush.IsEnabled) _flush.Start();
    }

    void Flush()
    {
        _flush.Stop();
        string chunk;
        lock (_pending)
        {
            if (_pending.Length == 0) return;
            chunk = _pending.ToString();
            _pending.Clear();
        }
        chunk = Ansi.Replace(chunk, "").Replace("\r\n", "\n");
        // a lone \r (progress bars) overwrites the line in a real console; here just drop it
        chunk = chunk.Replace('\r', '\n').Replace("\n\n\n\n", "\n\n");
        var doc = Output.Document;
        var tv = Output.TextArea.TextView;
        bool atEnd = tv.VerticalOffset + tv.ActualHeight >= tv.DocumentHeight - 4;
        doc.UndoStack.SizeLimit = 0;
        doc.Insert(doc.TextLength, chunk);
        if (doc.TextLength > MaxChars)
        {
            var cut = doc.GetLineByOffset(doc.TextLength - MaxChars * 3 / 4);
            doc.Remove(0, cut.Offset);
        }
        if (atEnd) Output.ScrollToEnd();
    }

    // ================================================================ input
    void Input_PreviewKeyDown(object sender, KeyEventArgs e)
    {
        switch (e.Key)
        {
            case Key.Enter:
            {
                string line = Input.Text;
                Input.Clear();
                if (line.Trim().Equals("cls", StringComparison.OrdinalIgnoreCase)) Clear();
                if (line.Length > 0 && (_history.Count == 0 || _history[^1] != line)) _history.Add(line);
                _historyPos = -1;
                Send(line);
                Output.ScrollToEnd();
                e.Handled = true;
                break;
            }
            case Key.Up when _history.Count > 0:
                _historyPos = _historyPos < 0 ? _history.Count - 1 : Math.Max(0, _historyPos - 1);
                Input.Text = _history[_historyPos];
                Input.CaretIndex = Input.Text.Length;
                e.Handled = true;
                break;
            case Key.Down when _historyPos >= 0:
                _historyPos++;
                if (_historyPos >= _history.Count) { _historyPos = -1; Input.Clear(); }
                else Input.Text = _history[_historyPos];
                Input.CaretIndex = Input.Text.Length;
                e.Handled = true;
                break;
            case Key.L when Keyboard.Modifiers == ModifierKeys.Control:
                Clear();
                e.Handled = true;
                break;
            case Key.C when Keyboard.Modifiers == ModifierKeys.Control && Input.SelectionLength == 0:
                Stop();
                e.Handled = true;
                break;
            case Key.Escape:
                CloseRequested?.Invoke(this, EventArgs.Empty);
                e.Handled = true;
                break;
        }
    }

    void Run_Click(object sender, RoutedEventArgs e) => RunRequested?.Invoke(this, EventArgs.Empty);
    void Stop_Click(object sender, RoutedEventArgs e) => Stop();
    void Clear_Click(object sender, RoutedEventArgs e) { Clear(); FocusInput(); }
    void Close_Click(object sender, RoutedEventArgs e) => CloseRequested?.Invoke(this, EventArgs.Empty);

    void Cd_Click(object sender, RoutedEventArgs e)
    {
        var dir = FileFolder?.Invoke();
        if (dir != null) Send($"cd /d \"{dir}\"");
        FocusInput();
    }

    /// <summary>Colors the run header, "stopped" and exit-code lines.</summary>
    sealed class ConsoleColorizer : DocumentColorizingTransformer
    {
        static readonly Brush Run = Frozen(Color.FromRgb(0x6E, 0xE0, 0x5A));
        static readonly Brush Ok = Frozen(Color.FromRgb(0x7A, 0x7A, 0x7A));
        static readonly Brush Bad = Frozen(Color.FromRgb(0xF0, 0x6A, 0x5A));
        static readonly Brush Stop = Frozen(Color.FromRgb(0xE0, 0xA0, 0x40));

        static Brush Frozen(Color c)
        {
            var b = new SolidColorBrush(c);
            b.Freeze();
            return b;
        }

        protected override void ColorizeLine(DocumentLine line)
        {
            if (line.Length == 0) return;
            string t = CurrentContext.Document.GetText(line.Offset, Math.Min(line.Length, 20));
            Brush b = null;
            if (t.StartsWith("▶ ")) b = Run;
            else if (t.StartsWith("■ ")) b = Stop;
            else if (t.StartsWith("[exit code 0]")) b = Ok;
            else if (t.StartsWith("[exit code")) b = Bad;
            else if (t.StartsWith("[shell exited]")) b = Ok;
            if (b == null) return;
            ChangeLinePart(line.Offset, line.EndOffset, el => el.TextRunProperties.SetForegroundBrush(b));
        }
    }
}
