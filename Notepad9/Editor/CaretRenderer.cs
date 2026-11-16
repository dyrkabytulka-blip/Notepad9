using System.Windows;
using System.Windows.Media;
using System.Windows.Threading;
using ICSharpCode.AvalonEdit.Editing;
using ICSharpCode.AvalonEdit.Rendering;

namespace Notepad9.Editor;

public enum CaretShape { Line, Block, Underline, Box }

/// <summary>
/// Draws the text caret instead of AvalonEdit (whose caret is made transparent), so its shape,
/// width, color and blinking can be customized. Overwrite mode always shows a block.
/// </summary>
public class CaretRenderer : IBackgroundRenderer
{
    readonly TextArea _area;
    readonly DispatcherTimer _blink;
    bool _on = true;
    Brush _brush = Brushes.Gainsboro;
    Brush _blockBrush = Brushes.Gainsboro;
    CaretShape _shape = CaretShape.Line;
    double _width = 1.5;
    bool _blinkEnabled = true;

    public CaretRenderer(TextArea area)
    {
        _area = area;
        _blink = new DispatcherTimer(DispatcherPriority.Render) { Interval = TimeSpan.FromMilliseconds(530) };
        _blink.Tick += (_, _) =>
        {
            _on = !_on;
            Invalidate();
        };
        area.Caret.PositionChanged += (_, _) => Restart();
        area.GotKeyboardFocus += (_, _) => Restart();
        area.LostKeyboardFocus += (_, _) => { _blink.Stop(); Invalidate(); };
        area.TextView.ScrollOffsetChanged += (_, _) => Invalidate();
        area.TextView.VisualLinesChanged += (_, _) => Invalidate();
    }

    public KnownLayer Layer => KnownLayer.Caret;

    public void Configure(Brush brush, CaretShape shape, double width, bool blink, int blinkMs)
    {
        _brush = brush;
        var c = brush is SolidColorBrush sc ? sc.Color : Colors.Gainsboro;
        // block carets are drawn see-through so the character under them stays readable
        var bc = new SolidColorBrush(Color.FromArgb((byte)Math.Min(c.A, (byte)150), c.R, c.G, c.B));
        bc.Freeze();
        _blockBrush = bc;
        _shape = shape;
        _width = Math.Clamp(width, 1, 12);
        _blinkEnabled = blink;
        _blink.Interval = TimeSpan.FromMilliseconds(Math.Clamp(blinkMs, 100, 3000));
        Restart();
    }

    void Restart()
    {
        _on = true;
        _blink.Stop();
        if (_blinkEnabled && _area.IsKeyboardFocusWithin) _blink.Start();
        Invalidate();
    }

    void Invalidate() => _area.TextView.InvalidateLayer(KnownLayer.Caret);

    public void Draw(TextView textView, DrawingContext dc)
    {
        if (!_on || !_area.IsKeyboardFocusWithin || !textView.VisualLinesValid) return;
        var caret = _area.Caret;
        var pos = caret.Position;
        var vl = textView.GetVisualLine(pos.Line);
        if (vl == null) return;
        var scroll = textView.ScrollOffset;
        var top = textView.GetVisualPosition(pos, VisualYPosition.LineTop) - scroll;
        var bottom = textView.GetVisualPosition(pos, VisualYPosition.LineBottom) - scroll;
        double h = Math.Max(1, bottom.Y - top.Y);

        // width of the character under the caret (or a space at the end of the line)
        double cw = textView.WideSpaceWidth;
        var next = new ICSharpCode.AvalonEdit.TextViewPosition(pos.Line, pos.Column + 1, pos.VisualColumn + 1);
        if (pos.Column <= vl.LastDocumentLine.Length + 1 && caret.Offset < textView.Document.TextLength
            && textView.Document.GetCharAt(caret.Offset) is not ('\r' or '\n'))
        {
            double nx = textView.GetVisualPosition(next, VisualYPosition.LineTop).X - scroll.X;
            if (nx - top.X > 1 && top.Y == textView.GetVisualPosition(next, VisualYPosition.LineTop).Y - scroll.Y)
                cw = nx - top.X;
        }

        var shape = _area.OverstrikeMode ? CaretShape.Block : _shape;
        double x = Math.Round(top.X), y = Math.Round(top.Y);
        switch (shape)
        {
            case CaretShape.Block:
                dc.DrawRectangle(_blockBrush, null, new Rect(x, y, Math.Max(2, cw), h));
                break;
            case CaretShape.Underline:
                dc.DrawRectangle(_brush, null, new Rect(x, y + h - _width, Math.Max(2, cw), _width));
                break;
            case CaretShape.Box:
            {
                var pen = new Pen(_brush, Math.Min(_width, 3));
                double half = pen.Thickness / 2;
                dc.DrawRectangle(null, pen, new Rect(x + half, y + half, Math.Max(2, cw) - pen.Thickness, h - pen.Thickness));
                break;
            }
            default:
                dc.DrawRectangle(_brush, null, new Rect(x, y, _width, h));
                break;
        }
    }
}
