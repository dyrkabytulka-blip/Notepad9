using System.Text.Json.Serialization;

namespace Notepad9.Services;

/// <summary>
/// JSON (de)serialization code generated at build time for settings, session and themes.
/// Avoids reflection, which is noticeably slower on start-up.
/// </summary>
[JsonSourceGenerationOptions(
    WriteIndented = true,
    NumberHandling = JsonNumberHandling.AllowNamedFloatingPointLiterals,
    UseStringEnumConverter = false)]
[JsonSerializable(typeof(AppSettings))]
[JsonSerializable(typeof(SessionData))]
[JsonSerializable(typeof(ThemeSettings))]
internal partial class N9Json : JsonSerializerContext
{
}
