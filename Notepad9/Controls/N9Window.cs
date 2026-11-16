using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shell;

namespace Notepad9.Controls;

/// <summary>
/// Window with a custom dark title bar instead of the default chrome.
/// </summary>
public class N9Window : Window
{
    Border _root;
    readonly WindowChrome _chrome = new()
    {
        CaptionHeight = 30,
        ResizeBorderThickness = new Thickness(6),
        GlassFrameThickness = new Thickness(0),
        CornerRadius = new CornerRadius(0),
        UseAeroCaptionButtons = false,
    };

    public N9Window()
    {
        SetResourceReference(StyleProperty, "N9WindowStyle");
        // chrome must be in place before the HWND exists, otherwise SizeToContent
        // windows keep the size of the default frame (black strip at the bottom/right)
        WindowChrome.SetWindowChrome(this, _chrome);
        SourceInitialized += (_, _) => ApplyDwmDark();
        StateChanged += (_, _) => FixMaximizedMargin();
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        _chrome.ResizeBorderThickness = ResizeMode is ResizeMode.CanResize or ResizeMode.CanResizeWithGrip
            ? new Thickness(6) : new Thickness(0);
        base.OnSourceInitialized(e);
        if (SizeToContent != SizeToContent.Manual) InvalidateMeasure();
    }

    protected override void OnContentRendered(EventArgs e)
    {
        base.OnContentRendered(e);
        if (SizeToContent != SizeToContent.Manual)
        {
            // re-run SizeToContent now that the client area equals the window area
            var mode = SizeToContent;
            SizeToContent = SizeToContent.Manual;
            SizeToContent = mode;
        }
    }

    public override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        _root = GetTemplateChild("PART_Root") as Border;
        if (GetTemplateChild("PART_Min") is Button min) min.Click += (_, _) => WindowState = WindowState.Minimized;
        if (GetTemplateChild("PART_Max") is Button max)
            max.Click += (_, _) => WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
        if (GetTemplateChild("PART_Close") is Button close) close.Click += (_, _) => Close();
        if (GetTemplateChild("PART_TitleBar") is FrameworkElement bar)
            bar.MouseRightButtonUp += (_, e) =>
            {
                var p = PointToScreen(e.GetPosition(this));
                SystemCommands.ShowSystemMenu(this, new Point(p.X / DpiScale(), p.Y / DpiScale()));
            };
        FixMaximizedMargin();
    }

    double DpiScale() => PresentationSource.FromVisual(this)?.CompositionTarget?.TransformToDevice.M11 ?? 1.0;

    void FixMaximizedMargin()
    {
        if (_root == null) return;
        if (WindowState == WindowState.Maximized)
        {
            var f = SystemParameters.WindowResizeBorderThickness;
            double pad = SystemParameters.WindowNonClientFrameThickness.Left - f.Left;
            double m = f.Left + Math.Max(0, pad);
            _root.Margin = new Thickness(m, m, m, m);
        }
        else _root.Margin = new Thickness(0);
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        base.OnKeyDown(e);
        if (!e.Handled && e.Key == Key.Escape && IsDialog)
        {
            e.Handled = true;
            Close();
        }
    }

    /// <summary>Esc closes the window.</summary>
    public bool IsDialog { get; set; }

    /// <summary>Multi-size .ico, so the taskbar / Alt+Tab pick a native frame.</summary>
    public static ImageSource AppIcon { get; } = LoadIcon();

    static ImageSource LoadIcon()
    {
        var f = BitmapFrame.Create(new Uri("pack://application:,,,/Assets/app.ico"));
        f.Freeze();
        return f;
    }

    // ---- DWM: dark native border / rounded corners on Windows 11
    [DllImport("dwmapi.dll")]
    static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int value, int size);

    /// <summary>Windows 11 corner style: 1 = square, 2 = round, 3 = small round.</summary>
    public void SetCorners(int preference)
    {
        _corners = preference;
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == IntPtr.Zero) return;
        try { DwmSetWindowAttribute(hwnd, 33, ref preference, sizeof(int)); } catch { }
    }

    /// <summary>Native window border color (Windows 11).</summary>
    public void SetBorderColor(Color c)
    {
        _border = c.R | (c.G << 8) | (c.B << 16);
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == IntPtr.Zero) return;
        int v = _border;
        try { DwmSetWindowAttribute(hwnd, 34, ref v, sizeof(int)); } catch { }
    }

    int _corners = 2;
    int _border = 0x003A3A3A;

    void ApplyDwmDark()
    {
        try
        {
            var hwnd = new WindowInteropHelper(this).Handle;
            int on = 1;
            DwmSetWindowAttribute(hwnd, 20, ref on, sizeof(int));          // DWMWA_USE_IMMERSIVE_DARK_MODE
            int border = _border;
            DwmSetWindowAttribute(hwnd, 34, ref border, sizeof(int));      // DWMWA_BORDER_COLOR
            int corner = _corners;
            DwmSetWindowAttribute(hwnd, 33, ref corner, sizeof(int));      // DWMWA_WINDOW_CORNER_PREFERENCE
        }
        catch { /* older Windows */ }
    }
}
