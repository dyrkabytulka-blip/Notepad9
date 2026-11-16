using System.Text.RegularExpressions;
using ICSharpCode.AvalonEdit;
using ICSharpCode.AvalonEdit.Document;

namespace Notepad9.Editor;

/// <summary>Markdown editing helpers: formatting buttons, list continuation on Enter, list indent on Tab.</summary>
public static class MarkdownTools
{
    static readonly Regex ListItem = new(@"^(?<indent>[ \t]*)(?<marker>[-*+]|\d+[.)])(?<space>[ \t]+)(?<task>\[[ xX]\][ \t]+)?", RegexOptions.Compiled);
    static readonly Regex QuoteLine = new(@"^(?<indent>[ \t]*)(?<marker>(>[ \t]?)+)", RegexOptions.Compiled);
    static readonly Regex HeadingRe = new(@"^(#{1,6})[ \t]+", RegexOptions.Compiled);

    static string Nl(TextDocument doc, int line) => TextUtilities.GetNewLineFromDocument(doc, line);

    // ------------------------------------------------------------------ inline
    /// <summary>Toggle inline markers (**bold**, *italic*, ~~strike~~, `code`) around the selection or word.</summary>
    public static void Wrap(TextEditor ed, string marker, string placeholder)
    {
        var doc = ed.Document;
        if (ed.SelectionLength == 0)
        {
            var (ws, we) = WordAt(doc, ed.CaretOffset);
            if (we > ws) ed.Select(ws, we - ws);
        }
        int s = ed.SelectionStart, len = ed.SelectionLength, m = marker.Length;
        if (len == 0)
        {
            doc.Insert(s, marker + placeholder + marker);
            ed.Select(s + m, placeholder.Length);
            return;
        }
        string sel = ed.SelectedText;
        // already wrapped inside the selection -> unwrap
        if (sel.Length >= 2 * m && sel.StartsWith(marker) && sel.EndsWith(marker))
        {
            string inner = sel[m..^m];
            doc.Replace(s, len, inner);
            ed.Select(s, inner.Length);
            return;
        }
        // wrapped just outside the selection -> unwrap
        if (s >= m && s + len + m <= doc.TextLength && doc.GetText(s - m, m) == marker && doc.GetText(s + len, m) == marker)
        {
            doc.BeginUpdate();
            doc.Remove(s + len, m);
            doc.Remove(s - m, m);
            doc.EndUpdate();
            ed.Select(s - m, len);
            return;
        }
        doc.Replace(s, len, marker + sel + marker);
        ed.Select(s + m, len);
    }

    static (int start, int end) WordAt(TextDocument doc, int offset)
    {
        int s = offset, e = offset;
        while (s > 0 && IsWordChar(doc.GetCharAt(s - 1))) s--;
        while (e < doc.TextLength && IsWordChar(doc.GetCharAt(e))) e++;
        return (s, e);
    }

    static bool IsWordChar(char c) => char.IsLetterOrDigit(c) || c == '_';

    public static void Link(TextEditor ed)
    {
        var doc = ed.Document;
        int s = ed.SelectionStart;
        string sel = ed.SelectedText;
        if (Uri.TryCreate(sel.Trim(), UriKind.Absolute, out _))
        {
            string t = $"[text]({sel.Trim()})";
            doc.Replace(s, sel.Length, t);
            ed.Select(s + 1, 4);
            return;
        }
        string label = sel.Length > 0 ? sel : "text";
        string text = $"[{label}](https://)";
        doc.Replace(s, sel.Length, text);
        if (sel.Length > 0) ed.Select(s + label.Length + 3, "https://".Length);
        else ed.Select(s + 1, label.Length);
    }

    public static void Image(TextEditor ed, string path)
    {
        string alt = ed.SelectionLength > 0 ? ed.SelectedText : "image";
        string text = $"![{alt}]({path.Replace(" ", "%20")})";
        int s = ed.SelectionStart;
        ed.Document.Replace(s, ed.SelectionLength, text);
        ed.Select(s + 2, alt.Length);
    }

    // ------------------------------------------------------------------ blocks
    static (int first, int last) Lines(TextEditor ed)
    {
        var doc = ed.Document;
        if (ed.SelectionLength == 0)
        {
            int n = doc.GetLineByOffset(ed.CaretOffset).LineNumber;
            return (n, n);
        }
        int s = ed.SelectionStart, e = s + ed.SelectionLength;
        var f = doc.GetLineByOffset(s);
        var l = doc.GetLineByOffset(e);
        if (l.Offset == e && l.LineNumber > f.LineNumber) l = l.PreviousLine;
        return (f.LineNumber, l.LineNumber);
    }

    /// <summary>Cycle the heading level of the current line: none → # → ## → ### → none.</summary>
    public static void Heading(TextEditor ed)
    {
        var doc = ed.Document;
        var line = doc.GetLineByOffset(ed.CaretOffset);
        string text = doc.GetText(line.Offset, line.Length);
        var m = HeadingRe.Match(text);
        int level = m.Success ? m.Groups[1].Length : 0;
        string body = m.Success ? text[m.Length..] : text.TrimStart();
        int next = level >= 3 ? 0 : level + 1;
        string newText = (next > 0 ? new string('#', next) + " " : "") + body;
        int caretCol = ed.CaretOffset - line.Offset - (text.Length - body.Length);
        doc.Replace(line.Offset, line.Length, newText);
        ed.CaretOffset = line.Offset + Math.Clamp(newText.Length - body.Length + caretCol, 0, newText.Length);
    }

    public enum ListKind { Bullet, Numbered, Task, Quote }

    /// <summary>Toggle a list / quote prefix on the selected lines.</summary>
    public static void ToggleList(TextEditor ed, ListKind kind)
    {
        var doc = ed.Document;
        var (first, last) = Lines(ed);
        var lines = Enumerable.Range(first, last - first + 1).Select(doc.GetLineByNumber).ToList();
        bool Has(DocumentLine l)
        {
            string t = doc.GetText(l.Offset, l.Length);
            if (kind == ListKind.Quote) return QuoteLine.IsMatch(t);
            var m = ListItem.Match(t);
            if (!m.Success) return false;
            bool numbered = char.IsDigit(m.Groups["marker"].Value[0]);
            bool task = m.Groups["task"].Success;
            return kind switch
            {
                ListKind.Task => task,
                ListKind.Numbered => numbered && !task,
                _ => !numbered && !task,
            };
        }
        bool remove = lines.Where(l => l.Length > 0).All(Has) && lines.Any(l => l.Length > 0);
        doc.BeginUpdate();
        try
        {
            int n = 1;
            foreach (var l in lines)
            {
                string t = doc.GetText(l.Offset, l.Length);
                if (t.Trim().Length == 0 && lines.Count > 1) continue;
                string stripped;
                string indent;
                var q = QuoteLine.Match(t);
                var li = ListItem.Match(t);
                if (kind == ListKind.Quote && q.Success)
                {
                    indent = q.Groups["indent"].Value;
                    stripped = t[q.Length..];
                }
                else if (kind != ListKind.Quote && li.Success)
                {
                    indent = li.Groups["indent"].Value;
                    stripped = t[li.Length..];
                }
                else
                {
                    indent = t[..(t.Length - t.TrimStart().Length)];
                    stripped = t.TrimStart();
                }
                string prefix = remove ? "" : kind switch
                {
                    ListKind.Bullet => "- ",
                    ListKind.Numbered => $"{n++}. ",
                    ListKind.Task => "- [ ] ",
                    _ => "> ",
                };
                doc.Replace(l.Offset, l.Length, indent + prefix + stripped);
            }
        }
        finally { doc.EndUpdate(); }
        var lastLine = doc.GetLineByNumber(last);
        if (first == last) ed.CaretOffset = lastLine.EndOffset;
        else ed.Select(doc.GetLineByNumber(first).Offset, lastLine.EndOffset - doc.GetLineByNumber(first).Offset);
    }

    public static void CodeBlock(TextEditor ed)
    {
        var doc = ed.Document;
        if (ed.SelectionLength == 0)
        {
            var line = doc.GetLineByOffset(ed.CaretOffset);
            string nl = Nl(doc, line.LineNumber);
            bool empty = line.Length == 0;
            string text = (empty ? "" : nl) + "```" + nl + nl + "```" + nl;
            int at = empty ? line.Offset : line.EndOffset;
            doc.Insert(at, text);
            ed.CaretOffset = at + (empty ? 0 : nl.Length) + 3;
            return;
        }
        var (first, last) = Lines(ed);
        var f = doc.GetLineByNumber(first);
        var l = doc.GetLineByNumber(last);
        string n2 = Nl(doc, first);
        string block = doc.GetText(f.Offset, l.EndOffset - f.Offset);
        string result = "```" + n2 + block + n2 + "```";
        doc.Replace(f.Offset, l.EndOffset - f.Offset, result);
        ed.CaretOffset = f.Offset + 3;
    }

    public static void Table(TextEditor ed)
    {
        var doc = ed.Document;
        var line = doc.GetLineByOffset(ed.CaretOffset);
        string nl = Nl(doc, line.LineNumber);
        string t = "| Column 1 | Column 2 | Column 3 |" + nl +
                   "| -------- | -------- | -------- |" + nl +
                   "|          |          |          |" + nl +
                   "|          |          |          |" + nl;
        bool empty = line.Length == 0;
        int at = empty ? line.Offset : line.EndOffset;
        string text = (empty ? "" : nl + nl) + t;
        doc.Insert(at, text);
        int start = at + (empty ? 0 : 2 * nl.Length) + 2;
        ed.Select(start, "Column 1".Length);
    }

    public static void HorizontalRule(TextEditor ed)
    {
        var doc = ed.Document;
        var line = doc.GetLineByOffset(ed.CaretOffset);
        string nl = Nl(doc, line.LineNumber);
        bool empty = line.Length == 0;
        int at = empty ? line.Offset : line.EndOffset;
        string text = (empty ? "" : nl + nl) + "---" + nl + nl;
        doc.Insert(at, text);
        ed.CaretOffset = at + text.Length;
    }

    // ------------------------------------------------------------------ smart keys
    /// <summary>Enter inside a list item continues the list; Enter on an empty item ends it.</summary>
    public static bool HandleEnter(TextEditor ed)
    {
        if (ed.SelectionLength > 0) return false;
        var doc = ed.Document;
        var line = doc.GetLineByOffset(ed.CaretOffset);
        int col = ed.CaretOffset - line.Offset;
        string text = doc.GetText(line.Offset, line.Length);
        string before = text[..col];
        string nl = Nl(doc, line.LineNumber);

        var m = ListItem.Match(text);
        if (m.Success && col >= m.Length)
        {
            if (text[m.Length..].Trim().Length == 0)
            {
                // empty item: remove the marker and stop the list
                doc.Replace(line.Offset, line.Length, m.Groups["indent"].Value);
                ed.CaretOffset = line.Offset + m.Groups["indent"].Length;
                return true;
            }
            string marker = m.Groups["marker"].Value;
            if (char.IsDigit(marker[0]))
            {
                int num = int.Parse(new string(marker.TakeWhile(char.IsDigit).ToArray()));
                marker = (num + 1) + marker[^1..];
            }
            string task = m.Groups["task"].Success ? "[ ] " : "";
            string insert = nl + m.Groups["indent"].Value + marker + m.Groups["space"].Value + task;
            doc.Insert(ed.CaretOffset, insert);
            return true;
        }
        var q = QuoteLine.Match(before);
        if (q.Success)
        {
            if (text[q.Length..].Trim().Length == 0)
            {
                doc.Replace(line.Offset, line.Length, "");
                return true;
            }
            doc.Insert(ed.CaretOffset, nl + q.Value);
            return true;
        }
        return false;
    }

    /// <summary>Tab / Shift+Tab on a list item indents / outdents the whole item.</summary>
    public static bool HandleTab(TextEditor ed, bool outdent)
    {
        var doc = ed.Document;
        var (first, last) = Lines(ed);
        var lines = Enumerable.Range(first, last - first + 1).Select(doc.GetLineByNumber).ToList();
        if (!lines.All(l => ListItem.IsMatch(doc.GetText(l.Offset, l.Length)))) return false;
        string unit = ed.Options.IndentationString;
        doc.BeginUpdate();
        try
        {
            foreach (var l in lines)
            {
                if (!outdent) doc.Insert(l.Offset, unit);
                else
                {
                    string t = doc.GetText(l.Offset, Math.Min(l.Length, unit.Length));
                    int remove = t.StartsWith('\t') ? 1 : t.TakeWhile(c => c == ' ').Count();
                    if (remove > 0) doc.Remove(l.Offset, remove);
                }
            }
        }
        finally { doc.EndUpdate(); }
        return true;
    }
}
