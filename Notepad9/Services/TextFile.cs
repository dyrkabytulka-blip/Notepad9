using System.Globalization;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;

namespace Notepad9.Services;

/// <summary>Binary = every byte is one character (Latin-1), saved back byte for byte.</summary>
public enum FileEncoding { Ansi, Utf8, Utf8Bom, Utf16Le, Utf16Be, Binary }
public enum Eol { CrLf, Lf, Cr }

public static class TextFile
{
    static TextFile() => Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

    public static Encoding Ansi
    {
        get
        {
            try { return Encoding.GetEncoding(CultureInfo.CurrentCulture.TextInfo.ANSICodePage); }
            catch { return Encoding.Latin1; }
        }
    }

    public static Encoding Get(FileEncoding e) => e switch
    {
        FileEncoding.Ansi => Ansi,
        FileEncoding.Utf8Bom => new UTF8Encoding(true),
        FileEncoding.Utf16Le => new UnicodeEncoding(false, true),
        FileEncoding.Utf16Be => new UnicodeEncoding(true, true),
        FileEncoding.Binary => Encoding.Latin1,
        _ => new UTF8Encoding(false),
    };

    public static string Name(FileEncoding e) => e switch
    {
        FileEncoding.Ansi => $"ANSI ({Ansi.CodePage})",
        FileEncoding.Utf8Bom => "Unicode (UTF-8 BOM)",
        FileEncoding.Utf16Le => "Unicode (UTF-16 LE BOM)",
        FileEncoding.Utf16Be => "Unicode (UTF-16 BE BOM)",
        FileEncoding.Binary => "Binary (byte-exact)",
        _ => "Unicode (UTF-8)",
    };

    public static string Name(Eol e) => e switch { Eol.Lf => "LF", Eol.Cr => "CR", _ => "CR+LF" };
    public static string Text(Eol e) => e switch { Eol.Lf => "\n", Eol.Cr => "\r", _ => "\r\n" };

    public static (string text, FileEncoding enc, Eol eol) Read(string path)
    {
        var bytes = File.ReadAllBytes(path);
        return Decode(bytes);
    }

    public static (string text, FileEncoding enc, Eol eol) Decode(byte[] b)
    {
        FileEncoding enc;
        string text;
        if (b.Length >= 3 && b[0] == 0xEF && b[1] == 0xBB && b[2] == 0xBF)
        {
            enc = FileEncoding.Utf8Bom;
            text = new UTF8Encoding(false).GetString(b, 3, b.Length - 3);
        }
        else if (b.Length >= 2 && b[0] == 0xFF && b[1] == 0xFE)
        {
            enc = FileEncoding.Utf16Le;
            text = Encoding.Unicode.GetString(b, 2, b.Length - 2);
        }
        else if (b.Length >= 2 && b[0] == 0xFE && b[1] == 0xFF)
        {
            enc = FileEncoding.Utf16Be;
            text = Encoding.BigEndianUnicode.GetString(b, 2, b.Length - 2);
        }
        else if (LooksBinary(b))
        {
            // any format (exe, images, archives ...): one char per byte, nothing is lost on save
            return (Encoding.Latin1.GetString(b), FileEncoding.Binary, Eol.CrLf);
        }
        else
        {
            try
            {
                text = new UTF8Encoding(false, true).GetString(b);
                enc = FileEncoding.Utf8;
            }
            catch (DecoderFallbackException)
            {
                text = Ansi.GetString(b);
                enc = FileEncoding.Ansi;
            }
        }
        return (text, enc, DetectEol(text));
    }

    /// <summary>NUL bytes or lots of control characters in the first 64 KB.</summary>
    public static bool LooksBinary(byte[] b)
    {
        int n = Math.Min(b.Length, 65536), control = 0;
        for (int i = 0; i < n; i++)
        {
            byte c = b[i];
            if (c == 0) return true;
            if (c < 0x09 || (c > 0x0D && c < 0x20 && c != 0x1B)) control++;
        }
        return n > 0 && control * 20 > n;
    }

    public static Eol DetectEol(string text)
    {
        int crlf = 0, lf = 0, cr = 0;
        int limit = Math.Min(text.Length, 1_000_000);
        for (int i = 0; i < limit; i++)
        {
            char c = text[i];
            if (c == '\r')
            {
                if (i + 1 < text.Length && text[i + 1] == '\n') { crlf++; i++; }
                else cr++;
            }
            else if (c == '\n') lf++;
        }
        if (lf > crlf && lf >= cr) return Eol.Lf;
        if (cr > crlf && cr > lf) return Eol.Cr;
        return Eol.CrLf;
    }

    static readonly Regex AnyEol = new("\r\n|\r|\n", RegexOptions.Compiled);

    public static string NormalizeEol(string text, Eol eol) => AnyEol.Replace(text, Text(eol));

    public static byte[] Encode(string text, FileEncoding enc, Eol eol)
    {
        if (enc == FileEncoding.Binary) return Encoding.Latin1.GetBytes(text);   // no line-ending changes
        var e = Get(enc);
        var body = e.GetBytes(NormalizeEol(text, eol));
        var pre = e.GetPreamble();
        if (pre.Length == 0) return body;
        var all = new byte[pre.Length + body.Length];
        pre.CopyTo(all, 0);
        body.CopyTo(all, pre.Length);
        return all;
    }

    public static void Write(string path, string text, FileEncoding enc, Eol eol)
    {
        var data = Encode(text, enc, eol);
        var tmp = path + ".n9tmp";
        File.WriteAllBytes(tmp, data);
        if (File.Exists(path)) File.Replace(tmp, path, null, true);
        else File.Move(tmp, path);
    }

    public static long ByteCount(string text, FileEncoding enc, Eol eol)
    {
        if (enc == FileEncoding.Binary) return text.Length;
        var e = Get(enc);
        long n = e.GetPreamble().Length;
        // count with EOL normalization without allocating a new string
        int extraPerLine = eol == Eol.CrLf ? 2 : 1;
        int unitBytes = enc is FileEncoding.Utf16Le or FileEncoding.Utf16Be ? 2 : 0;
        int start = 0;
        long eols = 0;
        var span = text.AsSpan();
        for (int i = 0; i < span.Length; i++)
        {
            char c = span[i];
            if (c == '\r' || c == '\n')
            {
                n += unitBytes > 0 ? (i - start) * 2L : e.GetByteCount(span[start..i]);
                if (c == '\r' && i + 1 < span.Length && span[i + 1] == '\n') i++;
                eols++;
                start = i + 1;
            }
        }
        n += unitBytes > 0 ? (span.Length - start) * 2L : e.GetByteCount(span[start..]);
        n += eols * extraPerLine * (unitBytes > 0 ? 2 : 1);
        return n;
    }
}
