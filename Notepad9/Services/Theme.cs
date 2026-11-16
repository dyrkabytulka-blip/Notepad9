using System.Globalization;
using System.IO;
using System.Text.Json;
using System.Windows.Media;

namespace Notepad9.Services;

/// <summary>Everything the "Customize Interface" window can change. Stored inside settings.json.</summary>
public class ThemeSettings
{
    public string Preset { get; set; } = ThemeCatalog.DefaultPreset;
    /// <summary>Colors changed by hand, on top of the preset (id → #AARRGGBB).</summary>
    public Dictionary<string, string> Colors { get; set; } = new();

    // background
    public string BgTarget { get; set; } = "Off";          // Off | Editor | Window
    public string BgImage { get; set; } = "";
    public double BgOpacity { get; set; } = 0.25;
    public double BgBlur { get; set; }
    public string BgStretch { get; set; } = "UniformToFill"; // Fill | Uniform | UniformToFill | None | Tile
    public string BgAlign { get; set; } = "Center";          // Center | TopLeft | TopRight | BottomLeft | BottomRight
    /// <summary>With a whole-window image: how opaque menus, toolbar, tabs, editor and status bar stay.</summary>
    public double PanelOpacity { get; set; } = 0.7;

    [System.Text.Json.Serialization.JsonIgnore]
    public bool WindowImageActive => BgTarget == "Window" && !string.IsNullOrWhiteSpace(BgImage) && File.Exists(BgImage);

    // text caret
    public string CaretStyle { get; set; } = "Line";        // Line | Block | Underline | Box
    public double CaretWidth { get; set; } = 1.5;
    public bool CaretBlink { get; set; } = true;
    public int CaretBlinkMs { get; set; } = 530;

    // mouse pointer
    public string Pointer { get; set; } = "IBeam";          // IBeam | Arrow | Cross | Hand | Pen | Custom
    public string PointerFile { get; set; } = "";
    public bool PointerWholeWindow { get; set; }

    // fonts and sizes
    public string UiFont { get; set; } = "Segoe UI";
    public double UiFontSize { get; set; } = 12;
    public double LineNumberScale { get; set; } = 0.75;
    public double ToolbarIconSize { get; set; } = 16;
    public double ScrollbarSize { get; set; } = 13;
    public string Corners { get; set; } = "Round";          // Round | Small | Square

    // tabs and status bar
    public string TabBarMode { get; set; } = "Always";      // Always | Multiple | Never
    public string TabPosition { get; set; } = "Top";        // Top | Bottom
    public bool TabCloseButtons { get; set; } = true;
    public double TabMaxWidth { get; set; } = 220;
    public List<string> HiddenStatus { get; set; } = new();

    public ThemeSettings Clone() =>
        JsonSerializer.Deserialize(JsonSerializer.Serialize(this, N9Json.Default.ThemeSettings), N9Json.Default.ThemeSettings) ?? new ThemeSettings();

    public void Export(string path) =>
        File.WriteAllText(path, JsonSerializer.Serialize(this, N9Json.Default.ThemeSettings));

    public static ThemeSettings Import(string path) =>
        JsonSerializer.Deserialize(File.ReadAllText(path), N9Json.Default.ThemeSettings) ?? new ThemeSettings();
}

public record ColorDef(string Id, string Group, string Label);

/// <summary>The list of customizable colors and the built-in presets.</summary>
public static class ThemeCatalog
{
    public const string DefaultPreset = "Notepad9 Dark";
    public const string GroupUi = "Interface";
    public const string GroupEditor = "Editor";
    public const string GroupSyntax = "Syntax";

    public static readonly ColorDef[] Defs =
    {
        new("ui.window", GroupUi, "Window background"),
        new("ui.title", GroupUi, "Title bar"),
        new("ui.titleText", GroupUi, "Title text"),
        new("ui.border", GroupUi, "Window border"),
        new("ui.menuBar", GroupUi, "Menu bar"),
        new("ui.text", GroupUi, "Menu and UI text"),
        new("ui.textDim", GroupUi, "Secondary text"),
        new("ui.textDisabled", GroupUi, "Disabled text"),
        new("ui.line", GroupUi, "Separator lines"),
        new("ui.toolbar", GroupUi, "Toolbar background"),
        new("ui.toolHover", GroupUi, "Toolbar button hover"),
        new("ui.toolHoverBorder", GroupUi, "Toolbar button hover border"),
        new("ui.toolPressed", GroupUi, "Toolbar button pressed"),
        new("ui.popup", GroupUi, "Menu popup background"),
        new("ui.popupBorder", GroupUi, "Menu popup border"),
        new("ui.menuHover", GroupUi, "Menu item highlight"),
        new("ui.tabBar", GroupUi, "Tab bar"),
        new("ui.tab", GroupUi, "Inactive tab"),
        new("ui.tabHover", GroupUi, "Tab under mouse"),
        new("ui.tabActive", GroupUi, "Active tab"),
        new("ui.tabText", GroupUi, "Tab text"),
        new("ui.tabActiveText", GroupUi, "Active tab text"),
        new("ui.tabAccent", GroupUi, "Active tab accent line"),
        new("ui.panel", GroupUi, "Panels (console and Markdown bars)"),
        new("ui.status", GroupUi, "Status bar"),
        new("ui.statusText", GroupUi, "Status bar text"),
        new("ui.statusSep", GroupUi, "Status bar separators"),
        new("ui.accent", GroupUi, "Accent (buttons, checkboxes)"),
        new("ui.field", GroupUi, "Input fields"),
        new("ui.fieldBorder", GroupUi, "Input field border"),
        new("ui.button", GroupUi, "Dialog buttons"),
        new("ui.scrollTrack", GroupUi, "Scrollbar track"),
        new("ui.scrollThumb", GroupUi, "Scrollbar thumb"),
        new("ui.console", GroupUi, "Console background"),
        new("ui.consoleText", GroupUi, "Console text"),

        new("ed.bg", GroupEditor, "Text background"),
        new("ed.fg", GroupEditor, "Text"),
        new("ed.curLine", GroupEditor, "Current line"),
        new("ed.selection", GroupEditor, "Selection"),
        new("ed.caret", GroupEditor, "Text cursor (caret)"),
        new("ed.gutter", GroupEditor, "Line number area"),
        new("ed.lineNum", GroupEditor, "Line numbers"),
        new("ed.lineNumCur", GroupEditor, "Current line number"),
        new("ed.whitespace", GroupEditor, "Whitespace symbols"),
        new("ed.ruler", GroupEditor, "Long line marker"),
        new("ed.occurrence", GroupEditor, "Marked occurrences"),
        new("ed.link", GroupEditor, "Links"),
        new("ed.fold", GroupEditor, "Folding markers"),

        new("syn.comment", GroupSyntax, "Comments"),
        new("syn.string", GroupSyntax, "Strings"),
        new("syn.keyword", GroupSyntax, "Keywords"),
        new("syn.number", GroupSyntax, "Numbers"),
        new("syn.type", GroupSyntax, "Types and classes"),
        new("syn.method", GroupSyntax, "Functions and methods"),
        new("syn.preprocessor", GroupSyntax, "Preprocessor"),
        new("syn.tag", GroupSyntax, "Markup tags"),
        new("syn.attribute", GroupSyntax, "Attributes and properties"),
        new("syn.entity", GroupSyntax, "Entities and selectors"),
        new("syn.operator", GroupSyntax, "Operators and punctuation"),
    };

    static readonly Dictionary<string, string> Dark = new()
    {
        ["ui.window"] = "#141414", ["ui.title"] = "#141414", ["ui.titleText"] = "#B4B4B4", ["ui.border"] = "#3A3A3A",
        ["ui.menuBar"] = "#141414", ["ui.text"] = "#DEDEDE", ["ui.textDim"] = "#9A9A9A", ["ui.textDisabled"] = "#6A6A6A",
        ["ui.line"] = "#414141", ["ui.toolbar"] = "#141414", ["ui.toolHover"] = "#CCE8FF", ["ui.toolHoverBorder"] = "#99D1FF",
        ["ui.toolPressed"] = "#99D1FF", ["ui.popup"] = "#2B2B2B", ["ui.popupBorder"] = "#4F4F4F", ["ui.menuHover"] = "#414141",
        ["ui.tabBar"] = "#1A1A1A", ["ui.tab"] = "#1A1A1A", ["ui.tabHover"] = "#242424", ["ui.tabActive"] = "#141414",
        ["ui.tabText"] = "#8E8E8E", ["ui.tabActiveText"] = "#F0F0F0", ["ui.tabAccent"] = "#C8A82E",
        ["ui.panel"] = "#1B1B1B", ["ui.status"] = "#414141", ["ui.statusText"] = "#C7C7C7", ["ui.statusSep"] = "#696969",
        ["ui.accent"] = "#2F6FB5", ["ui.field"] = "#1E1E1E", ["ui.fieldBorder"] = "#4F4F4F", ["ui.button"] = "#2F2F2F",
        ["ui.scrollTrack"] = "#1A1A1A", ["ui.scrollThumb"] = "#4A4A4A", ["ui.console"] = "#101010", ["ui.consoleText"] = "#CCCCCC",
        ["ed.bg"] = "#141414", ["ed.fg"] = "#DEDEDE", ["ed.curLine"] = "#404013", ["ed.selection"] = "#264F78",
        ["ed.caret"] = "#B5B5B5", ["ed.gutter"] = "#454545", ["ed.lineNum"] = "#A8A8A8", ["ed.lineNumCur"] = "#DEDEDE",
        ["ed.whitespace"] = "#5E5E5E", ["ed.ruler"] = "#3C3C3C", ["ed.occurrence"] = "#553E8EC8", ["ed.link"] = "#4FA3F7",
        ["ed.fold"] = "#A0A0A0",
        ["syn.comment"] = "#6A9955", ["syn.string"] = "#CE9178", ["syn.keyword"] = "#569CD6", ["syn.number"] = "#B5CEA8",
        ["syn.type"] = "#4EC9B0", ["syn.method"] = "#DCDCAA", ["syn.preprocessor"] = "#C586C0", ["syn.tag"] = "#569CD6",
        ["syn.attribute"] = "#9CDCFE", ["syn.entity"] = "#D7BA7D", ["syn.operator"] = "#D4D4D4",
    };

    /// <summary>Builds a full palette from a handful of base colors.</summary>
    static Dictionary<string, string> Make(string bg, string fg, string panel, string line, string accent, string curLine,
        string selection, string gutter, string status, string statusText, string tabAccent, bool light,
        Dictionary<string, string> syntax)
    {
        string Mix(string a, string b, double t) => Hex(Blend(Parse(a), Parse(b), t));
        var d = new Dictionary<string, string>
        {
            ["ui.window"] = bg, ["ui.title"] = panel, ["ui.titleText"] = Mix(fg, panel, 0.25), ["ui.border"] = line,
            ["ui.menuBar"] = panel, ["ui.text"] = fg, ["ui.textDim"] = Mix(fg, bg, 0.35), ["ui.textDisabled"] = Mix(fg, bg, 0.6),
            ["ui.line"] = line, ["ui.toolbar"] = panel, ["ui.toolHover"] = Mix(accent, light ? "#FFFFFF" : bg, 0.55),
            ["ui.toolHoverBorder"] = Mix(accent, light ? "#FFFFFF" : bg, 0.25), ["ui.toolPressed"] = Mix(accent, bg, 0.35),
            ["ui.popup"] = light ? "#FFFFFF" : Mix(panel, "#FFFFFF", 0.06), ["ui.popupBorder"] = line,
            ["ui.menuHover"] = light ? Mix(accent, "#FFFFFF", 0.8) : Mix(panel, "#FFFFFF", 0.14),
            ["ui.tabBar"] = Mix(panel, light ? "#000000" : "#000000", light ? 0.04 : 0.15), ["ui.tab"] = Mix(panel, "#000000", light ? 0.04 : 0.15),
            ["ui.tabHover"] = Mix(panel, light ? "#000000" : "#FFFFFF", 0.06), ["ui.tabActive"] = bg,
            ["ui.tabText"] = Mix(fg, bg, 0.4), ["ui.tabActiveText"] = fg, ["ui.tabAccent"] = tabAccent,
            ["ui.panel"] = panel, ["ui.status"] = status, ["ui.statusText"] = statusText, ["ui.statusSep"] = Mix(statusText, status, 0.55),
            ["ui.accent"] = accent, ["ui.field"] = light ? "#FFFFFF" : Mix(bg, "#FFFFFF", 0.04), ["ui.fieldBorder"] = line,
            ["ui.button"] = light ? "#F2F2F2" : Mix(panel, "#FFFFFF", 0.1),
            ["ui.scrollTrack"] = Mix(bg, light ? "#000000" : "#FFFFFF", 0.03), ["ui.scrollThumb"] = Mix(bg, fg, 0.25),
            ["ui.console"] = Mix(bg, "#000000", light ? 0.02 : 0.25), ["ui.consoleText"] = Mix(fg, bg, 0.1),
            ["ed.bg"] = bg, ["ed.fg"] = fg, ["ed.curLine"] = curLine, ["ed.selection"] = selection,
            ["ed.caret"] = fg, ["ed.gutter"] = gutter, ["ed.lineNum"] = Mix(fg, gutter, 0.4), ["ed.lineNumCur"] = fg,
            ["ed.whitespace"] = Mix(fg, bg, 0.7), ["ed.ruler"] = Mix(fg, bg, 0.82), ["ed.occurrence"] = "#55" + accent.TrimStart('#')[^6..],
            ["ed.link"] = accent, ["ed.fold"] = Mix(fg, gutter, 0.35),
        };
        foreach (var kv in syntax) d[kv.Key] = kv.Value;
        return d;
    }

    static Dictionary<string, string> Syn(string comment, string str, string keyword, string number, string type,
        string method, string pre, string tag, string attr, string entity, string op) => new()
    {
        ["syn.comment"] = comment, ["syn.string"] = str, ["syn.keyword"] = keyword, ["syn.number"] = number,
        ["syn.type"] = type, ["syn.method"] = method, ["syn.preprocessor"] = pre, ["syn.tag"] = tag,
        ["syn.attribute"] = attr, ["syn.entity"] = entity, ["syn.operator"] = op,
    };

    public static readonly Dictionary<string, Dictionary<string, string>> Presets = new()
    {
        [DefaultPreset] = Dark,
        ["Classic Light"] = Make("#FFFFFF", "#1E1E1E", "#F0F0F0", "#C8C8C8", "#0067C0", "#FFFFD2", "#ADD6FF", "#EDEDED",
            "#F0F0F0", "#333333", "#0067C0", true,
            Syn("#008000", "#A31515", "#0000FF", "#098658", "#267F99", "#795E26", "#AF00DB", "#800000", "#E50000", "#811F3F", "#333333")),
        ["Midnight Blue"] = Make("#0F1626", "#D6DEEB", "#111B2E", "#24324D", "#4A8FE7", "#16233A", "#1D3B66", "#131E33",
            "#0B1220", "#9FB2D0", "#4A8FE7", false,
            Syn("#637777", "#ECC48D", "#C792EA", "#F78C6C", "#FFCB8B", "#82AAFF", "#C792EA", "#7FDBCA", "#ADDB67", "#FFCB8B", "#7FDBCA")),
        ["Monokai"] = Make("#272822", "#F8F8F2", "#1E1F1C", "#3E3D32", "#A6E22E", "#3E3D32", "#49483E", "#2F302A",
            "#1E1F1C", "#CFCFC2", "#F92672", false,
            Syn("#75715E", "#E6DB74", "#F92672", "#AE81FF", "#66D9EF", "#A6E22E", "#F92672", "#F92672", "#A6E22E", "#FD971F", "#F8F8F2")),
        ["Solarized Dark"] = Make("#002B36", "#93A1A1", "#073642", "#0E4552", "#268BD2", "#073642", "#0F4B5A", "#05313D",
            "#073642", "#93A1A1", "#B58900", false,
            Syn("#586E75", "#2AA198", "#859900", "#D33682", "#B58900", "#268BD2", "#CB4B16", "#268BD2", "#93A1A1", "#CB4B16", "#839496")),
        ["Nord"] = Make("#2E3440", "#D8DEE9", "#3B4252", "#4C566A", "#88C0D0", "#3B4252", "#434C5E", "#353C4A",
            "#3B4252", "#D8DEE9", "#88C0D0", false,
            Syn("#616E88", "#A3BE8C", "#81A1C1", "#B48EAD", "#8FBCBB", "#88C0D0", "#5E81AC", "#81A1C1", "#8FBCBB", "#D08770", "#ECEFF4")),
        ["Dracula"] = Make("#282A36", "#F8F8F2", "#21222C", "#44475A", "#BD93F9", "#343746", "#44475A", "#2B2D3A",
            "#191A21", "#F8F8F2", "#FF79C6", false,
            Syn("#6272A4", "#F1FA8C", "#FF79C6", "#BD93F9", "#8BE9FD", "#50FA7B", "#FF79C6", "#FF79C6", "#50FA7B", "#FFB86C", "#F8F8F2")),
        ["Gruvbox"] = Make("#282828", "#EBDBB2", "#1D2021", "#3C3836", "#D79921", "#32302F", "#504945", "#32302F",
            "#3C3836", "#D5C4A1", "#FE8019", false,
            Syn("#928374", "#B8BB26", "#FB4934", "#D3869B", "#FABD2F", "#8EC07C", "#FE8019", "#83A598", "#8EC07C", "#FE8019", "#EBDBB2")),
        ["High Contrast"] = Make("#000000", "#FFFFFF", "#000000", "#FFFFFF", "#1AEBFF", "#202020", "#1A4B8C", "#000000",
            "#000000", "#FFFFFF", "#FFFF00", false,
            Syn("#7CFC00", "#FFD700", "#00FFFF", "#FF80FF", "#80FF80", "#FFFF80", "#FF8080", "#00FFFF", "#FFC0CB", "#FFA500", "#FFFFFF")),
    };

    /// <summary>Final color table: default dark → preset → user overrides.</summary>
    public static Dictionary<string, Color> Resolve(ThemeSettings t)
    {
        var raw = new Dictionary<string, string>(Dark);
        if (t.Preset != null && Presets.TryGetValue(t.Preset, out var p))
            foreach (var kv in p) raw[kv.Key] = kv.Value;
        if (t.Colors != null)
            foreach (var kv in t.Colors) raw[kv.Key] = kv.Value;
        var res = new Dictionary<string, Color>();
        foreach (var kv in raw)
        {
            try { res[kv.Key] = Parse(kv.Value); }
            catch { res[kv.Key] = Parse(Dark.TryGetValue(kv.Key, out var d) ? d : "#FF00FF"); }
        }
        return res;
    }

    public static Color PresetColor(string preset, string id)
    {
        if (preset != null && Presets.TryGetValue(preset, out var p) && p.TryGetValue(id, out var v)) return Parse(v);
        return Dark.TryGetValue(id, out var d) ? Parse(d) : Colors.Magenta;
    }

    // ---- color helpers
    public static Color Parse(string s) => (Color)ColorConverter.ConvertFromString(s.Trim());

    public static string Hex(Color c) => c.A == 255
        ? $"#{c.R:X2}{c.G:X2}{c.B:X2}"
        : $"#{c.A:X2}{c.R:X2}{c.G:X2}{c.B:X2}";

    public static Color Blend(Color a, Color b, double t)
    {
        t = Math.Clamp(t, 0, 1);
        return Color.FromArgb(
            (byte)Math.Round(a.A + (b.A - a.A) * t),
            (byte)Math.Round(a.R + (b.R - a.R) * t),
            (byte)Math.Round(a.G + (b.G - a.G) * t),
            (byte)Math.Round(a.B + (b.B - a.B) * t));
    }

    public static Color Lighten(Color c, double t) => Blend(c, Color.FromArgb(c.A, 255, 255, 255), t);
    public static Color Darken(Color c, double t) => Blend(c, Color.FromArgb(c.A, 0, 0, 0), t);

    public static double Luminance(Color c) => (0.299 * c.R + 0.587 * c.G + 0.114 * c.B) / 255.0;

    public static string F(double v) => v.ToString("0.##", CultureInfo.InvariantCulture);
}
