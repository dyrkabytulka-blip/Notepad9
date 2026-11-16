using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using Notepad9.Controls;

namespace Notepad9.Dialogs;

/// <summary>Dark replacement for MessageBox.</summary>
public class MessageDialog : N9Window
{
    public MessageBoxResult Result { get; private set; } = MessageBoxResult.Cancel;

    MessageDialog(Window owner, string title, string text, string icon, (string label, MessageBoxResult r, bool isDefault, bool isCancel)[] buttons)
    {
        Title = title;
        Owner = owner;
        IsDialog = true;
        ResizeMode = ResizeMode.NoResize;
        SizeToContent = SizeToContent.WidthAndHeight;
        WindowStartupLocation = owner != null ? WindowStartupLocation.CenterOwner : WindowStartupLocation.CenterScreen;
        ShowInTaskbar = owner == null;
        Icon = AppIcon;

        var root = new DockPanel { Margin = new Thickness(0), MinWidth = 340, MaxWidth = 560 };
        var bar = new Border
        {
            Background = (System.Windows.Media.Brush)FindResource("PopupBg"),
            BorderBrush = (System.Windows.Media.Brush)FindResource("Line"),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(16, 10, 12, 10),
        };
        DockPanel.SetDock(bar, Dock.Bottom);
        var buttonsPanel = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right };
        foreach (var b in buttons)
        {
            var btn = new Button { Content = b.label, IsDefault = b.isDefault, IsCancel = b.isCancel, Margin = new Thickness(8, 0, 0, 0) };
            var r = b.r;
            btn.Click += (_, _) => { Result = r; DialogResult = true; };
            buttonsPanel.Children.Add(btn);
        }
        bar.Child = buttonsPanel;
        root.Children.Add(bar);

        var body = new DockPanel { Margin = new Thickness(18, 16, 22, 18) };
        if (icon != null)
        {
            var img = new IconImage(icon) { Width = 32, Height = 32, VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 0, 14, 0) };
            DockPanel.SetDock(img, Dock.Left);
            body.Children.Add(img);
        }
        body.Children.Add(new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap, VerticalAlignment = VerticalAlignment.Center, LineHeight = 18 });
        root.Children.Add(body);
        Content = root;
        PreviewKeyDown += (_, e) =>
        {
            if (e.Key == Key.Escape && buttons.Any(x => x.isCancel)) { Result = MessageBoxResult.Cancel; DialogResult = false; }
        };
    }

    public static MessageBoxResult AskSave(Window owner, string fileName) =>
        Show(owner, "Notepad9", $"Save changes to \"{fileName}\"?", "save",
            ("_Save", MessageBoxResult.Yes, true, false),
            ("Do_n't Save", MessageBoxResult.No, false, false),
            ("Cancel", MessageBoxResult.Cancel, false, true));

    public static bool Confirm(Window owner, string text, string icon = "about") =>
        Show(owner, "Notepad9", text, icon,
            ("_Yes", MessageBoxResult.Yes, true, false),
            ("_No", MessageBoxResult.No, false, true)) == MessageBoxResult.Yes;

    public static void Info(Window owner, string text, string icon = "about") =>
        Show(owner, "Notepad9", text, icon, ("OK", MessageBoxResult.OK, true, true));

    public static MessageBoxResult Show(Window owner, string title, string text, string icon,
        params (string label, MessageBoxResult r, bool isDefault, bool isCancel)[] buttons)
    {
        var d = new MessageDialog(owner, title, text, icon, buttons);
        d.ShowDialog();
        return d.Result;
    }
}

/// <summary>Single-field input dialog (Go To line, long line column ...).</summary>
public class InputDialog : N9Window
{
    readonly TextBox _box;
    public string Value => _box.Text;

    public InputDialog(Window owner, string title, string label, string value, string hint = null)
    {
        Title = title;
        Owner = owner;
        IsDialog = true;
        ResizeMode = ResizeMode.NoResize;
        SizeToContent = SizeToContent.WidthAndHeight;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        ShowInTaskbar = false;
        Icon = AppIcon;

        var root = new StackPanel { Margin = new Thickness(18, 14, 18, 16), Width = 320 };
        root.Children.Add(new TextBlock { Text = label, Margin = new Thickness(0, 0, 0, 6), Foreground = (System.Windows.Media.Brush)FindResource("TextDim") });
        _box = new TextBox { Text = value ?? "" };
        root.Children.Add(_box);
        if (hint != null)
            root.Children.Add(new TextBlock { Text = hint, Margin = new Thickness(0, 6, 0, 0), FontSize = 11, Foreground = (System.Windows.Media.Brush)FindResource("TextDisabled") });
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 16, 0, 0) };
        var ok = new Button { Content = "OK", IsDefault = true };
        ok.Click += (_, _) => DialogResult = true;
        var cancel = new Button { Content = "Cancel", IsCancel = true, Margin = new Thickness(8, 0, 0, 0) };
        buttons.Children.Add(ok);
        buttons.Children.Add(cancel);
        root.Children.Add(buttons);
        Content = root;
        Loaded += (_, _) => { _box.Focus(); _box.SelectAll(); };
    }
}
