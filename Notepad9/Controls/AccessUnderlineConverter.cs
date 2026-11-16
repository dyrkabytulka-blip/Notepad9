using System.Globalization;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;

namespace Notepad9.Controls;

/// <summary>
/// "_File" -> TextBlock with the access letter always underlined (the reference shows
/// underlined menu letters even without Alt pressed).
/// </summary>
public class AccessUnderlineConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is not string s) return value;
        var tb = new TextBlock();
        int i = s.IndexOf('_');
        if (i < 0 || i == s.Length - 1)
        {
            tb.Text = s;
            return tb;
        }
        if (i > 0) tb.Inlines.Add(new Run(s[..i]));
        tb.Inlines.Add(new Underline(new Run(s[i + 1].ToString())));
        if (i + 2 < s.Length) tb.Inlines.Add(new Run(s[(i + 2)..]));
        return tb;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => throw new NotSupportedException();
}
