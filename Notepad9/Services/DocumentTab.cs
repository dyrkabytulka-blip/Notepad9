using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using ICSharpCode.AvalonEdit;
using ICSharpCode.AvalonEdit.Highlighting;
using Notepad9.Editor;

namespace Notepad9.Services;

/// <summary>One open document: its own editor (with undo history), file info and tab state.</summary>
public class DocumentTab : INotifyPropertyChanged
{
    public N9Editor Editor { get; }
    public AutoCompleter Complete { get; }

    string _filePath;
    bool _isActive, _isModified;

    public DocumentTab(N9Editor editor, int untitledNumber)
    {
        Editor = editor;
        UntitledNumber = untitledNumber;
        Complete = new AutoCompleter(editor);
        DependencyPropertyDescriptor.FromProperty(TextEditor.IsModifiedProperty, typeof(TextEditor))
            .AddValueChanged(editor, (_, _) => IsModified = editor.IsModified);
    }

    public int UntitledNumber { get; set; }
    public FileEncoding Encoding { get; set; } = FileEncoding.Utf8;
    public Eol Eol { get; set; } = Eol.CrLf;
    public DateTime LastWrite { get; set; }
    public IHighlightingDefinition Scheme { get; set; }

    public string FilePath
    {
        get => _filePath;
        set
        {
            _filePath = value;
            OnChanged();
            OnChanged(nameof(Title));
            OnChanged(nameof(ToolTip));
        }
    }

    public string Title => _filePath == null
        ? (UntitledNumber <= 1 ? "Untitled" : $"Untitled {UntitledNumber}")
        : Path.GetFileName(_filePath);

    public string ToolTip => _filePath ?? Title + " (not saved yet)";

    public bool IsActive
    {
        get => _isActive;
        set { _isActive = value; OnChanged(); }
    }

    public bool IsModified
    {
        get => _isModified;
        private set { if (_isModified != value) { _isModified = value; OnChanged(); } }
    }

    /// <summary>
    /// Restored from the session but not read from disk yet (fast start): it is loaded the
    /// first time the tab is shown.
    /// </summary>
    public SessionTab Pending { get; private set; }

    public void SetPending(SessionTab st)
    {
        Pending = st;
        if (st?.Modified == true) IsModified = true;   // unsaved text waiting in the session backup
    }

    public void ClearPending() => Pending = null;

    /// <summary>Empty, never saved, unchanged: a file opened now can reuse this tab.</summary>
    public bool IsPristine => _filePath == null && !Editor.IsModified && Editor.Document.TextLength == 0;

    public event PropertyChangedEventHandler PropertyChanged;
    void OnChanged([CallerMemberName] string name = null) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
