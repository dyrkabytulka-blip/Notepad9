using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using Markdig;
using Markdig.Renderers;
using Markdig.Renderers.Html;
using Markdig.Syntax;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.Wpf;

namespace Notepad9.Controls;

/// <summary>Rendered Markdown shown next to the source (WebView2 + Markdig, dark page styled like the editor).</summary>
public class MarkdownPreview : Border
{
    const string Host = "notepad9.local";
    // built on first use, not at startup
    static readonly Lazy<MarkdownPipeline> LazyPipeline = new(() => new MarkdownPipelineBuilder()
        .UseAdvancedExtensions()
        .UseEmojiAndSmiley(enableSmileys: false)
        .Build());
    static MarkdownPipeline Pipeline => LazyPipeline.Value;

    WebView2 _web;
    Task _init;
    bool _ready, _busy, _dirty;
    string _text = "", _dir, _mappedDir;
    int _lastLine = -1;

    /// <summary>Clicked a link to a local .md file.</summary>
    public event Action<string> OpenFileRequested;

    public MarkdownPreview()
    {
        Background = new SolidColorBrush(Color.FromRgb(0x14, 0x14, 0x14));
        BorderBrush = new SolidColorBrush(Color.FromRgb(0x2A, 0x2A, 0x2A));
        BorderThickness = new Thickness(1, 0, 0, 0);
    }

    Task EnsureAsync() => _init ??= InitAsync();

    async Task InitAsync()
    {
        try
        {
            _web = new WebView2 { DefaultBackgroundColor = System.Drawing.Color.FromArgb(255, 0x14, 0x14, 0x14) };
            Child = _web;
            string data = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Notepad9", "WebView2");
            var env = await CoreWebView2Environment.CreateAsync(null, data);
            await _web.EnsureCoreWebView2Async(env);
            var core = _web.CoreWebView2;
            core.Settings.AreDevToolsEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            core.Settings.IsZoomControlEnabled = true;
            core.NavigationStarting += OnNavigationStarting;
            core.NewWindowRequested += (_, e) =>
            {
                e.Handled = true;
                OpenLink(e.Uri);
            };
            var loaded = new TaskCompletionSource();
            void Done(object s, CoreWebView2NavigationCompletedEventArgs e)
            {
                core.NavigationCompleted -= Done;
                loaded.TrySetResult();
            }
            core.NavigationCompleted += Done;
            core.NavigateToString(Template);
            await loaded.Task;
            _ready = true;
            if (_colorsJs != null) await _web.ExecuteScriptAsync(_colorsJs);
        }
        catch (Exception ex)
        {
            Child = new TextBlock
            {
                Text = "Markdown preview needs the Microsoft Edge WebView2 Runtime.\n\n" + ex.Message,
                Foreground = new SolidColorBrush(Color.FromRgb(0xC8, 0xC8, 0xC8)),
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(16),
            };
        }
    }

    void OnNavigationStarting(object sender, CoreWebView2NavigationStartingEventArgs e)
    {
        var uri = e.Uri ?? "";
        if (uri.StartsWith("data:") || uri == "about:blank") return;
        e.Cancel = true;
        OpenLink(uri);
    }

    void OpenLink(string uri)
    {
        try
        {
            if (uri.StartsWith($"https://{Host}/", StringComparison.OrdinalIgnoreCase) && _mappedDir != null)
            {
                string rel = Uri.UnescapeDataString(uri[$"https://{Host}/".Length..]).Split('#', '?')[0];
                string local = Path.GetFullPath(Path.Combine(_mappedDir, rel));
                if (File.Exists(local) && Path.GetExtension(local).ToLowerInvariant() is ".md" or ".markdown" or ".txt")
                {
                    OpenFileRequested?.Invoke(local);
                    return;
                }
                if (File.Exists(local)) uri = local;
            }
            Process.Start(new ProcessStartInfo(uri) { UseShellExecute = true });
        }
        catch { }
    }

    /// <summary>Render <paramref name="markdown"/>. Relative links and images resolve against <paramref name="baseDir"/>.</summary>
    public async void Update(string markdown, string baseDir)
    {
        _text = markdown ?? "";
        _dir = baseDir;
        if (_busy)
        {
            _dirty = true;
            return;
        }
        _busy = true;
        try
        {
            await EnsureAsync();
            if (!_ready) return;
            do
            {
                _dirty = false;
                string md = _text;
                MapFolder(_dir);
                string html = await Task.Run(() => Render(md));
                await _web.ExecuteScriptAsync("setContent(" + JsonSerializer.Serialize(html) + ")");
                if (_lastLine >= 0) await _web.ExecuteScriptAsync($"scrollToLine({_lastLine})");
            } while (_dirty);
        }
        catch { }
        finally { _busy = false; }
    }

    string _colorsJs;

    /// <summary>Use the editor theme colors in the preview page.</summary>
    public void SetColors(Color bg, Color fg, Color link)
    {
        static string Css(Color c) => $"rgba({c.R},{c.G},{c.B},{(c.A / 255.0).ToString("0.###", CultureInfo.InvariantCulture)})";
        bool light = (0.299 * bg.R + 0.587 * bg.G + 0.114 * bg.B) / 255.0 > 0.6;
        Color Mix(Color a, Color b, double t) => Color.FromRgb((byte)(a.R + (b.R - a.R) * t), (byte)(a.G + (b.G - a.G) * t), (byte)(a.B + (b.B - a.B) * t));
        var opaque = Color.FromRgb(bg.R, bg.G, bg.B);
        _colorsJs = "setColors(" + JsonSerializer.Serialize(new
        {
            bg = Css(opaque), fg = Css(fg), link = Css(link),
            code = Css(Mix(opaque, fg, light ? 0.06 : 0.08)),
            border = Css(Mix(opaque, fg, 0.18)),
            heading = Css(Mix(fg, light ? Colors.Black : Colors.White, 0.3)),
            dim = Css(Mix(fg, opaque, 0.35)),
            scheme = light ? "light" : "dark",
        }) + ")";
        Background = new SolidColorBrush(opaque);
        if (_ready) _ = _web.ExecuteScriptAsync(_colorsJs);
        if (_web != null) _web.DefaultBackgroundColor = System.Drawing.Color.FromArgb(255, bg.R, bg.G, bg.B);
    }

    /// <summary>Scroll so that source line <paramref name="line0"/> (0-based) is at the top.</summary>
    public async void ScrollToLine(int line0)
    {
        if (line0 == _lastLine) return;
        _lastLine = line0;
        if (!_ready || _busy) return;
        try { await _web.ExecuteScriptAsync($"scrollToLine({line0.ToString(CultureInfo.InvariantCulture)})"); }
        catch { }
    }

    void MapFolder(string dir)
    {
        if (string.Equals(dir, _mappedDir, StringComparison.OrdinalIgnoreCase)) return;
        try
        {
            var core = _web.CoreWebView2;
            if (_mappedDir != null) core.ClearVirtualHostNameToFolderMapping(Host);
            _mappedDir = null;
            if (!string.IsNullOrEmpty(dir) && Directory.Exists(dir))
            {
                core.SetVirtualHostNameToFolderMapping(Host, dir, CoreWebView2HostResourceAccessKind.Allow);
                _mappedDir = dir;
            }
        }
        catch { }
    }

    static string Render(string md)
    {
        var doc = Markdown.Parse(md, Pipeline);
        foreach (var block in doc.Descendants<Block>())
            block.GetAttributes().AddPropertyIfNotExist("data-line", block.Line.ToString(CultureInfo.InvariantCulture));
        using var w = new StringWriter();
        var r = new HtmlRenderer(w);
        Pipeline.Setup(r);
        r.Render(doc);
        w.Flush();
        return w.ToString();
    }

    const string Template = """
<!doctype html>
<html><head><meta charset="utf-8">
<base href="https://notepad9.local/">
<style>
  :root { color-scheme: dark; --bg:#141414; --fg:#DEDEDE; --link:#4FA3F7; --code:#222222; --border:#333333; --heading:#F2F2F2; --dim:#A0A0A0; }
  html, body { background:var(--bg); color:var(--fg); margin:0; }
  body { font: 15px/1.65 "Segoe UI", system-ui, sans-serif; padding: 18px 30px 60vh 30px; }
  #c { max-width: 920px; }
  h1, h2, h3, h4, h5, h6 { color:var(--heading); font-weight:600; line-height:1.3; margin:1.4em 0 .6em; }
  h1 { font-size:1.9em; border-bottom:1px solid var(--border); padding-bottom:.3em; }
  h2 { font-size:1.5em; border-bottom:1px solid var(--border); padding-bottom:.25em; }
  h3 { font-size:1.25em; }
  #c > :first-child { margin-top: 0; }
  p, ul, ol, blockquote, table, pre { margin: 0 0 1em; }
  a { color:var(--link); text-decoration:none; }
  a:hover { text-decoration:underline; }
  strong { color:var(--heading); }
  code { font-family: Consolas, "Cascadia Mono", monospace; font-size:.9em; background:var(--code); padding:.15em .4em; border-radius:4px; }
  pre { background:var(--code); border:1px solid var(--border); border-radius:6px; padding:12px 14px; overflow:auto; }
  pre code { background:none; padding:0; font-size:.88em; line-height:1.5; }
  blockquote { border-left:4px solid var(--link); background:var(--code); color:var(--dim); padding:.5em 1em; border-radius:0 4px 4px 0; }
  blockquote > :last-child { margin-bottom:0; }
  hr { border:0; border-top:1px solid var(--border); margin:1.6em 0; }
  table { border-collapse:collapse; }
  th, td { border:1px solid var(--border); padding:6px 12px; }
  th { background:var(--code); color:var(--heading); }
  img { max-width:100%; border-radius:4px; }
  ul.contains-task-list, .task-list-item { list-style:none; }
  .task-list-item input { margin:0 .45em 0 -1.3em; accent-color:var(--link); }
  ::selection { background:rgba(38,79,120,.8); }
  ::-webkit-scrollbar { width:13px; height:13px; background:var(--bg); }
  ::-webkit-scrollbar-thumb { background:var(--border); border-radius:6px; border:3px solid var(--bg); }
</style>
<script>
  function setContent(h) { document.getElementById('c').innerHTML = h; }
  function setColors(c) { var r = document.documentElement.style; for (var k in c) { if (k === 'scheme') document.documentElement.style.colorScheme = c[k]; else r.setProperty('--' + k, c[k]); } }
  function scrollToLine(n) {
    var els = document.querySelectorAll('#c [data-line]'), a = null, b = null;
    for (var i = 0; i < els.length; i++) {
      var l = +els[i].getAttribute('data-line');
      if (l <= n) a = els[i]; else { b = els[i]; break; }
    }
    if (!a) { window.scrollTo(0, 0); return; }
    var top = function (e) { return e.getBoundingClientRect().top + window.scrollY; };
    var la = +a.getAttribute('data-line'), y = top(a);
    if (b) { var lb = +b.getAttribute('data-line'); if (lb > la) y += (top(b) - y) * (n - la) / (lb - la); }
    window.scrollTo(0, Math.max(0, y - 12));
  }
</script>
</head><body><div id="c"></div></body></html>
""";
}
