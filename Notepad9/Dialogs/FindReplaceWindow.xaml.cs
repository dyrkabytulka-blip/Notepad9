using System.Windows;
using Notepad9.Services;

namespace Notepad9.Dialogs;

public partial class FindReplaceWindow
{
    readonly MainWindow _main;
    bool _replaceMode;

    public FindReplaceWindow(MainWindow main)
    {
        InitializeComponent();
        _main = main;
        Owner = main;
        Icon = AppIcon;
        IsDialog = true;
        var s = main.Settings;
        MatchCase.IsChecked = s.FindMatchCase;
        WholeWord.IsChecked = s.FindWholeWord;
        UseRegex.IsChecked = s.FindRegex;
        Wrap.IsChecked = s.FindWrap;
        FindBox.Text = s.LastFind;
        ReplaceBox.Text = s.LastReplace;
        Closed += (_, _) => Store();
    }

    public void ShowMode(bool replace, string initial)
    {
        _replaceMode = replace;
        Title = replace ? "Replace" : "Find";
        var vis = replace ? Visibility.Visible : Visibility.Collapsed;
        ReplaceLabel.Visibility = ReplaceBox.Visibility = ReplaceBtn.Visibility = ReplaceAllBtn.Visibility = InSelection.Visibility = vis;
        if (!string.IsNullOrEmpty(initial)) FindBox.Text = initial;
        Status.Text = "";
        if (!IsVisible) Show();
        Activate();
        FindBox.Focus();
        FindBox.SelectAll();
    }

    public SearchOptions Options => new()
    {
        Find = FindBox.Text,
        Replace = ReplaceBox.Text,
        MatchCase = MatchCase.IsChecked == true,
        WholeWord = WholeWord.IsChecked == true,
        Regex = UseRegex.IsChecked == true,
        Wrap = Wrap.IsChecked == true,
    };

    void Store()
    {
        var s = _main.Settings;
        s.FindMatchCase = MatchCase.IsChecked == true;
        s.FindWholeWord = WholeWord.IsChecked == true;
        s.FindRegex = UseRegex.IsChecked == true;
        s.FindWrap = Wrap.IsChecked == true;
        s.LastFind = FindBox.Text;
        s.LastReplace = ReplaceBox.Text;
    }

    void Report(string msg) => Status.Text = msg ?? "";

    void FindNext_Click(object sender, RoutedEventArgs e) { Store(); Report(Search.FindNext(_main.Editor, Options, false)); }
    void FindPrev_Click(object sender, RoutedEventArgs e) { Store(); Report(Search.FindNext(_main.Editor, Options, true)); }
    void Replace_Click(object sender, RoutedEventArgs e) { Store(); Report(Search.ReplaceCurrent(_main.Editor, Options)); }

    void ReplaceAll_Click(object sender, RoutedEventArgs e)
    {
        Store();
        int n = Search.ReplaceAll(_main.Editor, Options, InSelection.IsChecked == true, out var err);
        Report(err ?? (n == 0 ? $"\"{FindBox.Text}\" not found" : $"{n} occurrence(s) replaced"));
    }

    void Count_Click(object sender, RoutedEventArgs e)
    {
        Store();
        int n = Search.Count(_main.Editor.Text, Options);
        Report($"{n} match(es)");
    }

    void Close_Click(object sender, RoutedEventArgs e) => Close();
}
