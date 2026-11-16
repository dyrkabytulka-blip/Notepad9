using ICSharpCode.AvalonEdit.Document;
using ICSharpCode.AvalonEdit.Folding;

namespace Notepad9.Editor;

/// <summary>Folds { ... } blocks that span more than one line.</summary>
public class BraceFoldingStrategy
{
    public void UpdateFoldings(FoldingManager manager, TextDocument document)
    {
        manager.UpdateFoldings(CreateNewFoldings(document), -1);
    }

    public IEnumerable<NewFolding> CreateNewFoldings(ITextSource document)
    {
        var list = new List<NewFolding>();
        var stack = new Stack<int>();
        var text = document.Text;
        for (int i = 0; i < text.Length; i++)
        {
            char c = text[i];
            if (c == '{') stack.Push(i);
            else if (c == '}' && stack.Count > 0)
            {
                int start = stack.Pop();
                if (text.IndexOf('\n', start, i - start) >= 0)
                    list.Add(new NewFolding(start, i + 1));
            }
        }
        list.Sort((a, b) => a.StartOffset.CompareTo(b.StartOffset));
        return list;
    }
}
