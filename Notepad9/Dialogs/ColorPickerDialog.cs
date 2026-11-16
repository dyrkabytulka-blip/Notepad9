using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using Notepad9.Controls;
using Notepad9.Services;

namespace Notepad9.Dialogs;

/// <summary>HSV color picker with alpha, hex field and a small palette. Reports changes live.</summary>
public class ColorPickerDialog : N9Window
{
    public Color Color { get; private set; }
    public event Action<Color> Changed;

    double _h, _s, _v, _a = 1;
    bool _sync;
    readonly SolidColorBrush _hueBrush = new(Colors.Red);
    readonly SolidColorBrush _newBrush = new(Colors.Red);
    readonly LinearGradientBrush _alphaBrush = new() { StartPoint = new Point(0, 0), EndPoint = new Point(0, 1) };
    readonly Canvas _svCanvas = new() { Width = 220, Height = 220, ClipToBounds = true };
    readonly Ellipse _svMarker = new() { Width = 12, Height = 12, Stroke = Brushes.White, StrokeThickness = 2, IsHitTestVisible = false };
    readonly Border _hueMarker = new() { Width = 24, Height = 5, BorderBrush = Brushes.White, BorderThickness = new Thickness(1.5), IsHitTestVisible = false };
    readonly Border _alphaMarker = new() { Width = 24, Height = 5, BorderBrush = Brushes.White, BorderThickness = new Thickness(1.5), IsHitTestVisible = false };
    readonly TextBox _hex = new() { Width = 110, FontFamily = new FontFamily("Consolas") };

    static readonly string[] Palette =
    {
        "#000000", "#141414", "#2B2B2B", "#454545", "#7A7A7A", "#B5B5B5", "#DEDEDE", "#FFFFFF",
        "#C42B1C", "#F06A5A", "#E0A040", "#C8A82E", "#FFE070", "#404013", "#6A9955", "#6EE05A",
        "#4EC9B0", "#2AA198", "#4FA3F7", "#2F6FB5", "#264F78", "#569CD6", "#C586C0", "#FF79C6",
    };

    public static DrawingBrush Checker(double cell = 6)
    {
        var g = new DrawingGroup();
        g.Children.Add(new GeometryDrawing(new SolidColorBrush(Color.FromRgb(0x50, 0x50, 0x50)), null, new RectangleGeometry(new Rect(0, 0, 2 * cell, 2 * cell))));
        var light = new SolidColorBrush(Color.FromRgb(0x8A, 0x8A, 0x8A));
        g.Children.Add(new GeometryDrawing(light, null, new RectangleGeometry(new Rect(0, 0, cell, cell))));
        g.Children.Add(new GeometryDrawing(light, null, new RectangleGeometry(new Rect(cell, cell, cell, cell))));
        var b = new DrawingBrush(g) { TileMode = TileMode.Tile, Viewport = new Rect(0, 0, 2 * cell, 2 * cell), ViewportUnits = BrushMappingMode.Absolute };
        b.Freeze();
        return b;
    }

    public ColorPickerDialog(Window owner, Color initial, string title)
    {
        Title = title;
        Owner = owner;
        IsDialog = true;
        ResizeMode = ResizeMode.NoResize;
        SizeToContent = SizeToContent.WidthAndHeight;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        ShowInTaskbar = false;
        Icon = AppIcon;
        Color = initial;

        // ---- saturation / value square
        var sv = new Grid { Width = 220, Height = 220, Cursor = Cursors.Cross };
        sv.Children.Add(new Rectangle { Fill = _hueBrush });
        sv.Children.Add(new Rectangle { Fill = new LinearGradientBrush(Colors.White, Color.FromArgb(0, 255, 255, 255), 0) });
        sv.Children.Add(new Rectangle { Fill = new LinearGradientBrush(Color.FromArgb(0, 0, 0, 0), Colors.Black, 90) });
        _svCanvas.Children.Add(_svMarker);
        sv.Children.Add(_svCanvas);
        Drag(sv, p =>
        {
            _s = Math.Clamp(p.X / 220, 0, 1);
            _v = 1 - Math.Clamp(p.Y / 220, 0, 1);
            Update(true);
        });

        // ---- hue bar
        var hueGrad = new LinearGradientBrush { StartPoint = new Point(0, 0), EndPoint = new Point(0, 1) };
        string[] hues = { "#FF0000", "#FFFF00", "#00FF00", "#00FFFF", "#0000FF", "#FF00FF", "#FF0000" };
        for (int i = 0; i < hues.Length; i++) hueGrad.GradientStops.Add(new GradientStop(ThemeCatalog.Parse(hues[i]), i / 6.0));
        var hue = new Grid { Width = 20, Height = 220, Margin = new Thickness(10, 0, 0, 0), Cursor = Cursors.Hand };
        hue.Children.Add(new Rectangle { Fill = hueGrad });
        var hueCanvas = new Canvas();
        hueCanvas.Children.Add(_hueMarker);
        hue.Children.Add(hueCanvas);
        Drag(hue, p =>
        {
            _h = Math.Clamp(p.Y / 220, 0, 0.9999) * 360;
            Update(true);
        });

        // ---- alpha bar
        var alpha = new Grid { Width = 20, Height = 220, Margin = new Thickness(8, 0, 0, 0), Cursor = Cursors.Hand };
        alpha.Children.Add(new Rectangle { Fill = Checker(5) });
        alpha.Children.Add(new Rectangle { Fill = _alphaBrush });
        var alphaCanvas = new Canvas();
        alphaCanvas.Children.Add(_alphaMarker);
        alpha.Children.Add(alphaCanvas);
        Drag(alpha, p =>
        {
            _a = 1 - Math.Clamp(p.Y / 220, 0, 1);
            Update(true);
        });

        // ---- right side
        var right = new StackPanel { Margin = new Thickness(16, 0, 0, 0), Width = 200 };
        var swatches = new Grid { Height = 40, Margin = new Thickness(0, 0, 0, 10) };
        swatches.ColumnDefinitions.Add(new ColumnDefinition());
        swatches.ColumnDefinitions.Add(new ColumnDefinition());
        var oldSw = new Border { Background = Checker(), Child = new Border { Background = new SolidColorBrush(initial) }, ToolTip = "Current" };
        var newSw = new Border { Background = Checker(), Child = new Border { Background = _newBrush }, ToolTip = "New" };
        Grid.SetColumn(newSw, 1);
        swatches.Children.Add(oldSw);
        swatches.Children.Add(newSw);
        oldSw.MouseLeftButtonUp += (_, _) => SetColor(initial, true);
        right.Children.Add(new TextBlock { Text = "Current            New", Foreground = (Brush)FindResource("TextDim"), Margin = new Thickness(0, 0, 0, 4) });
        right.Children.Add(swatches);

        var hexRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 0, 0, 12) };
        hexRow.Children.Add(new TextBlock { Text = "Hex", Width = 36, VerticalAlignment = VerticalAlignment.Center, Foreground = (Brush)FindResource("TextDim") });
        hexRow.Children.Add(_hex);
        _hex.ToolTip = "#RRGGBB or #AARRGGBB";
        _hex.TextChanged += (_, _) =>
        {
            if (_sync) return;
            try { SetColor(ThemeCatalog.Parse(_hex.Text), false); } catch { }
        };
        right.Children.Add(hexRow);

        var pal = new WrapPanel { Width = 200 };
        foreach (var hex in Palette)
        {
            var c = ThemeCatalog.Parse(hex);
            var b = new Border
            {
                Width = 21, Height = 21, Margin = new Thickness(0, 0, 4, 4), Background = new SolidColorBrush(c),
                BorderBrush = (Brush)FindResource("Line"), BorderThickness = new Thickness(1), Cursor = Cursors.Hand, ToolTip = hex,
            };
            b.MouseLeftButtonUp += (_, _) => SetColor(Color.FromArgb((byte)(_a * 255), c.R, c.G, c.B), true);
            pal.Children.Add(b);
        }
        right.Children.Add(pal);

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 14, 0, 0) };
        var ok = new Button { Content = "OK", IsDefault = true, MinWidth = 80 };
        ok.Click += (_, _) => DialogResult = true;
        var cancel = new Button { Content = "Cancel", IsCancel = true, Margin = new Thickness(8, 0, 0, 0), MinWidth = 80 };
        buttons.Children.Add(ok);
        buttons.Children.Add(cancel);
        right.Children.Add(buttons);

        var root = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(16, 14, 16, 16) };
        root.Children.Add(sv);
        root.Children.Add(hue);
        root.Children.Add(alpha);
        root.Children.Add(right);
        Content = root;

        SetColor(initial, true);
    }

    static void Drag(FrameworkElement el, Action<Point> onMove)
    {
        el.MouseLeftButtonDown += (_, e) =>
        {
            el.CaptureMouse();
            onMove(e.GetPosition(el));
        };
        el.MouseMove += (_, e) => { if (el.IsMouseCaptured) onMove(e.GetPosition(el)); };
        el.MouseLeftButtonUp += (_, _) => el.ReleaseMouseCapture();
    }

    void SetColor(Color c, bool updateHex)
    {
        ToHsv(c, out _h, out _s, out _v);
        _a = c.A / 255.0;
        Update(updateHex);
    }

    void Update(bool updateHex)
    {
        var c = FromHsv(_h, _s, _v);
        c.A = (byte)Math.Round(_a * 255);
        Color = c;
        _hueBrush.Color = FromHsv(_h, 1, 1);
        _newBrush.Color = c;
        _alphaBrush.GradientStops.Clear();
        _alphaBrush.GradientStops.Add(new GradientStop(Color.FromArgb(255, c.R, c.G, c.B), 0));
        _alphaBrush.GradientStops.Add(new GradientStop(Color.FromArgb(0, c.R, c.G, c.B), 1));
        Canvas.SetLeft(_svMarker, _s * 220 - 6);
        Canvas.SetTop(_svMarker, (1 - _v) * 220 - 6);
        _svMarker.Stroke = _v > 0.6 && _s < 0.4 ? Brushes.Black : Brushes.White;
        Canvas.SetLeft(_hueMarker, -2);
        Canvas.SetTop(_hueMarker, _h / 360 * 220 - 2);
        Canvas.SetLeft(_alphaMarker, -2);
        Canvas.SetTop(_alphaMarker, (1 - _a) * 220 - 2);
        if (updateHex)
        {
            _sync = true;
            _hex.Text = ThemeCatalog.Hex(c);
            _sync = false;
        }
        Changed?.Invoke(c);
    }

    static void ToHsv(Color c, out double h, out double s, out double v)
    {
        double r = c.R / 255.0, g = c.G / 255.0, b = c.B / 255.0;
        double max = Math.Max(r, Math.Max(g, b)), min = Math.Min(r, Math.Min(g, b)), d = max - min;
        h = 0;
        if (d > 1e-9)
        {
            if (max == r) h = 60 * (((g - b) / d) % 6);
            else if (max == g) h = 60 * ((b - r) / d + 2);
            else h = 60 * ((r - g) / d + 4);
        }
        if (h < 0) h += 360;
        s = max <= 0 ? 0 : d / max;
        v = max;
    }

    static Color FromHsv(double h, double s, double v)
    {
        double c = v * s, x = c * (1 - Math.Abs(h / 60 % 2 - 1)), m = v - c;
        (double r, double g, double b) = h switch
        {
            < 60 => (c, x, 0d),
            < 120 => (x, c, 0d),
            < 180 => (0d, c, x),
            < 240 => (0d, x, c),
            < 300 => (x, 0d, c),
            _ => (c, 0d, x),
        };
        return Color.FromRgb((byte)Math.Round((r + m) * 255), (byte)Math.Round((g + m) * 255), (byte)Math.Round((b + m) * 255));
    }
}
