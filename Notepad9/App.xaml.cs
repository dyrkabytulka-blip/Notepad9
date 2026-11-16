using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Windows;
using System.Windows.Threading;
using Notepad9.Dialogs;
using Notepad9.Services;

namespace Notepad9;

public partial class App
{
    public string[] StartFiles { get; init; } = Array.Empty<string>();
    public bool IsPrimary { get; init; } = true;
    public Mutex InstanceMutex { get; init; }
    /// <summary>Started at sign-in (/background): load everything but stay hidden.</summary>
    public bool StartHidden { get; init; }
    CancellationTokenSource _pipeStop;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        DispatcherUnhandledException += OnUnhandled;

        var settings = AppSettings.Load();
        var w = new MainWindow(settings, StartFiles, IsPrimary);
        MainWindow = w;
        SessionEnding += (_, _) => w.PrepareForShutdown();
        if (StartHidden) w.PrewarmHidden();
        else w.Show();
        if (IsPrimary) StartPipeServer(w);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _pipeStop?.Cancel();
        try { InstanceMutex?.ReleaseMutex(); } catch { }
        base.OnExit(e);
    }

    void StartPipeServer(MainWindow window)
    {
        _pipeStop = new CancellationTokenSource();
        var token = _pipeStop.Token;
        Task.Run(async () =>
        {
            while (!token.IsCancellationRequested)
            {
                try
                {
                    using var pipe = new NamedPipeServerStream(Program.PipeName, PipeDirection.In, 1,
                        PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
                    await pipe.WaitForConnectionAsync(token);
                    using var r = new StreamReader(pipe, Encoding.UTF8);
                    string data = await r.ReadToEndAsync(token);
                    var paths = data.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
                    _ = Dispatcher.BeginInvoke(() => window.OpenFromOtherInstance(paths));
                }
                catch (OperationCanceledException) { return; }
                catch { await Task.Delay(300); }
            }
        });
    }

    void OnUnhandled(object sender, DispatcherUnhandledExceptionEventArgs e)
    {
        e.Handled = true;
        try { MessageDialog.Info(MainWindow, "Unexpected error:\n\n" + e.Exception.Message, "exit"); }
        catch { MessageBox.Show(e.Exception.ToString(), "Notepad9"); }
    }
}
