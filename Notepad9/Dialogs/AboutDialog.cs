using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using Notepad9.Controls;

namespace Notepad9.Dialogs;

public class AboutDialog : N9Window
{
    public AboutDialog(Window owner)
    {
        Title = "About Notepad9";
        Owner = owner;
        IsDialog = true;
        ResizeMode = ResizeMode.NoResize;
        SizeToContent = SizeToContent.WidthAndHeight;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        ShowInTaskbar = false;
        Icon = AppIcon;

        var root = new StackPanel { Margin = new Thickness(28, 18, 28, 18), Width = 360 };
        var head = new DockPanel();
        var img = new IconImage("app") { Width = 64, Height = 64, Margin = new Thickness(0, 0, 16, 0) };
        head.Children.Add(img);
        var titles = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        titles.Children.Add(new TextBlock { Text = "Notepad9", FontSize = 26, FontWeight = FontWeights.SemiBold });
        var ver = typeof(AboutDialog).Assembly.GetName().Version;
        titles.Children.Add(new TextBlock { Text = $"Version {ver?.ToString(3)}", Foreground = (Brush)FindResource("TextDim") });
        head.Children.Add(titles);
        root.Children.Add(head);
        root.Children.Add(new TextBlock
        {
            Text = "A dark, lightweight text editor.\nC# · WPF · AvalonEdit\n\nFree software under the MIT License: use, copy, modify\nand distribute it without restriction.",
            Margin = new Thickness(0, 16, 0, 0),
            TextWrapping = TextWrapping.Wrap,
            LineHeight = 19,
        });
        root.Children.Add(new TextBlock
        {
            Text = $".NET {Environment.Version} · {Environment.OSVersion.VersionString}\n" +
                   (MainWindow.StartupMs > 0 ? $"Start-up time: {MainWindow.StartupMs:0} ms" : ""),
            Margin = new Thickness(0, 10, 0, 0),
            FontSize = 11,
            Foreground = (Brush)FindResource("TextDisabled"),
        });
        var ok = new Button { Content = "OK", IsDefault = true, IsCancel = true, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 18, 0, 0) };
        ok.Click += (_, _) => Close();
        root.Children.Add(ok);
        Content = root;
    }
}

public class ShortcutsDialog : N9Window
{
    static readonly (string keys, string what)[] Items =
    {
        ("Ctrl+N / Ctrl+T", "New tab"), ("Ctrl+W / Ctrl+F4", "Close tab"), ("Ctrl+Shift+T", "Reopen closed tab"),
        ("Ctrl+Tab / Ctrl+Shift+Tab", "Next / previous tab"), ("Ctrl+1 ... Ctrl+9", "Go to tab 1 ... last"),
        ("Ctrl+Shift+N", "New window"), ("Ctrl+O", "Open"), ("Ctrl+S", "Save"), ("Ctrl+Alt+S", "Save all"),
        ("Ctrl+Alt+P", "Customize interface"),
        ("F12 / Ctrl+Shift+S", "Save As"), ("Ctrl+R", "Reload from disk"),
        ("F5", "Run current file in the console"), ("Ctrl+Backtick", "Show / hide console"),
        ("Ctrl+Space", "Complete word"), ("Ctrl+F", "Find"), ("Ctrl+H", "Replace"),
        ("F3 / Shift+F3", "Find next / previous"), ("Ctrl+Shift+F", "Find in files"), ("Ctrl+G", "Go to line"),
        ("Alt+Z", "Word wrap"), ("Ctrl+Alt+F", "Focused view"), ("Ctrl+Plus / Minus / 0", "Zoom in / out / reset"),
        ("Ctrl+D", "Duplicate line"), ("Ctrl+Shift+D", "Delete line"),
        ("Alt+Up / Alt+Down", "Move line up / down"), ("Ctrl+Shift+U / Ctrl+U", "UPPERCASE / lowercase"),
        ("Ctrl+F5", "Insert date and time"), ("Insert", "Insert / overwrite mode"), ("Alt+T", "Always on top"),
        ("Markdown: Ctrl+B / I / K", "Bold / italic / link"), ("Markdown: Ctrl+Shift+H", "Heading level"),
        ("Markdown: Ctrl+Shift+V", "Show / hide preview"), ("Markdown: Enter / Tab", "Continue / indent list"),
        ("Ctrl+Q", "Exit"),
    };

    public ShortcutsDialog(Window owner)
    {
        Title = "Keyboard Shortcuts";
        Owner = owner;
        IsDialog = true;
        ResizeMode = ResizeMode.NoResize;
        SizeToContent = SizeToContent.WidthAndHeight;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        ShowInTaskbar = false;
        Icon = AppIcon;
        var grid = new Grid { Margin = new Thickness(20, 14, 24, 10) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(220) });
        for (int i = 0; i < Items.Length; i++)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var k = new TextBlock { Text = Items[i].keys, Foreground = (Brush)FindResource("TextDim"), Margin = new Thickness(0, 2, 24, 2) };
            var w = new TextBlock { Text = Items[i].what, Margin = new Thickness(0, 2, 0, 2) };
            Grid.SetRow(k, i);
            Grid.SetRow(w, i);
            Grid.SetColumn(w, 1);
            grid.Children.Add(k);
            grid.Children.Add(w);
        }
        var panel = new StackPanel();
        panel.Children.Add(grid);
        var ok = new Button { Content = "OK", IsDefault = true, IsCancel = true, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 4, 20, 16) };
        ok.Click += (_, _) => Close();
        panel.Children.Add(ok);
        Content = panel;
    }
}
