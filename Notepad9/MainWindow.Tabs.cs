using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Effects;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using ICSharpCode.AvalonEdit;
using ICSharpCode.AvalonEdit.Highlighting;
using Notepad9.Controls;
using Notepad9.Dialogs;
using Notepad9.Editor;
using Notepad9.Services;

namespace Notepad9;

/// <summary>Tabs, session and theme handling of the main window.</summary>
public partial class MainWindow
{
    // ===================================================================== tabs
    DocumentTab NewTab(bool activate = true, int untitledNumber = 0)
    {
        int n = untitledNumber;
        if (n <= 0)
        {
            n = 1;
            while (_tabs.Any(x => x.FilePath == null && x.UntitledNumber == n)) n++;
        }
        var t = new DocumentTab(new N9Editor(), n);
        WireEditor(t);
        int at = _tab == null ? _tabs.Count : _tabs.IndexOf(_tab) + 1;
        _tabs.Insert(Math.Clamp(at, 0, _tabs.Count), t);
        if (activate) ActivateTab(t);
        return t;
    }

    void WireEditor(DocumentTab t)
    {
        var ed = t.Editor;
        ed.ContextMenu = (ContextMenu)FindResource("EditorMenu");
        ed.TextChanged += (_, _) =>
        {
            _sessionDirty = true;
            if (t != _tab) return;
            ScheduleStatus();
            ScheduleUi();
            _bytesTimer.Stop();
            _bytesTimer.Start();
            if (IsPreviewShown) { _mdTimer.Stop(); _mdTimer.Start(); }
        };
        ed.TextArea.SelectionChanged += (_, _) => { if (t == _tab) { ScheduleStatus(); ScheduleUi(); } };
        ed.TextArea.Caret.PositionChanged += (_, _) => { if (t == _tab) ScheduleStatus(); };
        ed.FoldingStateChanged += (_, _) => { if (t == _tab) ScheduleUi(); };
        ed.PreviewMouseWheel += Editor_PreviewMouseWheel;
        ed.TextArea.TextView.ScrollOffsetChanged += (_, _) => { if (t == _tab) SyncPreviewScroll(); };
        ed.TextArea.PreviewKeyDown += EditorArea_PreviewKeyDown;
        t.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(DocumentTab.IsModified))
            {
                _sessionDirty = true;
                if (t == _tab) { UpdateTitle(); ScheduleUi(); }
            }
        };
        t.Complete.SchemeName = () => t.Scheme?.Name;
        ed.ApplyLook(_look);
        ApplyEditorSettings(t);
    }

    void ActivateTab(DocumentTab t)
    {
        if (t == null || t == _tab) return;
        if (_tab != null) _tab.IsActive = false;
        _tab = t;
        t.IsActive = true;
        EditorHost.Content = t.Editor;
        if (t.Pending != null) LoadPending(t);
        UpdateMarkdownMode();
        UpdateTitle();
        UpdateBytes();
        ScheduleStatus();
        ScheduleUi();
        Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
        {
            if (TabStrip.ItemContainerGenerator.ContainerFromItem(t) is FrameworkElement fe) fe.BringIntoView();
            if (t == _tab && IsActive) t.Editor.Focus();
        });
        CheckExternalChange();
    }

    void SwitchTab(int delta)
    {
        if (_tabs.Count < 2) return;
        int i = (_tabs.IndexOf(_tab) + delta + _tabs.Count) % _tabs.Count;
        ActivateTab(_tabs[i]);
    }

    /// <summary>Close a tab, asking to save unsaved changes. Returns false if the user cancelled.</summary>
    bool CloseTab(DocumentTab t, bool ask = true)
    {
        if (t == null) return true;
        if (ask && t.Pending?.Modified == true) ActivateTab(t);
        if (ask && t.Editor.IsModified)
        {
            ActivateTab(t);
            if (!ConfirmDiscard()) return false;
        }
        if (t.FilePath != null)
        {
            _closedTabs.Push(t.Pending ?? new SessionTab { Path = t.FilePath, Caret = t.Editor.CaretOffset, ScrollY = t.Editor.VerticalOffset });
        }
        int i = _tabs.IndexOf(t);
        bool wasActive = t == _tab;
        if (wasActive)
        {
            _tab = null;
            EditorHost.Content = null;
        }
        _tabs.Remove(t);
        t.Editor.ExitFocusedView();
        if (_tabs.Count == 0) NewTab();
        else if (wasActive) ActivateTab(_tabs[Math.Clamp(i, 0, _tabs.Count - 1)]);
        return true;
    }

    void CloseTabs(IEnumerable<DocumentTab> tabs)
    {
        foreach (var t in tabs.ToList())
            if (!CloseTab(t)) break;
    }

    void ReopenClosedTab()
    {
        while (_closedTabs.Count > 0)
        {
            var st = _closedTabs.Pop();
            if (!File.Exists(st.Path)) continue;
            if (OpenPath(st.Path))
            {
                RestoreView(_tab, st);
                return;
            }
        }
        ShowInfo("No closed tabs to reopen");
    }

    /// <summary>Open a file in a tab (switches to it if it is already open).</summary>
    public bool OpenPath(string path, FileEncoding? forced = null)
    {
        if (string.IsNullOrEmpty(path)) return false;
        var existing = _tabs.FirstOrDefault(x => string.Equals(x.FilePath, path, StringComparison.OrdinalIgnoreCase));
        if (existing != null)
        {
            ActivateTab(existing);
            return true;
        }
        bool created = false;
        if (_tab == null || !_tab.IsPristine)
        {
            NewTab();
            created = true;
        }
        if (LoadFile(path, forced)) return true;
        if (created) CloseTab(_tab, false);
        return false;
    }

    /// <summary>Files sent by a second Notepad9 process (e.g. "Open with" in Explorer).</summary>
    public void OpenFromOtherInstance(string[] files)
    {
        foreach (var f in files)
        {
            try { if (File.Exists(f)) OpenPath(Path.GetFullPath(f)); } catch { }
        }
        ShowFromBackground();
    }

    void SaveAll()
    {
        var current = _tab;
        foreach (var t in _tabs.Where(t => t.IsModified).ToList())
        {
            ActivateTab(t);
            if (!Save()) break;
        }
        ActivateTab(current);
    }

    void UpdateTabBarVisibility()
    {
        if (TabBar == null) return;
        string mode = Settings.Theme.TabBarMode;
        bool show = mode switch
        {
            "Never" => false,
            "Multiple" => _tabs.Count > 1,
            _ => true,
        };
        TabBar.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
    }

    // ---- tab strip mouse handling
    static DocumentTab TabOf(object sender) => (sender as FrameworkElement)?.DataContext as DocumentTab;

    void Tab_MouseDown(object sender, MouseButtonEventArgs e)
    {
        var t = TabOf(sender);
        if (t == null) return;
        if (e.ChangedButton == MouseButton.Left)
        {
            ActivateTab(t);
            _dragTab = t;
        }
        else if (e.ChangedButton == MouseButton.Middle) e.Handled = true;
    }

    void Tab_MouseUp(object sender, MouseButtonEventArgs e)
    {
        var t = TabOf(sender);
        if (t == null) return;
        switch (e.ChangedButton)
        {
            case MouseButton.Left:
                _dragTab = null;
                break;
            case MouseButton.Middle:
                CloseTab(t);
                e.Handled = true;
                break;
            case MouseButton.Right:
                ShowPopup(sender as FrameworkElement, BuildTabMenu(t));
                e.Handled = true;
                break;
        }
    }

    void Tab_MouseMove(object sender, MouseEventArgs e)
    {
        if (_dragTab == null) return;
        if (e.LeftButton != MouseButtonState.Pressed)
        {
            _dragTab = null;
            return;
        }
        var t = TabOf(sender);
        if (t == null || t == _dragTab) return;
        int from = _tabs.IndexOf(_dragTab), to = _tabs.IndexOf(t);
        if (from >= 0 && to >= 0) _tabs.Move(from, to);
    }

    void TabClose_Click(object sender, RoutedEventArgs e)
    {
        CloseTab(TabOf(sender));
        e.Handled = true;
    }

    void TabScroller_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        TabScroller.ScrollToHorizontalOffset(TabScroller.HorizontalOffset - e.Delta / 2.0);
        e.Handled = true;
    }

    void TabList_Click(object sender, RoutedEventArgs e)
    {
        var items = new List<object>();
        int i = 1;
        foreach (var t in _tabs)
        {
            var tab = t;
            string label = (i <= 9 ? $"_{i}  " : "    ") + Escape(t.Title) + (t.IsModified ? "  ●" : "");
            items.Add(Item(label, () => ActivateTab(tab), tab == _tab));
            i++;
        }
        items.Add(new Separator());
        items.Add(Item("Reopen Closed Tab", ReopenClosedTab, enabled: _closedTabs.Count > 0));
        items.Add(Item("Close All Tabs", () => CloseTabs(_tabs.ToList())));
        ShowPopup(sender as FrameworkElement, items);
    }

    List<object> BuildTabMenu(DocumentTab t)
    {
        int index = _tabs.IndexOf(t);
        var list = new List<object>
        {
            Item("_Close", () => CloseTab(t)),
            Item("Close _Others", () => CloseTabs(_tabs.Where(x => x != t)), enabled: _tabs.Count > 1),
            Item("Close Tabs to the _Right", () => CloseTabs(_tabs.Skip(index + 1)), enabled: index < _tabs.Count - 1),
            Item("Close _All", () => CloseTabs(_tabs.ToList())),
            new Separator(),
            Item("_Save", () => { ActivateTab(t); Save(); }, icon: "save", enabled: t.IsModified || t.FilePath == null),
            Item("Copy Full _Path", () => { try { Clipboard.SetText(t.FilePath); } catch { } }, enabled: t.FilePath != null),
            Item("Copy File _Name", () => { try { Clipboard.SetText(t.Title); } catch { } }),
            Item("Open Containing _Folder", () => Process.Start("explorer.exe", $"/select,\"{t.FilePath}\""), enabled: t.FilePath != null),
            new Separator(),
            Item("Reopen Closed _Tab", ReopenClosedTab, enabled: _closedTabs.Count > 0),
        };
        return list;
    }

    // ===================================================================== session
    void SaveSession(bool withBackups)
    {
        var data = new SessionData();
        var backups = new Dictionary<int, string>();
        foreach (var t in _tabs)
        {
            if (t.Pending is SessionTab pending)
            {
                if (t == _tab) data.Active = data.Tabs.Count;
                var copy = new SessionTab
                {
                    Path = pending.Path, UntitledNumber = pending.UntitledNumber, Caret = pending.Caret,
                    ScrollY = pending.ScrollY, ScrollX = pending.ScrollX, Encoding = pending.Encoding, Eol = pending.Eol,
                    Scheme = pending.Scheme, LastWrite = pending.LastWrite, Modified = pending.Modified && withBackups,
                };
                if (withBackups && pending.Backup != null && Session.ReadBackup(pending.Backup) is string saved)
                    backups[data.Tabs.Count] = saved;
                else copy.Modified = false;
                if (copy.Path == null && !copy.Modified) continue;
                data.Tabs.Add(copy);
                continue;
            }
            var ed = t.Editor;
            bool modified = ed.IsModified;
            if (t.FilePath == null && (!withBackups || ed.Document.TextLength == 0)) continue;
            if (t == _tab) data.Active = data.Tabs.Count;
            if (withBackups && modified) backups[data.Tabs.Count] = ed.Text;
            data.Tabs.Add(new SessionTab
            {
                Path = t.FilePath,
                UntitledNumber = t.UntitledNumber,
                Modified = withBackups && modified,
                Caret = ed.CaretOffset,
                ScrollY = ed.VerticalOffset,
                ScrollX = ed.HorizontalOffset,
                Encoding = t.Encoding,
                Eol = t.Eol,
                Scheme = t.Scheme?.Name,
                LastWrite = t.LastWrite,
            });
        }
        Session.Save(data, backups);
        _sessionDirty = false;
    }

    /// <summary>
    /// Recreates the tabs of the last session. Only the active tab is read from disk now;
    /// the others are loaded the first time they are shown, so start-up stays fast.
    /// </summary>
    void RestoreSession()
    {
        var data = Session.Load();
        var restored = new List<DocumentTab>();
        foreach (var st in data.Tabs)
        {
            bool hasBackup = st.Backup != null && File.Exists(Path.Combine(Session.Folder, st.Backup));
            bool fileOk = st.Path != null && File.Exists(st.Path);
            if (!hasBackup && !fileOk) continue;
            if (!hasBackup) st.Backup = null;
            var t = NewTab(false, st.Path == null ? st.UntitledNumber : 0);
            t.FilePath = st.Path;
            t.SetPending(st);
            restored.Add(t);
        }
        if (restored.Count > 0) ActivateTab(restored[Math.Clamp(data.Active, 0, restored.Count - 1)]);
        _sessionDirty = false;
    }

    /// <summary>Reads a tab restored from the session (it must be the active tab).</summary>
    void LoadPending(DocumentTab t)
    {
        var st = t.Pending;
        t.ClearPending();
        if (st == null) return;
        string text = st.Backup != null ? Session.ReadBackup(st.Backup) : null;
        if (text != null)
        {
            t.Editor.Text = text;
            t.FilePath = st.Path;
            t.Encoding = st.Encoding;
            t.Eol = st.Eol;
            t.LastWrite = st.LastWrite;
            var def = st.Scheme != null ? HighlightingManager.Instance.GetDefinition(st.Scheme) : null;
            if (def != null || st.Path == null) SetScheme(def);
            else AutoScheme();
            t.Editor.IsModified = true;
        }
        else
        {
            if (st.Path == null || !LoadFile(st.Path)) return;
            if (st.Scheme != null && st.Scheme != t.Scheme?.Name)
            {
                var def = HighlightingManager.Instance.GetDefinition(st.Scheme);
                if (def != null) SetScheme(def);
            }
        }
        RestoreView(t, st);
    }

    void RestoreView(DocumentTab t, SessionTab st)
    {
        var ed = t.Editor;
        Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
        {
            ed.CaretOffset = Math.Clamp(st.Caret, 0, ed.Document.TextLength);
            ed.ScrollToVerticalOffset(st.ScrollY);
            ed.ScrollToHorizontalOffset(st.ScrollX);
        });
    }

    // ===================================================================== theme / customization
    public void ApplyTheme()
    {
        var th = Settings.Theme ??= new ThemeSettings();
        var colors = ThemeApplier.ApplyResources(th);
        _look = ThemeApplier.BuildLook(th, colors);
        foreach (var t in _tabs) t.Editor.ApplyLook(_look);

        ApplyBackgroundImage(th);
        Cursor = th.PointerWholeWindow ? _look.Pointer : null;
        ApplyToolbarIconSize(th.ToolbarIconSize);

        bool bottom = th.TabPosition == "Bottom";
        DockPanel.SetDock(TabBar, bottom ? Dock.Bottom : Dock.Top);
        TabBar.BorderThickness = bottom ? new Thickness(0, 1, 0, 0) : new Thickness(0, 0, 0, 1);
        UpdateTabBarVisibility();
        ApplyStatusFields(th.HiddenStatus);

        SetCorners(th.Corners switch { "Square" => 1, "Small" => 3, _ => 2 });
        if (colors.TryGetValue("ui.border", out var border)) SetBorderColor(border);
        Preview.SetColors(colors["ed.bg"], colors["ed.fg"], colors["ed.link"]);
        ApplyFont();
        ScheduleStatus();
        ScheduleUi();
    }

    readonly Dictionary<string, (DateTime time, BitmapImage image)> _imageCache = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>Decodes an image at most <paramref name="maxWidth"/> pixels wide (0 = full size): big photos load much faster.</summary>
    BitmapImage LoadImage(string path, int maxWidth)
    {
        try
        {
            var time = File.GetLastWriteTimeUtc(path);
            string key = path + "|" + maxWidth;
            if (_imageCache.TryGetValue(key, out var hit) && hit.time == time) return hit.image;
            if (maxWidth > 0)
            {
                // read only the header: a picture smaller than the screen is decoded at its real size
                using var fs = File.OpenRead(path);
                var header = BitmapDecoder.Create(fs, BitmapCreateOptions.DelayCreation | BitmapCreateOptions.IgnoreColorProfile, BitmapCacheOption.None);
                if (header.Frames.Count == 0 || header.Frames[0].PixelWidth <= maxWidth) maxWidth = 0;
            }
            var b = new BitmapImage();
            b.BeginInit();
            b.UriSource = new Uri(path);
            b.CacheOption = BitmapCacheOption.OnLoad;
            b.CreateOptions = BitmapCreateOptions.IgnoreColorProfile;
            if (maxWidth > 0) b.DecodePixelWidth = maxWidth;
            b.EndInit();
            b.Freeze();
            _imageCache[key] = (time, b);
            return b;
        }
        catch { return null; }
    }

    void ApplyBackgroundImage(ThemeSettings th)
    {
        EditorBackImage.Visibility = Visibility.Collapsed;
        WindowBackImage.Visibility = Visibility.Collapsed;
        if (th.BgTarget is not ("Editor" or "Window") || string.IsNullOrWhiteSpace(th.BgImage) || !File.Exists(th.BgImage)) return;
        double scale = PresentationSource.FromVisual(this)?.CompositionTarget?.TransformToDevice.M11 ?? 1.0;
        int screenPx = (int)Math.Ceiling(SystemParameters.VirtualScreenWidth * scale);
        var img = LoadImage(th.BgImage, th.BgStretch is "Tile" or "None" ? 0 : Math.Max(1280, screenPx));
        if (img == null) return;
        var brush = new ImageBrush(img) { Opacity = Math.Clamp(th.BgOpacity, 0, 1) };
        switch (th.BgStretch)
        {
            case "Fill": brush.Stretch = Stretch.Fill; break;
            case "Uniform": brush.Stretch = Stretch.Uniform; break;
            case "None": brush.Stretch = Stretch.None; break;
            case "Tile":
                brush.Stretch = Stretch.Fill;
                brush.TileMode = TileMode.Tile;
                brush.ViewportUnits = BrushMappingMode.Absolute;
                brush.Viewport = new Rect(0, 0, Math.Max(1, img.Width), Math.Max(1, img.Height));
                break;
            default: brush.Stretch = Stretch.UniformToFill; break;
        }
        (brush.AlignmentX, brush.AlignmentY) = th.BgAlign switch
        {
            "TopLeft" => (AlignmentX.Left, AlignmentY.Top),
            "TopRight" => (AlignmentX.Right, AlignmentY.Top),
            "BottomLeft" => (AlignmentX.Left, AlignmentY.Bottom),
            "BottomRight" => (AlignmentX.Right, AlignmentY.Bottom),
            _ => (AlignmentX.Center, AlignmentY.Center),
        };
        brush.Freeze();
        var target = th.BgTarget == "Window" ? WindowBackImage : EditorBackImage;
        target.Background = brush;
        target.Effect = th.BgBlur > 0.5 ? new BlurEffect { Radius = th.BgBlur, KernelType = KernelType.Gaussian } : null;
        target.Visibility = Visibility.Visible;
    }

    void ApplyToolbarIconSize(double size)
    {
        size = Math.Clamp(size, 12, 48);
        foreach (var panel in new Panel[] { ToolbarPanel, MdBarPanel })
        foreach (var child in panel.Children)
        {
            if (child is ButtonBase b && b.Content is IconImage ii)
            {
                ii.Width = ii.Height = size;
                b.Width = b.Height = size + 9;
            }
        }
        Toolbar.MinHeight = size + 11;
    }

    /// <summary>Status bar fields that can be hidden from the Customize window.</summary>
    public static readonly (string id, string label)[] StatusFields =
    {
        ("ln", "Line"), ("col", "Column"), ("info", "Messages"), ("eval", "Eval (calculator)"), ("sel", "Selected characters"),
        ("sln", "Selected lines"), ("occ", "Occurrences"), ("bytes", "File size"), ("enc", "Encoding"), ("eol", "Line endings"),
        ("ins", "Insert / overwrite"), ("std", "Standard / secondary font"), ("scheme", "Syntax scheme"),
    };

    TextBlock StatusText(string id) => id switch
    {
        "ln" => SbLn, "col" => SbCol, "info" => SbInfo, "eval" => SbEval, "sel" => SbSel, "sln" => SbSln, "occ" => SbOcc,
        "bytes" => SbBytes, "enc" => SbEnc, "eol" => SbEol, "ins" => SbIns, "std" => SbStd, _ => SbScheme,
    };

    void ApplyStatusFields(List<string> hidden)
    {
        hidden ??= new List<string>();
        foreach (var (id, _) in StatusFields)
        {
            if (StatusText(id).Parent is not Border field) continue;
            int col = Grid.GetColumn(field);
            var def = StatusBar.ColumnDefinitions[col];
            if (!_statusWidths.ContainsKey(col)) _statusWidths[col] = def.Width;
            bool hide = hidden.Contains(id);
            field.Visibility = hide ? Visibility.Collapsed : Visibility.Visible;
            def.Width = hide ? new GridLength(0) : _statusWidths[col];
            if (col == StatusBar.ColumnDefinitions.Count - 1) def.MinWidth = hide ? 0 : 78;
        }
    }

    List<object> BuildPresetItems()
    {
        var list = new List<object>();
        foreach (var name in ThemeCatalog.Presets.Keys)
        {
            var n = name;
            bool current = Settings.Theme.Preset == n && Settings.Theme.Colors.Count == 0;
            list.Add(Item(Escape(n), () =>
            {
                Settings.Theme.Preset = n;
                Settings.Theme.Colors.Clear();
                ApplyTheme();
            }, current));
        }
        list.Add(new Separator());
        list.Add(Item("_Customize Interface...", ShowCustomize, icon: "settings"));
        return list;
    }

    void ShowCustomize()
    {
        var dlg = new CustomizeWindow(this);
        dlg.ShowDialog();
        ApplyTheme();
    }

    /// <summary>Re-applies the editor-related view settings after the Customize window changes fonts.</summary>
    public void ApplyAll()
    {
        ApplyTheme();
        ApplyViewSettings();
    }

    public void ChooseEditorFont() => ChooseFont(false);
}
