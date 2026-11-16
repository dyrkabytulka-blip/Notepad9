using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace Notepad9.Controls;

/// <summary>
/// Icon from Assets/Icons. The generator renders every icon at several exact pixel sizes;
/// this picks the one that matches Width x current DPI, so the bitmap is shown 1:1 without
/// resampling (crisp like a classic 16x16 toolbar). When disabled it shows the gray "_d" variant
/// (IsEnabled is inherited, so a disabled button or menu item grays its icon).
/// </summary>
public class IconImage : Image
{
    static readonly ConcurrentDictionary<string, ImageSource> Cache = new();
    static readonly int[] ToolSizes = { 16, 20, 24, 28, 32, 40, 48, 64 };
    static readonly int[] AppSizes = { 16, 20, 24, 28, 32, 40, 48, 64, 80, 96, 128, 256 };

    [DllImport("user32.dll")]
    static extern uint GetDpiForSystem();

    static readonly double SystemScale = GetSystemScale();

    static double GetSystemScale()
    {
        try { return GetDpiForSystem() / 96.0; } catch { return 1.0; }
    }

    double _scale = SystemScale;

    public static readonly DependencyProperty IconProperty = DependencyProperty.Register(
        nameof(Icon), typeof(string), typeof(IconImage), new PropertyMetadata(null, (d, _) => ((IconImage)d).Refresh()));

    public string Icon
    {
        get => (string)GetValue(IconProperty);
        set => SetValue(IconProperty, value);
    }

    public IconImage()
    {
        Width = 16;
        Height = 16;
        Stretch = Stretch.Fill;
        SnapsToDevicePixels = true;
        UseLayoutRounding = true;
        IsEnabledChanged += (_, _) => Refresh();
        Loaded += (_, _) =>
        {
            _scale = VisualTreeHelper.GetDpi(this).DpiScaleX;
            Refresh();
        };
    }

    public IconImage(string icon) : this() => Icon = icon;

    protected override void OnDpiChanged(DpiScale oldDpi, DpiScale newDpi)
    {
        base.OnDpiChanged(oldDpi, newDpi);
        _scale = newDpi.DpiScaleX;
        Refresh();
    }

    protected override void OnPropertyChanged(DependencyPropertyChangedEventArgs e)
    {
        base.OnPropertyChanged(e);
        if (e.Property == WidthProperty) Refresh();
    }

    void Refresh()
    {
        // icons of closed menus are never shown: decode the bitmap only once the icon is on screen
        if (!IsLoaded) return;
        if (string.IsNullOrEmpty(Icon))
        {
            Source = null;
            return;
        }
        bool isApp = Icon == "app";
        double dip = double.IsNaN(Width) ? 16 : Width;
        double want = dip * _scale;
        var sizes = isApp ? AppSizes : ToolSizes;
        int px = sizes.FirstOrDefault(s => s >= want - 0.5);
        if (px == 0) px = sizes[^1];
        bool exact = Math.Abs(px - want) < 0.5;
        RenderOptions.SetBitmapScalingMode(this, exact ? BitmapScalingMode.NearestNeighbor : BitmapScalingMode.HighQuality);
        Source = Get($"{Icon}_{px}{(IsEnabled || isApp ? "" : "_d")}");
    }

    /// <summary>Bitmap for an icon shown at <paramref name="dip"/> size on the system DPI (for places that take an ImageSource).</summary>
    public static ImageSource GetSized(string icon, double dip = 16)
    {
        double want = dip * SystemScale;
        int px = ToolSizes.FirstOrDefault(s => s >= want - 0.5);
        return Get($"{icon}_{(px == 0 ? ToolSizes[^1] : px)}");
    }

    public static ImageSource Get(string name) => Cache.GetOrAdd(name, n =>
    {
        var b = new BitmapImage();
        b.BeginInit();
        b.UriSource = new Uri($"pack://application:,,,/Assets/Icons/{n}.png");
        b.CacheOption = BitmapCacheOption.OnLoad;
        b.EndInit();
        b.Freeze();
        return b;
    });
}
