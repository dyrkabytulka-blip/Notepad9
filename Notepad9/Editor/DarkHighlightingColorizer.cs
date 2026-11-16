using System.Windows;
using System.Windows.Media;
using ICSharpCode.AvalonEdit.Highlighting;
using ICSharpCode.AvalonEdit.Rendering;

namespace Notepad9.Editor;

/// <summary>
/// AvalonEdit's built-in schemes are designed for white backgrounds. This colorizer
/// remaps every highlighting color to a palette readable on #141414.
/// </summary>
public class DarkHighlightingColorizer : HighlightingColorizer
{
    static readonly Dictionary<HighlightingColor, HighlightingColor> Map = new();

    public DarkHighlightingColorizer(IHighlightingDefinition def) : base(def) { }

    protected override void ApplyColorToElement(VisualLineElement element, HighlightingColor color)
    {
        base.ApplyColorToElement(element, Remap(color));
    }

    /// <summary>Name fragment of an AvalonEdit color → theme syntax category.</summary>
    static readonly (string key, string cat)[] ByName =
    {
        ("comment", "comment"), ("doc", "comment"), ("xmldoc", "comment"),
        ("string", "string"), ("char", "string"), ("verbatim", "string"), ("attributevalue", "string"),
        ("attributename", "attribute"), ("attribute", "attribute"), ("property", "attribute"), ("variable", "attribute"),
        ("number", "number"), ("digit", "number"),
        ("preprocessor", "preprocessor"), ("directive", "preprocessor"), ("position", "preprocessor"),
        ("method", "method"), ("function", "method"), ("call", "method"), ("command", "method"), ("strong", "method"),
        ("entity", "entity"), ("cdata", "string"), ("selector", "entity"),
        ("tag", "tag"), ("element", "tag"), ("header", "tag"), ("heading", "tag"),
        ("type", "type"), ("class", "type"),
        ("keyword", "keyword"), ("visibility", "keyword"), ("modifier", "keyword"), ("this", "keyword"),
        ("null", "keyword"), ("truefalse", "keyword"), ("bool", "keyword"),
        ("emphasis", "string"), ("code", "string"), ("math", "string"), ("link", "link"),
        ("punctuation", "operator"), ("operator", "operator"), ("brace", "operator"),
        ("added", "added"), ("deleted", "deleted"), ("unchanged", "unchanged"),
    };

    static Dictionary<string, Color> _palette = new()
    {
        ["comment"] = C("#6A9955"), ["string"] = C("#CE9178"), ["keyword"] = C("#569CD6"), ["number"] = C("#B5CEA8"),
        ["type"] = C("#4EC9B0"), ["method"] = C("#DCDCAA"), ["preprocessor"] = C("#C586C0"), ["tag"] = C("#569CD6"),
        ["attribute"] = C("#9CDCFE"), ["entity"] = C("#D7BA7D"), ["operator"] = C("#D4D4D4"), ["link"] = C("#4FA3F7"),
        ["added"] = C("#89D185"), ["deleted"] = C("#F48771"), ["unchanged"] = C("#9A9A9A"),
    };
    static bool _lightBackground;

    static Color C(string hex) => (Color)ColorConverter.ConvertFromString(hex);

    /// <summary>Replace the syntax palette (theme change). Editors must redraw afterwards.</summary>
    public static void SetPalette(IDictionary<string, Color> palette, bool lightBackground)
    {
        lock (Map)
        {
            foreach (var kv in palette) _palette[kv.Key] = kv.Value;
            _lightBackground = lightBackground;
            Map.Clear();
        }
    }

    static HighlightingColor Remap(HighlightingColor c)
    {
        if (c == null) return null;
        lock (Map)
        {
            if (Map.TryGetValue(c, out var hit)) return hit;
            var n = new HighlightingColor
            {
                Name = c.Name,
                FontStyle = c.FontStyle,
                FontWeight = c.FontWeight == FontWeights.Bold ? FontWeights.SemiBold : c.FontWeight,
                Underline = c.Underline,
            };
            var named = FromName(c.Name);
            if (named != null) n.Foreground = new SimpleHighlightingBrush(named.Value);
            else if (c.Foreground?.GetColor(null) is Color fg) n.Foreground = new SimpleHighlightingBrush(_lightBackground ? fg : Lighten(fg));
            n.Freeze();
            Map[c] = n;
            return n;
        }
    }

    static Color? FromName(string name)
    {
        if (string.IsNullOrEmpty(name)) return null;
        var l = name.ToLowerInvariant();
        foreach (var (key, cat) in ByName)
            if (l.Contains(key) && _palette.TryGetValue(cat, out var col)) return col;
        return null;
    }

    /// <summary>Raise lightness of a light-theme color so it reads on a dark background.</summary>
    static Color Lighten(Color c)
    {
        double r = c.R / 255.0, g = c.G / 255.0, b = c.B / 255.0;
        double max = Math.Max(r, Math.Max(g, b)), min = Math.Min(r, Math.Min(g, b));
        double h = 0, s, l = (max + min) / 2;
        double d = max - min;
        if (d < 1e-6) s = 0;
        else
        {
            s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
            if (max == r) h = (g - b) / d + (g < b ? 6 : 0);
            else if (max == g) h = (b - r) / d + 2;
            else h = (r - g) / d + 4;
            h /= 6;
        }
        l = 0.62 + (1 - l) * 0.15;
        if (s < 0.05) l = 0.80;
        s = Math.Min(s, 0.65);
        return FromHsl(h, s, Math.Min(l, 0.85));
    }

    static Color FromHsl(double h, double s, double l)
    {
        double Hue(double p, double q, double t)
        {
            if (t < 0) t += 1;
            if (t > 1) t -= 1;
            if (t < 1 / 6.0) return p + (q - p) * 6 * t;
            if (t < 1 / 2.0) return q;
            if (t < 2 / 3.0) return p + (q - p) * (2 / 3.0 - t) * 6;
            return p;
        }
        double r, g, b;
        if (s == 0) r = g = b = l;
        else
        {
            double q = l < 0.5 ? l * (1 + s) : l + s - l * s;
            double p = 2 * l - q;
            r = Hue(p, q, h + 1 / 3.0);
            g = Hue(p, q, h);
            b = Hue(p, q, h - 1 / 3.0);
        }
        return Color.FromRgb((byte)(r * 255), (byte)(g * 255), (byte)(b * 255));
    }
}
