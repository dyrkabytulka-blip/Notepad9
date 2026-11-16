using System.Text.RegularExpressions;
using Notepad9.Editor;

namespace Notepad9.Services;

public class SearchOptions
{
    public string Find = "";
    public string Replace = "";
    public bool MatchCase;
    public bool WholeWord;
    public bool Regex;
    public bool Wrap = true;
}

public static class Search
{
    public static Regex Build(SearchOptions o, bool rightToLeft = false)
    {
        if (string.IsNullOrEmpty(o.Find)) return null;
        string p = o.Regex ? o.Find : System.Text.RegularExpressions.Regex.Escape(o.Find);
        if (o.WholeWord) p = @"(?<![\w])(?:" + p + @")(?![\w])";
        var opts = RegexOptions.Multiline | RegexOptions.CultureInvariant;
        if (!o.MatchCase) opts |= RegexOptions.IgnoreCase;
        if (rightToLeft) opts |= RegexOptions.RightToLeft;
        return new Regex(p, opts, TimeSpan.FromSeconds(3));
    }

    /// <summary>Returns null on success, otherwise a status message.</summary>
    public static string FindNext(N9Editor ed, SearchOptions o, bool backwards)
    {
        Regex re;
        try { re = Build(o, backwards); }
        catch (ArgumentException ex) { return "Invalid expression: " + ex.Message; }
        if (re == null) return "Nothing to find";
        string text = ed.Text;
        Match m;
        string note = null;
        if (!backwards)
        {
            int start = Math.Min(text.Length, ed.SelectionStart + ed.SelectionLength);
            m = re.Match(text, start);
            if (m.Success && m.Length == 0 && m.Index == start && start < text.Length) m = re.Match(text, start + 1);
            if (!m.Success && o.Wrap) { m = re.Match(text, 0); note = "Search wrapped to the top"; }
        }
        else
        {
            int start = ed.SelectionStart;
            m = re.Match(text, 0, start);
            if (!m.Success && o.Wrap) { m = re.Match(text, text.Length); note = "Search wrapped to the bottom"; }
        }
        if (!m.Success) return $"\"{o.Find}\" not found";
        SelectAndReveal(ed, m.Index, m.Length);
        return note ?? "";
    }

    public static void SelectAndReveal(N9Editor ed, int start, int length)
    {
        ed.Select(start, length);
        var loc = ed.Document.GetLocation(start);
        ed.ScrollTo(loc.Line, loc.Column);
        ed.TextArea.Caret.BringCaretToView();
    }

    public static string ReplaceCurrent(N9Editor ed, SearchOptions o)
    {
        Regex re;
        try { re = Build(o); }
        catch (ArgumentException ex) { return "Invalid expression: " + ex.Message; }
        if (re == null) return "Nothing to find";
        if (ed.SelectionLength > 0 || o.Regex)
        {
            var m = re.Match(ed.Text, ed.SelectionStart);
            if (m.Success && m.Index == ed.SelectionStart && m.Length == ed.SelectionLength)
            {
                string repl = o.Regex ? m.Result(o.Replace ?? "") : (o.Replace ?? "");
                ed.Document.Replace(m.Index, m.Length, repl);
                ed.Select(m.Index + repl.Length, 0);
            }
        }
        return FindNext(ed, o, false);
    }

    public static int ReplaceAll(N9Editor ed, SearchOptions o, bool inSelection, out string error)
    {
        error = null;
        Regex re;
        try { re = Build(o); }
        catch (ArgumentException ex) { error = "Invalid expression: " + ex.Message; return 0; }
        if (re == null) return 0;
        int from = 0, to = ed.Document.TextLength;
        if (inSelection && ed.SelectionLength > 0) { from = ed.SelectionStart; to = from + ed.SelectionLength; }
        string text = ed.Text;
        var matches = new List<Match>();
        foreach (Match m in re.Matches(text.Substring(0, to), from)) matches.Add(m);
        if (matches.Count == 0) return 0;
        var doc = ed.Document;
        doc.BeginUpdate();
        try
        {
            for (int i = matches.Count - 1; i >= 0; i--)
            {
                var m = matches[i];
                string repl = o.Regex ? m.Result(o.Replace ?? "") : (o.Replace ?? "");
                doc.Replace(m.Index, m.Length, repl);
            }
        }
        finally { doc.EndUpdate(); }
        return matches.Count;
    }

    public static int Count(string text, SearchOptions o)
    {
        try
        {
            var re = Build(o);
            return re == null ? 0 : re.Matches(text).Count;
        }
        catch { return 0; }
    }
}
