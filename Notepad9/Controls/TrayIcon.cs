using System.Drawing;
using Notepad9.Services;
using WinForms = System.Windows.Forms;

namespace Notepad9.Controls;

/// <summary>Notification-area icon shown while Notepad9 waits in the background.</summary>
public sealed class TrayIcon : IDisposable
{
    readonly WinForms.NotifyIcon _icon;

    public TrayIcon(MainWindow window)
    {
        var menu = new WinForms.ContextMenuStrip
        {
            Renderer = new WinForms.ToolStripProfessionalRenderer(new DarkColors()),
            BackColor = Color.FromArgb(0x2B, 0x2B, 0x2B),
            ForeColor = Color.FromArgb(0xDE, 0xDE, 0xDE),
            ShowImageMargin = false,
            ShowCheckMargin = true,
            Font = new Font("Segoe UI", 9f),
        };
        var open = new WinForms.ToolStripMenuItem("Open Notepad9") { Font = new Font("Segoe UI", 9f, FontStyle.Bold) };
        open.Click += (_, _) => window.ShowFromBackground();
        var newTab = new WinForms.ToolStripMenuItem("New Tab");
        newTab.Click += (_, _) => window.NewTabFromTray();
        var autostart = new WinForms.ToolStripMenuItem("Start with Windows") { CheckOnClick = true };
        autostart.Click += (_, _) => AutoStart.Set(autostart.Checked);
        var quit = new WinForms.ToolStripMenuItem("Quit Notepad9");
        quit.Click += (_, _) => window.QuitCompletely();
        menu.Items.AddRange(new WinForms.ToolStripItem[] { open, newTab, new WinForms.ToolStripSeparator(), autostart, new WinForms.ToolStripSeparator(), quit });
        menu.Opening += (_, _) => autostart.Checked = AutoStart.IsEnabled;

        _icon = new WinForms.NotifyIcon
        {
            Text = "Notepad9 (ready in the background)",
            Icon = LoadIcon(),
            ContextMenuStrip = menu,
            Visible = false,
        };
        _icon.MouseClick += (_, e) => { if (e.Button == WinForms.MouseButtons.Left) window.ShowFromBackground(); };
    }

    public bool Visible
    {
        get => _icon.Visible;
        set => _icon.Visible = value;
    }

    static Icon LoadIcon()
    {
        try
        {
            var res = System.Windows.Application.GetResourceStream(new Uri("pack://application:,,,/Assets/app.ico"));
            if (res != null)
            {
                using var s = res.Stream;
                return new Icon(s, WinForms.SystemInformation.SmallIconSize);
            }
        }
        catch { }
        return SystemIcons.Application;
    }

    public void Dispose()
    {
        _icon.Visible = false;
        _icon.ContextMenuStrip?.Dispose();
        _icon.Dispose();
    }

    sealed class DarkColors : WinForms.ProfessionalColorTable
    {
        static readonly Color Back = Color.FromArgb(0x2B, 0x2B, 0x2B);
        static readonly Color Hover = Color.FromArgb(0x41, 0x41, 0x41);
        static readonly Color Line = Color.FromArgb(0x4F, 0x4F, 0x4F);
        public override Color ToolStripDropDownBackground => Back;
        public override Color MenuBorder => Line;
        public override Color MenuItemBorder => Hover;
        public override Color MenuItemSelected => Hover;
        public override Color MenuItemSelectedGradientBegin => Hover;
        public override Color MenuItemSelectedGradientEnd => Hover;
        public override Color ImageMarginGradientBegin => Back;
        public override Color ImageMarginGradientMiddle => Back;
        public override Color ImageMarginGradientEnd => Back;
        public override Color SeparatorDark => Line;
        public override Color SeparatorLight => Back;
        public override Color CheckBackground => Color.FromArgb(0x2F, 0x6F, 0xB5);
        public override Color CheckSelectedBackground => Color.FromArgb(0x3A, 0x80, 0xCC);
        public override Color CheckPressedBackground => Color.FromArgb(0x3A, 0x80, 0xCC);
    }
}
