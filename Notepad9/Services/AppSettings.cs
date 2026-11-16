using System.IO;
using System.Text.Json;

namespace Notepad9.Services;

public class AppSettings
{
    public double Left { get; set; } = double.NaN;
    public double Top { get; set; } = double.NaN;
    public double Width { get; set; } = 1100;
    public double Height { get; set; } = 700;
    public bool Maximized { get; set; }

    public string FontFamily { get; set; } = "Consolas";
    public double FontSize { get; set; } = 14.667;
    public bool FontBold { get; set; }
    public bool FontItalic { get; set; }
    public string SecondFontFamily { get; set; } = "Segoe UI";
    public bool UseSecondFont { get; set; }
    public double Zoom { get; set; } = 1.0;

    public bool WordWrap { get; set; }
    public bool LineNumbers { get; set; } = true;
    public bool ShowWhitespace { get; set; }
    public bool ShowEol { get; set; }
    // not persisted: the marker is off on every start
    [System.Text.Json.Serialization.JsonIgnore]
    public bool LongLineMarker { get; set; }
    public int LongLineColumn { get; set; } = 80;
    public bool HighlightCurrentLine { get; set; } = true;
    public bool MarkOccurrences { get; set; } = true;
    public bool CodeFolding { get; set; } = true;
    public bool ShowToolbar { get; set; } = true;
    public bool ShowStatusBar { get; set; } = true;
    // not persisted: a window that silently starts topmost is confusing
    [System.Text.Json.Serialization.JsonIgnore]
    public bool AlwaysOnTop { get; set; }

    public double ConsoleHeight { get; set; } = 220;
    public bool MdPreview { get; set; } = true;
    public bool MdSyncScroll { get; set; } = true;
    public bool AutoComplete { get; set; } = true;
    public bool RestoreSession { get; set; } = true;
    /// <summary>Closing the window keeps Notepad9 ready in the background (tray icon), so it opens instantly.</summary>
    public bool InstantStart { get; set; } = true;
    /// <summary>Keep unsaved text between runs instead of asking to save on exit.</summary>
    public bool HotExit { get; set; } = true;
    public ThemeSettings Theme { get; set; } = new();

    public int TabWidth { get; set; } = 4;
    public bool TabsAsSpaces { get; set; }
    public bool AutoIndent { get; set; } = true;
    public bool AutoDetectScheme { get; set; } = true;
    public bool DetectFileChanges { get; set; } = true;
    public bool RememberRecent { get; set; } = true;
    public bool SaveOnExit { get; set; } = true;

    public bool FindMatchCase { get; set; }
    public bool FindWholeWord { get; set; }
    public bool FindRegex { get; set; }
    public bool FindWrap { get; set; } = true;
    public string LastFind { get; set; } = "";
    public string LastReplace { get; set; } = "";
    public string LastFindFolder { get; set; } = "";
    public string LastFindMask { get; set; } = "*.*";

    public List<string> Recent { get; set; } = new();
    public List<string> Favorites { get; set; } = new();

    // -------------------------------------------------------------- persistence
    public static string Folder => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Notepad9");
    static string FilePath => Path.Combine(Folder, "settings.json");

    public static AppSettings Load()
    {
        try
        {
            if (File.Exists(FilePath))
                return JsonSerializer.Deserialize(File.ReadAllText(FilePath), N9Json.Default.AppSettings) ?? new AppSettings();
        }
        catch { }
        return new AppSettings();
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(Folder);
            File.WriteAllText(FilePath, JsonSerializer.Serialize(this, N9Json.Default.AppSettings));
        }
        catch { }
    }

    public static void Reset()
    {
        try { if (File.Exists(FilePath)) File.Delete(FilePath); } catch { }
    }

    public void AddRecent(string path)
    {
        if (!RememberRecent || string.IsNullOrEmpty(path)) return;
        Recent.RemoveAll(p => string.Equals(p, path, StringComparison.OrdinalIgnoreCase));
        Recent.Insert(0, path);
        if (Recent.Count > 16) Recent.RemoveRange(16, Recent.Count - 16);
    }
}
