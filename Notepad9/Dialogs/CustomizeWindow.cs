using System.Globalization;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using Microsoft.Win32;
using Notepad9.Controls;
using Notepad9.Services;

namespace Notepad9.Dialogs;

/// <summary>
/// "Customize Interface": presets, every color, background image, caret and mouse pointer,
/// fonts, sizes, tabs and status bar. All changes are shown live; Cancel restores the old look.
/// </summary>
public class CustomizeWindow : N9Window
{
    readonly MainWindow _main;
    readonly ThemeSettings _snapshot;
    readonly bool _snapshotSession, _snapshotHotExit;
    bool _accepted;
    readonly ListBox _nav = new() { Width = 200, BorderThickness = new Thickness(0) };
    readonly ScrollViewer _scroll = new() { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled };
    readonly DispatcherTimer _applyTimer;
    bool _fullApply;
    int _group;

    ThemeSettings T => _main.Settings.Theme;

    readonly (string name, Func<FrameworkElement> build)[] _pages;

    public CustomizeWindow(MainWindow main)
    {
        _main = main;
        _snapshot = main.Settings.Theme.Clone();
        _snapshotSession = main.Settings.RestoreSession;
        _snapshotHotExit = main.Settings.HotExit;
        Title = "Customize Interface";
        Owner = main;
        IsDialog = true;
        Width = 940;
        Height = 660;
        MinWidth = 700;
        MinHeight = 420;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        ShowInTaskbar = false;
        Icon = AppIcon;

        _applyTimer = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromMilliseconds(60) };
        _applyTimer.Tick += (_, _) =>
        {
            _applyTimer.Stop();
            if (_fullApply) _main.ApplyAll();
            else _main.ApplyTheme();
            _fullApply = false;
        };

        _pages = new (string, Func<FrameworkElement>)[]
        {
            ("Theme presets", PagePresets),
            ("Interface colors", () => PageColors(ThemeCatalog.GroupUi, "Interface colors",
                "Menus, toolbar, tabs, status bar, dialogs and scrollbars.")),
            ("Editor colors", () => PageColors(ThemeCatalog.GroupEditor, "Editor colors",
                "Text area, current line, selection, caret, line numbers.")),
            ("Syntax colors", () => PageColors(ThemeCatalog.GroupSyntax, "Syntax colors",
                "Used by every language scheme (C#, Python, HTML, Markdown ...).")),
            ("Background", PageBackground),
            ("Caret and pointer", PageCaret),
            ("Fonts and sizes", PageFonts),
            ("Tabs and status bar", PageTabs),
        };

        foreach (var (name, _) in _pages) _nav.Items.Add(new ListBoxItem { Content = name, Padding = new Thickness(14, 7, 8, 7) });
        _nav.SelectionChanged += (_, _) => ShowPage(_nav.SelectedIndex);

        // ---- bottom bar
        var bar = new DockPanel { Margin = new Thickness(14, 10, 14, 12) };
        var right = new StackPanel { Orientation = Orientation.Horizontal };
        DockPanel.SetDock(right, Dock.Right);
        var ok = new Button { Content = "OK", IsDefault = true };
        ok.Click += (_, _) => { _accepted = true; _main.Settings.Save(); Close(); };
        var cancel = new Button { Content = "Cancel", IsCancel = true, Margin = new Thickness(8, 0, 0, 0) };
        cancel.Click += (_, _) => Close();
        right.Children.Add(ok);
        right.Children.Add(cancel);
        bar.Children.Add(right);
        var left = new StackPanel { Orientation = Orientation.Horizontal };
        left.Children.Add(Btn("Import...", Import));
        left.Children.Add(Btn("Export...", Export, 8));
        left.Children.Add(Btn("Reset everything", ResetAll, 8));
        bar.Children.Add(left);

        var bottom = new Border { Child = bar, BorderBrush = (Brush)FindResource("Line"), BorderThickness = new Thickness(0, 1, 0, 0), Background = (Brush)FindResource("PanelBg") };
        DockPanel.SetDock(bottom, Dock.Bottom);
        var navBorder = new Border { Child = _nav, BorderBrush = (Brush)FindResource("Line"), BorderThickness = new Thickness(0, 0, 1, 0) };
        _nav.SetResourceReference(BackgroundProperty, "PanelBg");
        DockPanel.SetDock(navBorder, Dock.Left);

        var root = new DockPanel();
        root.Children.Add(bottom);
        root.Children.Add(navBorder);
        root.Children.Add(_scroll);
        Content = root;

        _nav.SelectedIndex = 0;
        Closed += (_, _) =>
        {
            if (_accepted) return;
            _main.Settings.Theme = _snapshot;
            _main.Settings.RestoreSession = _snapshotSession;
            _main.Settings.HotExit = _snapshotHotExit;
            _main.ApplyAll();
        };
    }

    void Apply(bool full = false)
    {
        _fullApply |= full;
        _applyTimer.Stop();
        _applyTimer.Start();
    }

    void ShowPage(int i)
    {
        if (i < 0 || i >= _pages.Length) return;
        double offset = _scroll.VerticalOffset;
        _scroll.Content = _pages[i].build();
        _scroll.ScrollToVerticalOffset(0);
    }

    void Rebuild()
    {
        double offset = _scroll.VerticalOffset;
        ShowPage(_nav.SelectedIndex);
        _scroll.ScrollToVerticalOffset(offset);
    }

    // ================================================================ building blocks
    static Button Btn(string text, Action click, double left = 0)
    {
        var b = new Button { Content = text, Margin = new Thickness(left, 0, 0, 0) };
        b.Click += (_, _) => click();
        return b;
    }

    StackPanel Page(string title, string hint)
    {
        var p = new StackPanel { Margin = new Thickness(26, 18, 26, 24) };
        p.Children.Add(new TextBlock { Text = title, FontSize = 20, FontWeight = FontWeights.SemiBold, Margin = new Thickness(0, 0, 0, 4) });
        if (hint != null)
        {
            var h = new TextBlock { Text = hint, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 0, 0, 14) };
            h.SetResourceReference(TextBlock.ForegroundProperty, "TextDim");
            p.Children.Add(h);
        }
        return p;
    }

    static TextBlock Section(string text)
    {
        var t = new TextBlock { Text = text.ToUpperInvariant(), FontSize = 11, FontWeight = FontWeights.SemiBold, Margin = new Thickness(0, 18, 0, 6) };
        t.SetResourceReference(TextBlock.ForegroundProperty, "TextDim");
        return t;
    }

    static Grid Row(string label, UIElement control, string hint = null)
    {
        var g = new Grid { Margin = new Thickness(0, 4, 0, 4), MinHeight = 28 };
        g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(230) });
        g.ColumnDefinitions.Add(new ColumnDefinition());
        var l = new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 0, 12, 0) };
        if (hint != null) l.ToolTip = hint;
        g.Children.Add(l);
        Grid.SetColumn(control, 1);
        g.Children.Add(control);
        return g;
    }

    Grid SliderRow(string label, double min, double max, double value, Action<double> set, Func<double, string> fmt, double step = 0)
    {
        var panel = new DockPanel();
        var val = new TextBlock { Width = 64, VerticalAlignment = VerticalAlignment.Center, TextAlignment = TextAlignment.Right, Text = fmt(value) };
        DockPanel.SetDock(val, Dock.Right);
        var sl = new Slider { Minimum = min, Maximum = max, Value = value, MaxWidth = 380, HorizontalAlignment = HorizontalAlignment.Left, Width = 380 };
        if (step > 0) { sl.TickFrequency = step; sl.IsSnapToTickEnabled = true; sl.SmallChange = step; sl.LargeChange = step * 4; }
        sl.ValueChanged += (_, e) =>
        {
            val.Text = fmt(e.NewValue);
            set(e.NewValue);
            Apply(true);
        };
        panel.Children.Add(val);
        panel.Children.Add(sl);
        return Row(label, panel);
    }

    Grid ChoiceRow(string label, (string value, string text)[] options, string current, Action<string> set, bool rebuild = false)
    {
        var wrap = new WrapPanel();
        string group = "g" + (++_group);
        foreach (var (value, text) in options)
        {
            var v = value;
            var rb = new RadioButton { Content = text, GroupName = group, IsChecked = v == current, Margin = new Thickness(0, 3, 18, 3) };
            rb.Checked += (_, _) =>
            {
                set(v);
                Apply(true);
                if (rebuild) Dispatcher.BeginInvoke(Rebuild);
            };
            wrap.Children.Add(rb);
        }
        return Row(label, wrap);
    }

    Grid CheckRow(string label, bool value, Action<bool> set, string text = "On")
    {
        var cb = new CheckBox { Content = text, IsChecked = value };
        cb.Checked += (_, _) => { set(true); Apply(true); };
        cb.Unchecked += (_, _) => { set(false); Apply(true); };
        return Row(label, cb);
    }

    Grid FileRow(string label, string value, Action<string> set, string filter)
    {
        var panel = new DockPanel();
        var box = new TextBox { Text = value ?? "" };
        var browse = new Button { Content = "Browse...", Margin = new Thickness(8, 0, 0, 0), MinWidth = 80 };
        var clear = new Button { Content = "Clear", Margin = new Thickness(6, 0, 0, 0), MinWidth = 60 };
        DockPanel.SetDock(clear, Dock.Right);
        DockPanel.SetDock(browse, Dock.Right);
        browse.Click += (_, _) =>
        {
            var dlg = new OpenFileDialog { Filter = filter };
            if (File.Exists(box.Text)) dlg.InitialDirectory = Path.GetDirectoryName(box.Text);
            if (dlg.ShowDialog(this) == true) box.Text = dlg.FileName;
        };
        clear.Click += (_, _) => box.Text = "";
        box.TextChanged += (_, _) => { set(box.Text.Trim().Trim('"')); Apply(true); };
        panel.Children.Add(clear);
        panel.Children.Add(browse);
        panel.Children.Add(box);
        return Row(label, panel);
    }

    /// <summary>Color swatch + hex field + reset button for one theme color.</summary>
    Grid ColorRow(ColorDef def)
    {
        var colors = ThemeCatalog.Resolve(T);
        var current = colors.TryGetValue(def.Id, out var c) ? c : Colors.Magenta;
        var swatchColor = new SolidColorBrush(current);
        var swatch = new Border
        {
            Width = 46, Height = 24, Cursor = Cursors.Hand, CornerRadius = new CornerRadius(3),
            Background = ColorPickerDialog.Checker(), BorderThickness = new Thickness(1),
            Child = new Border { Background = swatchColor, CornerRadius = new CornerRadius(2) },
            ToolTip = "Click to pick a color",
        };
        swatch.SetResourceReference(Border.BorderBrushProperty, "FieldBorder");
        var hex = new TextBox { Text = ThemeCatalog.Hex(current), Width = 100, Margin = new Thickness(10, 0, 0, 0), FontFamily = new FontFamily("Consolas") };
        bool overridden = T.Colors.ContainsKey(def.Id);
        var reset = new Button
        {
            Content = "Reset", MinWidth = 60, Margin = new Thickness(8, 0, 0, 0),
            Visibility = overridden ? Visibility.Visible : Visibility.Hidden,
            ToolTip = "Back to the preset color",
        };
        bool sync = false;
        void SetValue(Color col, bool updateHex)
        {
            T.Colors[def.Id] = ThemeCatalog.Hex(col);
            swatchColor.Color = col;
            reset.Visibility = Visibility.Visible;
            if (updateHex)
            {
                sync = true;
                hex.Text = ThemeCatalog.Hex(col);
                sync = false;
            }
            Apply();
        }
        swatch.MouseLeftButtonUp += (_, _) =>
        {
            var before = T.Colors.TryGetValue(def.Id, out var b) ? b : null;
            var dlg = new ColorPickerDialog(this, swatchColor.Color, def.Label);
            dlg.Changed += col => SetValue(col, true);
            if (dlg.ShowDialog() != true)
            {
                if (before == null) T.Colors.Remove(def.Id);
                else T.Colors[def.Id] = before;
                var restored = ThemeCatalog.Resolve(T)[def.Id];
                swatchColor.Color = restored;
                sync = true;
                hex.Text = ThemeCatalog.Hex(restored);
                sync = false;
                reset.Visibility = before == null ? Visibility.Hidden : Visibility.Visible;
                Apply();
            }
        };
        hex.TextChanged += (_, _) =>
        {
            if (sync) return;
            try { SetValue(ThemeCatalog.Parse(hex.Text), false); } catch { }
        };
        reset.Click += (_, _) =>
        {
            T.Colors.Remove(def.Id);
            var col = ThemeCatalog.Resolve(T)[def.Id];
            swatchColor.Color = col;
            sync = true;
            hex.Text = ThemeCatalog.Hex(col);
            sync = false;
            reset.Visibility = Visibility.Hidden;
            Apply();
        };
        var panel = new StackPanel { Orientation = Orientation.Horizontal };
        panel.Children.Add(swatch);
        panel.Children.Add(hex);
        panel.Children.Add(reset);
        return Row(def.Label, panel);
    }

    static string Pct(double v) => $"{Math.Round(v * 100)}%";
    static string Px(double v) => v.ToString("0.#", CultureInfo.InvariantCulture) + " px";

    // ================================================================ pages
    FrameworkElement PagePresets()
    {
        var p = Page("Theme presets", "Pick a starting point. Every color can then be changed on the next pages; your changes are kept on top of the preset.");
        var wrap = new WrapPanel();
        foreach (var name in ThemeCatalog.Presets.Keys)
        {
            var n = name;
            Color C(string id) => ThemeCatalog.PresetColor(n, id);
            bool selected = T.Preset == n;

            var preview = new Grid { Height = 92 };
            preview.RowDefinitions.Add(new RowDefinition { Height = new GridLength(10) });
            preview.RowDefinitions.Add(new RowDefinition { Height = new GridLength(10) });
            preview.RowDefinitions.Add(new RowDefinition());
            preview.RowDefinitions.Add(new RowDefinition { Height = new GridLength(9) });
            preview.Children.Add(new Border { Background = new SolidColorBrush(C("ui.menuBar")) });
            var tb = new Border { Background = new SolidColorBrush(C("ui.tabBar")) };
            Grid.SetRow(tb, 1);
            preview.Children.Add(tb);
            var ed = new Grid { Background = new SolidColorBrush(C("ed.bg")) };
            ed.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(14) });
            ed.ColumnDefinitions.Add(new ColumnDefinition());
            ed.Children.Add(new Border { Background = new SolidColorBrush(C("ed.gutter")) });
            var lines = new StackPanel { Margin = new Thickness(6, 6, 6, 0) };
            lines.Children.Add(Bar(C("syn.keyword"), 34, C("syn.type"), 40));
            lines.Children.Add(Bar(C("ed.fg"), 18, C("syn.string"), 56, C("ed.curLine")));
            lines.Children.Add(Bar(C("syn.method"), 46, C("syn.number"), 14));
            lines.Children.Add(Bar(C("syn.comment"), 80, C("syn.comment"), 0));
            Grid.SetColumn(lines, 1);
            ed.Children.Add(lines);
            Grid.SetRow(ed, 2);
            preview.Children.Add(ed);
            var st = new Border { Background = new SolidColorBrush(C("ui.status")) };
            Grid.SetRow(st, 3);
            preview.Children.Add(st);

            var card = new StackPanel();
            card.Children.Add(new Border { Child = preview, CornerRadius = new CornerRadius(3), ClipToBounds = true });
            card.Children.Add(new TextBlock { Text = n, Margin = new Thickness(2, 6, 0, 0) });
            var border = new Border
            {
                Width = 176, Padding = new Thickness(6), Margin = new Thickness(0, 0, 12, 12), Cursor = Cursors.Hand,
                CornerRadius = new CornerRadius(5), BorderThickness = new Thickness(2), Child = card,
            };
            border.SetResourceReference(Border.BorderBrushProperty, selected ? "Accent" : "FieldBorder");
            border.SetResourceReference(Border.BackgroundProperty, "FieldBg");
            border.MouseLeftButtonUp += (_, _) =>
            {
                T.Preset = n;
                T.Colors.Clear();
                Apply(true);
                Dispatcher.BeginInvoke(Rebuild);
            };
            wrap.Children.Add(border);
        }
        p.Children.Add(wrap);
        if (T.Colors.Count > 0)
        {
            var note = new TextBlock { Text = $"{T.Colors.Count} color(s) changed by hand on top of \"{T.Preset}\".", Margin = new Thickness(0, 6, 0, 6) };
            note.SetResourceReference(TextBlock.ForegroundProperty, "TextDim");
            p.Children.Add(note);
            p.Children.Add(new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Children = { Btn("Drop my color changes", () => { T.Colors.Clear(); Apply(true); Rebuild(); }) },
            });
        }
        return p;
    }

    static StackPanel Bar(Color a, double wa, Color b, double wb, Color? back = null)
    {
        var s = new StackPanel { Orientation = Orientation.Horizontal, Height = 9, Margin = new Thickness(0, 0, 0, 5) };
        if (back is Color bg) s.Background = new SolidColorBrush(bg);
        s.Children.Add(new Border { Width = wa, Height = 4, Background = new SolidColorBrush(a), CornerRadius = new CornerRadius(2), Margin = new Thickness(0, 0, 5, 0) });
        if (wb > 0) s.Children.Add(new Border { Width = wb, Height = 4, Background = new SolidColorBrush(b), CornerRadius = new CornerRadius(2) });
        return s;
    }

    FrameworkElement PageColors(string group, string title, string hint)
    {
        var p = Page(title, hint + " Click a swatch to pick a color (with transparency), or type #RRGGBB / #AARRGGBB.");
        foreach (var d in ThemeCatalog.Defs.Where(d => d.Group == group)) p.Children.Add(ColorRow(d));
        p.Children.Add(new StackPanel
        {
            Orientation = Orientation.Horizontal, Margin = new Thickness(0, 14, 0, 0),
            Children =
            {
                Btn("Reset this page", () =>
                {
                    foreach (var d in ThemeCatalog.Defs.Where(d => d.Group == group)) T.Colors.Remove(d.Id);
                    Apply(true);
                    Rebuild();
                }),
            },
        });
        return p;
    }

    FrameworkElement PageBackground()
    {
        var p = Page("Background", "Put a picture behind the text or behind the whole window. Colors with transparency (alpha) let it show through more.");
        p.Children.Add(ChoiceRow("Show background image", new[] { ("Off", "Off"), ("Editor", "Behind the text"), ("Window", "Behind the whole window") },
            T.BgTarget, v => T.BgTarget = v, true));
        p.Children.Add(FileRow("Image file", T.BgImage, v => T.BgImage = v,
            "Images (*.png;*.jpg;*.jpeg;*.bmp;*.gif;*.webp;*.tif)|*.png;*.jpg;*.jpeg;*.bmp;*.gif;*.webp;*.tif|All Files (*.*)|*.*"));
        p.Children.Add(SliderRow("Image opacity", 0, 1, T.BgOpacity, v => T.BgOpacity = v, Pct));
        p.Children.Add(SliderRow("Blur", 0, 40, T.BgBlur, v => T.BgBlur = v, Px, 1));
        p.Children.Add(ChoiceRow("Size", new[] { ("UniformToFill", "Cover"), ("Uniform", "Fit"), ("Fill", "Stretch"), ("None", "Original"), ("Tile", "Tile") },
            T.BgStretch, v => T.BgStretch = v));
        p.Children.Add(ChoiceRow("Position", new[] { ("Center", "Center"), ("TopLeft", "Top left"), ("TopRight", "Top right"), ("BottomLeft", "Bottom left"), ("BottomRight", "Bottom right") },
            T.BgAlign, v => T.BgAlign = v));
        p.Children.Add(Section("Whole-window image"));
        p.Children.Add(SliderRow("Panel opacity", 0, 1, T.PanelOpacity, v => T.PanelOpacity = v, Pct));
        var hint = new TextBlock { Text = "How solid the menu, toolbar, tabs, text area and status bar stay over a whole-window image.", TextWrapping = TextWrapping.Wrap };
        hint.SetResourceReference(TextBlock.ForegroundProperty, "TextDim");
        p.Children.Add(hint);
        p.Children.Add(Section("Background color"));
        p.Children.Add(ColorRow(ThemeCatalog.Defs.First(d => d.Id == "ed.bg")));
        p.Children.Add(ColorRow(ThemeCatalog.Defs.First(d => d.Id == "ui.window")));
        return p;
    }

    FrameworkElement PageCaret()
    {
        var p = Page("Caret and pointer", "The blinking text cursor and the mouse pointer.");
        p.Children.Add(Section("Text cursor (caret)"));
        p.Children.Add(ChoiceRow("Shape", new[] { ("Line", "Line  |"), ("Block", "Block  █"), ("Underline", "Underline  _"), ("Box", "Frame  ▯") },
            T.CaretStyle, v => T.CaretStyle = v));
        p.Children.Add(SliderRow("Thickness", 1, 8, T.CaretWidth, v => T.CaretWidth = v, Px, 0.5));
        p.Children.Add(CheckRow("Blinking", T.CaretBlink, v => T.CaretBlink = v, "Blink"));
        p.Children.Add(SliderRow("Blink speed", 150, 1500, T.CaretBlinkMs, v => T.CaretBlinkMs = (int)v, v => $"{v:0} ms", 10));
        p.Children.Add(ColorRow(ThemeCatalog.Defs.First(d => d.Id == "ed.caret")));
        var tip = new TextBlock { Text = "Overwrite mode (Insert key) always shows a block.", Margin = new Thickness(0, 2, 0, 0) };
        tip.SetResourceReference(TextBlock.ForegroundProperty, "TextDim");
        p.Children.Add(tip);

        p.Children.Add(Section("Mouse pointer"));
        p.Children.Add(ChoiceRow("Pointer over text", new[] { ("IBeam", "I-beam"), ("Arrow", "Arrow"), ("Cross", "Cross"), ("Hand", "Hand"), ("Pen", "Pen"), ("Custom", "From file") },
            T.Pointer, v => T.Pointer = v));
        p.Children.Add(FileRow("Custom pointer file", T.PointerFile, v => T.PointerFile = v, "Cursors (*.cur;*.ani)|*.cur;*.ani|All Files (*.*)|*.*"));
        p.Children.Add(CheckRow("Use it everywhere", T.PointerWholeWindow, v => T.PointerWholeWindow = v, "Also over menus, toolbar and panels"));
        return p;
    }

    FrameworkElement PageFonts()
    {
        var p = Page("Fonts and sizes", null);
        p.Children.Add(Section("Interface"));
        var fontPanel = new StackPanel { Orientation = Orientation.Horizontal };
        var fontName = new TextBlock { Text = T.UiFont, VerticalAlignment = VerticalAlignment.Center, Width = 200, FontFamily = new FontFamily(T.UiFont) };
        var choose = new Button { Content = "Choose..." };
        choose.Click += (_, _) =>
        {
            var dlg = new FontDialog(this, T.UiFont, T.UiFontSize * 96 / 72, false, false);
            if (dlg.ShowDialog() != true) return;
            T.UiFont = dlg.FamilyName;
            fontName.Text = dlg.FamilyName;
            fontName.FontFamily = new FontFamily(dlg.FamilyName);
            Apply(true);
        };
        fontPanel.Children.Add(fontName);
        fontPanel.Children.Add(choose);
        p.Children.Add(Row("Menu and dialog font", fontPanel));
        p.Children.Add(SliderRow("Interface text size", 9, 18, T.UiFontSize, v => T.UiFontSize = v, v => $"{v:0.#} pt", 0.5));
        p.Children.Add(ChoiceRow("Toolbar icons", new[] { ("16", "16 px"), ("20", "20 px"), ("24", "24 px"), ("32", "32 px") },
            T.ToolbarIconSize.ToString(CultureInfo.InvariantCulture), v => T.ToolbarIconSize = double.Parse(v, CultureInfo.InvariantCulture)));
        p.Children.Add(SliderRow("Scrollbar width", 4, 24, T.ScrollbarSize, v => T.ScrollbarSize = v, Px, 1));
        p.Children.Add(ChoiceRow("Window corners", new[] { ("Round", "Round"), ("Small", "Slightly round"), ("Square", "Square") },
            T.Corners, v => T.Corners = v));

        p.Children.Add(Section("Editor"));
        var edFont = new Button { Content = "Editor font...", HorizontalAlignment = HorizontalAlignment.Left };
        edFont.Click += (_, _) => _main.ChooseEditorFont();
        p.Children.Add(Row("Text font", edFont));
        p.Children.Add(SliderRow("Line number size", 0.5, 1.2, T.LineNumberScale, v => T.LineNumberScale = v, v => $"{v * 100:0}% of text", 0.05));
        return p;
    }

    FrameworkElement PageTabs()
    {
        var p = Page("Tabs and status bar", null);
        p.Children.Add(Section("Tabs"));
        p.Children.Add(ChoiceRow("Show tab bar", new[] { ("Always", "Always"), ("Multiple", "Only with 2+ tabs"), ("Never", "Never") },
            T.TabBarMode, v => T.TabBarMode = v));
        p.Children.Add(ChoiceRow("Tab bar position", new[] { ("Top", "Top"), ("Bottom", "Bottom") }, T.TabPosition, v => T.TabPosition = v));
        p.Children.Add(CheckRow("Close buttons on tabs", T.TabCloseButtons, v => T.TabCloseButtons = v, "Show"));
        p.Children.Add(SliderRow("Maximum tab width", 100, 400, T.TabMaxWidth, v => T.TabMaxWidth = v, Px, 10));

        p.Children.Add(Section("Session"));
        var s = _main.Settings;
        p.Children.Add(CheckRow("Restore tabs on start", s.RestoreSession, v => s.RestoreSession = v, "Reopen the files that were open"));
        p.Children.Add(CheckRow("Unsaved changes", s.HotExit, v => s.HotExit = v, "Keep them on exit instead of asking (they come back next time)"));

        p.Children.Add(Section("Status bar fields"));
        var grid = new WrapPanel();
        foreach (var (id, label) in MainWindow.StatusFields)
        {
            var key = id;
            var cb = new CheckBox { Content = label, IsChecked = !T.HiddenStatus.Contains(key), Width = 220, Margin = new Thickness(0, 3, 0, 3) };
            cb.Checked += (_, _) => { T.HiddenStatus.Remove(key); Apply(true); };
            cb.Unchecked += (_, _) => { if (!T.HiddenStatus.Contains(key)) T.HiddenStatus.Add(key); Apply(true); };
            grid.Children.Add(cb);
        }
        p.Children.Add(grid);
        return p;
    }

    // ================================================================ import / export
    void Import()
    {
        var dlg = new OpenFileDialog { Filter = "Notepad9 theme (*.n9theme)|*.n9theme|JSON (*.json)|*.json|All Files (*.*)|*.*" };
        if (dlg.ShowDialog(this) != true) return;
        try
        {
            _main.Settings.Theme = ThemeSettings.Import(dlg.FileName);
            Apply(true);
            Rebuild();
        }
        catch (Exception ex) { MessageDialog.Info(this, "Could not read the theme.\n\n" + ex.Message, "exit"); }
    }

    void Export()
    {
        var dlg = new SaveFileDialog { Filter = "Notepad9 theme (*.n9theme)|*.n9theme", FileName = "My theme.n9theme" };
        if (dlg.ShowDialog(this) != true) return;
        try { T.Export(dlg.FileName); }
        catch (Exception ex) { MessageDialog.Info(this, "Could not save the theme.\n\n" + ex.Message, "exit"); }
    }

    void ResetAll()
    {
        if (!MessageDialog.Confirm(this, "Reset colors, background, caret, fonts and layout to the defaults?", "settings")) return;
        _main.Settings.Theme = new ThemeSettings();
        Apply(true);
        Rebuild();
    }
}
