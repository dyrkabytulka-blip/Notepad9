using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Windows.Media.Effects;
using System.Windows.Media.Imaging;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Threading;
using ICSharpCode.AvalonEdit;
using ICSharpCode.AvalonEdit.Document;
using ICSharpCode.AvalonEdit.Editing;
using ICSharpCode.AvalonEdit.Highlighting;
using ICSharpCode.AvalonEdit.Indentation;
using ICSharpCode.AvalonEdit.Rendering;
using Microsoft.Win32;
using Notepad9.Controls;
using Notepad9.Dialogs;
using Notepad9.Editor;
using Notepad9.Services;

namespace Notepad9;

public partial class MainWindow
{
    public AppSettings Settings { get; }

    // ---- tabs: every document has its own editor; these properties always refer to the active one
    readonly ObservableCollection<DocumentTab> _tabs = new();
    DocumentTab _tab;
    readonly Stack<SessionTab> _closedTabs = new();
    DocumentTab _dragTab;
    readonly DispatcherTimer _sessionTimer;
    bool _sessionDirty;
    EditorLook _look = new();
    readonly string[] _startFiles;
    readonly Dictionary<int, GridLength> _statusWidths = new();

    /// <summary>The first Notepad9 process owns the session; extra windows ("New Window") do not.</summary>
    public bool IsPrimaryInstance { get; }

    public N9Editor Editor => _tab?.Editor;
    public string FilePath { get => _tab?.FilePath; private set => _tab.FilePath = value; }
    FileEncoding DocEncoding { get => _tab.Encoding; set => _tab.Encoding = value; }
    Eol DocEol { get => _tab.Eol; set => _tab.Eol = value; }
    DateTime DocLastWrite { get => _tab.LastWrite; set => _tab.LastWrite = value; }
    IHighlightingDefinition DocScheme { get => _tab?.Scheme; set => _tab.Scheme = value; }
    AutoCompleter DocComplete => _tab.Complete;
    FindReplaceWindow _find;
    FindInFilesWindow _findFiles;
    bool _statusPending, _uiPending, _clipHasText, _checkingExternal;
    string _evalResult;
    readonly DispatcherTimer _bytesTimer;
    readonly DispatcherTimer _infoTimer;
    readonly DispatcherTimer _mdTimer;
    int _lastPreviewLine = -1;

    const string AppName = "Notepad9";

    /// <summary>Shortcuts that act on the text; ignored while typing in the console.</summary>
    static readonly HashSet<string> EditorOnly = new()
    {
        "complete", "dup_line", "del_line", "line_up", "line_down", "upper", "lower", "datetime", "wrap",
        "md_bold", "md_italic", "md_link", "md_heading", "find", "replace", "goto", "focus",
    };

    static readonly Dictionary<string, string> SchemeNames = new()
    {
        ["C#"] = "C# Source Code", ["C++"] = "C/C++ Source Code", ["Java"] = "Java Source Code",
        ["JavaScript"] = "JavaScript", ["HTML"] = "Web Source Code", ["ASP/XHTML"] = "ASP / XHTML",
        ["XML"] = "XML Document", ["CSS"] = "CSS Style Sheets", ["PHP"] = "PHP Script", ["Python"] = "Python Script",
        ["PowerShell"] = "PowerShell Script", ["TSQL"] = "SQL Query", ["VB"] = "VB.NET Source Code",
        ["MarkDown"] = "Markdown", ["Json"] = "JSON", ["Patch"] = "Diff / Patch", ["TeX"] = "TeX / LaTeX",
        ["Boo"] = "Boo Source Code", ["Coco"] = "Coco/R Grammar",
    };

    static readonly Dictionary<(ModifierKeys, Key), string> KeyMap = new()
    {
        [(ModifierKeys.Control, Key.N)] = "new",
        [(ModifierKeys.Control, Key.T)] = "new",
        [(ModifierKeys.Control, Key.W)] = "close_tab",
        [(ModifierKeys.Control, Key.F4)] = "close_tab",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.T)] = "reopen_tab",
        [(ModifierKeys.Control, Key.Tab)] = "next_tab",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.Tab)] = "prev_tab",
        [(ModifierKeys.Control, Key.PageDown)] = "next_tab",
        [(ModifierKeys.Control, Key.PageUp)] = "prev_tab",
        [(ModifierKeys.Control | ModifierKeys.Alt, Key.S)] = "save_all",
        [(ModifierKeys.Control | ModifierKeys.Alt, Key.P)] = "customize",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.N)] = "window_new",
        [(ModifierKeys.Control, Key.O)] = "open",
        [(ModifierKeys.None, Key.F5)] = "run",
        [(ModifierKeys.Control, Key.R)] = "reload",
        [(ModifierKeys.Control, Key.Oem3)] = "console",
        [(ModifierKeys.Control, Key.Space)] = "complete",
        [(ModifierKeys.Control, Key.B)] = "md_bold",
        [(ModifierKeys.Control, Key.I)] = "md_italic",
        [(ModifierKeys.Control, Key.K)] = "md_link",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.H)] = "md_heading",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.V)] = "md_preview",
        [(ModifierKeys.Control, Key.S)] = "save",
        [(ModifierKeys.None, Key.F12)] = "save_as",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.S)] = "save_as",
        [(ModifierKeys.Control, Key.P)] = "print",
        [(ModifierKeys.Control, Key.Q)] = "exit",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.Q)] = "quit",
        [(ModifierKeys.Control, Key.F)] = "find",
        [(ModifierKeys.None, Key.F3)] = "find_next",
        [(ModifierKeys.Shift, Key.F3)] = "find_prev",
        [(ModifierKeys.Control, Key.H)] = "replace",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.F)] = "find_files",
        [(ModifierKeys.Control, Key.G)] = "goto",
        [(ModifierKeys.Control | ModifierKeys.Alt, Key.F)] = "focus",
        [(ModifierKeys.Alt, Key.Z)] = "wrap",
        [(ModifierKeys.Control, Key.OemPlus)] = "zoom_in",
        [(ModifierKeys.Control, Key.Add)] = "zoom_in",
        [(ModifierKeys.Control, Key.OemMinus)] = "zoom_out",
        [(ModifierKeys.Control, Key.Subtract)] = "zoom_out",
        [(ModifierKeys.Control, Key.D0)] = "zoom_reset",
        [(ModifierKeys.Control, Key.NumPad0)] = "zoom_reset",
        [(ModifierKeys.Alt, Key.T)] = "pin",
        [(ModifierKeys.Control, Key.D)] = "dup_line",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.D)] = "del_line",
        [(ModifierKeys.Alt, Key.Up)] = "line_up",
        [(ModifierKeys.Alt, Key.Down)] = "line_down",
        [(ModifierKeys.Control | ModifierKeys.Shift, Key.U)] = "upper",
        [(ModifierKeys.Control, Key.U)] = "lower",
        [(ModifierKeys.Control, Key.F5)] = "datetime",
        [(ModifierKeys.None, Key.F1)] = "shortcuts",
    };

    public MainWindow(AppSettings settings, string[] files, bool primaryInstance)
    {
        Settings = settings;
        IsPrimaryInstance = primaryInstance;
        _startFiles = files ?? Array.Empty<string>();
        InitializeComponent();
        Icon = AppIcon;
        TabStrip.ItemsSource = _tabs;
        _tabs.CollectionChanged += (_, _) => { UpdateTabBarVisibility(); _sessionDirty = true; };

        RestorePlacement();
        TrackStartupTime();

        _bytesTimer = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromMilliseconds(250) };
        _bytesTimer.Tick += (_, _) => { _bytesTimer.Stop(); UpdateBytes(); };
        _infoTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(4) };
        _infoTimer.Tick += (_, _) => { _infoTimer.Stop(); ScheduleStatus(); };
        // crash safety: keep the session (and unsaved text) on disk every 20 s while something changed
        _sessionTimer = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromSeconds(20) };
        _sessionTimer.Tick += (_, _) =>
        {
            if (_sessionDirty && IsPrimaryInstance && Settings.RestoreSession && Settings.HotExit) SaveSession(true);
        };

        PreviewKeyDown += Window_PreviewKeyDown;
        PreviewDragOver += Window_PreviewDragOver;
        PreviewDrop += Window_PreviewDrop;
        Activated += (_, _) => CheckExternalChange();
        Closing += Window_Closing;
        SourceInitialized += (_, _) => HookClipboard();

        // ---- console

        // ---- markdown
        _mdTimer = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromMilliseconds(250) };
        _mdTimer.Tick += (_, _) => { _mdTimer.Stop(); RefreshPreview(); };
        Preview.OpenFileRequested += path => OpenPath(path);

        ApplyTheme();
        ApplyViewSettings();

        Loaded += (_, _) =>
        {
            if (IsPrimaryInstance && Settings.RestoreSession) RestoreSession();
            foreach (var f in _startFiles)
            {
                try { OpenPath(Path.GetFullPath(f)); } catch { }
            }
            if (_tabs.Count == 0) NewTab();
            UpdateTitle();
            UpdateStatus();
            UpdateBytes();
            RefreshClipboard();
            _sessionTimer.Start();
            Editor.Focus();
        };
    }

    // ===================================================================== settings / view
    void RestorePlacement()
    {
        var s = Settings;
        Width = Math.Max(MinWidth, s.Width);
        Height = Math.Max(MinHeight, s.Height);
        if (!double.IsNaN(s.Left) && !double.IsNaN(s.Top) &&
            s.Left >= SystemParameters.VirtualScreenLeft - 50 && s.Top >= SystemParameters.VirtualScreenTop - 50 &&
            s.Left < SystemParameters.VirtualScreenLeft + SystemParameters.VirtualScreenWidth - 100 &&
            s.Top < SystemParameters.VirtualScreenTop + SystemParameters.VirtualScreenHeight - 100)
        {
            WindowStartupLocation = WindowStartupLocation.Manual;
            Left = s.Left;
            Top = s.Top;
        }
        else WindowStartupLocation = WindowStartupLocation.CenterScreen;
        if (s.Maximized) WindowState = WindowState.Maximized;
    }

    void StorePlacement()
    {
        var r = WindowState == WindowState.Normal ? new Rect(Left, Top, Width, Height) : RestoreBounds;
        Settings.Left = r.Left;
        Settings.Top = r.Top;
        Settings.Width = r.Width;
        Settings.Height = r.Height;
        Settings.Maximized = WindowState == WindowState.Maximized;
    }

    void ApplyViewSettings()
    {
        var s = Settings;
        foreach (var t in _tabs) ApplyEditorSettings(t);
        Toolbar.Visibility = s.ShowToolbar ? Visibility.Visible : Visibility.Collapsed;
        StatusBar.Visibility = s.ShowStatusBar ? Visibility.Visible : Visibility.Collapsed;
        Topmost = s.AlwaysOnTop;
        UpdateTabBarVisibility();
        ScheduleUi();
        ScheduleStatus();
    }

    void ApplyEditorSettings(DocumentTab t)
    {
        var s = Settings;
        var ed = t.Editor;
        ed.WordWrap = s.WordWrap;
        ed.SetShowLineNumbers(s.LineNumbers);
        ed.Options.ShowSpaces = s.ShowWhitespace;
        ed.Options.ShowTabs = s.ShowWhitespace;
        ed.Options.ShowEndOfLine = s.ShowEol;
        ed.SetLongLineMarker(s.LongLineMarker, Math.Clamp(s.LongLineColumn, 1, 1000));
        ed.Options.HighlightCurrentLine = s.HighlightCurrentLine;
        ed.Options.IndentationSize = Math.Clamp(s.TabWidth, 1, 16);
        ed.Options.ConvertTabsToSpaces = s.TabsAsSpaces;
        ed.TextArea.IndentationStrategy = s.AutoIndent ? new DefaultIndentationStrategy() : new NoIndentation();
        ed.Occurrences.Enabled = s.MarkOccurrences;
        t.Complete.Enabled = s.AutoComplete;
        ApplyFont(ed);
        ed.TextArea.TextView.Redraw();
    }

    sealed class NoIndentation : IIndentationStrategy
    {
        public void IndentLine(TextDocument document, DocumentLine line) { }
        public void IndentLines(TextDocument document, int beginLine, int endLine) { }
    }

    void ApplyFont()
    {
        foreach (var t in _tabs) ApplyFont(t.Editor);
    }

    void ApplyFont(N9Editor ed)
    {
        var s = Settings;
        string fam = s.UseSecondFont ? s.SecondFontFamily : s.FontFamily;
        ed.ApplyFont(new FontFamily(string.IsNullOrWhiteSpace(fam) ? "Consolas" : fam),
            Math.Clamp(s.FontSize * s.Zoom, 4, 400),
            s.FontBold ? FontWeights.Bold : FontWeights.Normal,
            s.FontItalic ? FontStyles.Italic : FontStyles.Normal);
    }

    // ===================================================================== command dispatch
    void Cmd(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement fe && fe.Tag is string id) Run(id, fe);
    }

    void Run(string id, FrameworkElement source = null)
    {
        var ed = Editor;
        var s = Settings;
        switch (id)
        {
            // ---- file
            case "new": NewTab(); break;
            case "close_tab": CloseTab(_tab); break;
            case "close_all": CloseTabs(_tabs.ToList()); break;
            case "reopen_tab": ReopenClosedTab(); break;
            case "next_tab": SwitchTab(+1); break;
            case "prev_tab": SwitchTab(-1); break;
            case "save_all": SaveAll(); break;
            case "customize": ShowCustomize(); break;
            case "toggle_session": s.RestoreSession = !s.RestoreSession; break;
            case "toggle_hotexit": s.HotExit = !s.HotExit; break;
            case "window_new": StartNewInstance(null); break;
            case "open": OpenFileDialog(); break;
            case "reload": Reload(null); break;
            case "save": Save(); break;
            case "save_as": SaveAs(false); break;
            case "save_copy": SaveAs(true); break;
            case "recent_menu": ShowPopup(source, BuildRecentItems()); break;
            case "fav_menu": ShowPopup(source, BuildFavoriteItems()); break;
            case "star_add": AddFavorite(); break;
            case "open_folder":
                if (FilePath != null) Process.Start("explorer.exe", $"/select,\"{FilePath}\"");
                break;
            case "terminal_ext": OpenTerminal(); break;
            case "console": ShowConsole(!IsConsoleShown, focus: true); break;
            case "run": RunCurrentFile(); break;
            case "print": Print(); break;
            case "exit": Close(); break;
            case "quit": QuitCompletely(); break;
            case "toggle_instant": s.InstantStart = !s.InstantStart; break;
            case "toggle_autostart": AutoStart.Set(!AutoStart.IsEnabled); break;

            // ---- edit
            case "undo": if (ed.CanUndo) ed.Undo(); break;
            case "redo": if (ed.CanRedo) ed.Redo(); break;
            case "cut": ed.Cut(); break;
            case "copy": ed.Copy(); break;
            case "paste": ed.Paste(); break;
            case "delete": EditingCommands.Delete.Execute(null, ed.TextArea); break;
            case "select_all": ed.SelectAll(); break;
            case "dup_line": DuplicateLine(); break;
            case "del_line": DeleteLines(); break;
            case "line_up": MoveLines(true); break;
            case "line_down": MoveLines(false); break;
            case "sort_asc": TransformLines(l => l.OrderBy(x => x, StringComparer.CurrentCulture)); break;
            case "sort_desc": TransformLines(l => l.OrderByDescending(x => x, StringComparer.CurrentCulture)); break;
            case "uniq": TransformLines(l => l.Distinct()); break;
            case "no_empty": TransformLines(l => l.Where(x => x.Trim().Length > 0)); break;
            case "join": TransformLines(l => l.Select(x => x.Trim()), " "); break;
            case "upper": TransformSelection(t => t.ToUpper(CultureInfo.CurrentCulture)); break;
            case "lower": TransformSelection(t => t.ToLower(CultureInfo.CurrentCulture)); break;
            case "title": TransformSelection(t => CultureInfo.CurrentCulture.TextInfo.ToTitleCase(t.ToLower(CultureInfo.CurrentCulture))); break;
            case "invert": TransformSelection(t => new string(t.Select(c => char.IsUpper(c) ? char.ToLower(c) : char.ToUpper(c)).ToArray())); break;
            case "tabs2spaces": TransformLines(l => l.Select(TabsToSpaces)); break;
            case "spaces2tabs": TransformLines(l => l.Select(LeadingSpacesToTabs)); break;
            case "trim": TransformLines(l => l.Select(x => x.TrimEnd(' ', '\t'))); break;
            case "datetime":
                ed.Document.Replace(ed.SelectionStart, ed.SelectionLength, DateTime.Now.ToString("g", CultureInfo.CurrentCulture));
                break;
            case "complete": DocComplete.ShowManual(); break;

            // ---- markdown
            case "md_bold": if (IsMarkdown || source != null) MarkdownTools.Wrap(ed, "**", "bold"); break;
            case "md_italic": if (IsMarkdown || source != null) MarkdownTools.Wrap(ed, "*", "italic"); break;
            case "md_strike": MarkdownTools.Wrap(ed, "~~", "text"); break;
            case "md_code": MarkdownTools.Wrap(ed, "`", "code"); break;
            case "md_heading": if (IsMarkdown || source != null) MarkdownTools.Heading(ed); break;
            case "md_codeblock": MarkdownTools.CodeBlock(ed); break;
            case "md_link": if (IsMarkdown || source != null) MarkdownTools.Link(ed); break;
            case "md_image": InsertImage(); break;
            case "md_ul": MarkdownTools.ToggleList(ed, MarkdownTools.ListKind.Bullet); break;
            case "md_ol": MarkdownTools.ToggleList(ed, MarkdownTools.ListKind.Numbered); break;
            case "md_task": MarkdownTools.ToggleList(ed, MarkdownTools.ListKind.Task); break;
            case "md_quote": MarkdownTools.ToggleList(ed, MarkdownTools.ListKind.Quote); break;
            case "md_table": MarkdownTools.Table(ed); break;
            case "md_hr": MarkdownTools.HorizontalRule(ed); break;
            case "md_preview":
                if (IsMarkdown)
                {
                    s.MdPreview = !IsPreviewShown;
                    ShowPreview(s.MdPreview);
                }
                break;
            case "md_sync":
                s.MdSyncScroll = !s.MdSyncScroll;
                _lastPreviewLine = -1;
                SyncPreviewScroll();
                break;

            // ---- search
            case "find": ShowFind(false); break;
            case "replace": ShowFind(true); break;
            case "find_next": FindAgain(false); break;
            case "find_prev": FindAgain(true); break;
            case "find_files": ShowFindInFiles(); break;
            case "goto": GoTo(); break;
            case "focus": ToggleFocusedView(); break;

            // ---- view
            case "toggle_toolbar": s.ShowToolbar = !s.ShowToolbar; ApplyViewSettings(); break;
            case "toggle_status": s.ShowStatusBar = !s.ShowStatusBar; ApplyViewSettings(); break;
            case "wrap": s.WordWrap = !s.WordWrap; ApplyViewSettings(); break;
            case "toggle_lnum": s.LineNumbers = !s.LineNumbers; ApplyViewSettings(); break;
            case "toggle_folding": s.CodeFolding = !s.CodeFolding; SetScheme(DocScheme); break;
            case "fold": ed.ToggleAllFolds(); break;
            case "toggle_ws": s.ShowWhitespace = !s.ShowWhitespace; ApplyViewSettings(); break;
            case "toggle_eol": s.ShowEol = !s.ShowEol; ApplyViewSettings(); break;
            case "ruler": s.LongLineMarker = !s.LongLineMarker; ApplyViewSettings(); break;
            case "ruler_col": AskLongLineColumn(); break;
            case "toggle_curline": s.HighlightCurrentLine = !s.HighlightCurrentLine; ApplyViewSettings(); break;
            case "toggle_occ": s.MarkOccurrences = !s.MarkOccurrences; ApplyViewSettings(); break;
            case "zoom_in": Zoom(s.Zoom * 1.1); break;
            case "zoom_out": Zoom(s.Zoom / 1.1); break;
            case "zoom_reset": Zoom(1.0); break;
            case "pin": s.AlwaysOnTop = !s.AlwaysOnTop; ApplyViewSettings(); break;

            // ---- appearance
            case "font": ChooseFont(false); break;
            case "font2": ChooseFont(true); break;
            case "toggle_2nd": s.UseSecondFont = !s.UseSecondFont; ApplyViewSettings(); break;
            case "toggle_autoscheme": s.AutoDetectScheme = !s.AutoDetectScheme; if (s.AutoDetectScheme) AutoScheme(); break;
            case "toggle_tabspaces": s.TabsAsSpaces = !s.TabsAsSpaces; ApplyViewSettings(); break;

            // ---- settings
            case "toggle_autoindent": s.AutoIndent = !s.AutoIndent; ApplyViewSettings(); break;
            case "toggle_autocomplete": s.AutoComplete = !s.AutoComplete; ApplyViewSettings(); break;
            case "toggle_detect": s.DetectFileChanges = !s.DetectFileChanges; break;
            case "toggle_remember":
                s.RememberRecent = !s.RememberRecent;
                if (!s.RememberRecent) s.Recent.Clear();
                break;
            case "toggle_saveonexit": s.SaveOnExit = !s.SaveOnExit; break;
            case "save_settings": StorePlacement(); s.Save(); ShowInfo("Settings saved"); break;
            case "settings_folder":
                Directory.CreateDirectory(AppSettings.Folder);
                Process.Start("explorer.exe", $"\"{AppSettings.Folder}\"");
                break;
            case "reset_settings":
                if (MessageDialog.Confirm(this, "Reset all settings to defaults? Notepad9 will use them the next time it starts.", "settings"))
                {
                    AppSettings.Reset();
                    s.SaveOnExit = false;
                }
                break;

            // ---- help
            case "shortcuts": new ShortcutsDialog(this).ShowDialog(); break;
            case "about": new AboutDialog(this).ShowDialog(); break;

            // ---- status bar
            case "enc_menu": ShowPopup(source, BuildEncodingItems(), PlacementMode.Top); break;
            case "eol_menu": ShowPopup(source, BuildEolItems(), PlacementMode.Top); break;
            case "scheme_menu": ShowPopup(source, BuildSchemeItems(), PlacementMode.Top); break;
            case "ins":
                ed.TextArea.OverstrikeMode = !ed.TextArea.OverstrikeMode;
                ScheduleStatus();
                break;
            case "eval_copy":
                if (_evalResult != null)
                {
                    try { Clipboard.SetText(_evalResult); ShowInfo("Result copied: " + _evalResult); } catch { }
                }
                break;
        }
        ScheduleUi();
        ScheduleStatus();
        if (source is ButtonBase or MenuItem && id is not ("exit" or "console" or "run" or "recent_menu" or "fav_menu" or "enc_menu" or "eol_menu" or "scheme_menu"))
            Dispatcher.BeginInvoke(DispatcherPriority.Input, () => { if (IsActive) Editor.Focus(); });
    }

    void Status_Click(object sender, MouseButtonEventArgs e)
    {
        if (sender is FrameworkElement fe && fe.Tag is string id) Run(id, fe);
    }

    // ===================================================================== keyboard / mouse
    void Window_PreviewKeyDown(object sender, KeyEventArgs e)
    {
        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        var mods = Keyboard.Modifiers & (ModifierKeys.Control | ModifierKeys.Shift | ModifierKeys.Alt);
        if (key == Key.Insert && mods == ModifierKeys.None)
        {
            Dispatcher.BeginInvoke(DispatcherPriority.Background, UpdateStatus);
            return;
        }
        if (key == Key.Escape && Editor.IsFocusedView && mods == ModifierKeys.None)
        {
            ToggleFocusedView();
            e.Handled = true;
            return;
        }
        if (mods == ModifierKeys.Control && key is >= Key.D1 and <= Key.D9)
        {
            int n = key - Key.D1;
            if (n < _tabs.Count) ActivateTab(key == Key.D9 ? _tabs[^1] : _tabs[n]);
            e.Handled = true;
            return;
        }
        if (KeyMap.TryGetValue((mods, key), out var id))
        {
            if (EditorOnly.Contains(id) && _term?.IsKeyboardFocusWithin == true) return;
            if (id is "md_bold" or "md_italic" or "md_link" or "md_heading" or "md_preview" && !IsMarkdown) return;
            e.Handled = true;
            Run(id);
        }
    }

    /// <summary>Markdown: Enter continues lists, Tab / Shift+Tab indent list items.</summary>
    void EditorArea_PreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (Editor == null || !IsMarkdown || DocComplete.IsOpen) return;
        var mods = Keyboard.Modifiers;
        if (e.Key == Key.Enter && mods == ModifierKeys.None)
        {
            if (MarkdownTools.HandleEnter(Editor))
            {
                e.Handled = true;
                Editor.TextArea.Caret.BringCaretToView();
            }
        }
        else if (e.Key == Key.Tab && mods is ModifierKeys.None or ModifierKeys.Shift)
        {
            if (MarkdownTools.HandleTab(Editor, mods == ModifierKeys.Shift)) e.Handled = true;
        }
    }

    void Editor_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if ((Keyboard.Modifiers & ModifierKeys.Control) == 0) return;
        e.Handled = true;
        Zoom(e.Delta > 0 ? Settings.Zoom * 1.1 : Settings.Zoom / 1.1);
    }

    void Zoom(double z)
    {
        Settings.Zoom = Math.Abs(z - 1.0) < 0.04 ? 1.0 : Math.Clamp(z, 0.3, 5.0);
        ApplyFont();
        ShowInfo($"Zoom {Settings.Zoom * 100:0}%");
    }

    void Window_PreviewDragOver(object sender, DragEventArgs e)
    {
        if (e.Data.GetDataPresent(DataFormats.FileDrop))
        {
            e.Effects = DragDropEffects.Copy;
            e.Handled = true;
        }
    }

    void Window_PreviewDrop(object sender, DragEventArgs e)
    {
        if (e.Data.GetData(DataFormats.FileDrop) is string[] files && files.Length > 0)
        {
            e.Handled = true;
            Activate();
            foreach (var f in files) if (File.Exists(f)) OpenPath(f);
        }
    }

    // ===================================================================== clipboard watcher
    [DllImport("user32.dll", SetLastError = true)]
    static extern bool AddClipboardFormatListener(IntPtr hwnd);

    void HookClipboard()
    {
        var h = new WindowInteropHelper(this).Handle;
        AddClipboardFormatListener(h);
        HwndSource.FromHwnd(h)?.AddHook(WndProc);
    }

    IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        if (msg == 0x031D) Dispatcher.BeginInvoke(DispatcherPriority.Background, RefreshClipboard); // WM_CLIPBOARDUPDATE
        return IntPtr.Zero;
    }

    void RefreshClipboard()
    {
        try { _clipHasText = Clipboard.ContainsText(); } catch { }
        ScheduleUi();
    }

    // ===================================================================== title / status / ui state
    string DisplayName => _tab?.Title ?? "Untitled";

    void UpdateTitle()
    {
        if (_tab != null) Title = $"{(Editor.IsModified ? "* " : "")}{DisplayName} - {AppName}";
    }


    void ScheduleStatus()
    {
        if (_statusPending) return;
        _statusPending = true;
        Dispatcher.BeginInvoke(DispatcherPriority.Background, UpdateStatus);
    }

    void ScheduleUi()
    {
        if (_uiPending) return;
        _uiPending = true;
        Dispatcher.BeginInvoke(DispatcherPriority.Background, UpdateUi);
    }

    static string N(long v) => v.ToString("#,0", CultureInfo.InvariantCulture);

    void UpdateStatus()
    {
        _statusPending = false;
        if (Editor == null) return;
        var doc = Editor.Document;
        var caret = Editor.TextArea.Caret;
        var line = doc.GetLineByNumber(Math.Clamp(caret.Line, 1, doc.LineCount));
        SbLn.Text = $"Ln  {N(caret.Line)} / {N(doc.LineCount)}";
        SbCol.Text = $"Col  {N(caret.Column)} / {N(line.Length)}";

        var sel = Editor.TextArea.Selection;
        int selLen = Editor.SelectionLength;
        string selText = null;
        if (!sel.IsEmpty)
        {
            SbSel.Text = $"Sel  {N(selLen)}";
            int l1 = sel.StartPosition.Line, l2 = sel.EndPosition.Line;
            SbSln.Text = $"SLn  {N(Math.Abs(l2 - l1) + 1)}";
            if (selLen <= 1000) selText = Editor.SelectedText;
        }
        else
        {
            SbSel.Text = "Sel  --";
            SbSln.Text = "SLn  --";
        }

        if (selText != null && selText.Length <= 256 && ExprEvaluator.TryEval(selText, out var v))
        {
            _evalResult = ExprEvaluator.Format(v);
            SbEval.Text = $"Eval  {_evalResult}";
        }
        else
        {
            _evalResult = null;
            SbEval.Text = "Eval  --";
        }

        string term = selText != null && selText.Length <= 200 && selText.IndexOfAny(new[] { '\r', '\n' }) < 0 && selText.Trim().Length > 0 ? selText : null;
        Editor.Occurrences.Term = term;
        Editor.TextArea.TextView.InvalidateLayer(KnownLayer.Selection);
        if (term != null && doc.TextLength < 50_000_000)
        {
            int count = 0, index = 0, pos = 0, selStart = Editor.SelectionStart;
            while (count < 100_000 && (pos = doc.IndexOf(term, pos, doc.TextLength - pos, StringComparison.Ordinal)) >= 0)
            {
                count++;
                if (pos == selStart) index = count;
                pos += term.Length;
            }
            SbOcc.Text = $"Occ  {(index > 0 ? N(index) : "-")}/{N(count)}";
        }
        else SbOcc.Text = "Occ  -/-";

        SbEnc.Text = TextFile.Name(DocEncoding);
        SbEol.Text = TextFile.Name(DocEol);
        SbIns.Text = Editor.TextArea.OverstrikeMode ? "OVR" : "INS";
        SbStd.Text = Settings.UseSecondFont ? "2ND" : "STD";
        SbScheme.Text = SchemeName(DocScheme);
        if (!_infoTimer.IsEnabled)
            SbInfo.Text = Editor.IsFocusedView ? "Focused view  (Esc to exit)" : "";
    }

    void ShowInfo(string text)
    {
        SbInfo.Text = text;
        _infoTimer.Stop();
        _infoTimer.Start();
    }

    void UpdateBytes()
    {
        if (Editor == null) return;
        long n = TextFile.ByteCount(Editor.Text, DocEncoding, DocEol);
        SbBytes.Text = n == 1 ? "1 byte" : $"{N(n)} bytes";
    }

    void UpdateUi()
    {
        _uiPending = false;
        if (Editor == null) return;
        var ed = Editor;
        bool hasText = ed.Document.TextLength > 0;
        bool hasSel = ed.SelectionLength > 0;
        TbSave.IsEnabled = ed.IsModified;
        TbUndo.IsEnabled = ed.CanUndo;
        TbRedo.IsEnabled = ed.CanRedo;
        TbCut.IsEnabled = hasSel;
        TbCopy.IsEnabled = hasSel;
        TbPaste.IsEnabled = _clipHasText;
        TbFind.IsEnabled = hasText;
        TbReplace.IsEnabled = hasText;
        TbFold.IsEnabled = ed.FoldingAvailable;
        TbFocus.IsEnabled = hasText || ed.IsFocusedView;
        TbFocus.IsChecked = ed.IsFocusedView;
        TbAddFav.IsEnabled = FilePath != null && !Settings.Favorites.Contains(FilePath, StringComparer.OrdinalIgnoreCase);
        TbWrap.IsChecked = Settings.WordWrap;
        TbRuler.IsChecked = Settings.LongLineMarker;
        TbPin.IsChecked = Settings.AlwaysOnTop;
        TbRun.IsEnabled = FilePath != null || hasText;
        TbConsole.IsChecked = IsConsoleShown;
        TbMdPreview.IsChecked = IsPreviewShown;
        TbMdSync.IsChecked = Settings.MdSyncScroll;
        TbMdSyncText.Foreground = Settings.MdSyncScroll ? Brushes.Black : new SolidColorBrush(Color.FromRgb(0xDE, 0xDE, 0xDE));
    }

    void Menu_Opened(object sender, RoutedEventArgs e)
    {
        if (Editor == null) return;
        var s = Settings;
        var ed = Editor;
        bool hasSel = ed.SelectionLength > 0;
        MiReload.IsEnabled = FilePath != null;
        MiOpenFolder.IsEnabled = FilePath != null;
        MiAddFav.IsEnabled = TbAddFav.IsEnabled;
        MiUndo.IsEnabled = ed.CanUndo;
        MiRedo.IsEnabled = ed.CanRedo;
        MiCut.IsEnabled = MiCopy.IsEnabled = MiDelete.IsEnabled = hasSel;
        MiPaste.IsEnabled = _clipHasText;
        MiFoldAll.IsEnabled = ed.FoldingAvailable;
        MiFocus.IsChecked = ed.IsFocusedView;
        MiToolbar.IsChecked = s.ShowToolbar;
        MiStatus.IsChecked = s.ShowStatusBar;
        MiWrap.IsChecked = s.WordWrap;
        MiLineNumbers.IsChecked = s.LineNumbers;
        MiFolding.IsChecked = s.CodeFolding;
        MiWhitespace.IsChecked = s.ShowWhitespace;
        MiShowEol.IsChecked = s.ShowEol;
        MiLongLine.IsChecked = s.LongLineMarker;
        MiCurLine.IsChecked = s.HighlightCurrentLine;
        MiMarkOcc.IsChecked = s.MarkOccurrences;
        MiTopmost.IsChecked = s.AlwaysOnTop;
        MiUse2nd.IsChecked = s.UseSecondFont;
        MiAutoScheme.IsChecked = s.AutoDetectScheme;
        MiTabsSpaces.IsChecked = s.TabsAsSpaces;
        MiAutoIndent.IsChecked = s.AutoIndent;
        MiDetect.IsChecked = s.DetectFileChanges;
        MiRemember.IsChecked = s.RememberRecent;
        MiSaveOnExit.IsChecked = s.SaveOnExit;
        MiAutoComplete.IsChecked = s.AutoComplete;
        MiConsole.IsChecked = IsConsoleShown;
        MiMdPreview.IsChecked = IsPreviewShown;
        MiMdPreview.IsEnabled = IsMarkdown;
        MiMdSync.IsChecked = s.MdSyncScroll;
        MiMdSync.IsEnabled = IsMarkdown;
        MiRun.IsEnabled = FilePath != null || ed.Document.TextLength > 0;
        MiRestoreSession.IsChecked = s.RestoreSession;
        MiInstant.IsChecked = s.InstantStart;
        MiAutoStart.IsChecked = AutoStart.IsEnabled;
        MiQuit.IsEnabled = IsPrimaryInstance;
        MiHotExit.IsChecked = s.HotExit;
        MiReopenTab.IsEnabled = _closedTabs.Count > 0;
        Fill(MiPresets, BuildPresetItems());

        Fill(MiRecent, BuildRecentItems());
        Fill(MiFavorites, BuildFavoriteItems());
        Fill(MiEncoding, BuildEncodingItems());
        Fill(MiEol, BuildEolItems());
        Fill(MiScheme, BuildSchemeItems());
        Fill(MiTabWidth, BuildTabWidthItems());
    }

    void EditorMenu_Opened(object sender, RoutedEventArgs e)
    {
        if (sender is not ContextMenu cm || Editor == null) return;
        bool hasSel = Editor.SelectionLength > 0;
        foreach (var mi in cm.Items.OfType<MenuItem>())
            mi.IsEnabled = mi.Tag switch
            {
                "undo" => Editor.CanUndo,
                "redo" => Editor.CanRedo,
                "cut" or "copy" or "delete" => hasSel,
                "paste" => _clipHasText,
                _ => true,
            };
    }

    // ===================================================================== dynamic menus
    static void Fill(MenuItem parent, List<object> items)
    {
        // a submenu being rebuilt while it is open would close; only rebuild closed ones
        if (parent.IsSubmenuOpen) return;
        parent.Items.Clear();
        foreach (var i in items) parent.Items.Add(i);
    }

    static MenuItem Item(string header, Action click, bool isChecked = false, string icon = null, bool enabled = true)
    {
        var mi = new MenuItem { Header = header, IsChecked = isChecked, IsEnabled = enabled };
        if (icon != null) mi.Icon = new IconImage(icon);
        mi.Click += (_, _) => click();
        return mi;
    }

    static string Escape(string s) => s.Replace("_", "__");

    void ShowPopup(FrameworkElement target, List<object> items, PlacementMode placement = PlacementMode.Bottom)
    {
        var cm = new ContextMenu { PlacementTarget = target, Placement = placement };
        foreach (var i in items) cm.Items.Add(i);
        cm.Closed += (_, _) => Editor.Focus();
        cm.IsOpen = true;
    }

    List<object> BuildRecentItems()
    {
        var list = new List<object>();
        int n = 1;
        foreach (var p in Settings.Recent.ToList())
        {
            var path = p;
            string prefix = n <= 9 ? $"_{n} " : "   ";
            list.Add(Item(prefix + Escape(path), () => OpenPath(path), enabled: File.Exists(path)));
            n++;
        }
        if (list.Count == 0) list.Add(new MenuItem { Header = "(empty)", IsEnabled = false });
        list.Add(new Separator());
        list.Add(Item("_Clear List", () => Settings.Recent.Clear(), enabled: Settings.Recent.Count > 0));
        return list;
    }

    List<object> BuildFavoriteItems()
    {
        var list = new List<object>();
        foreach (var p in Settings.Favorites.ToList())
        {
            var path = p;
            list.Add(Item(Escape(Path.GetFileName(path)) + "    " + Escape(Path.GetDirectoryName(path) ?? ""),
                () => OpenPath(path), icon: "star", enabled: File.Exists(path)));
        }
        if (list.Count == 0) list.Add(new MenuItem { Header = "(no favorites yet)", IsEnabled = false });
        list.Add(new Separator());
        list.Add(Item("_Add Current File", AddFavorite, icon: "star_add", enabled: FilePath != null && !Settings.Favorites.Contains(FilePath, StringComparer.OrdinalIgnoreCase)));
        list.Add(Item("_Remove Current File", () => { Settings.Favorites.RemoveAll(f => string.Equals(f, FilePath, StringComparison.OrdinalIgnoreCase)); ScheduleUi(); },
            enabled: FilePath != null && Settings.Favorites.Contains(FilePath, StringComparer.OrdinalIgnoreCase)));
        list.Add(Item("_Clear Favorites", () => { Settings.Favorites.Clear(); ScheduleUi(); }, enabled: Settings.Favorites.Count > 0));
        return list;
    }

    void AddFavorite()
    {
        if (FilePath == null) return;
        if (!Settings.Favorites.Contains(FilePath, StringComparer.OrdinalIgnoreCase)) Settings.Favorites.Add(FilePath);
        ShowInfo("Added to favorites");
        ScheduleUi();
    }

    List<object> BuildEncodingItems()
    {
        var list = new List<object> { new MenuItem { Header = "Save as", IsEnabled = false } };
        foreach (FileEncoding enc in Enum.GetValues<FileEncoding>())
        {
            var e = enc;
            list.Add(Item(TextFile.Name(e), () =>
            {
                if (DocEncoding == e) return;
                DocEncoding = e;
                Editor.IsModified = true;
                UpdateBytes();
                ScheduleStatus();
            }, DocEncoding == e));
        }
        list.Add(new Separator());
        bool can = FilePath != null && File.Exists(FilePath);
        list.Add(new MenuItem { Header = "Reload as", IsEnabled = false });
        foreach (var e in new[] { FileEncoding.Utf8, FileEncoding.Ansi, FileEncoding.Utf16Le, FileEncoding.Utf16Be, FileEncoding.Binary })
        {
            var ee = e;
            list.Add(Item(TextFile.Name(ee), () => Reload(ee), enabled: can));
        }
        return list;
    }

    List<object> BuildEolItems()
    {
        var list = new List<object>();
        foreach (Eol eol in Enum.GetValues<Eol>())
        {
            var e = eol;
            string label = e switch { Eol.CrLf => "Windows (CR+LF)", Eol.Lf => "Unix (LF)", _ => "Mac (CR)" };
            list.Add(Item(label, () => ConvertEol(e), DocEol == e));
        }
        return list;
    }

    List<object> BuildSchemeItems()
    {
        var list = new List<object> { Item("Text Files", () => SetScheme(null), DocScheme == null) };
        list.Add(new Separator());
        foreach (var d in HighlightingManager.Instance.HighlightingDefinitions
                     .Where(d => d.Name is not ("XmlDoc" or "MarkDownWithFontSize"))
                     .OrderBy(SchemeName))
        {
            var def = d;
            list.Add(Item(Escape(SchemeName(def)), () => SetScheme(def), DocScheme == def));
        }
        return list;
    }

    List<object> BuildTabWidthItems()
    {
        var list = new List<object>();
        foreach (int w in new[] { 2, 3, 4, 8 })
        {
            int ww = w;
            list.Add(Item(w.ToString(CultureInfo.InvariantCulture), () => { Settings.TabWidth = ww; ApplyViewSettings(); }, Settings.TabWidth == w));
        }
        return list;
    }

    // ===================================================================== schemes
    static string SchemeName(IHighlightingDefinition d) =>
        d == null ? "Text Files" : SchemeNames.TryGetValue(d.Name, out var n) ? n : d.Name;

    static FoldMode FoldModeFor(IHighlightingDefinition d) => d?.Name switch
    {
        "C#" or "C++" or "Java" or "JavaScript" or "CSS" or "PHP" or "Json" or "PowerShell" or "Boo" => FoldMode.Brace,
        "XML" or "HTML" or "ASP/XHTML" or "XmlDoc" => FoldMode.Xml,
        _ => FoldMode.None,
    };

    void SetScheme(IHighlightingDefinition d)
    {
        DocScheme = d;
        Editor.SyntaxHighlighting = d;
        Editor.SetFoldMode(FoldModeFor(d), Settings.CodeFolding);
        UpdateMarkdownMode();
        ScheduleStatus();
        ScheduleUi();
    }

    void AutoScheme()
    {
        IHighlightingDefinition d = null;
        if (Settings.AutoDetectScheme && FilePath != null)
        {
            var ext = Path.GetExtension(FilePath);
            if (!string.IsNullOrEmpty(ext)) d = HighlightingManager.Instance.GetDefinitionByExtension(ext);
        }
        SetScheme(d);
    }

    // ===================================================================== files
    bool ConfirmDiscard()
    {
        if (!Editor.IsModified) return true;
        var r = MessageDialog.AskSave(this, DisplayName);
        if (r == MessageBoxResult.Yes) return Save();
        return r == MessageBoxResult.No;
    }


    void AfterDocumentReplaced()
    {
        UpdateTitle();
        UpdateBytes();
        ScheduleStatus();
        ScheduleUi();
        Editor.ScrollToHome();
    }

    static string FileFilter =>
        "All Files (*.*)|*.*|Text Files (*.txt;*.log;*.ini;*.md)|*.txt;*.log;*.ini;*.md|" +
        "Source Code (*.cs;*.cpp;*.h;*.c;*.java;*.js;*.ts;*.py;*.ps1)|*.cs;*.cpp;*.h;*.c;*.java;*.js;*.ts;*.py;*.ps1|" +
        "Web / XML (*.html;*.htm;*.css;*.xml;*.xaml;*.json)|*.html;*.htm;*.css;*.xml;*.xaml;*.json";

    void OpenFileDialog()
    {
        var dlg = new OpenFileDialog { Filter = FileFilter, Title = "Open", Multiselect = true };
        if (FilePath != null) dlg.InitialDirectory = Path.GetDirectoryName(FilePath);
        if (dlg.ShowDialog(this) == true)
            foreach (var f in dlg.FileNames) OpenPath(f);
    }

    public bool LoadFile(string path, FileEncoding? forced = null)
    {
        try
        {
            long size = new FileInfo(path).Length;
            if (size > 200L * 1024 * 1024 &&
                !MessageDialog.Confirm(this, $"\"{Path.GetFileName(path)}\" is {size / (1024 * 1024)} MB. Opening it can take a while and use a lot of memory.\n\nOpen anyway?", "about"))
                return false;
            string text;
            FileEncoding enc;
            Eol eol;
            if (forced is FileEncoding f)
            {
                var bytes = File.ReadAllBytes(path);
                var e = TextFile.Get(f);
                int skip = 0;
                var pre = e.GetPreamble();
                if (pre.Length > 0 && bytes.AsSpan().StartsWith(pre)) skip = pre.Length;
                text = e.GetString(bytes, skip, bytes.Length - skip);
                enc = f == FileEncoding.Utf8 && skip > 0 ? FileEncoding.Utf8Bom : f;
                eol = TextFile.DetectEol(text);
            }
            else (text, enc, eol) = TextFile.Read(path);

            Editor.ExitFocusedView();
            Editor.Text = text;
            Editor.IsModified = false;
            FilePath = path;
            DocEncoding = enc;
            DocEol = eol;
            DocLastWrite = File.GetLastWriteTimeUtc(path);
            Settings.AddRecent(path);
            if (enc == FileEncoding.Binary) SetScheme(null);
            else AutoScheme();
            AfterDocumentReplaced();
            if (enc == FileEncoding.Binary) ShowInfo("Binary file: opened byte for byte, saving keeps every byte");
            return true;
        }
        catch (Exception ex)
        {
            MessageDialog.Info(this, $"Could not open \"{path}\".\n\n{ex.Message}", "exit");
            return false;
        }
    }

    void Reload(FileEncoding? enc)
    {
        if (FilePath == null || !File.Exists(FilePath)) return;
        if (Editor.IsModified && !MessageDialog.Confirm(this, $"Discard changes and reload \"{DisplayName}\"?", "recent")) return;
        int line = Editor.TextArea.Caret.Line;
        double scroll = Editor.VerticalOffset;
        if (LoadFile(FilePath, enc))
        {
            Editor.TextArea.Caret.Line = Math.Min(line, Editor.Document.LineCount);
            Editor.ScrollToVerticalOffset(scroll);
        }
    }

    public bool Save()
    {
        if (FilePath == null) return SaveAs(false);
        return WriteTo(FilePath, true);
    }

    bool SaveAs(bool copy)
    {
        var dlg = new SaveFileDialog
        {
            Filter = "All Files (*.*)|*.*|Text Files (*.txt)|*.txt",
            Title = copy ? "Save Copy" : "Save As",
            FileName = FilePath != null ? Path.GetFileName(FilePath) : "Untitled.txt",
            OverwritePrompt = true,
        };
        if (FilePath != null) dlg.InitialDirectory = Path.GetDirectoryName(FilePath);
        if (dlg.ShowDialog(this) != true) return false;
        if (copy) return WriteTo(dlg.FileName, false);
        if (!WriteTo(dlg.FileName, true)) return false;
        FilePath = dlg.FileName;
        DocLastWrite = File.GetLastWriteTimeUtc(FilePath);
        Settings.AddRecent(FilePath);
        if (Settings.AutoDetectScheme) AutoScheme();
        UpdateTitle();
        ScheduleUi();
        return true;
    }

    bool WriteTo(string path, bool markSaved)
    {
        try
        {
            TextFile.Write(path, Editor.Text, DocEncoding, DocEol);
            if (markSaved)
            {
                Editor.IsModified = false;
                DocLastWrite = File.GetLastWriteTimeUtc(path);
            }
            ShowInfo("Saved " + Path.GetFileName(path));
            return true;
        }
        catch (Exception ex)
        {
            MessageDialog.Info(this, $"Could not save \"{path}\".\n\n{ex.Message}", "exit");
            return false;
        }
    }

    void CheckExternalChange()
    {
        if (_tab == null) return;
        if (_checkingExternal || !Settings.DetectFileChanges || FilePath == null || !File.Exists(FilePath)) return;
        DateTime t;
        try { t = File.GetLastWriteTimeUtc(FilePath); } catch { return; }
        if (t == DocLastWrite) return;
        DocLastWrite = t;
        _checkingExternal = true;
        Dispatcher.BeginInvoke(DispatcherPriority.Background, () =>
        {
            try
            {
                string msg = Editor.IsModified
                    ? $"\"{DisplayName}\" was changed by another program.\nReload it and lose your changes?"
                    : $"\"{DisplayName}\" was changed by another program.\nReload it?";
                if (MessageDialog.Confirm(this, msg, "recent"))
                {
                    Editor.IsModified = false;
                    Reload(null);
                }
            }
            finally { _checkingExternal = false; }
        });
    }

    void Window_Closing(object sender, CancelEventArgs e)
    {
        if (!_quitting && IsPrimaryInstance && Settings.InstantStart)
        {
            e.Cancel = true;
            GoToBackground();
            return;
        }
        bool hotExit = IsPrimaryInstance && Settings.RestoreSession && Settings.HotExit;
        if (hotExit) SaveSession(true);
        else
        {
            foreach (var t in _tabs.Where(t => t.IsModified).ToList())
            {
                ActivateTab(t);
                if (!ConfirmDiscard())
                {
                    e.Cancel = true;
                    return;
                }
            }
            if (IsPrimaryInstance)
            {
                if (Settings.RestoreSession) SaveSession(false);
                else Session.Clear();
            }
        }
        _sessionTimer.Stop();
        _find?.Close();
        _findFiles?.Close();
        if (IsConsoleShown) Settings.ConsoleHeight = ConsoleRow.ActualHeight;
        _term?.Shutdown();
        _tray?.Dispose();
        if (Settings.SaveOnExit)
        {
            StorePlacement();
            Settings.Save();
        }
    }

    void StartNewInstance(string file)
    {
        try
        {
            var psi = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false };
            psi.ArgumentList.Add("/new");
            if (file != null) psi.ArgumentList.Add(file);
            StorePlacement();
            Settings.Save();
            Process.Start(psi);
        }
        catch (Exception ex) { MessageDialog.Info(this, ex.Message, "exit"); }
    }

    public void OpenFileAt(string path, int line, int column, int length)
    {
        if (!OpenPath(path)) return;
        var doc = Editor.Document;
        line = Math.Clamp(line, 1, doc.LineCount);
        var dl = doc.GetLineByNumber(line);
        int off = dl.Offset + Math.Clamp(column - 1, 0, dl.Length);
        Search.SelectAndReveal(Editor, off, Math.Min(length, doc.TextLength - off));
        Activate();
        Editor.Focus();
    }

    void OpenTerminal()
    {
        string dir = FilePath != null ? Path.GetDirectoryName(FilePath) : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        try { Process.Start(new ProcessStartInfo("cmd.exe") { WorkingDirectory = dir, UseShellExecute = true }); }
        catch (Exception ex) { MessageDialog.Info(this, ex.Message, "exit"); }
    }

    void Print()
    {
        var dlg = new PrintDialog();
        if (dlg.ShowDialog() != true) return;
        var doc = new FlowDocument(new Paragraph(new Run(Editor.Text)))
        {
            FontFamily = new FontFamily("Consolas"),
            FontSize = 13,
            PagePadding = new Thickness(60),
            PageWidth = dlg.PrintableAreaWidth,
            PageHeight = dlg.PrintableAreaHeight,
            ColumnWidth = dlg.PrintableAreaWidth,
            Foreground = Brushes.Black,
            Background = Brushes.White,
        };
        dlg.PrintDocument(((IDocumentPaginatorSource)doc).DocumentPaginator, DisplayName);
    }

    void ConvertEol(Eol eol)
    {
        if (DocEol == eol) return;
        if (DocEncoding == FileEncoding.Binary)
        {
            ShowInfo("Line endings are not changed in binary files");
            return;
        }
        DocEol = eol;
        var text = Editor.Text;
        var converted = TextFile.NormalizeEol(text, eol);
        if (converted != text)
        {
            var loc = Editor.TextArea.Caret.Location;
            Editor.Document.Replace(0, text.Length, converted);
            Editor.TextArea.Caret.Location = loc;
        }
        Editor.IsModified = true;
        UpdateBytes();
        ScheduleStatus();
    }

    // ===================================================================== dialogs
    void ShowFind(bool replace)
    {
        if (_find == null)
        {
            _find = new FindReplaceWindow(this);
            _find.Closed += (_, _) => { _find = null; Activate(); };
        }
        string initial = Editor.SelectionLength is > 0 and < 500 && !Editor.SelectedText.Contains('\n') ? Editor.SelectedText : null;
        _find.ShowMode(replace, initial);
    }

    void FindAgain(bool backwards)
    {
        var o = _find?.Options ?? new SearchOptions
        {
            Find = Settings.LastFind, MatchCase = Settings.FindMatchCase, WholeWord = Settings.FindWholeWord,
            Regex = Settings.FindRegex, Wrap = Settings.FindWrap,
        };
        if (string.IsNullOrEmpty(o.Find) && Editor.SelectionLength is > 0 and < 500 && !Editor.SelectedText.Contains('\n'))
        {
            o.Find = Editor.SelectedText;
            Settings.LastFind = o.Find;
        }
        if (string.IsNullOrEmpty(o.Find)) { ShowFind(false); return; }
        var msg = Search.FindNext(Editor, o, backwards);
        if (!string.IsNullOrEmpty(msg)) ShowInfo(msg);
    }

    void ShowFindInFiles()
    {
        if (_findFiles == null)
        {
            _findFiles = new FindInFilesWindow(this);
            _findFiles.Closed += (_, _) => _findFiles = null;
            _findFiles.Show();
        }
        else _findFiles.Activate();
    }

    void GoTo()
    {
        var caret = Editor.TextArea.Caret;
        var dlg = new InputDialog(this, "Go To", $"Line number (1 - {N(Editor.Document.LineCount)}), optionally line:column",
            caret.Line.ToString(CultureInfo.InvariantCulture));
        if (dlg.ShowDialog() != true) return;
        var parts = dlg.Value.Split(new[] { ':', ',', ' ' }, StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length == 0 || !int.TryParse(parts[0], out int line)) return;
        int col = parts.Length > 1 && int.TryParse(parts[1], out var c) ? c : 1;
        var doc = Editor.Document;
        line = Math.Clamp(line, 1, doc.LineCount);
        var dl = doc.GetLineByNumber(line);
        col = Math.Clamp(col, 1, dl.Length + 1);
        Editor.Select(dl.Offset + col - 1, 0);
        Editor.ScrollTo(line, col);
        Editor.Focus();
    }

    void AskLongLineColumn()
    {
        var dlg = new InputDialog(this, "Long Line Marker", "Column:", Settings.LongLineColumn.ToString(CultureInfo.InvariantCulture));
        if (dlg.ShowDialog() == true && int.TryParse(dlg.Value, out int col) && col is > 0 and <= 1000)
        {
            Settings.LongLineColumn = col;
            Settings.LongLineMarker = true;
            ApplyViewSettings();
        }
    }

    void ChooseFont(bool secondary)
    {
        var s = Settings;
        var dlg = new Dialogs.FontDialog(this, secondary ? s.SecondFontFamily : s.FontFamily, s.FontSize, s.FontBold, s.FontItalic);
        if (dlg.ShowDialog() != true) return;
        if (secondary)
        {
            s.SecondFontFamily = dlg.FamilyName;
            s.UseSecondFont = true;
        }
        else
        {
            s.FontFamily = dlg.FamilyName;
            s.UseSecondFont = false;
        }
        s.FontSize = dlg.SizeDip;
        s.FontBold = dlg.IsBold;
        s.FontItalic = dlg.IsItalic;
        ApplyViewSettings();
    }

    void ToggleFocusedView()
    {
        if (Editor.IsFocusedView)
        {
            Editor.ExitFocusedView();
        }
        else
        {
            string term = Editor.SelectionLength is > 0 and < 500 && !Editor.SelectedText.Contains('\n')
                ? Editor.SelectedText : Settings.LastFind;
            if (string.IsNullOrEmpty(term))
            {
                ShowInfo("Focused view: select some text first");
            }
            else
            {
                int n = Editor.EnterFocusedView(term, Settings.FindMatchCase ? StringComparison.Ordinal : StringComparison.OrdinalIgnoreCase);
                ShowInfo(n == 0 ? $"\"{term}\" not found" : $"Focused view: {N(n)} line(s) with \"{term}\"");
            }
        }
        ScheduleUi();
        ScheduleStatus();
    }

    // ===================================================================== console / run
    ConsolePanel _term;

    /// <summary>The console is created the first time it is needed (keeps start-up light).</summary>
    ConsolePanel Term
    {
        get
        {
            if (_term != null) return _term;
            _term = new ConsolePanel { FileFolder = () => FilePath != null ? Path.GetDirectoryName(FilePath) : null };
            _term.RunRequested += (_, _) => Run("run");
            _term.CloseRequested += (_, _) => ShowConsole(false);
            TermHost.Content = _term;
            return _term;
        }
    }

    bool IsConsoleShown => TermHost.Visibility == Visibility.Visible;

    void ShowConsole(bool show, bool focus = false)
    {
        if (show == IsConsoleShown)
        {
            if (show && focus) Term.FocusInput();
            return;
        }
        if (show)
        {
            _ = Term;
            TermHost.Visibility = Visibility.Visible;
            ConsoleSplitter.Visibility = Visibility.Visible;
            ConsoleSplitRow.Height = new GridLength(5);
            ConsoleRow.Height = new GridLength(Math.Clamp(Settings.ConsoleHeight, 80, Math.Max(80, WorkArea.ActualHeight - 120)));
            Term.EnsureShell(FilePath != null ? Path.GetDirectoryName(FilePath) : null);
            if (focus) Term.FocusInput();
        }
        else
        {
            Settings.ConsoleHeight = ConsoleRow.ActualHeight;
            TermHost.Visibility = Visibility.Collapsed;
            ConsoleSplitter.Visibility = Visibility.Collapsed;
            ConsoleSplitRow.Height = new GridLength(0);
            ConsoleRow.Height = new GridLength(0);
            Editor.Focus();
        }
        ScheduleUi();
    }

    /// <summary>F5 / Run button: save, then run the file in the internal console (or open it with its program).</summary>
    void RunCurrentFile()
    {
        if (FilePath == null || Editor.IsModified)
        {
            if (!Save()) return;
        }
        if (FilePath == null) return;
        var (kind, command) = Runner.For(FilePath);
        switch (kind)
        {
            case RunKind.Preview:
                Run("md_preview");
                break;
            case RunKind.Shell:
                try
                {
                    Process.Start(new ProcessStartInfo(FilePath) { UseShellExecute = true });
                    ShowInfo("Opened " + DisplayName);
                }
                catch (Exception ex) { MessageDialog.Info(this, ex.Message, "exit"); }
                break;
            default:
                ShowConsole(true);
                Term.RunCommand(command, Path.GetDirectoryName(FilePath));
                break;
        }
    }

    // ===================================================================== markdown
    bool IsMarkdown => DocScheme?.Name == "MarkDown" ||
                       (FilePath != null && Path.GetExtension(FilePath).ToLowerInvariant() is ".md" or ".markdown");

    bool IsPreviewShown => Preview.Visibility == Visibility.Visible;

    void UpdateMarkdownMode()
    {
        bool md = IsMarkdown;
        MdBar.Visibility = md ? Visibility.Visible : Visibility.Collapsed;
        ShowPreview(md && Settings.MdPreview);
    }

    void ShowPreview(bool show)
    {
        if (show == IsPreviewShown)
        {
            if (show) RefreshPreview();
            return;
        }
        if (show)
        {
            Preview.Visibility = Visibility.Visible;
            PreviewSplitter.Visibility = Visibility.Visible;
            PreviewSplitCol.Width = new GridLength(5);
            EditorCol.Width = new GridLength(1, GridUnitType.Star);
            PreviewCol.Width = new GridLength(1, GridUnitType.Star);
            _lastPreviewLine = -1;
            RefreshPreview();
        }
        else
        {
            Preview.Visibility = Visibility.Collapsed;
            PreviewSplitter.Visibility = Visibility.Collapsed;
            PreviewSplitCol.Width = new GridLength(0);
            PreviewCol.Width = new GridLength(0);
            EditorCol.Width = new GridLength(1, GridUnitType.Star);
        }
        ScheduleUi();
    }

    void RefreshPreview()
    {
        if (!IsPreviewShown) return;
        Preview.Update(Editor.Text, FilePath != null ? Path.GetDirectoryName(FilePath) : null);
        _lastPreviewLine = -1;
        SyncPreviewScroll();
    }

    void SyncPreviewScroll()
    {
        if (!IsPreviewShown || !Settings.MdSyncScroll) return;
        var tv = Editor.TextArea.TextView;
        var line = tv.GetDocumentLineByVisualTop(tv.ScrollOffset.Y);
        int n = (line?.LineNumber ?? 1) - 1;
        if (n == _lastPreviewLine) return;
        _lastPreviewLine = n;
        Preview.ScrollToLine(n);
    }

    void InsertImage()
    {
        var dlg = new OpenFileDialog
        {
            Title = "Insert Image",
            Filter = "Images (*.png;*.jpg;*.jpeg;*.gif;*.svg;*.webp;*.bmp)|*.png;*.jpg;*.jpeg;*.gif;*.svg;*.webp;*.bmp|All Files (*.*)|*.*",
        };
        if (FilePath != null) dlg.InitialDirectory = Path.GetDirectoryName(FilePath);
        if (dlg.ShowDialog(this) != true) return;
        string path = dlg.FileName;
        if (FilePath != null)
        {
            string rel = Path.GetRelativePath(Path.GetDirectoryName(FilePath)!, path);
            if (!rel.StartsWith("..") && !Path.IsPathRooted(rel)) path = rel;
        }
        MarkdownTools.Image(Editor, path.Replace('\\', '/'));
        Editor.Focus();
    }

    // ===================================================================== line / text operations
    (DocumentLine first, DocumentLine last) SelectedLines()
    {
        var doc = Editor.Document;
        if (Editor.SelectionLength == 0)
        {
            var l = doc.GetLineByOffset(Editor.CaretOffset);
            return (l, l);
        }
        int s = Editor.SelectionStart, e = s + Editor.SelectionLength;
        var f = doc.GetLineByOffset(s);
        var la = doc.GetLineByOffset(e);
        if (la.Offset == e && la.LineNumber > f.LineNumber) la = la.PreviousLine;
        return (f, la);
    }

    string Nl => TextFile.Text(DocEol);

    void DuplicateLine()
    {
        var doc = Editor.Document;
        if (Editor.SelectionLength > 0)
        {
            string t = Editor.SelectedText;
            int end = Editor.SelectionStart + Editor.SelectionLength;
            doc.Insert(end, t);
            Editor.Select(end, t.Length);
            return;
        }
        var l = doc.GetLineByOffset(Editor.CaretOffset);
        int col = Editor.CaretOffset - l.Offset;
        string text = doc.GetText(l.Offset, l.Length);
        doc.Insert(l.EndOffset, Nl + text);
        var next = l.NextLine;
        if (next != null) Editor.CaretOffset = next.Offset + Math.Min(col, next.Length);
    }

    void DeleteLines()
    {
        var doc = Editor.Document;
        var (f, l) = SelectedLines();
        int start = f.Offset, end = l.Offset + l.TotalLength;
        if (l.DelimiterLength == 0 && f.PreviousLine != null) start = f.PreviousLine.EndOffset;
        doc.Remove(start, end - start);
    }

    void MoveLines(bool up)
    {
        var doc = Editor.Document;
        var (f, l) = SelectedLines();
        int selStart = Editor.SelectionStart - f.Offset, selLen = Editor.SelectionLength, caret = Editor.CaretOffset - f.Offset;
        string block = doc.GetText(f.Offset, l.EndOffset - f.Offset);
        int newBase;
        if (up)
        {
            var prev = f.PreviousLine;
            if (prev == null) return;
            string prevText = doc.GetText(prev.Offset, prev.Length);
            string nl = doc.GetText(prev.EndOffset, prev.DelimiterLength);
            doc.Replace(prev.Offset, l.EndOffset - prev.Offset, block + nl + prevText);
            newBase = prev.Offset;
        }
        else
        {
            var next = l.NextLine;
            if (next == null) return;
            string nextText = doc.GetText(next.Offset, next.Length);
            string nl = doc.GetText(l.EndOffset, l.DelimiterLength);
            doc.Replace(f.Offset, next.EndOffset - f.Offset, nextText + nl + block);
            newBase = f.Offset + nextText.Length + nl.Length;
        }
        if (selLen > 0) Editor.Select(newBase + selStart, selLen);
        else Editor.CaretOffset = newBase + caret;
    }

    void TransformLines(Func<List<string>, IEnumerable<string>> f, string joiner = null)
    {
        var doc = Editor.Document;
        bool hadSel = Editor.SelectionLength > 0;
        DocumentLine first, last;
        if (hadSel) (first, last) = SelectedLines();
        else
        {
            first = doc.GetLineByNumber(1);
            last = doc.GetLineByNumber(doc.LineCount);
        }
        int start = first.Offset, end = last.EndOffset;
        var lines = new List<string>();
        for (var l = first; l != null; l = l.NextLine)
        {
            lines.Add(doc.GetText(l.Offset, l.Length));
            if (l == last) break;
        }
        string result = string.Join(joiner ?? Nl, f(lines));
        if (result == doc.GetText(start, end - start)) return;
        doc.Replace(start, end - start, result);
        if (hadSel) Editor.Select(start, result.Length);
    }

    void TransformSelection(Func<string, string> f)
    {
        if (Editor.SelectionLength == 0)
        {
            // apply to the word at the caret
            var doc = Editor.Document;
            int off = Editor.CaretOffset;
            int s = TextUtilities.GetNextCaretPosition(doc, Math.Min(off + 1, doc.TextLength), LogicalDirection.Backward, CaretPositioningMode.WordStart);
            int e = TextUtilities.GetNextCaretPosition(doc, Math.Max(off - 1, 0), LogicalDirection.Forward, CaretPositioningMode.WordBorder);
            if (s < 0 || e <= s) return;
            Editor.Select(s, e - s);
        }
        int start = Editor.SelectionStart;
        string t = Editor.SelectedText;
        string r = f(t);
        if (r == t) return;
        Editor.Document.Replace(start, t.Length, r);
        Editor.Select(start, r.Length);
    }

    string TabsToSpaces(string line)
    {
        if (!line.Contains('\t')) return line;
        int w = Math.Max(1, Settings.TabWidth);
        var sb = new StringBuilder();
        foreach (char c in line)
        {
            if (c == '\t') sb.Append(' ', w - sb.Length % w);
            else sb.Append(c);
        }
        return sb.ToString();
    }

    string LeadingSpacesToTabs(string line)
    {
        int w = Math.Max(1, Settings.TabWidth);
        int i = 0, col = 0;
        while (i < line.Length && (line[i] == ' ' || line[i] == '\t'))
        {
            col = line[i] == '\t' ? (col / w + 1) * w : col + 1;
            i++;
        }
        if (i == 0) return line;
        return new string('\t', col / w) + new string(' ', col % w) + line[i..];
    }
}
