//! Behaviour graph expressions (`hkbExpressionCondition`, `hkbEvaluateExpressionModifier`):
//! `IsNPC == 1`, `( !bBlendOutSlow ) && ( IsFirstPerson == 0 )`,
//! `turnSpeedMult = fabs(TurnDelta/112.5)`, `weaponSheathe if (iCombatStance == 0)`.
//!
//! Values are floats; comparisons and logic give 0 or 1.

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f32),
    Var(String),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

/// A parsed `hkbExpressionData` line.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// `variable = expression`
    Assign(String, Expr),
    /// `Event if (condition)`
    Raise(String, Expr),
    /// A bare condition.
    Test(Expr),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f32),
    Name(String),
    Sym(&'static str),
}

fn tokens(s: &str) -> Option<Vec<Tok>> {
    const SYMS: [&str; 16] = ["==", "!=", "<=", ">=", "&&", "||", "<", ">", "!", "=", "+", "-", "*", "/", "(", ")"];
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == ',' {
            out.push(Tok::Sym(","));
            i += 1;
            continue;
        }
        // Names may start with a digit (`1stPRot`) or contain dots (`SoundPlay.WPNBowZoomIn`);
        // a word that parses as a number is one.
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
            let w = &s[start..i];
            out.push(match w.parse::<f32>() {
                Ok(n) => Tok::Num(n),
                Err(_) => Tok::Name(w.to_owned()),
            });
            continue;
        }
        let sym = SYMS.iter().find(|p| s[i..].starts_with(**p))?;
        out.push(Tok::Sym(sym));
        i += sym.len();
    }
    Some(out)
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn eat(&mut self, sym: &'static str) -> bool {
        let hit = self.peek() == Some(&Tok::Sym(sym));
        self.at += usize::from(hit);
        hit
    }

    fn binary(&mut self, level: usize) -> Option<Expr> {
        const LEVELS: [&[(&str, Op)]; 5] = [
            &[("||", Op::Or)],
            &[("&&", Op::And)],
            &[("==", Op::Eq), ("!=", Op::Ne), ("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)],
            &[("+", Op::Add), ("-", Op::Sub)],
            &[("*", Op::Mul), ("/", Op::Div)],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut lhs = self.binary(level + 1)?;
        loop {
            let op = match self.peek() {
                Some(Tok::Sym(s)) => LEVELS[level].iter().find(|(t, _)| t == s).map(|x| x.1),
                _ => None,
            };
            let Some(op) = op else { return Some(lhs) };
            self.at += 1;
            let rhs = self.binary(level + 1)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn unary(&mut self) -> Option<Expr> {
        match self.peek()?.clone() {
            Tok::Sym("!") => {
                self.at += 1;
                Some(Expr::Not(Box::new(self.unary()?)))
            }
            Tok::Sym("-") => {
                self.at += 1;
                Some(Expr::Neg(Box::new(self.unary()?)))
            }
            Tok::Sym("(") => {
                self.at += 1;
                let e = self.binary(0)?;
                self.eat(")").then_some(e)
            }
            Tok::Num(n) => {
                self.at += 1;
                Some(Expr::Num(n))
            }
            Tok::Name(n) => {
                self.at += 1;
                if self.eat("(") {
                    let mut args = Vec::new();
                    if !self.eat(")") {
                        loop {
                            args.push(self.binary(0)?);
                            if self.eat(")") {
                                break;
                            }
                            if !self.eat(",") {
                                return None;
                            }
                        }
                    }
                    Some(Expr::Call(n.to_ascii_lowercase(), args))
                } else {
                    Some(Expr::Var(n))
                }
            }
            Tok::Sym(_) => None,
        }
    }

    fn done(&self) -> bool {
        self.at == self.toks.len()
    }
}

/// Parse a condition expression.
pub fn parse(s: &str) -> Option<Expr> {
    let mut p = Parser { toks: tokens(s)?, at: 0 };
    let e = p.binary(0)?;
    p.done().then_some(e)
}

/// Parse an expression modifier line: an assignment, an event raised on a
/// condition, or a bare condition.
pub fn parse_statement(s: &str) -> Option<Statement> {
    let toks = tokens(s)?;
    if let [Tok::Name(var), Tok::Sym("="), ..] = &toks[..] {
        let mut p = Parser { toks: toks[2..].to_vec(), at: 0 };
        let e = p.binary(0)?;
        return p.done().then(|| Statement::Assign(var.clone(), e));
    }
    if let [Tok::Name(event), Tok::Name(kw), ..] = &toks[..]
        && kw.eq_ignore_ascii_case("if")
    {
        let mut p = Parser { toks: toks[2..].to_vec(), at: 0 };
        let e = p.binary(0)?;
        return p.done().then(|| Statement::Raise(event.clone(), e));
    }
    parse(s).map(Statement::Test)
}

impl Expr {
    /// Evaluate with `var` giving variable values by name (unknown names are 0).
    pub fn eval(&self, var: &dyn Fn(&str) -> f32) -> f32 {
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        match self {
            Expr::Num(n) => *n,
            Expr::Var(v) => var(v),
            Expr::Not(e) => b(e.eval(var) == 0.0),
            Expr::Neg(e) => -e.eval(var),
            Expr::Bin(op, l, r) => {
                let x = l.eval(var);
                // Short-circuit logic.
                match op {
                    Op::And if x == 0.0 => return 0.0,
                    Op::Or if x != 0.0 => return 1.0,
                    _ => {}
                }
                let y = r.eval(var);
                match op {
                    Op::Add => x + y,
                    Op::Sub => x - y,
                    Op::Mul => x * y,
                    Op::Div => {
                        if y == 0.0 {
                            0.0
                        } else {
                            x / y
                        }
                    }
                    Op::Lt => b(x < y),
                    Op::Le => b(x <= y),
                    Op::Gt => b(x > y),
                    Op::Ge => b(x >= y),
                    Op::Eq => b(x == y),
                    Op::Ne => b(x != y),
                    Op::And | Op::Or => b(y != 0.0),
                }
            }
            Expr::Call(f, args) => {
                let a: Vec<f32> = args.iter().map(|e| e.eval(var)).collect();
                let arg = |i: usize| a.get(i).copied().unwrap_or(0.0);
                match f.as_str() {
                    "sin" => arg(0).sin(),
                    "cos" => arg(0).cos(),
                    "sind" => arg(0).to_radians().sin(),
                    "cosd" => arg(0).to_radians().cos(),
                    "fabs" | "abs" => arg(0).abs(),
                    "sqrt" => arg(0).max(0.0).sqrt(),
                    "clamp" => arg(0).clamp(arg(1).min(arg(2)), arg(2).max(arg(1))),
                    "min" => arg(0).min(arg(1)),
                    "max" => arg(0).max(arg(1)),
                    _ => 0.0,
                }
            }
        }
    }

    /// True when the expression is nonzero.
    pub fn test(&self, var: &dyn Fn(&str) -> f32) -> bool {
        self.eval(var) != 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(name: &str) -> f32 {
        match name {
            "IsNPC" => 1.0,
            "TurnDelta" => -225.0,
            "Direction" => 0.25,
            "RotMax" => 10.0,
            "Speed" => 2.0,
            "iLeftHandType" => 7.0,
            _ => 0.0,
        }
    }

    #[test]
    fn conditions() {
        let t = |s: &str| parse(s).unwrap_or_else(|| panic!("{s}")).test(&vars);
        assert!(t("IsNPC == 1"));
        assert!(!t("IsNPC == 0"));
        assert!(t("( !bBlendOutSlow ) && ( IsNPC == 1 )"));
        assert!(t("(iWantBlock == 0) || (iLeftHandType == 7) || (iLeftHandType == 12)"));
        assert!(t("(staggerDirection < .25) || (staggerDirection > .75)"));
        assert!(t("!bIsSynced && !bIsRiding"));
        assert!(!t("Speed < fMinSpeed + 1"));
    }

    #[test]
    fn statements() {
        let Some(Statement::Assign(v, e)) = parse_statement("turnSpeedMult = fabs(TurnDelta/112.5)") else { panic!() };
        assert_eq!(v, "turnSpeedMult");
        assert_eq!(e.eval(&vars), 2.0);
        let Some(Statement::Assign(v, e)) = parse_statement("1stPRot = sind( Direction * 360 ) * RotMax * clamp(Speed, 0, 1)") else { panic!() };
        assert_eq!(v, "1stPRot");
        assert!((e.eval(&vars) - 10.0).abs() < 1e-4);
        let Some(Statement::Raise(ev, c)) = parse_statement("SoundPlay.WPNBowZoomIn if (iWantBlock)") else { panic!() };
        assert_eq!(ev, "SoundPlay.WPNBowZoomIn");
        assert!(!c.test(&vars));
        assert!(matches!(parse_statement("weaponSheathe if(1)"), Some(Statement::Raise(_, _))));
        assert!(matches!(parse_statement("iCombatStance = 1 "), Some(Statement::Assign(_, Expr::Num(1.0)))));
        assert!(parse("a == (").is_none());
    }
}
