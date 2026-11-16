using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shapes;
using System.Windows.Threading;
using ICSharpCode.AvalonEdit;
using ICSharpCode.AvalonEdit.Document;
using ICSharpCode.AvalonEdit.Folding;
using ICSharpCode.AvalonEdit.Highlighting;
using ICSharpCode.AvalonEdit.Rendering;

namespace Notepad9.Editor;

public enum FoldMode { None, Brace, Xml }

/// <summary>Colors and cursor settings applied to every editor (from the theme).</summary>
public class EditorLook
{
    public Color Fore = Color.FromRgb(0xDE, 0xDE, 0xDE);
    public Color CurrentLine = Color.FromRgb(0x40, 0x40, 0x13);
    public Color Selection = Color.FromRgb(0x26, 0x4F, 0x78);
    public Color Caret = Color.FromRgb(0xB5, 0xB5, 0xB5);
    public Color Gutter = Color.FromRgb(0x45, 0x45, 0x45);
    public Color LineNumber = Color.FromRgb(0xA8, 0xA8, 0xA8);
    public Color LineNumberCurrent = Color.FromRgb(0xDE, 0xDE, 0xDE);
    public Color Whitespace = Color.FromRgb(0x5E, 0x5E, 0x5E);
    public Color Ruler = Color.FromRgb(0x3C, 0x3C, 0x3C);
    public Color Occurrence = Color.FromArgb(0x55, 0x3E, 0x8E, 0xC8);
    public Color Link = Color.FromRgb(0x4F, 0xA3, 0xF7);
    public Color Fold = Color.FromRgb(0xA0, 0xA0, 0xA0);
    public CaretShape CaretShape = CaretShape.Line;
    public double CaretWidth = 1.5;
    public bool CaretBlink = true;
    public int CaretBlinkMs = 530;
    public double LineNumberScale = 0.75;
    public System.Windows.Input.Cursor Pointer;
}

/// <summary>AvalonEdit configured to look like the reference (dark, gray gutter, olive current line).</summary>
public class N9Editor : TextEditor
{
    public GutterMargin Gutter { get; }
    public OccurrenceRenderer Occurrences { get; }
    public CaretRenderer CaretLook { get; }
    readonly Rectangle _gap;
    FoldingManager _folding;
    FoldMode _foldMode;
    readonly BraceFoldingStrategy _brace = new();
    readonly XmlFoldingStrategy _xml = new();
    readonly DispatcherTimer _foldTimer;
    List<CollapsedLineSection> _focus;
    EditorLook _look = new();
    bool _rulerOn;
    double _fontSize = 14.667;

    public event EventHandler FoldingStateChanged;

    public N9Editor()
    {
        // the editor is transparent: the window paints the background color / image behind it
        Background = Brushes.Transparent;
        FontFamily = new FontFamily("Consolas");
        FontSize = _fontSize;
        HorizontalScrollBarVisibility = ScrollBarVisibility.Auto;
        VerticalScrollBarVisibility = ScrollBarVisibility.Auto;
        ShowLineNumbers = false;
        FocusVisualStyle = null;
        TextOptions.SetTextFormattingMode(this, TextFormattingMode.Display);

        Options.HighlightCurrentLine = true;
        Options.EnableHyperlinks = true;
        Options.EnableEmailHyperlinks = true;
        Options.RequireControlModifierForHyperlinkClick = true;
        Options.AllowToggleOverstrikeMode = true;
        Options.EnableRectangularSelection = true;
        Options.CutCopyWholeLine = true;
        Options.IndentationSize = 4;
        Options.ColumnRulerPosition = 80;
        Options.EnableTextDragDrop = true;

        var tv = TextArea.TextView;
        tv.CurrentLineBorder = new Pen(Brushes.Transparent, 0);
        TextArea.SelectionBorder = null;
        TextArea.SelectionForeground = null;
        TextArea.SelectionCornerRadius = 0;
        TextArea.FocusVisualStyle = null;

        Gutter = new GutterMargin();
        TextArea.LeftMargins.Insert(0, Gutter);
        _gap = new Rectangle { Width = 1, Fill = Brushes.Transparent };
        TextArea.LeftMargins.Add(_gap);

        Occurrences = new OccurrenceRenderer();
        tv.BackgroundRenderers.Add(Occurrences);
        CaretLook = new CaretRenderer(TextArea);
        tv.BackgroundRenderers.Add(CaretLook);

        TextArea.Caret.PositionChanged += (_, _) => Gutter.InvalidateVisual();

        _foldTimer = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromMilliseconds(600) };
        _foldTimer.Tick += (_, _) =>
        {
            _foldTimer.Stop();
            UpdateFoldings();
        };
        TextChanged += (_, _) =>
        {
            if (_folding != null) { _foldTimer.Stop(); _foldTimer.Start(); }
        };
        Loaded += (_, _) => PaintMarginStrip();
        ApplyLook(_look);
    }

    static SolidColorBrush B(Color c)
    {
        var b = new SolidColorBrush(c);
        b.Freeze();
        return b;
    }

    protected override IVisualLineTransformer CreateColorizer(IHighlightingDefinition highlightingDefinition) =>
        new DarkHighlightingColorizer(highlightingDefinition);

    /// <summary>The folding margin is transparent; give the margin strip the gutter color.</summary>
    void PaintMarginStrip()
    {
        DependencyObject d = Gutter;
        while (d != null && d is not ItemsControl) d = VisualTreeHelper.GetParent(d);
        if (d is ItemsControl ic) ic.Background = B(_look.Gutter);
    }

    /// <summary>Apply theme colors and cursor settings.</summary>
    public void ApplyLook(EditorLook look)
    {
        _look = look;
        var tv = TextArea.TextView;
        Foreground = B(look.Fore);
        tv.CurrentLineBackground = B(look.CurrentLine);
        TextArea.SelectionBrush = B(look.Selection);
        tv.LinkTextForegroundBrush = B(look.Link);
        tv.NonPrintableCharacterBrush = B(look.Whitespace);
        TextArea.Caret.CaretBrush = Brushes.Transparent;   // drawn by CaretLook instead
        CaretLook.Configure(B(look.Caret), look.CaretShape, look.CaretWidth, look.CaretBlink, look.CaretBlinkMs);
        Gutter.SetColors(B(look.Gutter), B(look.LineNumber), B(look.LineNumberCurrent));
        Occurrences.SetColor(look.Occurrence);
        SetLongLineMarker(_rulerOn, Options.ColumnRulerPosition);
        foreach (var m in TextArea.LeftMargins.OfType<FoldingMargin>()) StyleFoldingMargin(m);
        PaintMarginStrip();
        var cursor = look.Pointer ?? System.Windows.Input.Cursors.IBeam;
        tv.Cursor = cursor;
        Gutter.Refresh();
        ApplyFont(FontFamily, _fontSize, FontWeight, FontStyle);
        tv.Redraw();
    }

    void StyleFoldingMargin(FoldingMargin m)
    {
        m.FoldingMarkerBrush = B(_look.Fold);
        m.FoldingMarkerBackgroundBrush = B(_look.Gutter);
        m.SelectedFoldingMarkerBrush = B(_look.Fore);
        m.SelectedFoldingMarkerBackgroundBrush = B(ThemeMix(_look.Gutter, _look.Fore, 0.2));
    }

    static Color ThemeMix(Color a, Color b, double t) => Color.FromArgb(
        (byte)(a.A + (b.A - a.A) * t), (byte)(a.R + (b.R - a.R) * t), (byte)(a.G + (b.G - a.G) * t), (byte)(a.B + (b.B - a.B) * t));

    /// <summary>AvalonEdit draws the ruler whenever a pen is set, so hide it with a transparent pen.</summary>
    public void SetLongLineMarker(bool on, int column)
    {
        _rulerOn = on;
        Options.ColumnRulerPosition = column;
        Options.ShowColumnRuler = on;
        var pen = new Pen(on ? B(_look.Ruler) : Brushes.Transparent, 1);
        pen.Freeze();
        TextArea.TextView.ColumnRulerPen = pen;
    }

    // ------------------------------------------------------------------ zoom / fonts
    public void ApplyFont(FontFamily family, double size, FontWeight weight, FontStyle style)
    {
        _fontSize = size;
        FontFamily = family;
        FontSize = size;
        FontWeight = weight;
        FontStyle = style;
        Gutter.NumberSize = Math.Max(6, size * _look.LineNumberScale);
        Gutter.Refresh();
    }

    public void SetShowLineNumbers(bool show)
    {
        Gutter.ShowNumbers = show;
        Gutter.Refresh();
    }

    // ------------------------------------------------------------------ folding
    public bool FoldingAvailable => _folding != null && _folding.AllFoldings.Any();
    public bool FoldingEnabled { get; private set; } = true;

    public void SetFoldMode(FoldMode mode, bool enabled)
    {
        _foldMode = mode;
        FoldingEnabled = enabled;
        if (_folding != null)
        {
            FoldingManager.Uninstall(_folding);
            _folding = null;
        }
        if (mode != FoldMode.None && enabled)
        {
            _folding = FoldingManager.Install(TextArea);
            foreach (var m in TextArea.LeftMargins.OfType<FoldingMargin>()) StyleFoldingMargin(m);
            TextArea.LeftMargins.Remove(_gap);
            TextArea.LeftMargins.Add(_gap);
            UpdateFoldings();
        }
        FoldingStateChanged?.Invoke(this, EventArgs.Empty);
    }

    public void UpdateFoldings()
    {
        if (_folding == null) return;
        try
        {
            if (_foldMode == FoldMode.Brace) _brace.UpdateFoldings(_folding, Document);
            else if (_foldMode == FoldMode.Xml) _xml.UpdateFoldings(_folding, Document);
        }
        catch { /* malformed text while typing */ }
        FoldingStateChanged?.Invoke(this, EventArgs.Empty);
    }

    public void ToggleAllFolds()
    {
        if (_folding == null) return;
        bool anyFolded = _folding.AllFoldings.Any(f => f.IsFolded);
        foreach (var f in _folding.AllFoldings) f.IsFolded = !anyFolded;
    }

    // ------------------------------------------------------------------ focused view
    public bool IsFocusedView => _focus != null;

    /// <summary>Hide every line that does not contain <paramref name="term"/>. Returns the number of matching lines.</summary>
    public int EnterFocusedView(string term, StringComparison cmp)
    {
        ExitFocusedView();
        if (string.IsNullOrEmpty(term)) return 0;
        var doc = Document;
        var tv = TextArea.TextView;
        var sections = new List<CollapsedLineSection>();
        var hits = new List<int>();
        int runStart = -1;
        for (int i = 1; i <= doc.LineCount; i++)
        {
            var line = doc.GetLineByNumber(i);
            bool hit = doc.GetText(line.Offset, line.Length).IndexOf(term, cmp) >= 0;
            if (hit)
            {
                hits.Add(i);
                if (runStart > 0)
                {
                    sections.Add(tv.CollapseLines(doc.GetLineByNumber(runStart), doc.GetLineByNumber(i - 1)));
                    runStart = -1;
                }
            }
            else if (runStart < 0) runStart = i;
        }
        if (hits.Count == 0)
        {
            foreach (var s in sections) s.Uncollapse();
            return 0;
        }
        if (runStart > 0) sections.Add(tv.CollapseLines(doc.GetLineByNumber(runStart), doc.GetLineByNumber(doc.LineCount)));
        _focus = sections;
        if (!hits.Contains(TextArea.Caret.Line))
        {
            TextArea.Caret.Line = hits[0];
            TextArea.Caret.Column = 1;
        }
        TextArea.Caret.BringCaretToView();
        return hits.Count;
    }

    public void ExitFocusedView()
    {
        if (_focus == null) return;
        foreach (var s in _focus) s.Uncollapse();
        _focus = null;
        TextArea.TextView.Redraw();
    }
}

/// <summary>Marks every occurrence of the selected text.</summary>
public class OccurrenceRenderer : IBackgroundRenderer
{
    Brush Fill;
    Pen Border;

    public OccurrenceRenderer() => SetColor(Color.FromArgb(0x55, 0x3E, 0x8E, 0xC8));

    public void SetColor(Color c)
    {
        var f = new SolidColorBrush(c);
        f.Freeze();
        var b = new SolidColorBrush(Color.FromArgb((byte)Math.Min(255, c.A * 2 + 40), c.R, c.G, c.B));
        b.Freeze();
        var p = new Pen(b, 1);
        p.Freeze();
        Fill = f;
        Border = p;
    }

    public string Term { get; set; }
    public bool MatchCase { get; set; } = true;
    public bool Enabled { get; set; } = true;
    public KnownLayer Layer => KnownLayer.Selection;

    public void Draw(TextView textView, DrawingContext drawingContext)
    {
        if (!Enabled || string.IsNullOrEmpty(Term) || !textView.VisualLinesValid || textView.Document == null) return;
        var doc = textView.Document;
        var cmp = MatchCase ? StringComparison.Ordinal : StringComparison.OrdinalIgnoreCase;
        foreach (var vl in textView.VisualLines)
        {
            int start = vl.FirstDocumentLine.Offset;
            int end = vl.LastDocumentLine.EndOffset;
            if (end <= start) continue;
            string text = doc.GetText(start, end - start);
            int i = 0;
            while ((i = text.IndexOf(Term, i, cmp)) >= 0)
            {
                var seg = new TextSegment { StartOffset = start + i, Length = Term.Length };
                foreach (var r in BackgroundGeometryBuilder.GetRectsForSegment(textView, seg))
                {
                    var rr = new Rect(Math.Round(r.X) + 0.5, Math.Round(r.Y) + 0.5, Math.Max(1, Math.Round(r.Width) - 1), Math.Round(r.Height) - 1);
                    drawingContext.DrawRectangle(Fill, Border, rr);
                }
                i += Term.Length;
            }
        }
    }
}
