using System.Collections.ObjectModel;
using System.IO;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Input;
using Microsoft.Win32;
using Notepad9.Services;

namespace Notepad9.Dialogs;

public record FileHit(string Path, int Line, int Column, string Display, string LineText);

public partial class FindInFilesWindow
{
    readonly MainWindow _main;
    readonly ObservableCollection<FileHit> _hits = new();
    CancellationTokenSource _cts;

    public FindInFilesWindow(MainWindow main)
    {
        InitializeComponent();
        _main = main;
        Owner = main;
        Icon = AppIcon;
        IsDialog = true;
        Results.ItemsSource = _hits;
        var s = main.Settings;
        FindBox.Text = main.Editor.SelectionLength is > 0 and < 200 && !main.Editor.SelectedText.Contains('\n') ? main.Editor.SelectedText : s.LastFind;
        FolderBox.Text = !string.IsNullOrEmpty(main.FilePath) ? Path.GetDirectoryName(main.FilePath)
            : (string.IsNullOrEmpty(s.LastFindFolder) ? Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments) : s.LastFindFolder);
        MaskBox.Text = s.LastFindMask;
        MatchCase.IsChecked = s.FindMatchCase;
        Loaded += (_, _) => { FindBox.Focus(); FindBox.SelectAll(); };
        Closed += (_, _) => _cts?.Cancel();
    }

    void Browse_Click(object sender, RoutedEventArgs e)
    {
        var dlg = new OpenFolderDialog { InitialDirectory = Directory.Exists(FolderBox.Text) ? FolderBox.Text : null };
        if (dlg.ShowDialog(this) == true) FolderBox.Text = dlg.FolderName;
    }

    async void Search_Click(object sender, RoutedEventArgs e)
    {
        if (_cts != null)
        {
            _cts.Cancel();
            return;
        }
        string folder = FolderBox.Text.Trim();
        if (!Directory.Exists(folder)) { Status.Text = "Folder not found."; return; }
        Regex re;
        try
        {
            re = Search.Build(new SearchOptions { Find = FindBox.Text, MatchCase = MatchCase.IsChecked == true, Regex = UseRegex.IsChecked == true });
        }
        catch (ArgumentException ex) { Status.Text = "Invalid expression: " + ex.Message; return; }
        if (re == null) { Status.Text = "Enter text to find."; return; }

        var s = _main.Settings;
        s.LastFind = FindBox.Text;
        s.LastFindFolder = folder;
        s.LastFindMask = MaskBox.Text;

        string[] masks = MaskBox.Text.Split(';', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (masks.Length == 0) masks = new[] { "*.*" };
        var opt = new EnumerationOptions { RecurseSubdirectories = Subfolders.IsChecked == true, IgnoreInaccessible = true, AttributesToSkip = FileAttributes.System };

        _hits.Clear();
        _cts = new CancellationTokenSource();
        var token = _cts.Token;
        SearchBtn.Content = "_Stop";
        int files = 0, matched = 0;
        var progress = new Progress<(FileHit[] hits, int files)>(p =>
        {
            foreach (var h in p.hits) _hits.Add(h);
            Status.Text = $"Searching... {p.files} files, {_hits.Count} matches";
        });
        try
        {
            await Task.Run(() =>
            {
                var reporter = (IProgress<(FileHit[], int)>)progress;
                foreach (var mask in masks)
                foreach (var file in Directory.EnumerateFiles(folder, mask, opt))
                {
                    token.ThrowIfCancellationRequested();
                    files++;
                    try
                    {
                        var fi = new FileInfo(file);
                        if (fi.Length > 20_000_000) continue;
                        var bytes = File.ReadAllBytes(file);
                        if (LooksBinary(bytes)) continue;
                        var (text, _, _) = TextFile.Decode(bytes);
                        var found = new List<FileHit>();
                        int lineNo = 1, lineStart = 0;
                        foreach (Match m in re.Matches(text))
                        {
                            while (true)
                            {
                                int nl = text.IndexOf('\n', lineStart);
                                if (nl < 0 || nl >= m.Index) break;
                                lineStart = nl + 1;
                                lineNo++;
                            }
                            int end = text.IndexOf('\n', lineStart);
                            if (end < 0) end = text.Length;
                            string line = text[lineStart..end].TrimEnd('\r').Trim();
                            if (line.Length > 220) line = line[..220] + "…";
                            string rel = Path.GetRelativePath(folder, file);
                            found.Add(new FileHit(file, lineNo, m.Index - lineStart + 1, $"{rel}({lineNo}): ", line));
                            if (found.Count >= 500) break;
                        }
                        if (found.Count > 0)
                        {
                            matched++;
                            reporter.Report((found.ToArray(), files));
                        }
                        else if (files % 200 == 0) reporter.Report((Array.Empty<FileHit>(), files));
                    }
                    catch (IOException) { }
                    catch (UnauthorizedAccessException) { }
                }
            }, token);
            Status.Text = $"{_hits.Count} matches in {matched} of {files} files.";
        }
        catch (OperationCanceledException) { Status.Text = $"Stopped. {_hits.Count} matches."; }
        catch (Exception ex) { Status.Text = ex.Message; }
        finally
        {
            _cts = null;
            SearchBtn.Content = "_Search";
        }
    }

    static bool LooksBinary(byte[] b)
    {
        int n = Math.Min(b.Length, 8000);
        if (n >= 2 && ((b[0] == 0xFF && b[1] == 0xFE) || (b[0] == 0xFE && b[1] == 0xFF))) return false;
        for (int i = 0; i < n; i++) if (b[i] == 0) return true;
        return false;
    }

    void OpenSelected()
    {
        if (Results.SelectedItem is FileHit h) _main.OpenFileAt(h.Path, h.Line, h.Column, FindBox.Text.Length);
    }

    void Results_DoubleClick(object sender, MouseButtonEventArgs e) => OpenSelected();

    void Results_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter) { OpenSelected(); e.Handled = true; }
    }
}
