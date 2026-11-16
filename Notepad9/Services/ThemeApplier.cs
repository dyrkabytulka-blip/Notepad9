using System.IO;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using Notepad9.Editor;

namespace Notepad9.Services;

/// <summary>Pushes a theme into the application resources (all DynamicResource users update live).</summary>
public static class ThemeApplier
{
    static void Set(string key, Color c)
    {
        var b = new SolidColorBrush(c);
        b.Freeze();
        Application.Current.Resources[key] = b;
    }

    public static Dictionary<string, Color> ApplyResources(ThemeSettings t)
    {
        var c = ThemeCatalog.Resolve(t);
        Color G(string id) => c.TryGetValue(id, out var v) ? v : Colors.Magenta;
        var L = ThemeCatalog.Lighten;
        var D = ThemeCatalog.Darken;
        bool light = ThemeCatalog.Luminance(G("ui.window")) > 0.6;

        Set("Bg", G("ui.window"));
        Set("TitleBg", G("ui.title"));
        Set("TitleFg", G("ui.titleText"));
        Set("WindowBorder", G("ui.border"));
        Set("MenuBarBg", G("ui.menuBar"));
        Set("ToolbarBg", G("ui.toolbar"));
        Set("Text", G("ui.text"));
        Set("TextDim", G("ui.textDim"));
        Set("TextDisabled", G("ui.textDisabled"));
        Set("Line", G("ui.line"));
        Set("SepDark", light ? D(G("ui.toolbar"), 0.25) : D(G("ui.toolbar"), 0.5));
        Set("SepLight", light ? L(G("ui.toolbar"), 0.6) : L(G("ui.toolbar"), 0.3));
        Set("SplitterBg", ThemeCatalog.Blend(G("ui.window"), G("ui.line"), 0.5));
        Set("ToolHoverBg", G("ui.toolHover"));
        Set("ToolHoverBorder", G("ui.toolHoverBorder"));
        Set("ToolPressedBg", G("ui.toolPressed"));
        Set("ToolPressedBorder", D(G("ui.toolPressed"), 0.2));
        Set("PopupBg", G("ui.popup"));
        Set("PopupBorder", G("ui.popupBorder"));
        Set("MenuHover", G("ui.menuHover"));
        Set("MenuTopHover", ThemeCatalog.Blend(G("ui.menuBar"), G("ui.menuHover"), 0.6));
        Set("TabBarBg", G("ui.tabBar"));
        Set("TabBg", G("ui.tab"));
        Set("TabHoverBg", G("ui.tabHover"));
        Set("TabActiveBg", G("ui.tabActive"));
        Set("TabText", G("ui.tabText"));
        Set("TabActiveText", G("ui.tabActiveText"));
        Set("TabAccent", G("ui.tabAccent"));
        Set("PanelBg", G("ui.panel"));
        Set("StatusBg", G("ui.status"));
        Set("StatusFg", G("ui.statusText"));
        Set("StatusSep", G("ui.statusSep"));
        Set("StatusHover", ThemeCatalog.Blend(G("ui.status"), G("ui.statusText"), 0.12));
        Set("Accent", G("ui.accent"));
        Set("AccentHover", L(G("ui.accent"), 0.15));
        Set("FieldBg", G("ui.field"));
        Set("FieldBorder", G("ui.fieldBorder"));
        Set("FieldFocus", L(G("ui.accent"), 0.2));
        Set("BtnBg", G("ui.button"));
        Set("BtnHover", light ? D(G("ui.button"), 0.06) : L(G("ui.button"), 0.08));
        Set("BtnPressed", light ? D(G("ui.button"), 0.12) : D(G("ui.button"), 0.2));
        Set("BtnBorder", ThemeCatalog.Blend(G("ui.button"), G("ui.text"), 0.25));
        Set("ScrollTrack", G("ui.scrollTrack"));
        Set("ScrollThumbBg", G("ui.scrollThumb"));
        Set("ScrollThumbHover", light ? D(G("ui.scrollThumb"), 0.15) : L(G("ui.scrollThumb"), 0.15));
        Set("ScrollThumbDrag", light ? D(G("ui.scrollThumb"), 0.3) : L(G("ui.scrollThumb"), 0.3));
        Set("ConsoleBg", G("ui.console"));
        Set("ConsoleFg", G("ui.consoleText"));
        Set("Selection", G("ed.selection"));
        Set("Gutter", G("ed.gutter"));
        Set("EditorBg", G("ed.bg"));

        // whole-window background image: let it shine through the panels
        if (t.WindowImageActive)
        {
            double k = Math.Clamp(t.PanelOpacity, 0, 1);
            foreach (var key in new[] { "MenuBarBg", "ToolbarBg", "TitleBg", "TabBarBg", "TabBg", "TabActiveBg", "TabHoverBg",
                                        "StatusBg", "PanelBg", "EditorBg", "ConsoleBg" })
            {
                if (Application.Current.Resources[key] is SolidColorBrush sb)
                    Set(key, Color.FromArgb((byte)(sb.Color.A * k), sb.Color.R, sb.Color.G, sb.Color.B));
            }
        }

        var res = Application.Current.Resources;
        res["UiFont"] = new FontFamily(string.IsNullOrWhiteSpace(t.UiFont) ? "Segoe UI" : t.UiFont);
        res["UiFontSize"] = Math.Clamp(t.UiFontSize, 8, 24);
        res["ScrollSize"] = Math.Clamp(t.ScrollbarSize, 4, 30);
        res["TabMaxWidth"] = Math.Clamp(t.TabMaxWidth, 80, 600);
        res["TabCloseVisibility"] = t.TabCloseButtons ? Visibility.Visible : Visibility.Collapsed;

        var syntax = new Dictionary<string, Color>();
        foreach (var kv in c)
            if (kv.Key.StartsWith("syn.")) syntax[kv.Key[4..]] = kv.Value;
        syntax["link"] = G("ed.link");
        DarkHighlightingColorizer.SetPalette(syntax, ThemeCatalog.Luminance(G("ed.bg")) > 0.6);
        return c;
    }

    public static EditorLook BuildLook(ThemeSettings t, Dictionary<string, Color> c)
    {
        Color G(string id) => c.TryGetValue(id, out var v) ? v : Colors.Magenta;
        var gutter = G("ed.gutter");
        if (t.WindowImageActive) gutter = Color.FromArgb((byte)(gutter.A * Math.Clamp(t.PanelOpacity, 0, 1)), gutter.R, gutter.G, gutter.B);
        return new EditorLook
        {
            Fore = G("ed.fg"),
            CurrentLine = G("ed.curLine"),
            Selection = G("ed.selection"),
            Caret = G("ed.caret"),
            Gutter = gutter,
            LineNumber = G("ed.lineNum"),
            LineNumberCurrent = G("ed.lineNumCur"),
            Whitespace = G("ed.whitespace"),
            Ruler = G("ed.ruler"),
            Occurrence = G("ed.occurrence"),
            Link = G("ed.link"),
            Fold = G("ed.fold"),
            CaretShape = Enum.TryParse<CaretShape>(t.CaretStyle, out var shape) ? shape : CaretShape.Line,
            CaretWidth = t.CaretWidth,
            CaretBlink = t.CaretBlink,
            CaretBlinkMs = t.CaretBlinkMs,
            LineNumberScale = Math.Clamp(t.LineNumberScale, 0.4, 1.5),
            Pointer = PointerCursor(t),
        };
    }

    static readonly Dictionary<string, Cursor> CustomCursors = new(StringComparer.OrdinalIgnoreCase);

    public static Cursor PointerCursor(ThemeSettings t) => t.Pointer switch
    {
        "Arrow" => Cursors.Arrow,
        "Cross" => Cursors.Cross,
        "Hand" => Cursors.Hand,
        "Pen" => Cursors.Pen,
        "Custom" => LoadCursor(t.PointerFile) ?? Cursors.IBeam,
        _ => Cursors.IBeam,
    };

    static Cursor LoadCursor(string path)
    {
        if (string.IsNullOrWhiteSpace(path) || !File.Exists(path)) return null;
        if (CustomCursors.TryGetValue(path, out var c)) return c;
        try
        {
            c = new Cursor(path);
            CustomCursors[path] = c;
            return c;
        }
        catch { return null; }
    }
}
