//! A compiler for a subset of Papyrus, for test mods: source text to
//! [`ScriptBuilder`]s, so tests read as scripts rather than instruction by
//! instruction. What it takes:
//!
//! - `Scriptname Name extends Parent`; `Type Property Name Auto`; script
//!   variables `Type Name` or `Type Name = literal`;
//! - `[Type] Function Name(Type a, Type b = literal) [Global]` ...
//!   `EndFunction`, and `Event Name(...)` ... `EndEvent`;
//! - statements: `Type name [= expr]`, `lvalue = expr` (and `+=`, `-=`,
//!   `*=`, `/=`), a call, `If` / `ElseIf` / `Else` / `EndIf`,
//!   `While` / `EndWhile`, `Return [expr]`;
//! - expressions: literals (`0x` hex too), names, `self`, calls (own
//!   functions, `obj.Method(...)`, `Class.Global(...)`), `as` casts,
//!   arrays (`new Int[n]`, `a[i]`, `a.Length`, `a.Find(v)`), arithmetic,
//!   comparisons, `!`, `&&` / `||` (short-circuit), string concatenation
//!   with `+` (other operands cast to strings).
//!
//! Comments: `;` to the end of the line, `{ ... }` blocks. A `\` at the end
//! of a line continues it.
//!
//! Types follow the declarations a [`Signatures`] gives for classes outside
//! the scripts compiled together. Arguments and assigned values are cast to
//! the type they go to, as the Creation Kit's compiler casts them (the game
//! refuses a native call given an object of a subclass). One addition: when
//! a script defines `String Num(Float f)`, floats concatenated into strings
//! go through it rather than a cast's six decimals.

use std::collections::HashMap;

use crate::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, discard, id};
use crate::pex::op;

/// A function's declaration.
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub ret: String,
    /// Name, type, default value.
    pub params: Vec<(String, String, Option<Arg>)>,
    pub global: bool,
}

/// What the compiler knows of classes it doesn't compile: their parents and
/// functions.
pub trait Signatures {
    /// `class`'s parent, `Some("")` for a root, `None` for an unknown class.
    fn parent(&self, class: &str) -> Option<String>;
    /// `class`'s own function `name` (not its parents').
    fn function(&self, class: &str, name: &str) -> Option<Signature>;
}

/// A compile error, with the script and line.
#[derive(Debug, thiserror::Error)]
#[error("{script}:{line}: {message}")]
pub struct CompileError {
    pub script: String,
    pub line: usize,
    pub message: String,
}

type R<T> = std::result::Result<T, CompileError>;

// --- Lexing -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Int(i32),
    Float(f32),
    Str(String),
    Sym(&'static str),
    Newline,
}

const SYMS: [&str; 23] = [
    "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "+", "-", "*", "/", "%", "<", ">",
    "!", "=", "(", ")", "[", "]",
];

fn lex(src: &str) -> std::result::Result<Vec<(Tok, usize)>, (usize, String)> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                out.push((Tok::Newline, line));
                line += 1;
                i += 1;
            }
            ' ' | '\t' | '\r' => i += 1,
            ';' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '{' => {
                while i < chars.len() && chars[i] != '}' {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            '\\' => {
                // A continued line.
                i += 1;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                i += 1;
                line += 1;
            }
            ',' => {
                out.push((Tok::Sym(","), line));
                i += 1;
            }
            '.' if !chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()) => {
                out.push((Tok::Sym("."), line));
                i += 1;
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None | Some('\n') => return Err((line, "unterminated string".into())),
                        Some('"') => break,
                        Some('\\') => {
                            i += 1;
                            s.push(match chars.get(i) {
                                Some('n') => '\n',
                                Some('t') => '\t',
                                Some(&other) => other,
                                None => return Err((line, "unterminated string".into())),
                            });
                        }
                        Some(&ch) => s.push(ch),
                    }
                    i += 1;
                }
                i += 1;
                out.push((Tok::Str(s), line));
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                if c == '0' && matches!(chars.get(i + 1), Some('x' | 'X')) {
                    i += 2;
                    while i < chars.len() && chars[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                    let text: String = chars[start + 2..i].iter().collect();
                    let v = u32::from_str_radix(&text, 16).map_err(|e| (line, e.to_string()))?;
                    out.push((Tok::Int(v as i32), line));
                    continue;
                }
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                if text.contains('.') {
                    out.push((
                        Tok::Float(text.parse().map_err(|_| (line, text.clone()))?),
                        line,
                    ));
                } else {
                    out.push((
                        Tok::Int(text.parse().map_err(|_| (line, text.clone()))?),
                        line,
                    ));
                }
            }
            c if c.is_alphabetic() || c == '_' || c == ':' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == ':')
                {
                    i += 1;
                }
                out.push((Tok::Ident(chars[start..i].iter().collect()), line));
            }
            _ => {
                let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
                let Some(sym) = SYMS.iter().find(|s| rest.starts_with(**s)) else {
                    return Err((line, format!("unexpected {c:?}")));
                };
                out.push((Tok::Sym(sym), line));
                i += sym.len();
            }
        }
    }
    out.push((Tok::Newline, line));
    Ok(out)
}

// --- Syntax -----------------------------------------------------------

#[derive(Debug, Clone)]
enum Expr {
    Lit(Arg),
    Name(String),
    SelfRef,
    Call(Option<Box<Expr>>, String, Vec<Expr>),
    Unary(&'static str, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Cast(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Length(Box<Expr>),
    NewArray(String, Box<Expr>),
}

#[derive(Debug, Clone)]
enum Stmt {
    Local(usize, String, String, Option<Expr>),
    Assign(usize, Expr, Option<&'static str>, Expr),
    Expr(usize, Expr),
    If(usize, Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    While(usize, Expr, Vec<Stmt>),
    Return(usize, Option<Expr>),
}

#[derive(Debug, Clone)]
struct Func {
    line: usize,
    name: String,
    ret: String,
    params: Vec<(String, String, Option<Arg>)>,
    global: bool,
    body: Vec<Stmt>,
}

/// A parsed script.
#[derive(Debug, Clone)]
pub struct Script {
    name: String,
    parent: String,
    properties: Vec<(String, String)>,
    variables: Vec<(String, String, Arg)>,
    functions: Vec<Func>,
}

impl Script {
    pub fn name(&self) -> &str {
        &self.name
    }
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    pos: usize,
    script: String,
}

fn kw(t: &Tok, word: &str) -> bool {
    matches!(t, Tok::Ident(s) if s.eq_ignore_ascii_case(word))
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].0
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].0
    }
    fn line(&self) -> usize {
        self.toks[self.pos.min(self.toks.len() - 1)].1
    }
    fn err<T>(&self, message: impl Into<String>) -> R<T> {
        Err(CompileError {
            script: self.script.clone(),
            line: self.line(),
            message: message.into(),
        })
    }
    fn next(&mut self) -> Tok {
        let t = self.peek().clone();
        self.pos += 1;
        t
    }
    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }
    fn skip_newlines(&mut self) {
        while !self.at_end() && *self.peek() == Tok::Newline {
            self.pos += 1;
        }
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if matches!(self.peek(), Tok::Sym(x) if *x == s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str) -> R<()> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            self.err(format!("expected {s:?}, found {:?}", self.peek()))
        }
    }
    fn eat_kw(&mut self, word: &str) -> bool {
        if kw(self.peek(), word) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn ident(&mut self) -> R<String> {
        match self.next() {
            Tok::Ident(s) => Ok(s),
            t => {
                self.pos -= 1;
                self.err(format!("expected a name, found {t:?}"))
            }
        }
    }
    fn end_line(&mut self) -> R<()> {
        match self.next() {
            Tok::Newline => Ok(()),
            t => {
                self.pos -= 1;
                self.err(format!("expected the end of the line, found {t:?}"))
            }
        }
    }
    /// A type: a name, optionally `[]`.
    fn ty(&mut self) -> R<String> {
        let mut t = self.ident()?;
        if matches!(self.peek(), Tok::Sym("[")) && matches!(self.peek_at(1), Tok::Sym("]")) {
            self.pos += 2;
            t.push_str("[]");
        }
        Ok(t)
    }
    fn literal(&mut self) -> R<Arg> {
        let negative = self.eat_sym("-");
        Ok(match self.next() {
            Tok::Int(v) => Arg::Int(if negative { -v } else { v }),
            Tok::Float(v) => Arg::Float(if negative { -v } else { v }),
            Tok::Str(s) if !negative => Arg::Str(s),
            Tok::Ident(s) if s.eq_ignore_ascii_case("true") => Arg::Bool(true),
            Tok::Ident(s) if s.eq_ignore_ascii_case("false") => Arg::Bool(false),
            Tok::Ident(s) if s.eq_ignore_ascii_case("none") => Arg::None,
            t => {
                self.pos -= 1;
                return self.err(format!("expected a literal, found {t:?}"));
            }
        })
    }

    fn script(&mut self) -> R<Script> {
        self.skip_newlines();
        if !self.eat_kw("scriptname") {
            return self.err("expected Scriptname");
        }
        let name = self.ident()?;
        let parent = if self.eat_kw("extends") {
            self.ident()?
        } else {
            String::new()
        };
        while !matches!(self.peek(), Tok::Newline) {
            self.next();
        }
        self.script = name.clone();
        let mut s = Script {
            name,
            parent,
            properties: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
        };
        loop {
            self.skip_newlines();
            if self.at_end() {
                break;
            }
            let line = self.line();
            if self.eat_kw("event") {
                let f = self.function(line, "None".into(), "endevent")?;
                s.functions.push(f);
                continue;
            }
            if self.eat_kw("function") {
                let f = self.function(line, "None".into(), "endfunction")?;
                s.functions.push(f);
                continue;
            }
            let ty = self.ty()?;
            if self.eat_kw("function") {
                let f = self.function(line, ty, "endfunction")?;
                s.functions.push(f);
                continue;
            }
            if self.eat_kw("property") {
                let name = self.ident()?;
                if !self.eat_kw("auto") {
                    return self.err("only Auto properties are supported");
                }
                s.properties.push((name, ty));
                self.end_line()?;
                continue;
            }
            let name = self.ident()?;
            let init = if self.eat_sym("=") {
                self.literal()?
            } else {
                Arg::None
            };
            s.variables.push((name, ty, init));
            self.end_line()?;
        }
        Ok(s)
    }

    fn function(&mut self, line: usize, ret: String, end: &str) -> R<Func> {
        let name = self.ident()?;
        self.expect_sym("(")?;
        let mut params = Vec::new();
        if !self.eat_sym(")") {
            loop {
                let ty = self.ty()?;
                let pname = self.ident()?;
                let default = if self.eat_sym("=") {
                    Some(self.literal()?)
                } else {
                    None
                };
                params.push((pname, ty, default));
                if self.eat_sym(")") {
                    break;
                }
                self.expect_sym(",")?;
            }
        }
        let global = self.eat_kw("global");
        self.end_line()?;
        let body = self.block(&[end])?;
        self.next();
        Ok(Func {
            line,
            name,
            ret,
            params,
            global,
            body,
        })
    }

    /// Statements up to (not past) one of `ends`.
    fn block(&mut self, ends: &[&str]) -> R<Vec<Stmt>> {
        let mut out = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_end() {
                return self.err(format!("expected {}", ends[0]));
            }
            if ends.iter().any(|e| kw(self.peek(), e)) {
                return Ok(out);
            }
            out.push(self.statement()?);
        }
    }

    fn statement(&mut self) -> R<Stmt> {
        let line = self.line();
        if self.eat_kw("if") {
            let mut arms = Vec::new();
            let mut cond = self.expr()?;
            self.end_line()?;
            loop {
                let body = self.block(&["elseif", "else", "endif"])?;
                arms.push((cond, body));
                if self.eat_kw("elseif") {
                    cond = self.expr()?;
                    self.end_line()?;
                    continue;
                }
                break;
            }
            let mut otherwise = Vec::new();
            if self.eat_kw("else") {
                self.end_line()?;
                otherwise = self.block(&["endif"])?;
            }
            self.next();
            self.end_line()?;
            return Ok(Stmt::If(line, arms, otherwise));
        }
        if self.eat_kw("while") {
            let cond = self.expr()?;
            self.end_line()?;
            let body = self.block(&["endwhile"])?;
            self.next();
            self.end_line()?;
            return Ok(Stmt::While(line, cond, body));
        }
        if self.eat_kw("return") {
            let value = if matches!(self.peek(), Tok::Newline) {
                None
            } else {
                Some(self.expr()?)
            };
            self.end_line()?;
            return Ok(Stmt::Return(line, value));
        }
        // A declaration: a type then a name (`Int x`, `Actor[] a`).
        let is_decl = matches!(self.peek(), Tok::Ident(_))
            && (matches!(self.peek_at(1), Tok::Ident(_))
                || (matches!(self.peek_at(1), Tok::Sym("["))
                    && matches!(self.peek_at(2), Tok::Sym("]"))
                    && matches!(self.peek_at(3), Tok::Ident(_))));
        if is_decl {
            let ty = self.ty()?;
            let name = self.ident()?;
            let init = if self.eat_sym("=") {
                Some(self.expr()?)
            } else {
                None
            };
            self.end_line()?;
            return Ok(Stmt::Local(line, ty, name, init));
        }
        let e = self.expr()?;
        for (sym, op) in [
            ("=", None),
            ("+=", Some("+")),
            ("-=", Some("-")),
            ("*=", Some("*")),
            ("/=", Some("/")),
        ] {
            if self.eat_sym(sym) {
                let value = self.expr()?;
                self.end_line()?;
                return Ok(Stmt::Assign(line, e, op, value));
            }
        }
        self.end_line()?;
        Ok(Stmt::Expr(line, e))
    }

    fn expr(&mut self) -> R<Expr> {
        self.binary(0)
    }

    fn binary(&mut self, level: usize) -> R<Expr> {
        const LEVELS: [&[&str]; 5] = [
            &["||"],
            &["&&"],
            &["==", "!=", "<", ">", "<=", ">="],
            &["+", "-"],
            &["*", "/", "%"],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        loop {
            let op = match self.peek() {
                Tok::Sym(s) if LEVELS[level].contains(s) => *s,
                _ => return Ok(left),
            };
            self.next();
            let right = self.binary(level + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self) -> R<Expr> {
        if self.eat_sym("!") {
            return Ok(Expr::Unary("!", Box::new(self.unary()?)));
        }
        if self.eat_sym("-") {
            return Ok(match self.unary()? {
                Expr::Lit(Arg::Int(v)) => Expr::Lit(Arg::Int(-v)),
                Expr::Lit(Arg::Float(v)) => Expr::Lit(Arg::Float(-v)),
                e => Expr::Unary("-", Box::new(e)),
            });
        }
        let mut e = self.postfix()?;
        while self.eat_kw("as") {
            let ty = self.ty()?;
            e = Expr::Cast(Box::new(e), ty);
        }
        Ok(e)
    }

    fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.eat_sym(".") {
                let name = self.ident()?;
                if self.eat_sym("(") {
                    let args = self.args()?;
                    e = Expr::Call(Some(Box::new(e)), name, args);
                } else if name.eq_ignore_ascii_case("length") {
                    e = Expr::Length(Box::new(e));
                } else {
                    return self.err(format!("property access ({name}) isn't supported"));
                }
            } else if self.eat_sym("[") {
                let i = self.expr()?;
                self.expect_sym("]")?;
                e = Expr::Index(Box::new(e), Box::new(i));
            } else {
                return Ok(e);
            }
        }
    }

    fn args(&mut self) -> R<Vec<Expr>> {
        let mut args = Vec::new();
        if self.eat_sym(")") {
            return Ok(args);
        }
        loop {
            args.push(self.expr()?);
            if self.eat_sym(")") {
                return Ok(args);
            }
            self.expect_sym(",")?;
        }
    }

    fn primary(&mut self) -> R<Expr> {
        match self.next() {
            Tok::Int(v) => Ok(Expr::Lit(Arg::Int(v))),
            Tok::Float(v) => Ok(Expr::Lit(Arg::Float(v))),
            Tok::Str(s) => Ok(Expr::Lit(Arg::Str(s))),
            Tok::Sym("(") => {
                let e = self.expr()?;
                self.expect_sym(")")?;
                Ok(e)
            }
            Tok::Ident(s) => {
                let lower = s.to_ascii_lowercase();
                match lower.as_str() {
                    "true" => return Ok(Expr::Lit(Arg::Bool(true))),
                    "false" => return Ok(Expr::Lit(Arg::Bool(false))),
                    "none" => return Ok(Expr::Lit(Arg::None)),
                    "self" => return Ok(Expr::SelfRef),
                    "new" => {
                        let ty = self.ident()?;
                        self.expect_sym("[")?;
                        let n = self.expr()?;
                        self.expect_sym("]")?;
                        return Ok(Expr::NewArray(ty, Box::new(n)));
                    }
                    _ => {}
                }
                if self.eat_sym("(") {
                    let args = self.args()?;
                    return Ok(Expr::Call(None, s, args));
                }
                Ok(Expr::Name(s))
            }
            t => {
                self.pos -= 1;
                self.err(format!("unexpected {t:?}"))
            }
        }
    }
}

/// Parses a script.
pub fn parse(source: &str) -> R<Script> {
    let toks = lex(source).map_err(|(line, message)| CompileError {
        script: "?".into(),
        line,
        message,
    })?;
    Parser {
        toks,
        pos: 0,
        script: "?".into(),
    }
    .script()
}

// --- Types and code ---------------------------------------------------

fn prim(t: &str) -> Option<&'static str> {
    match t.to_ascii_lowercase().as_str() {
        "int" => Some("int"),
        "float" => Some("float"),
        "bool" => Some("bool"),
        "string" => Some("string"),
        "none" => Some("none"),
        _ => None,
    }
}

fn is_array(t: &str) -> bool {
    t.ends_with("[]")
}

/// The scripts compiled together, and the signatures of everything else.
struct World<'a> {
    natives: &'a dyn Signatures,
    scripts: &'a [Script],
}

impl World<'_> {
    fn script(&self, class: &str) -> Option<&Script> {
        self.scripts
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(class))
    }

    fn parent(&self, class: &str) -> Option<String> {
        match self.script(class) {
            Some(s) => Some(s.parent.clone()),
            None => self.natives.parent(class),
        }
    }

    fn is_class(&self, class: &str) -> bool {
        self.parent(class).is_some()
    }

    /// `class.name`, looked up through its parents.
    fn function(&self, class: &str, name: &str) -> Option<Signature> {
        let mut c = class.to_owned();
        for _ in 0..32 {
            let found = match self.script(&c) {
                Some(s) => s
                    .functions
                    .iter()
                    .find(|f| f.name.eq_ignore_ascii_case(name))
                    .map(|f| Signature {
                        name: f.name.clone(),
                        ret: f.ret.clone(),
                        params: f.params.clone(),
                        global: f.global,
                    }),
                None => self.natives.function(&c, name),
            };
            if found.is_some() {
                return found;
            }
            match self.parent(&c) {
                Some(p) if !p.is_empty() => c = p,
                _ => return None,
            }
        }
        None
    }
}

struct Gen<'a> {
    world: &'a World<'a>,
    script: &'a Script,
    f: FunctionBuilder,
    /// Locals and parameters: name (lower case), declared name, type.
    locals: HashMap<String, (String, String)>,
    ret: String,
    line: usize,
    has_num: bool,
}

impl Gen<'_> {
    fn err<T>(&self, message: impl Into<String>) -> R<T> {
        Err(CompileError {
            script: self.script.name.clone(),
            line: self.line,
            message: message.into(),
        })
    }

    fn temp(&mut self, ty: &str) -> Arg {
        self.f.temp(ty)
    }

    /// A name's storage and type: a local or parameter, a script variable,
    /// or an auto property's variable.
    fn name(&self, name: &str) -> Option<(Arg, String)> {
        if let Some((n, t)) = self.locals.get(&name.to_ascii_lowercase()) {
            return Some((id(n), t.clone()));
        }
        // Through the script's parents, for variables and properties of
        // ours.
        let mut s = Some(self.script);
        while let Some(script) = s {
            if let Some((n, t, _)) = script
                .variables
                .iter()
                .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
            {
                return Some((id(n), t.clone()));
            }
            if let Some((n, t)) = script
                .properties
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
            {
                return Some((auto_var(n), t.clone()));
            }
            s = self.world.script(&script.parent);
        }
        None
    }

    /// `value` (of type `from`) as type `to`: unchanged when it already is
    /// one, else cast into a temporary.
    fn coerce(&mut self, value: Arg, from: &str, to: &str) -> R<Arg> {
        if from.eq_ignore_ascii_case(to) {
            return Ok(value);
        }
        let (pf, pt) = (prim(from), prim(to));
        if pf == Some("none") {
            // None goes to any object or array as it is.
            if pt.is_none() {
                return Ok(value);
            }
        }
        match (pf, pt) {
            (_, Some("string")) => {
                if pf == Some("float") && self.has_num {
                    let dest = self.temp("String");
                    self.f.call_method(id("self"), "Num", dest.clone(), [value]);
                    return Ok(dest);
                }
                if let Arg::Str(_) = value {
                    return Ok(value);
                }
                let dest = self.temp("String");
                self.f.cast(dest.clone(), value);
                Ok(dest)
            }
            (_, Some("bool")) => {
                let dest = self.temp("Bool");
                self.f.cast(dest.clone(), value);
                Ok(dest)
            }
            (Some("int"), Some("float")) => {
                if let Arg::Int(v) = value {
                    return Ok(Arg::Float(v as f32));
                }
                let dest = self.temp("Float");
                self.f.cast(dest.clone(), value);
                Ok(dest)
            }
            (Some("float"), Some("int")) => {
                let dest = self.temp("Int");
                self.f.cast(dest.clone(), value);
                Ok(dest)
            }
            (None, None) if is_array(from) || is_array(to) => {
                self.err(format!("can't convert {from} to {to}"))
            }
            (None, None) => {
                // Up or down: both are casts in the game.
                if !self.world.is_class(from) || !self.world.is_class(to) {
                    return self.err(format!("unknown class in {from} as {to}"));
                }
                let dest = self.temp(to);
                self.f.cast(dest.clone(), value);
                Ok(dest)
            }
            _ => self.err(format!("can't convert {from} to {to}")),
        }
    }

    fn lit_type(a: &Arg) -> &'static str {
        match a {
            Arg::Int(_) => "Int",
            Arg::Float(_) => "Float",
            Arg::Bool(_) => "Bool",
            Arg::Str(_) => "String",
            _ => "None",
        }
    }

    fn expr(&mut self, e: &Expr) -> R<(Arg, String)> {
        match e {
            Expr::Lit(a) => Ok((a.clone(), Self::lit_type(a).into())),
            Expr::SelfRef => Ok((id("self"), self.script.name.clone())),
            Expr::Name(n) => match self.name(n) {
                Some(found) => Ok(found),
                None => self.err(format!("unknown name {n}")),
            },
            Expr::Cast(inner, ty) => {
                let (v, from) = self.expr(inner)?;
                let to = ty.clone();
                if prim(&from) == prim(&to) && prim(&to).is_some() {
                    return Ok((v, to));
                }
                // An explicit cast always casts (`x as String` on a float
                // gives the game's own text).
                let dest = self.temp(&to);
                self.f.cast(dest.clone(), v);
                Ok((dest, to))
            }
            Expr::Unary(op, inner) => {
                let (v, t) = self.expr(inner)?;
                if *op == "!" {
                    let dest = self.temp("Bool");
                    self.f.op(op::NOT, [dest.clone(), v]);
                    return Ok((dest, "Bool".into()));
                }
                let float = prim(&t) == Some("float");
                let dest = self.temp(if float { "Float" } else { "Int" });
                self.f
                    .op(if float { op::FNEG } else { op::INEG }, [dest.clone(), v]);
                Ok((dest, t))
            }
            Expr::Binary(op, a, b) => self.binary(op, a, b),
            Expr::Index(a, i) => {
                let (av, at) = self.expr(a)?;
                let Some(elem) = at.strip_suffix("[]").map(str::to_owned) else {
                    return self.err(format!("{at} isn't an array"));
                };
                let (iv, it) = self.expr(i)?;
                let iv = self.coerce(iv, &it, "Int")?;
                let dest = self.temp(&elem);
                self.f.op(op::ARRAY_GETELEMENT, [dest.clone(), av, iv]);
                Ok((dest, elem))
            }
            Expr::Length(a) => {
                let (av, at) = self.expr(a)?;
                if !is_array(&at) {
                    return self.err(format!("{at} has no Length"));
                }
                let dest = self.temp("Int");
                self.f.op(op::ARRAY_LENGTH, [dest.clone(), av]);
                Ok((dest, "Int".into()))
            }
            Expr::NewArray(ty, n) => {
                let (nv, nt) = self.expr(n)?;
                let nv = self.coerce(nv, &nt, "Int")?;
                let t = format!("{ty}[]");
                let dest = self.temp(&t);
                self.f.op(op::ARRAY_CREATE, [dest.clone(), nv]);
                Ok((dest, t))
            }
            Expr::Call(obj, name, args) => self.call(obj.as_deref(), name, args),
        }
    }

    fn binary(&mut self, op_name: &str, a: &Expr, b: &Expr) -> R<(Arg, String)> {
        if op_name == "&&" || op_name == "||" {
            let dest = self.temp("Bool");
            let (av, at) = self.expr(a)?;
            let av = self.coerce(av, &at, "Bool")?;
            self.f.assign(dest.clone(), av);
            let end = self.f.label();
            if op_name == "&&" {
                self.f.jmpf(dest.clone(), end);
            } else {
                self.f.jmpt(dest.clone(), end);
            }
            let (bv, bt) = self.expr(b)?;
            let bv = self.coerce(bv, &bt, "Bool")?;
            self.f.assign(dest.clone(), bv);
            self.f.bind(end);
            return Ok((dest, "Bool".into()));
        }
        let (av, at) = self.expr(a)?;
        let (bv, bt) = self.expr(b)?;
        let (pa, pb) = (prim(&at), prim(&bt));
        if op_name == "+" && (pa == Some("string") || pb == Some("string")) {
            let av = self.coerce(av, &at, "String")?;
            let bv = self.coerce(bv, &bt, "String")?;
            let dest = self.temp("String");
            self.f.strcat(dest.clone(), av, bv);
            return Ok((dest, "String".into()));
        }
        let numeric = |p: Option<&str>| matches!(p, Some("int" | "float"));
        let compare = matches!(op_name, "==" | "!=" | "<" | ">" | "<=" | ">=");
        if numeric(pa) && numeric(pb) {
            let float = pa == Some("float") || pb == Some("float");
            let ty = if float { "Float" } else { "Int" };
            let av = self.coerce(av, &at, ty)?;
            let bv = self.coerce(bv, &bt, ty)?;
            if compare {
                return Ok((self.compare(op_name, av, bv), "Bool".into()));
            }
            let code = match (op_name, float) {
                ("+", false) => op::IADD,
                ("+", true) => op::FADD,
                ("-", false) => op::ISUB,
                ("-", true) => op::FSUB,
                ("*", false) => op::IMUL,
                ("*", true) => op::FMUL,
                ("/", false) => op::IDIV,
                ("/", true) => op::FDIV,
                ("%", false) => op::IMOD,
                _ => return self.err(format!("{op_name} on {at} and {bt}")),
            };
            let dest = self.temp(ty);
            self.f.op(code, [dest.clone(), av, bv]);
            return Ok((dest, ty.into()));
        }
        if compare && matches!(op_name, "==" | "!=") {
            return Ok((self.compare(op_name, av, bv), "Bool".into()));
        }
        self.err(format!("{op_name} on {at} and {bt}"))
    }

    fn compare(&mut self, op_name: &str, a: Arg, b: Arg) -> Arg {
        let dest = self.temp("Bool");
        let code = match op_name {
            "==" | "!=" => op::CMP_EQ,
            "<" => op::CMP_LT,
            ">" => op::CMP_GT,
            "<=" => op::CMP_LE,
            _ => op::CMP_GE,
        };
        self.f.op(code, [dest.clone(), a, b]);
        if op_name == "!=" {
            self.f.op(op::NOT, [dest.clone(), dest.clone()]);
        }
        dest
    }

    /// The arguments for `sig`, cast to their parameters' types, defaults
    /// filling the rest.
    fn call_args(&mut self, sig: &Signature, args: &[Expr]) -> R<Vec<Arg>> {
        if args.len() > sig.params.len() {
            return self.err(format!(
                "{} takes {} arguments, given {}",
                sig.name,
                sig.params.len(),
                args.len()
            ));
        }
        let mut out = Vec::new();
        for (i, (pname, pty, default)) in sig.params.iter().enumerate() {
            match args.get(i) {
                Some(e) => {
                    let (v, t) = self.expr(e)?;
                    out.push(self.coerce(v, &t, pty)?);
                }
                None => match default {
                    Some(d) => {
                        let dt = Self::lit_type(d);
                        out.push(self.coerce(d.clone(), dt, pty)?);
                    }
                    None => {
                        return self.err(format!("{}: no argument for {pname}", sig.name));
                    }
                },
            }
        }
        Ok(out)
    }

    fn result(&mut self, ret: &str) -> Arg {
        if prim(ret) == Some("none") {
            discard()
        } else {
            self.temp(ret)
        }
    }

    fn call(&mut self, obj: Option<&Expr>, name: &str, args: &[Expr]) -> R<(Arg, String)> {
        // A global of a class: `Class.Function(...)`.
        if let Some(Expr::Name(class)) = obj
            && self.name(class).is_none()
        {
            if !self.world.is_class(class) {
                return self.err(format!("unknown name {class}"));
            }
            let Some(sig) = self.world.function(class, name) else {
                return self.err(format!("{class}.{name} isn't declared"));
            };
            if !sig.global {
                return self.err(format!("{class}.{name} isn't global"));
            }
            let args = self.call_args(&sig, args)?;
            let dest = self.result(&sig.ret);
            // Called on the class that declares it.
            let mut decl = class.clone();
            while !self.declares(&decl, name) {
                decl = self.world.parent(&decl).unwrap_or_default();
            }
            self.f.call_static(&decl, &sig.name, dest.clone(), args);
            return Ok((dest, sig.ret));
        }
        // An array's Find.
        if let Some(o) = obj
            && name.eq_ignore_ascii_case("find")
        {
            let (av, at) = self.expr(o)?;
            if let Some(elem) = at.strip_suffix("[]").map(str::to_owned) {
                if args.is_empty() || args.len() > 2 {
                    return self.err("Find takes a value and a start");
                }
                let (v, t) = self.expr(&args[0])?;
                let v = self.coerce(v, &t, &elem)?;
                let start = match args.get(1) {
                    Some(e) => {
                        let (s, st) = self.expr(e)?;
                        self.coerce(s, &st, "Int")?
                    }
                    None => Arg::Int(0),
                };
                let dest = self.temp("Int");
                self.f
                    .op(op::ARRAY_FINDELEMENT, [av, dest.clone(), v, start]);
                return Ok((dest, "Int".into()));
            }
        }
        let (target, class) = match obj {
            Some(o) => self.expr(o)?,
            None => (id("self"), self.script.name.clone()),
        };
        if prim(&class).is_some() || is_array(&class) {
            return self.err(format!("{name} called on a {class}"));
        }
        let Some(sig) = self.world.function(&class, name) else {
            return self.err(format!("{class}.{name} isn't declared"));
        };
        let args = self.call_args(&sig, args)?;
        let dest = self.result(&sig.ret);
        if sig.global {
            if obj.is_some() {
                return self.err(format!("{class}.{name} is global"));
            }
            let mut decl = class.clone();
            while !self.declares(&decl, name) {
                decl = self.world.parent(&decl).unwrap_or_default();
            }
            self.f.call_static(&decl, &sig.name, dest.clone(), args);
        } else {
            self.f.call_method(target, &sig.name, dest.clone(), args);
        }
        Ok((dest, sig.ret))
    }

    fn declares(&self, class: &str, name: &str) -> bool {
        if class.is_empty() {
            return true;
        }
        match self.world.script(class) {
            Some(s) => s
                .functions
                .iter()
                .any(|f| f.name.eq_ignore_ascii_case(name)),
            None => self.world.natives.function(class, name).is_some(),
        }
    }

    fn declare(&mut self, name: &str, ty: &str) -> R<Arg> {
        let key = name.to_ascii_lowercase();
        if let Some((n, t)) = self.locals.get(&key) {
            if !t.eq_ignore_ascii_case(ty) {
                return self.err(format!("{name} declared again as {ty}"));
            }
            return Ok(id(n));
        }
        if prim(ty).is_none() && !is_array(ty) && !self.world.is_class(ty) {
            return self.err(format!("unknown type {ty}"));
        }
        self.locals.insert(key, (name.to_owned(), ty.to_owned()));
        Ok(self.f.local(name, ty))
    }

    fn stmts(&mut self, body: &[Stmt]) -> R<()> {
        for s in body {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> R<()> {
        match s {
            Stmt::Local(line, ty, name, init) => {
                self.line = *line;
                let var = self.declare(name, ty)?;
                let value = match init {
                    Some(e) => {
                        let (v, t) = self.expr(e)?;
                        self.coerce(v, &t, ty)?
                    }
                    // Declared again in a loop: the type's default, as the
                    // game gives a fresh local.
                    None => match prim(ty) {
                        Some("int") => Arg::Int(0),
                        Some("float") => Arg::Float(0.0),
                        Some("bool") => Arg::Bool(false),
                        Some("string") => Arg::Str(String::new()),
                        _ => Arg::None,
                    },
                };
                self.f.assign(var, value);
            }
            Stmt::Assign(line, target, op_name, value) => {
                self.line = *line;
                let rhs = match op_name {
                    Some(op_name) => {
                        Expr::Binary(op_name, Box::new(target.clone()), Box::new(value.clone()))
                    }
                    None => value.clone(),
                };
                match target {
                    Expr::Name(n) => {
                        let Some((var, ty)) = self.name(n) else {
                            return self.err(format!("unknown name {n}"));
                        };
                        let (v, t) = self.expr(&rhs)?;
                        let v = self.coerce(v, &t, &ty)?;
                        self.f.assign(var, v);
                    }
                    Expr::Index(a, i) => {
                        let (av, at) = self.expr(a)?;
                        let Some(elem) = at.strip_suffix("[]").map(str::to_owned) else {
                            return self.err(format!("{at} isn't an array"));
                        };
                        let (iv, it) = self.expr(i)?;
                        let iv = self.coerce(iv, &it, "Int")?;
                        let (v, t) = self.expr(&rhs)?;
                        let v = self.coerce(v, &t, &elem)?;
                        self.f.op(op::ARRAY_SETELEMENT, [av, iv, v]);
                    }
                    _ => return self.err("can't assign to that"),
                }
            }
            Stmt::Expr(line, e) => {
                self.line = *line;
                if !matches!(e, Expr::Call(..)) {
                    return self.err("a statement must be a call");
                }
                self.expr(e)?;
            }
            Stmt::If(line, arms, otherwise) => {
                self.line = *line;
                let end = self.f.label();
                for (cond, body) in arms {
                    let next = self.f.label();
                    let (c, _) = self.expr(cond)?;
                    self.f.jmpf(c, next);
                    self.stmts(body)?;
                    self.f.jmp(end);
                    self.f.bind(next);
                }
                self.stmts(otherwise)?;
                self.f.bind(end);
            }
            Stmt::While(line, cond, body) => {
                self.line = *line;
                let (top, end) = (self.f.label(), self.f.label());
                self.f.bind(top);
                let (c, _) = self.expr(cond)?;
                self.f.jmpf(c, end);
                self.stmts(body)?;
                self.f.jmp(top);
                self.f.bind(end);
            }
            Stmt::Return(line, value) => {
                self.line = *line;
                let v = match value {
                    Some(e) => {
                        let (v, t) = self.expr(e)?;
                        let ret = self.ret.clone();
                        self.coerce(v, &t, &ret)?
                    }
                    None => Arg::None,
                };
                self.f.ret(v);
            }
        }
        Ok(())
    }
}

/// Compiles `scripts` (parsed with [`parse`]) together: each may call the
/// others' functions. Classes outside them come from `natives`.
pub fn compile(scripts: &[Script], natives: &dyn Signatures) -> R<Vec<ScriptBuilder>> {
    let world = World { natives, scripts };
    let mut out = Vec::new();
    for script in scripts {
        let mut b = ScriptBuilder::new(&script.name, &script.parent);
        for (n, t, init) in &script.variables {
            b.variable(n, t, init.clone());
        }
        for (n, t) in &script.properties {
            b.auto_property(n, t);
        }
        let has_num = script.functions.iter().any(|f| {
            f.name.eq_ignore_ascii_case("num")
                && prim(&f.ret) == Some("string")
                && f.params.len() == 1
                && prim(&f.params[0].1) == Some("float")
        });
        for func in &script.functions {
            let mut f = FunctionBuilder::new(&func.ret);
            if func.global {
                f = f.global();
            }
            let mut locals = HashMap::new();
            for (n, t, _) in &func.params {
                f.param(n, t);
                locals.insert(n.to_ascii_lowercase(), (n.clone(), t.clone()));
            }
            let mut g = Gen {
                world: &world,
                script,
                f,
                locals,
                ret: func.ret.clone(),
                line: func.line,
                // Num itself must cast.
                has_num: has_num && !func.name.eq_ignore_ascii_case("num"),
            };
            g.stmts(&func.body)?;
            g.f.ret(Arg::None);
            b.function(&func.name, g.f);
        }
        out.push(b);
    }
    Ok(out)
}

/// Parses and compiles `sources` together into `.pex` bytes by script name.
pub fn compile_sources(
    sources: &[&str],
    natives: &dyn Signatures,
) -> std::result::Result<Vec<(String, Vec<u8>)>, String> {
    // An error with the line it names quoted.
    let quoted = |e: CompileError| {
        let line = sources
            .iter()
            .find(|s| {
                parse(s).is_ok_and(|p| p.name.eq_ignore_ascii_case(&e.script)) || e.script == "?"
            })
            .and_then(|s| s.lines().nth(e.line.saturating_sub(1)))
            .unwrap_or("");
        format!("{e}\n    {}", line.trim())
    };
    let scripts: Vec<Script> = sources
        .iter()
        .map(|s| parse(s))
        .collect::<R<_>>()
        .map_err(quoted)?;
    let built = compile(&scripts, natives).map_err(quoted)?;
    built
        .into_iter()
        .zip(&scripts)
        .map(|(b, s)| {
            let pex = b.build().map_err(|e| e.to_string())?;
            let bytes = crate::pex::write(&pex).map_err(|e| e.to_string())?;
            Ok((s.name.clone(), bytes))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{Host, NativeResult, Vm};
    use crate::{ObjectId, Value};

    /// Natives for the tests: `Debug.Trace(String)`, `Form.GetFormID()`.
    struct Natives;

    impl Signatures for Natives {
        fn parent(&self, class: &str) -> Option<String> {
            match class.to_ascii_lowercase().as_str() {
                "form" | "debug" => Some(String::new()),
                "objectreference" => Some("Form".into()),
                "actor" => Some("ObjectReference".into()),
                "quest" => Some("Form".into()),
                _ => None,
            }
        }
        fn function(&self, class: &str, name: &str) -> Option<Signature> {
            let (c, n) = (class.to_ascii_lowercase(), name.to_ascii_lowercase());
            match (c.as_str(), n.as_str()) {
                ("debug", "trace") => Some(Signature {
                    name: "Trace".into(),
                    ret: "None".into(),
                    params: vec![
                        ("asText".into(), "String".into(), None),
                        ("aiSeverity".into(), "Int".into(), Some(Arg::Int(0))),
                    ],
                    global: true,
                }),
                ("form", "getformid") => Some(Signature {
                    name: "GetFormID".into(),
                    ret: "Int".into(),
                    params: vec![],
                    global: false,
                }),
                ("objectreference", "getdistance") => Some(Signature {
                    name: "GetDistance".into(),
                    ret: "Float".into(),
                    params: vec![("akOther".into(), "ObjectReference".into(), None)],
                    global: false,
                }),
                _ => None,
            }
        }
    }

    struct Host1 {
        scripts: HashMap<String, Vec<u8>>,
        traces: Vec<String>,
        calls: Vec<String>,
    }

    impl Host for Host1 {
        fn load_script(&mut self, name: &str) -> Option<Vec<u8>> {
            self.scripts.get(&name.to_ascii_lowercase()).cloned()
        }
        fn call_native(
            &mut self,
            class: &str,
            func: &str,
            this: Option<&Value>,
            args: &[Value],
        ) -> NativeResult {
            self.calls.push(format!("{class}.{func} {this:?}"));
            match func.to_ascii_lowercase().as_str() {
                "trace" => {
                    if let Some(Value::String(s)) = args.first() {
                        self.traces.push(s.to_string());
                    }
                    NativeResult::Value(Value::None)
                }
                "getformid" => NativeResult::Value(Value::Int(0x14)),
                "getdistance" => NativeResult::Value(Value::Float(2.5)),
                _ => NativeResult::Value(Value::None),
            }
        }
        fn is_native_type(&self, _obj: ObjectId, _class: &str) -> bool {
            true
        }
    }

    fn natives_pex() -> Vec<(String, Vec<u8>)> {
        let decl = |class: &str, parent: &str, fs: &[crate::build::NativeDecl]| {
            (
                class.to_ascii_lowercase(),
                crate::build::native_class(class, parent, fs).unwrap(),
            )
        };
        vec![
            decl("Form", "", &[("GetFormID", "Int", &[], false)]),
            decl(
                "Debug",
                "",
                &[(
                    "Trace",
                    "None",
                    &[("asText", "String"), ("aiSeverity", "Int")],
                    true,
                )],
            ),
            decl(
                "ObjectReference",
                "Form",
                &[(
                    "GetDistance",
                    "Float",
                    &[("akOther", "ObjectReference")],
                    false,
                )],
            ),
            decl("Actor", "ObjectReference", &[]),
            decl("Quest", "Form", &[]),
        ]
    }

    const SOURCE: &str = r#"
Scriptname TestScript extends Quest
{ A test: every statement kind. }
Actor Property Player Auto
Int Count = 2
String Result

String Function Num(Float f)
    return (f as String) + "!"
EndFunction

Int Function Twice(Int x) global
    return x * 2
EndFunction

Event OnInit()
    Int i = 0
    String s = "a" + 1 + " " + 1.5 + " " + true
    While i < Count && !(i == 5)
        i += 1
    EndWhile
    If i == 2
        s += " two"
    ElseIf i == 3
        s += " three"
    Else
        s += " other"
    EndIf
    Int[] a = new Int[3]
    a[1] = Twice(21)
    s += " " + a[1] + " len " + a.Length + " at " + a.Find(42)
    Float d = Player.GetDistance(Player) + 1
    s += " d " + d + " id " + Player.GetFormID()
    s += " " + (3 / 2) + " " + (3.0 / 2) + " " + (7 % 3) + " " + -i
    If i > 1 || Player.GetFormID() == 0
        s += " or"
    EndIf
    Debug.Trace(s)
    Result = s
EndEvent
"#;

    #[test]
    fn compiles_and_runs() {
        let built = compile_sources(&[SOURCE], &Natives).unwrap();
        let mut scripts: HashMap<String, Vec<u8>> = natives_pex().into_iter().collect();
        for (n, b) in built {
            scripts.insert(n.to_ascii_lowercase(), b);
        }
        let mut host = Host1 {
            scripts,
            traces: Vec::new(),
            calls: Vec::new(),
        };
        let mut vm = Vm::new();
        let quest = ObjectId::Form(0x0100_0800);
        assert!(vm.attach(
            &mut host,
            quest,
            "TestScript",
            &[(
                "Player".into(),
                Value::Object(ObjectId::Form(0x14), "Actor".into())
            )]
        ));
        vm.send_event(&mut host, quest, "OnInit", vec![]);
        vm.run(&mut host, 0.0, 10_000);
        assert_eq!(
            host.traces,
            vec![
                "a1 1.500000! TRUE two 42 len 3 at 1 d 3.500000! id 20 1 1.500000! 1 -2 or"
                    .to_owned()
            ],
            "{:?}",
            host.calls
        );
    }

    #[test]
    fn errors_name_the_line() {
        let e = compile_sources(
            &["Scriptname S extends Quest\nEvent OnInit()\n  Foo.Bar()\nEndEvent\n"],
            &Natives,
        )
        .unwrap_err();
        assert!(e.contains("S:3"), "{e}");
        let e = compile_sources(
            &["Scriptname S extends Quest\nEvent OnInit()\n  Debug.Trace()\nEndEvent\n"],
            &Natives,
        )
        .unwrap_err();
        assert!(e.contains("no argument for asText"), "{e}");
    }
}
