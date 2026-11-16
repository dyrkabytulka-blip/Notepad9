using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace Notepad9.Dialogs;

public partial class FontDialog
{
    // sizes are shown in points, stored in DIP (1pt = 96/72 DIP)
    static readonly double[] PtSizes = { 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 18, 20, 22, 24, 28, 32, 36, 48, 72 };
    bool _sync;

    public string FamilyName { get; private set; }
    public double SizeDip { get; private set; }
    public bool IsBold => Bold.IsChecked == true;
    public bool IsItalic => Italic.IsChecked == true;

    public FontDialog(Window owner, string family, double sizeDip, bool bold, bool italic)
    {
        InitializeComponent();
        Owner = owner;
        Icon = AppIcon;
        IsDialog = true;
        var names = Fonts.SystemFontFamilies.Select(f => f.Source).Distinct().OrderBy(n => n).ToList();
        Families.ItemsSource = names;
        Sizes.ItemsSource = PtSizes.Select(p => p.ToString(CultureInfo.InvariantCulture)).ToList();
        FamilyName = family;
        SizeDip = sizeDip;
        Bold.IsChecked = bold;
        Italic.IsChecked = italic;
        _sync = true;
        FamilyBox.Text = family;
        SizeBox.Text = Math.Round(sizeDip * 72 / 96, 1).ToString(CultureInfo.InvariantCulture);
        Families.SelectedItem = names.FirstOrDefault(n => n.Equals(family, StringComparison.OrdinalIgnoreCase));
        Sizes.SelectedItem = SizeBox.Text;
        _sync = false;
        Loaded += (_, _) =>
        {
            if (Families.SelectedItem != null) Families.ScrollIntoView(Families.SelectedItem);
            if (Sizes.SelectedItem != null) Sizes.ScrollIntoView(Sizes.SelectedItem);
            FamilyBox.Focus();
            FamilyBox.SelectAll();
        };
        UpdatePreview();
    }

    void UpdatePreview()
    {
        Preview.FontFamily = new FontFamily(FamilyName);
        Preview.FontSize = Math.Clamp(SizeDip, 6, 40);
        Preview.FontWeight = IsBold ? FontWeights.Bold : FontWeights.Normal;
        Preview.FontStyle = IsItalic ? FontStyles.Italic : FontStyles.Normal;
    }

    void Families_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_sync || Families.SelectedItem is not string s) return;
        _sync = true;
        FamilyName = s;
        FamilyBox.Text = s;
        _sync = false;
        UpdatePreview();
    }

    void FamilyBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        if (_sync) return;
        var items = (List<string>)Families.ItemsSource;
        var match = items.FirstOrDefault(n => n.StartsWith(FamilyBox.Text, StringComparison.OrdinalIgnoreCase));
        if (match == null) return;
        _sync = true;
        Families.SelectedItem = match;
        Families.ScrollIntoView(match);
        FamilyName = match;
        _sync = false;
        UpdatePreview();
    }

    void Sizes_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_sync || Sizes.SelectedItem is not string s) return;
        _sync = true;
        SizeBox.Text = s;
        SizeDip = double.Parse(s, CultureInfo.InvariantCulture) * 96 / 72;
        _sync = false;
        UpdatePreview();
    }

    void SizeBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        if (_sync) return;
        if (double.TryParse(SizeBox.Text.Replace(',', '.'), NumberStyles.Float, CultureInfo.InvariantCulture, out var pt) && pt >= 4 && pt <= 200)
        {
            SizeDip = pt * 96 / 72;
            UpdatePreview();
        }
    }

    void Style_Changed(object sender, RoutedEventArgs e)
    {
        if (Preview != null) UpdatePreview();
    }

    void Ok_Click(object sender, RoutedEventArgs e) => DialogResult = true;
}
