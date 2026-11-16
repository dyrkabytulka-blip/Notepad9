using Microsoft.Win32;

namespace Notepad9.Services;

/// <summary>"Start with Windows": a Run entry that starts Notepad9 hidden (/background) at sign-in.</summary>
public static class AutoStart
{
    const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    const string Name = "Notepad9";

    public static bool IsEnabled
    {
        get
        {
            try
            {
                using var cu = Registry.CurrentUser.OpenSubKey(RunKey);
                if (cu?.GetValue(Name) != null) return true;
                using var lm = Registry.LocalMachine.OpenSubKey(RunKey);
                return lm?.GetValue(Name) != null;
            }
            catch { return false; }
        }
    }

    public static void Set(bool on)
    {
        try
        {
            using var k = Registry.CurrentUser.CreateSubKey(RunKey);
            if (on) k.SetValue(Name, $"\"{Environment.ProcessPath}\" /background");
            else k.DeleteValue(Name, false);
        }
        catch { }
        if (!on)
        {
            // an all-users install puts the entry in HKLM; removing it needs admin rights
            try
            {
                using var lm = Registry.LocalMachine.OpenSubKey(RunKey, true);
                lm?.DeleteValue(Name, false);
            }
            catch { }
        }
    }
}
