using System.IO;
using System.IO.Pipes;
using System.Runtime;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

namespace Notepad9;

/// <summary>
/// Entry point. Decides as early as possible (before WPF is loaded) whether a Notepad9 window
/// is already running; if so the files are handed to it and this process exits right away.
/// </summary>
public static class Program
{
    static readonly string InstanceId = "Notepad9." + Environment.UserName;
    public static string PipeName => InstanceId + ".Pipe";

    [STAThread]
    public static int Main(string[] args)
    {
        bool forceNew = args.Any(a => a.Equals("/new", StringComparison.OrdinalIgnoreCase));
        bool background = args.Any(a => a.Equals("/background", StringComparison.OrdinalIgnoreCase));
        var files = args.Where(a => !a.StartsWith('/') && !a.StartsWith('-')).Select(FullPath).Where(p => p != null).ToArray();

        var mutex = new Mutex(true, InstanceId + ".Mutex", out bool primary);
        if (!primary && background) return 0;   // sign-in start, but Notepad9 is already running
        if (!primary && !forceNew && SendToRunningInstance(files)) return 0;

        // background JIT of the methods recorded on earlier runs (uses another CPU core)
        try
        {
            string dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Notepad9");
            Directory.CreateDirectory(dir);
            ProfileOptimization.SetProfileRoot(dir);
            ProfileOptimization.StartProfile(primary ? "startup.jitprofile" : "startup2.jitprofile");
        }
        catch { }

        return RunApp(files, primary, mutex, background && primary);
    }

    // kept separate so the quick hand-off path above never loads the WPF assemblies
    [MethodImpl(MethodImplOptions.NoInlining)]
    static int RunApp(string[] files, bool primary, Mutex mutex, bool background)
    {
        var app = new App { StartFiles = files, IsPrimary = primary, InstanceMutex = mutex, StartHidden = background };
        app.InitializeComponent();
        return app.Run();
    }

    static string FullPath(string p)
    {
        try { return Path.GetFullPath(p); } catch { return null; }
    }

    [DllImport("user32.dll")]
    static extern bool AllowSetForegroundWindow(int processId);

    /// <summary>Hand the files to the window that is already open. Returns false if nobody answered.</summary>
    static bool SendToRunningInstance(string[] files)
    {
        try
        {
            AllowSetForegroundWindow(-1);   // let the running window come to the front
            using var pipe = new NamedPipeClientStream(".", PipeName, PipeDirection.Out);
            pipe.Connect(6000);   // the first window may still be starting (several files opened at once)
            using var w = new StreamWriter(pipe, new UTF8Encoding(false));
            w.Write(string.Join("\n", files.Length > 0 ? files : new[] { "" }));
            w.Flush();
            return true;
        }
        catch { return false; }
    }
}
