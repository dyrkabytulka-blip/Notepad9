using System.Globalization;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using ICSharpCode.AvalonEdit.Document;
using ICSharpCode.AvalonEdit.Editing;
using ICSharpCode.AvalonEdit.Rendering;

namespace Notepad9.Editor;

/// <summary>
/// Gray line-number gutter like the reference: #454545 background, numbers left aligned.
/// Click selects a line, drag extends the selection.
/// </summary>
public class GutterMargin : AbstractMargin
{
    Brush BackBrush = Freeze(new SolidColorBrush(Color.FromRgb(0x45, 0x45, 0x45)));
    Brush NumberBrush = Freeze(new SolidColorBrush(Color.FromRgb(0xA8, 0xA8, 0xA8)));
    Brush CurrentBrush = Freeze(new SolidColorBrush(Color.FromRgb(0xDE, 0xDE, 0xDE)));

    public void SetColors(Brush back, Brush number, Brush current)
    {
        BackBrush = back;
        NumberBrush = number;
        CurrentBrush = current;
        InvalidateVisual();
    }
    static readonly Typeface Face = new(new FontFamily("Consolas"), FontStyles.Normal, FontWeights.Normal, FontStretches.Normal);

    public const double MinGutterWidth = 46;
    public double NumberSize { get; set; } = 11;
    public bool ShowNumbers { get; set; } = true;
    int _digits = 1;
    int _dragStartLine = -1;

    static Brush Freeze(Brush b) { b.Freeze(); return b; }

    protected override void OnTextViewChanged(TextView oldTextView, TextView newTextView)
    {
        if (oldTextView != null) oldTextView.VisualLinesChanged -= OnVisualLinesChanged;
        base.OnTextViewChanged(oldTextView, newTextView);
        if (newTextView != null) newTextView.VisualLinesChanged += OnVisualLinesChanged;
        InvalidateVisual();
    }

    protected override void OnDocumentChanged(TextDocument oldDocument, TextDocument newDocument)
    {
        if (oldDocument != null) oldDocument.TextChanged -= OnLineCountChanged;
        base.OnDocumentChanged(oldDocument, newDocument);
        if (newDocument != null) newDocument.TextChanged += OnLineCountChanged;
        OnLineCountChanged(null, EventArgs.Empty);
    }

    void OnLineCountChanged(object sender, EventArgs e)
    {
        int d = (Document?.LineCount ?? 1).ToString(CultureInfo.InvariantCulture).Length;
        if (d != _digits)
        {
            _digits = d;
            InvalidateMeasure();
        }
    }

    void OnVisualLinesChanged(object sender, EventArgs e) => InvalidateVisual();

    public void Refresh()
    {
        InvalidateMeasure();
        InvalidateVisual();
    }

    FormattedText Text(string s, Brush b) =>
        new(s, CultureInfo.InvariantCulture, FlowDirection.LeftToRight, Face, NumberSize, b,
            VisualTreeHelper.GetDpi(this).PixelsPerDip);

    protected override Size MeasureOverride(Size availableSize)
    {
        if (!ShowNumbers) return new Size(6, 0);
        var ft = Text(new string('9', Math.Max(_digits, 2)), NumberBrush);
        return new Size(Math.Max(MinGutterWidth, Math.Ceiling(ft.Width + 14)), 0);
    }

    protected override void OnRender(DrawingContext dc)
    {
        dc.DrawRectangle(BackBrush, null, new Rect(RenderSize));
        var tv = TextView;
        if (!ShowNumbers || tv == null || !tv.VisualLinesValid) return;
        int caretLine = (tv.Services.GetService(typeof(TextArea)) as TextArea)?.Caret.Line ?? -1;
        foreach (var line in tv.VisualLines)
        {
            int n = line.FirstDocumentLine.LineNumber;
            var ft = Text(n.ToString(CultureInfo.InvariantCulture), n == caretLine ? CurrentBrush : NumberBrush);
            var tl = line.TextLines[0];
            double top = line.GetTextLineVisualYPosition(tl, VisualYPosition.TextTop) - tv.VerticalOffset;
            double y = top + tl.Baseline - ft.Baseline;
            dc.DrawText(ft, new Point(6, Math.Round(y)));
        }
    }

    protected override HitTestResult HitTestCore(PointHitTestParameters hitTestParameters) =>
        new PointHitTestResult(this, hitTestParameters.HitPoint);

    TextArea Area => TextView?.Services.GetService(typeof(TextArea)) as TextArea;

    int LineAt(Point p)
    {
        var tv = TextView;
        if (tv == null) return -1;
        var vl = tv.GetVisualLineFromVisualTop(p.Y + tv.VerticalOffset);
        return vl?.FirstDocumentLine.LineNumber ?? Document.LineCount;
    }

    protected override void OnMouseLeftButtonDown(MouseButtonEventArgs e)
    {
        base.OnMouseLeftButtonDown(e);
        var area = Area;
        if (area == null || Document == null) return;
        int line = LineAt(e.GetPosition(TextView));
        if (line < 1) return;
        e.Handled = true;
        area.Focus();
        _dragStartLine = (Keyboard.Modifiers & ModifierKeys.Shift) != 0 ? area.Caret.Line : line;
        SelectLines(_dragStartLine, line);
        CaptureMouse();
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        if (_dragStartLine > 0 && IsMouseCaptured)
            SelectLines(_dragStartLine, LineAt(e.GetPosition(TextView)));
    }

    protected override void OnMouseLeftButtonUp(MouseButtonEventArgs e)
    {
        base.OnMouseLeftButtonUp(e);
        _dragStartLine = -1;
        ReleaseMouseCapture();
    }

    void SelectLines(int a, int b)
    {
        var area = Area;
        if (area == null || a < 1 || b < 1) return;
        var doc = Document;
        a = Math.Min(a, doc.LineCount);
        b = Math.Min(b, doc.LineCount);
        var first = doc.GetLineByNumber(Math.Min(a, b));
        var last = doc.GetLineByNumber(Math.Max(a, b));
        int start = first.Offset, end = last.Offset + last.TotalLength;
        if (b >= a)
        {
            area.Selection = Selection.Create(area, start, end);
            area.Caret.Offset = end;
        }
        else
        {
            area.Selection = Selection.Create(area, end, start);
            area.Caret.Offset = start;
        }
    }
}
