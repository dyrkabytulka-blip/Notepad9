//! Tiny safe calculator for the "Eval" status field: + - * / % ^, parentheses, functions.

pub fn eval(s: &str) -> Option<f64> {
    if s.trim().is_empty() || s.len() > 256 {
        return None;
    }
    let mut p = P { s: s.as_bytes(), i: 0 };
    let v = p.expr()?;
    p.ws();
    if p.i == p.s.len() && v.is_finite() { Some(v) } else { None }
}

pub fn format(v: f64) -> String {
    if (v - v.round()).abs() < 1e-12 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.10}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.i < self.s.len() && self.s[self.i] == c {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat(b'+') {
                v += self.term()?;
            } else if self.eat(b'-') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
        }
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        loop {
            if self.eat(b'*') {
                v *= self.factor()?;
            } else if self.eat(b'/') || self.eat(b':') {
                v /= self.factor()?;
            } else if self.eat(b'%') {
                v %= self.factor()?;
            } else {
                return Some(v);
            }
        }
    }
    fn factor(&mut self) -> Option<f64> {
        let b = self.unary()?;
        if self.eat(b'^') {
            return Some(b.powf(self.factor()?));
        }
        Some(b)
    }
    fn unary(&mut self) -> Option<f64> {
        if self.eat(b'-') {
            return Some(-self.unary()?);
        }
        if self.eat(b'+') {
            return self.unary();
        }
        self.primary()
    }
    fn primary(&mut self) -> Option<f64> {
        self.ws();
        if self.eat(b'(') {
            let v = self.expr()?;
            return if self.eat(b')') { Some(v) } else { None };
        }
        if self.i < self.s.len() && (self.s[self.i] as char).is_ascii_alphabetic() {
            let st = self.i;
            while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_alphanumeric() {
                self.i += 1;
            }
            let id = std::str::from_utf8(&self.s[st..self.i]).ok()?.to_lowercase();
            match id.as_str() {
                "pi" => return Some(std::f64::consts::PI),
                "e" => return Some(std::f64::consts::E),
                _ => {}
            }
            if !self.eat(b'(') {
                return None;
            }
            let a = self.expr()?;
            if !self.eat(b')') {
                return None;
            }
            return Some(match id.as_str() {
                "sqrt" => a.sqrt(),
                "abs" => a.abs(),
                "sin" => a.sin(),
                "cos" => a.cos(),
                "tan" => a.tan(),
                "ln" => a.ln(),
                "log" => a.log10(),
                "exp" => a.exp(),
                "round" => a.round(),
                "floor" => a.floor(),
                "ceil" => a.ceil(),
                _ => return None,
            });
        }
        if self.i + 1 < self.s.len() && self.s[self.i] == b'0' && (self.s[self.i + 1] | 0x20) == b'x' {
            self.i += 2;
            let st = self.i;
            while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_hexdigit() {
                self.i += 1;
            }
            return i64::from_str_radix(std::str::from_utf8(&self.s[st..self.i]).ok()?, 16).ok().map(|v| v as f64);
        }
        let st = self.i;
        while self.i < self.s.len() && ((self.s[self.i] as char).is_ascii_digit() || self.s[self.i] == b'.' || self.s[self.i] == b',') {
            self.i += 1;
        }
        if self.i < self.s.len() && (self.s[self.i] | 0x20) == b'e' && self.i > st {
            let save = self.i;
            self.i += 1;
            if self.i < self.s.len() && (self.s[self.i] == b'+' || self.s[self.i] == b'-') {
                self.i += 1;
            }
            if self.i < self.s.len() && (self.s[self.i] as char).is_ascii_digit() {
                while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_digit() {
                    self.i += 1;
                }
            } else {
                self.i = save;
            }
        }
        if self.i == st {
            return None;
        }
        std::str::from_utf8(&self.s[st..self.i]).ok()?.replace(',', ".").parse().ok()
    }
}
