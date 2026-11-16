using System.IO;
using System.Text;
using System.Text.Json;

namespace Notepad9.Services;

public class SessionTab
{
    public string Path { get; set; }
    public int UntitledNumber { get; set; }
    /// <summary>File with the unsaved text (hot exit), relative to the session folder.</summary>
    public string Backup { get; set; }
    public bool Modified { get; set; }
    public int Caret { get; set; }
    public double ScrollY { get; set; }
    public double ScrollX { get; set; }
    public FileEncoding Encoding { get; set; } = FileEncoding.Utf8;
    public Eol Eol { get; set; } = Eol.CrLf;
    public string Scheme { get; set; }
    public DateTime LastWrite { get; set; }
}

public class SessionData
{
    public List<SessionTab> Tabs { get; set; } = new();
    public int Active { get; set; }
}

/// <summary>Open tabs saved between runs, including unsaved text ("hot exit").</summary>
public static class Session
{
    public static string Folder => System.IO.Path.Combine(AppSettings.Folder, "session");
    static string FilePath => System.IO.Path.Combine(Folder, "session.json");

    public static SessionData Load()
    {
        try
        {
            if (File.Exists(FilePath))
                return JsonSerializer.Deserialize(File.ReadAllText(FilePath), N9Json.Default.SessionData) ?? new SessionData();
        }
        catch { }
        return new SessionData();
    }

    public static string ReadBackup(string name)
    {
        try
        {
            var p = System.IO.Path.Combine(Folder, name);
            return File.Exists(p) ? File.ReadAllText(p, Encoding.UTF8) : null;
        }
        catch { return null; }
    }

    /// <summary>Writes the session. <paramref name="backups"/> maps tab index → unsaved text.</summary>
    public static void Save(SessionData data, Dictionary<int, string> backups)
    {
        try
        {
            Directory.CreateDirectory(Folder);
            var keep = new HashSet<string>(StringComparer.OrdinalIgnoreCase) { "session.json" };
            foreach (var (index, text) in backups)
            {
                string name = $"backup_{index + 1}.txt";
                File.WriteAllText(System.IO.Path.Combine(Folder, name), text, new UTF8Encoding(false));
                data.Tabs[index].Backup = name;
                keep.Add(name);
            }
            string tmp = FilePath + ".tmp";
            File.WriteAllText(tmp, JsonSerializer.Serialize(data, N9Json.Default.SessionData));
            File.Move(tmp, FilePath, true);
            foreach (var f in Directory.GetFiles(Folder))
                if (!keep.Contains(System.IO.Path.GetFileName(f))) File.Delete(f);
        }
        catch { }
    }

    public static void Clear()
    {
        try { if (Directory.Exists(Folder)) Directory.Delete(Folder, true); } catch { }
    }
}
