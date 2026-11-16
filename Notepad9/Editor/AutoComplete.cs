using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using ICSharpCode.AvalonEdit.CodeCompletion;
using ICSharpCode.AvalonEdit.Document;
using ICSharpCode.AvalonEdit.Editing;
using Notepad9.Controls;

namespace Notepad9.Editor;

public class CompletionItem : ICompletionData
{
    public CompletionItem(string text, bool keyword)
    {
        Text = text;
        IsKeyword = keyword;
    }

    public bool IsKeyword { get; }
    public string Text { get; }
    public object Content => Text;
    public object Description => IsKeyword ? "keyword" : "word from this document";
    public ImageSource Image => IconImage.GetSized(IsKeyword ? "ac_key" : "ac_word");
    public double Priority => IsKeyword ? 1 : 0;

    public void Complete(TextArea textArea, ISegment completionSegment, EventArgs insertionRequestEventArgs) =>
        textArea.Document.Replace(completionSegment, Text);
}

/// <summary>
/// Word completion: words already in the document + keywords of the current language.
/// Pops up automatically after 2 typed letters, or on Ctrl+Space. Tab / Enter accept.
/// </summary>
public class AutoCompleter
{
    readonly N9Editor _ed;
    CompletionWindow _win;
    HashSet<string> _words = new();
    ITextSourceVersion _wordsVersion;
    static readonly Regex WordRe = new(@"[\p{L}_][\p{L}\p{Nd}_]{2,}", RegexOptions.Compiled);

    public bool Enabled { get; set; } = true;
    /// <summary>Name of the current highlighting scheme (C#, Python ...), used to pick keywords.</summary>
    public Func<string> SchemeName { get; set; }

    public AutoCompleter(N9Editor ed)
    {
        _ed = ed;
        ed.TextArea.TextEntered += OnTextEntered;
        ed.TextArea.TextEntering += OnTextEntering;
    }

    public bool IsOpen => _win != null;

    void OnTextEntering(object sender, TextCompositionEventArgs e)
    {
        // typing a non-word character closes the list without inserting anything
        if (_win != null && e.Text.Length > 0 && !IsWordChar(e.Text[0])) _win.Close();
    }

    void OnTextEntered(object sender, TextCompositionEventArgs e)
    {
        if (!Enabled || _win != null || e.Text.Length != 1 || !IsWordChar(e.Text[0])) return;
        var (start, prefix) = WordBeforeCaret();
        if (prefix.Length >= 2 && !char.IsDigit(prefix[0])) Show(start, prefix, true);
    }

    /// <summary>Ctrl+Space.</summary>
    public void ShowManual()
    {
        if (_win != null) return;
        var (start, prefix) = WordBeforeCaret();
        Show(start, prefix, false);
    }

    (int start, string prefix) WordBeforeCaret()
    {
        var doc = _ed.Document;
        int caret = _ed.CaretOffset, s = caret;
        while (s > 0 && IsWordChar(doc.GetCharAt(s - 1))) s--;
        return (s, doc.GetText(s, caret - s));
    }

    static bool IsWordChar(char c) => char.IsLetterOrDigit(c) || c == '_';

    void Show(int start, string prefix, bool automatic)
    {
        var items = Collect(prefix);
        if (items.Count == 0) return;
        if (automatic && items.Count == 1 && items[0].Text.Equals(prefix, StringComparison.Ordinal)) return;

        var w = new CompletionWindow(_ed.TextArea)
        {
            StartOffset = start,
            CloseWhenCaretAtBeginning = automatic,
            CloseAutomatically = true,
            Width = 280,
            MaxHeight = 260,
            Background = new SolidColorBrush(Color.FromRgb(0x25, 0x25, 0x26)),
            Foreground = new SolidColorBrush(Color.FromRgb(0xDE, 0xDE, 0xDE)),
            BorderBrush = new SolidColorBrush(Color.FromRgb(0x4F, 0x4F, 0x4F)),
            BorderThickness = new Thickness(1),
            FontFamily = new FontFamily("Consolas"),
            FontSize = 13,
            ResizeMode = ResizeMode.NoResize,
        };
        foreach (var i in items) w.CompletionList.CompletionData.Add(i);
        w.Loaded += (_, _) => StyleList(w);
        w.Closed += (_, _) => { if (_win == w) _win = null; };
        _win = w;
        w.Show();
        if (prefix.Length > 0) w.CompletionList.SelectItem(prefix);
    }

    static void StyleList(CompletionWindow w)
    {
        var lb = w.CompletionList.ListBox;
        if (lb == null) return;
        if (Application.Current.TryFindResource(typeof(ListBox)) is Style ls) lb.Style = ls;
        lb.Background = w.Background;
        lb.BorderThickness = new Thickness(0);
        if (Application.Current.TryFindResource(typeof(ListBoxItem)) is Style its)
        {
            var item = new Style(typeof(ListBoxItem), its);
            item.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(4, 1, 6, 1)));
            lb.ItemContainerStyle = item;
        }
    }

    List<CompletionItem> Collect(string prefix)
    {
        var result = new Dictionary<string, CompletionItem>(StringComparer.Ordinal);
        bool Match(string w) =>
            prefix.Length == 0 || (w.Length > prefix.Length || !w.Equals(prefix, StringComparison.Ordinal))
            && w.Contains(prefix, StringComparison.OrdinalIgnoreCase);

        foreach (var k in Keywords.For(SchemeName?.Invoke()))
            if (Match(k)) result[k] = new CompletionItem(k, true);

        RefreshWords();
        foreach (var w in _words)
            if (!result.ContainsKey(w) && Match(w) && !w.Equals(prefix, StringComparison.Ordinal))
                result[w] = new CompletionItem(w, false);

        return result.Values
            .OrderByDescending(i => i.Text.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            .ThenBy(i => i.Text, StringComparer.OrdinalIgnoreCase)
            .Take(400)
            .ToList();
    }

    void RefreshWords()
    {
        var doc = _ed.Document;
        var v = doc.Version;
        if (_wordsVersion != null && v != null && _wordsVersion.BelongsToSameDocumentAs(v) && _wordsVersion.CompareAge(v) == 0) return;
        _wordsVersion = v;
        var set = new HashSet<string>(StringComparer.Ordinal);
        if (doc.TextLength <= 4_000_000)
        {
            // skip the word under the caret: it is what the user is typing right now
            var (start, prefix) = WordBeforeCaret();
            foreach (Match m in WordRe.Matches(doc.Text))
            {
                if (m.Index == start && m.Length >= prefix.Length) continue;
                if (m.Length <= 60) set.Add(m.Value);
            }
        }
        _words = set;
    }
}

/// <summary>Keyword lists for the schemes AvalonEdit ships.</summary>
public static class Keywords
{
    static readonly Dictionary<string, string[]> Map = new(StringComparer.OrdinalIgnoreCase)
    {
        ["C#"] = S("abstract as async await base bool break byte case catch char checked class const continue decimal default delegate do double else enum event explicit extern false finally fixed float for foreach get goto if implicit in init int interface internal is lock long namespace new null object operator out override params private protected public readonly record ref required return sbyte sealed set short sizeof stackalloc static string struct switch this throw true try typeof uint ulong unchecked unsafe ushort using value var virtual void volatile when where while with yield Console WriteLine ReadLine List Dictionary Task string.Empty nameof"),
        ["Python"] = S("False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield print input len range enumerate zip open self super isinstance list dict set tuple str int float bool sorted reversed min max sum abs map filter __init__ __name__ __main__"),
        ["JavaScript"] = S("async await break case catch class const continue debugger default delete do else export extends false finally for function if import in instanceof let new null return super switch this throw true try typeof undefined var void while with yield console log document window JSON parse stringify Promise addEventListener querySelector getElementById setTimeout fetch Array Object Math"),
        ["Java"] = S("abstract assert boolean break byte case catch char class const continue default do double else enum extends final finally float for if implements import instanceof int interface long native new null package private protected public return short static strictfp super switch synchronized this throw throws transient true false try void volatile while String System out println ArrayList HashMap Override"),
        ["C++"] = S("alignas auto bool break case catch char class const constexpr const_cast continue decltype default delete do double dynamic_cast else enum explicit export extern false float for friend goto if inline int long mutable namespace new noexcept nullptr operator private protected public register reinterpret_cast return short signed sizeof static static_cast struct switch template this throw true try typedef typename union unsigned using virtual void volatile while include define ifdef ifndef endif std cout cin endl vector string map unique_ptr shared_ptr"),
        ["PHP"] = S("abstract and array as break callable case catch class clone const continue declare default do echo else elseif empty enddeclare endfor endforeach endif endswitch endwhile extends final finally fn for foreach function global goto if implements include include_once instanceof insteadof interface isset list match namespace new null or print private protected public readonly require require_once return static switch throw trait try unset use var while yield true false"),
        ["TSQL"] = S("SELECT FROM WHERE INSERT INTO VALUES UPDATE SET DELETE CREATE TABLE ALTER DROP INDEX VIEW PROCEDURE FUNCTION JOIN INNER LEFT RIGHT OUTER FULL ON AND OR NOT NULL IS IN EXISTS BETWEEN LIKE GROUP BY ORDER HAVING DISTINCT TOP AS UNION ALL CASE WHEN THEN ELSE END BEGIN COMMIT ROLLBACK TRANSACTION PRIMARY KEY FOREIGN REFERENCES DEFAULT COUNT SUM AVG MIN MAX CAST CONVERT VARCHAR NVARCHAR INT BIGINT DATETIME DECIMAL"),
        ["PowerShell"] = S("begin break catch class continue data define do dynamicparam else elseif end enum exit filter finally for foreach from function hidden if in param process return static switch throw trap try until using var while Write-Host Write-Output Get-ChildItem Get-Content Set-Content Get-Item Remove-Item New-Item Copy-Item Move-Item Test-Path Select-Object Where-Object ForEach-Object Sort-Object Invoke-WebRequest Start-Process"),
        ["VB"] = S("AddHandler AddressOf Alias And AndAlso As Boolean ByRef Byte ByVal Call Case Catch Class Const Continue Date Decimal Declare Default Delegate Dim Do Double Each Else ElseIf End Enum Erase Error Event Exit False Finally For Friend Function Get Global GoTo Handles If Implements Imports In Inherits Integer Interface Is Let Lib Like Long Loop Me Mod Module MustInherit MustOverride MyBase Namespace New Next Not Nothing Object Of On Option Optional Or OrElse Overloads Overridable Overrides ParamArray Private Property Protected Public RaiseEvent ReadOnly ReDim Return Select Set Shared Short Single Static Step Stop String Structure Sub SyncLock Then Throw To True Try TypeOf Using When While With WithEvents WriteOnly"),
        ["HTML"] = S("html head body title meta link script style div span section article header footer nav main aside h1 h2 h3 h4 h5 h6 p a img ul ol li table thead tbody tr th td form input button label select option textarea iframe canvas video audio source br hr strong em code pre blockquote class id href src alt type name value placeholder charset viewport"),
        ["CSS"] = S("display flex grid block inline none position absolute relative fixed sticky top right bottom left width height min-width max-width margin padding border border-radius background background-color color font font-family font-size font-weight line-height text-align text-decoration overflow opacity transform transition animation box-shadow z-index cursor justify-content align-items gap flex-direction important hover focus active before after media"),
        ["MarkDown"] = Array.Empty<string>(),
    };

    static string[] S(string s) => s.Split(' ', StringSplitOptions.RemoveEmptyEntries).Distinct().ToArray();

    public static IEnumerable<string> For(string scheme)
    {
        if (scheme == null) return Array.Empty<string>();
        if (scheme is "ASP/XHTML") scheme = "HTML";
        return Map.TryGetValue(scheme, out var k) ? k : Array.Empty<string>();
    }
}
