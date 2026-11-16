using System.Diagnostics;
using System.Runtime;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Threading;
using Notepad9.Controls;
using Notepad9.Services;

namespace Notepad9;

/// <summary>
/// "Instant start": closing the window hides it instead of exiting, and Notepad9 can start
/// hidden at sign-in. Opening a file then only has to show an already loaded window.
/// </summary>
public partial class MainWindow
{
    bool _quitting;
    TrayIcon _tray;
    bool _measured;

    /// <summary>Time from process start to the first rendered frame (shown in About).</summary>
    public static double StartupMs { get; private set; }

    void TrackStartupTime()
    {
        ContentRendered += (_, _) =>
        {
            if (_measured) return;
            _measured = true;
            try
            {
                StartupMs = (DateTime.Now - Process.GetCurrentProcess().StartTime).TotalMilliseconds;
                if (IsVisible && Left > -10000) ShowInfo($"Started in {StartupMs:0} ms");
            }
            catch { }
        };
    }

    /// <summary>Hide the window and keep everything loaded (tray icon stays).</summary>
    void GoToBackground()
    {
        bool hot = Settings.RestoreSession && Settings.HotExit;
        if (!hot)
        {
            foreach (var t in _tabs.Where(t => t.IsModified).ToList())
            {
                ActivateTab(t);
                if (!ConfirmDiscard()) return;   // the user cancelled: stay open
            }
        }
        if (Settings.RestoreSession) SaveSession(hot);
        else Session.Clear();

        if (IsConsoleShown) Settings.ConsoleHeight = ConsoleRow.ActualHeight;
        ShowConsole(false);
        _term?.Shutdown();
        _find?.Close();
        _findFiles?.Close();
        StorePlacement();
        if (Settings.SaveOnExit) Settings.Save();

        if (!hot)
        {
            // the save questions were answered: next time start from what is on disk
            _tab = null;
            EditorHost.Content = null;
            _tabs.Clear();
            _closedTabs.Clear();
            if (Settings.RestoreSession) RestoreSession();
            if (_tabs.Count == 0) NewTab();
        }

        Hide();
        _tray ??= new TrayIcon(this);
        _tray.Visible = true;
        Dispatcher.BeginInvoke(DispatcherPriority.ApplicationIdle, TrimMemory);
    }

    /// <summary>Show the window again (from the tray, a shortcut or "Edit in Notepad9").</summary>
    public void ShowFromBackground()
    {
        if (!IsVisible) Show();
        if (WindowState == WindowState.Minimized) WindowState = WindowState.Normal;
        Activate();
        bool top = Topmost;
        Topmost = true;
        Topmost = top;
        Focus();
        Editor?.Focus();
    }

    public void NewTabFromTray()
    {
        ShowFromBackground();
        Run("new");
    }

    /// <summary>Really exit (tray "Quit", File → Quit Completely, Windows shutdown).</summary>
    public void QuitCompletely()
    {
        _quitting = true;
        if (!IsVisible)
        {
            // hidden: the session is already saved; just leave
            _tray?.Dispose();
            _tray = null;
        }
        Close();
        if (IsLoaded && IsVisible) _quitting = false;   // the user cancelled a save question
    }

    /// <summary>Windows is shutting down / signing out: do not hide, exit normally.</summary>
    public void PrepareForShutdown() => _quitting = true;

    /// <summary>
    /// Started at sign-in with /background: build and lay out the window off-screen once
    /// (so its first real appearance is instant), then hide it.
    /// </summary>
    public void PrewarmHidden()
    {
        var state = WindowState;
        double left = Left, top = Top;
        if (double.IsNaN(left) || double.IsNaN(top))
        {
            var wa = SystemParameters.WorkArea;
            left = wa.Left + Math.Max(0, (wa.Width - Width) / 2);
            top = wa.Top + Math.Max(0, (wa.Height - Height) / 2);
        }
        WindowState = WindowState.Normal;
        WindowStartupLocation = WindowStartupLocation.Manual;
        ShowInTaskbar = false;
        ShowActivated = false;
        Left = -32000;
        Top = -32000;
        Show();
        Dispatcher.Invoke(() => { }, DispatcherPriority.ApplicationIdle);   // layout, Loaded, first render
        Hide();
        Left = left;
        Top = top;
        WindowState = state;
        ShowInTaskbar = true;
        ShowActivated = true;
        _tray ??= new TrayIcon(this);
        _tray.Visible = true;
        Dispatcher.BeginInvoke(DispatcherPriority.ApplicationIdle, TrimMemory);
    }

    [DllImport("kernel32.dll")]
    static extern bool SetProcessWorkingSetSize(IntPtr process, IntPtr min, IntPtr max);

    /// <summary>Give memory back to Windows while waiting in the background.</summary>
    static void TrimMemory()
    {
        try
        {
            GCSettings.LargeObjectHeapCompactionMode = GCLargeObjectHeapCompactionMode.CompactOnce;
            GC.Collect(2, GCCollectionMode.Forced, true, true);
            GC.WaitForPendingFinalizers();
            SetProcessWorkingSetSize(Process.GetCurrentProcess().Handle, -1, -1);
        }
        catch { }
    }
}
