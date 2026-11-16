using System.Windows.Markup;

namespace Notepad9.Controls;

/// <summary>XAML shortcut: Icon="{c:Ico save}" creates a fresh IconImage.</summary>
[MarkupExtensionReturnType(typeof(IconImage))]
public class IcoExtension : MarkupExtension
{
    public IcoExtension() { }
    public IcoExtension(string name) => Name = name;

    [ConstructorArgument("name")]
    public string Name { get; set; }

    public override object ProvideValue(IServiceProvider serviceProvider) => new IconImage(Name);
}
