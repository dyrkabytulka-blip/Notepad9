using System.Globalization;

namespace Notepad9.Services;

/// <summary>Tiny safe calculator used by the "Eval" status field: + - * / % ^, parentheses, functions.</summary>
public static class ExprEvaluator
{
    public static bool TryEval(string s, out double value)
    {
        value = 0;
        if (string.IsNullOrWhiteSpace(s) || s.Length > 256) return false;
        try
        {
            var p = new Parser(s);
            value = p.ParseExpr();
            p.SkipWs();
            return p.AtEnd && !double.IsNaN(value) && !double.IsInfinity(value);
        }
        catch { return false; }
    }

    public static string Format(double v)
    {
        if (Math.Abs(v - Math.Round(v)) < 1e-12 && Math.Abs(v) < 1e15) return Math.Round(v).ToString("0", CultureInfo.InvariantCulture);
        return v.ToString("0.##########", CultureInfo.InvariantCulture);
    }

    sealed class Parser
    {
        readonly string _s;
        int _i;
        public Parser(string s) => _s = s;
        public bool AtEnd => _i >= _s.Length;

        public void SkipWs() { while (_i < _s.Length && char.IsWhiteSpace(_s[_i])) _i++; }

        bool Eat(char c)
        {
            SkipWs();
            if (_i < _s.Length && _s[_i] == c) { _i++; return true; }
            return false;
        }

        public double ParseExpr()
        {
            double v = ParseTerm();
            while (true)
            {
                if (Eat('+')) v += ParseTerm();
                else if (Eat('-')) v -= ParseTerm();
                else return v;
            }
        }

        double ParseTerm()
        {
            double v = ParseFactor();
            while (true)
            {
                if (Eat('*') || Eat('×')) v *= ParseFactor();
                else if (Eat('/') || Eat('÷') || Eat(':')) v /= ParseFactor();
                else if (Eat('%')) v %= ParseFactor();
                else return v;
            }
        }

        double ParseFactor()
        {
            double b = ParseUnary();
            if (Eat('^')) return Math.Pow(b, ParseFactor());
            return b;
        }

        double ParseUnary()
        {
            if (Eat('-')) return -ParseUnary();
            if (Eat('+')) return ParseUnary();
            return ParsePrimary();
        }

        double ParsePrimary()
        {
            SkipWs();
            if (Eat('('))
            {
                double v = ParseExpr();
                if (!Eat(')')) throw new FormatException();
                return v;
            }
            if (_i < _s.Length && char.IsLetter(_s[_i]))
            {
                int st = _i;
                while (_i < _s.Length && char.IsLetterOrDigit(_s[_i])) _i++;
                string id = _s[st.._i].ToLowerInvariant();
                switch (id)
                {
                    case "pi": return Math.PI;
                    case "e": return Math.E;
                }
                if (!Eat('(')) throw new FormatException();
                double a = ParseExpr();
                if (!Eat(')')) throw new FormatException();
                return id switch
                {
                    "sqrt" => Math.Sqrt(a), "abs" => Math.Abs(a), "sin" => Math.Sin(a), "cos" => Math.Cos(a),
                    "tan" => Math.Tan(a), "ln" => Math.Log(a), "log" => Math.Log10(a), "exp" => Math.Exp(a),
                    "round" => Math.Round(a), "floor" => Math.Floor(a), "ceil" => Math.Ceiling(a),
                    _ => throw new FormatException(),
                };
            }
            if (_i + 1 < _s.Length && _s[_i] == '0' && (_s[_i + 1] == 'x' || _s[_i + 1] == 'X'))
            {
                _i += 2;
                int st = _i;
                while (_i < _s.Length && Uri.IsHexDigit(_s[_i])) _i++;
                return long.Parse(_s[st.._i], NumberStyles.HexNumber, CultureInfo.InvariantCulture);
            }
            int start = _i;
            while (_i < _s.Length && (char.IsDigit(_s[_i]) || _s[_i] == '.' || _s[_i] == ',')) _i++;
            if (_i < _s.Length && (_s[_i] == 'e' || _s[_i] == 'E') && _i > start)
            {
                int save = _i;
                _i++;
                if (_i < _s.Length && (_s[_i] == '+' || _s[_i] == '-')) _i++;
                if (_i < _s.Length && char.IsDigit(_s[_i])) { while (_i < _s.Length && char.IsDigit(_s[_i])) _i++; }
                else _i = save;
            }
            if (_i == start) throw new FormatException();
            return double.Parse(_s[start.._i].Replace(',', '.'), NumberStyles.Float, CultureInfo.InvariantCulture);
        }
    }
}
