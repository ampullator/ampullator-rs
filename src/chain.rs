/// Chain DSL parser for constructing GraphFacade structs.
///
/// A Chain string is translated into `register: HashMap<String, Facade>` and
/// `connect: Vec<(String, String)>` containers that can be used with
/// `register_and_connect()` to build a signal graph.
///
/// # Grammar (informal)
///
/// ```text
/// chain        = segment ("|" segment)*
/// segment      = addmul_expr
/// addmul_expr  = arrow_chain (("+" | "*") arrow_chain)*
/// arrow_chain  = named_atom ("->" port_spec? named_atom)*
/// named_atom   = atom ("=>" Ident)?
/// atom         = ugen_call | Ident | Number | "(" addmul_expr ")"
/// ugen_call    = Ident ("(" args ")")?
/// args         = (arg_pair ("," arg_pair)*)?
/// arg_pair     = Ident "=" (Number | Ident)
/// port_spec    = (Ident)? ":" (Ident)?
/// ```
use std::collections::HashMap;

use crate::graph_facade::Facade;
use crate::graph_facade::UGFacade;

// ---------------------------------------------------------------------------
// Tokeniser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Pipe,          // |
    Arrow,         // ->
    FatArrow,      // =>
    Colon,         // :
    LParen,        // (
    RParen,        // )
    Comma,         // ,
    Assign,        // =
    Plus,          // +
    Star,          // *
    Ident(String), // identifier
    Number(f32),   // numeric literal
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            c if c.is_ascii_whitespace() => {
                i += 1;
            }
            '|' => {
                tokens.push(Token::Pipe);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            ':' => {
                tokens.push(Token::Colon);
                i += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '-' if i + 1 < chars.len() && chars[i + 1] == '>' => {
                tokens.push(Token::Arrow);
                i += 2;
            }
            '=' if i + 1 < chars.len() && chars[i + 1] == '>' => {
                tokens.push(Token::FatArrow);
                i += 2;
            }
            '=' => {
                tokens.push(Token::Assign);
                i += 1;
            }
            // Number starting with a digit or a lone '.'
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                let n: f32 = num_str
                    .parse()
                    .map_err(|_| format!("Invalid number: '{num_str}'"))?;
                tokens.push(Token::Number(n));
            }
            // Negative number: '-' followed by digit (and NOT '>')
            '-' if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() => {
                let start = i;
                i += 1; // consume '-'
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                let n: f32 = num_str
                    .parse()
                    .map_err(|_| format!("Invalid number: '{num_str}'"))?;
                tokens.push(Token::Number(n));
            }
            // Identifier
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                tokens.push(Token::Ident(ident));
            }
            c => {
                return Err(format!("Unexpected character: '{c}' at position {i}"));
            }
        }
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct ChainParser {
    tokens: Vec<Token>,
    pos: usize,
    pub register: HashMap<String, Facade>,
    pub connect: Vec<(String, String)>,
    counter: usize,
}

impl ChainParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            register: HashMap::new(),
            connect: Vec::new(),
            counter: 0,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_at(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset)
    }

    fn consume(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn gen_name(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("_{prefix}_{}", self.counter)
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.consume() {
            Some(ref t) if t == expected => Ok(()),
            Some(t) => Err(format!("Expected {expected:?}, got {t:?}")),
            None => Err(format!("Expected {expected:?}, got end of input")),
        }
    }

    /// Get the default output connection string "name.first_output" for a registered node.
    fn default_output(&self, name: &str) -> Result<String, String> {
        match self.register.get(name) {
            Some(facade) => {
                let ugen = facade.to_ugen();
                let outs = ugen.output_names();
                if outs.is_empty() {
                    Err(format!("UGen '{name}' has no outputs"))
                } else {
                    Ok(format!("{name}.{}", outs[0]))
                }
            }
            None => Err(format!("Unknown node: '{name}'")),
        }
    }

    /// Get the default input connection string "name.first_input" for a registered node.
    fn default_input(&self, name: &str) -> Result<String, String> {
        match self.register.get(name) {
            Some(facade) => {
                let ugen = facade.to_ugen();
                let ins = ugen.input_names();
                if ins.is_empty() {
                    Err(format!("UGen '{name}' has no inputs"))
                } else {
                    Ok(format!("{name}.{}", ins[0]))
                }
            }
            None => Err(format!("Unknown node: '{name}'")),
        }
    }

    /// Rename an already-registered node, updating all connect entries.
    fn rename(&mut self, old: &str, new_name: &str) {
        if old == new_name {
            return;
        }
        if let Some(facade) = self.register.remove(old) {
            self.register.insert(new_name.to_string(), facade);
            let old_prefix = format!("{old}.");
            let new_prefix = format!("{new_name}.");
            for (src, dst) in &mut self.connect {
                if let Some(port) = src.strip_prefix(&old_prefix) {
                    *src = format!("{new_prefix}{port}");
                }
                if let Some(port) = dst.strip_prefix(&old_prefix) {
                    *dst = format!("{new_prefix}{port}");
                }
            }
        }
    }

    /// Return true if `name` is a UGFacade variant (a UGen type name).
    fn is_ugen_type(name: &str) -> bool {
        matches!(
            name,
            "AsHz"
                | "Ceil"
                | "Clock"
                | "Const"
                | "EnvBreakPoint"
                | "EnvAR"
                | "Floor"
                | "HighPass"
                | "HighPassQ"
                | "LowPass"
                | "LowPassQ"
                | "Parametric"
                | "ParametricConst"
                | "Mult"
                | "PulseSelect"
                | "Round"
                | "Select"
                | "Sine"
                | "Sum"
                | "Trigger"
                | "White"
        )
    }

    /// Build a `Facade` from a UGen type name and a key→value argument map.
    ///
    /// Values are parsed as numbers where possible; everything else is treated
    /// as a string (for enum discriminants such as `mode = Samples`).
    ///
    /// The existing `UGFacade` serde deserialisation is re-used by constructing
    /// the tagged-array JSON form `["TypeName", {fields…}]`.
    fn make_facade(
        type_name: &str,
        args: &HashMap<String, String>,
    ) -> Result<Facade, String> {
        let mut obj = serde_json::Map::new();

        for (k, v) in args {
            if let Ok(n) = v.parse::<f64>() {
                if let Some(num) = serde_json::Number::from_f64(n) {
                    obj.insert(k.clone(), serde_json::Value::Number(num));
                } else {
                    // Infinity / NaN – keep as string and let serde reject it
                    obj.insert(k.clone(), serde_json::Value::String(v.clone()));
                }
            } else {
                obj.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
        }

        // Sensible chain-DSL defaults for fields that are required in the
        // serde schema but are commonly omitted when writing chain strings.
        let defaults: &[(&str, &[(&str, serde_json::Value)])] = &[
            (
                "LowPass",
                &[("roll_off_db", serde_json::Value::Number(6.into()))],
            ),
            (
                "LowPassQ",
                &[("roll_off_db", serde_json::Value::Number(6.into()))],
            ),
            (
                "HighPass",
                &[("roll_off_db", serde_json::Value::Number(6.into()))],
            ),
            (
                "HighPassQ",
                &[("roll_off_db", serde_json::Value::Number(6.into()))],
            ),
            ("AsHz", &[("mode", serde_json::Value::String("Hz".into()))]),
            (
                "Round",
                &[
                    ("places", serde_json::Value::Number(0.into())),
                    ("mode", serde_json::Value::String("Round".into())),
                ],
            ),
        ];
        for (tname, fields) in defaults {
            if *tname == type_name {
                for (k, v) in *fields {
                    obj.entry(k.to_string()).or_insert_with(|| v.clone());
                }
            }
        }

        // Default `input_count = 2` for Sum / Mult when not explicitly supplied.
        if (type_name == "Sum" || type_name == "Mult") && !obj.contains_key("input_count")
        {
            obj.insert(
                "input_count".to_string(),
                serde_json::Value::Number(2.into()),
            );
        }

        let json_val = serde_json::Value::Array(vec![
            serde_json::Value::String(type_name.to_string()),
            serde_json::Value::Object(obj),
        ]);

        let ug_facade: UGFacade = serde_json::from_value(json_val)
            .map_err(|e| format!("Failed to parse UGen '{type_name}': {e}"))?;

        Ok(Facade::Full(ug_facade))
    }

    /// Parse keyword args inside `(…)`.  Returns key → value-string pairs.
    fn parse_args(&mut self) -> Result<HashMap<String, String>, String> {
        let mut args = HashMap::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(args);
        }
        loop {
            let key = match self.consume() {
                Some(Token::Ident(s)) => s,
                t => return Err(format!("Expected argument name, got {t:?}")),
            };
            self.expect(&Token::Assign)?;
            let value = match self.consume() {
                Some(Token::Number(n)) => format!("{n}"),
                Some(Token::Ident(s)) => s,
                t => return Err(format!("Expected argument value, got {t:?}")),
            };
            args.insert(key, value);

            match self.peek() {
                Some(Token::Comma) => {
                    self.consume();
                }
                Some(Token::RParen) | None => break,
                t => {
                    return Err(format!("Expected ',' or ')', got {t:?}"));
                }
            }
        }
        Ok(args)
    }

    /// Parse a UGen call (type name + optional `(args)`) and immediately register it.
    fn parse_ugen_call(&mut self) -> Result<String, String> {
        let type_name = match self.consume() {
            Some(Token::Ident(s)) => s,
            t => return Err(format!("Expected UGen type name, got {t:?}")),
        };

        let args = if self.peek() == Some(&Token::LParen) {
            self.consume(); // consume '('
            let args = self.parse_args()?;
            self.expect(&Token::RParen)?;
            args
        } else {
            HashMap::new()
        };

        let facade = Self::make_facade(&type_name, &args)?;
        let name = self.gen_name(&type_name.to_lowercase());
        self.register.insert(name.clone(), facade);
        Ok(name)
    }

    /// Attempt to read a port name (an identifier that is not a UGen type) and
    /// return it, or return `None` if the next token cannot be a port name.
    fn try_read_port_name(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Ident(s)) if !Self::is_ugen_type(s) => {
                let s = s.clone();
                self.consume();
                Some(s)
            }
            _ => None,
        }
    }

    /// Parse an optional port specification after `->`.
    ///
    /// Grammar: `(Ident)? ":" (Ident)?`
    ///
    /// Detection rules (2-token lookahead):
    /// - next = `:`               → `:dst` form (src defaults to first output)
    /// - next = Ident, then `:`   → `src:dst` form
    /// - otherwise                → no port spec
    fn parse_port_spec_opt(
        &mut self,
    ) -> Result<(Option<String>, Option<String>), String> {
        match (self.peek(), self.peek_at(1)) {
            (Some(Token::Colon), _) => {
                self.consume(); // consume ':'
                let dst = self.try_read_port_name();
                Ok((None, dst))
            }
            (Some(Token::Ident(_)), Some(Token::Colon)) => {
                let src = match self.consume() {
                    Some(Token::Ident(s)) => s,
                    _ => unreachable!(),
                };
                self.consume(); // consume ':'
                let dst = self.try_read_port_name();
                Ok((Some(src), dst))
            }
            _ => Ok((None, None)),
        }
    }

    /// Parse an atom and return its registered node name.
    ///
    /// Atoms are: UGen call, name reference, numeric literal, or `(` expr `)`.
    fn parse_atom(&mut self) -> Result<String, String> {
        match self.peek() {
            Some(Token::Number(_)) => {
                let n = match self.consume() {
                    Some(Token::Number(n)) => n,
                    _ => unreachable!(),
                };
                let name = self.gen_name("const");
                self.register.insert(name.clone(), Facade::Short(n));
                Ok(name)
            }
            Some(Token::Ident(id)) => {
                let id = id.clone();
                if Self::is_ugen_type(&id) {
                    self.parse_ugen_call()
                } else {
                    self.consume();
                    if !self.register.contains_key(&id) {
                        return Err(format!("Unknown name reference: '{id}'"));
                    }
                    Ok(id)
                }
            }
            Some(Token::LParen) => {
                self.consume(); // consume '('
                let result = self.parse_addmul_expr()?;
                self.expect(&Token::RParen)?;
                Ok(result)
            }
            t => Err(format!(
                "Expected atom (UGen, name, number, or '('), got {t:?}"
            )),
        }
    }

    /// Parse `atom ("=>" Ident)?`.
    ///
    /// If `=>` is present the node is (re-)registered under the user-supplied name.
    fn parse_named_atom(&mut self) -> Result<String, String> {
        let name = self.parse_atom()?;

        if self.peek() == Some(&Token::FatArrow) {
            self.consume(); // consume '=>'
            match self.consume() {
                Some(Token::Ident(alias)) => {
                    self.rename(&name, &alias);
                    Ok(alias)
                }
                t => Err(format!("Expected name after '=>', got {t:?}")),
            }
        } else {
            Ok(name)
        }
    }

    /// Parse a chain of atoms joined by `->` (with optional port specs).
    ///
    /// Returns the name of the rightmost node.
    fn parse_arrow_chain(&mut self) -> Result<String, String> {
        let mut current = self.parse_named_atom()?;

        while self.peek() == Some(&Token::Arrow) {
            self.consume(); // consume '->'
            let (src_port, dst_port) = self.parse_port_spec_opt()?;
            let next = self.parse_named_atom()?;

            let src_str = match src_port {
                Some(port) => format!("{current}.{port}"),
                None => self.default_output(&current)?,
            };
            let dst_str = match dst_port {
                Some(port) => format!("{next}.{port}"),
                None => self.default_input(&next)?,
            };

            self.connect.push((src_str, dst_str));
            current = next;
        }

        Ok(current)
    }

    /// Parse `arrow_chain (("+" | "*") arrow_chain)*`.
    ///
    /// `+` creates a `Sum` UGen; `*` creates a `Mult` UGen.
    /// The two operands are connected to `in1` and `in2` of the new node.
    fn parse_addmul_expr(&mut self) -> Result<String, String> {
        let mut lhs = self.parse_arrow_chain()?;

        while matches!(self.peek(), Some(Token::Plus) | Some(Token::Star)) {
            let op = self.consume().unwrap();
            let rhs = self.parse_arrow_chain()?;

            let (type_name, prefix) = match op {
                Token::Plus => ("Sum", "sum"),
                Token::Star => ("Mult", "mult"),
                _ => unreachable!(),
            };

            let op_name = self.gen_name(prefix);
            let facade = Self::make_facade(type_name, &HashMap::new())?;
            self.register.insert(op_name.clone(), facade);

            let lhs_out = self.default_output(&lhs)?;
            let rhs_out = self.default_output(&rhs)?;
            self.connect.push((lhs_out, format!("{op_name}.in1")));
            self.connect.push((rhs_out, format!("{op_name}.in2")));

            lhs = op_name;
        }

        Ok(lhs)
    }

    /// Parse the full chain: one or more segments separated by `|`.
    pub fn parse(&mut self) -> Result<(), String> {
        if self.peek().is_none() {
            return Ok(());
        }

        loop {
            self.parse_addmul_expr()?;

            match self.peek() {
                Some(Token::Pipe) => {
                    self.consume();
                }
                None => break,
                t => {
                    return Err(format!("Expected '|' or end of input, got {t:?}"));
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a Chain DSL string and return the resulting `register` and `connect`
/// containers that can be passed to `GraphFacade::from_chain` /
/// `register_and_connect`.
pub fn parse_chain(
    input: &str,
) -> Result<(HashMap<String, Facade>, Vec<(String, String)>), String> {
    let tokens = tokenize(input)?;
    let mut parser = ChainParser::new(tokens);
    parser.parse()?;
    Ok((parser.register, parser.connect))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::Recorder;

    // Helper: parse a chain and return (register, connect) with sorted connect
    // for deterministic comparisons.
    fn parse(chain: &str) -> (HashMap<String, Facade>, Vec<(String, String)>) {
        parse_chain(chain).expect("chain parse failed")
    }

    // ---------------------------------------------------------------------------
    // Tokeniser
    // ---------------------------------------------------------------------------

    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("White() => noise -> LowPass()").unwrap();
        assert!(tokens.contains(&Token::Arrow));
        assert!(tokens.contains(&Token::FatArrow));
        assert!(tokens.contains(&Token::LParen));
        assert!(tokens.contains(&Token::RParen));
        assert!(tokens.contains(&Token::Ident("White".to_string())));
        assert!(tokens.contains(&Token::Ident("noise".to_string())));
        assert!(tokens.contains(&Token::Ident("LowPass".to_string())));
    }

    #[test]
    fn test_tokenize_number_forms() {
        let tokens = tokenize("4000 .5 -12.3").unwrap();
        assert_eq!(tokens[0], Token::Number(4000.0));
        assert_eq!(tokens[1], Token::Number(0.5));
        assert_eq!(tokens[2], Token::Number(-12.3));
    }

    #[test]
    fn test_tokenize_pipe_and_operators() {
        let tokens = tokenize("a | b + c * d").unwrap();
        assert!(tokens.contains(&Token::Pipe));
        assert!(tokens.contains(&Token::Plus));
        assert!(tokens.contains(&Token::Star));
    }

    // ---------------------------------------------------------------------------
    // Register only (no connections)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_simple_ugens_no_connections() {
        let (reg, conn) = parse(
            "White() | ParametricConst(gain=1, bw=.6, freq=2000) | Clock(value=20.0, mode=Samples)",
        );
        // Three UGens registered, no connections
        assert_eq!(reg.len(), 3);
        assert!(conn.is_empty());
    }

    #[test]
    fn test_chain_named_ugens_no_connections() {
        let (reg, conn) = parse(
            "White() => noise | ParametricConst(gain=1, bw=.6, freq=2000) => f1 | Clock(value=20.0, mode=Samples) => metro",
        );
        assert!(reg.contains_key("noise"), "register should contain 'noise'");
        assert!(reg.contains_key("f1"), "register should contain 'f1'");
        assert!(reg.contains_key("metro"), "register should contain 'metro'");
        assert_eq!(reg.len(), 3);
        assert!(conn.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Connections
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_simple_arrow_connections() {
        // White -> LowPass -> HighPass: two connections
        let (reg, conn) = parse("White() -> LowPass() -> HighPass()");
        assert_eq!(reg.len(), 3);
        assert_eq!(conn.len(), 2);
        // Connections should use default ports: White.out->LowPass.in, LowPass.out->HighPass.in
        let (src0, dst0) = &conn[0];
        assert!(src0.ends_with(".out"), "src should be .out port: {src0}");
        assert!(dst0.ends_with(".in"), "dst should be .in port: {dst0}");
    }

    #[test]
    fn test_chain_named_arrow_connections() {
        let (reg, conn) =
            parse("White() => noise -> LowPass() => lpf -> HighPass() => hpf");
        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("lpf"));
        assert!(reg.contains_key("hpf"));
        assert_eq!(conn.len(), 2);
        assert!(conn.contains(&("noise.out".to_string(), "lpf.in".to_string())));
        assert!(conn.contains(&("lpf.out".to_string(), "hpf.in".to_string())));
    }

    // ---------------------------------------------------------------------------
    // Port specifications
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_port_spec_dst_only() {
        // Const ->:cutoff lpf  — connects first output to the named 'cutoff' input
        let (reg, conn) = parse(
            "White() => noise -> LowPass() => lpf | Const(value=4000) ->:cutoff lpf",
        );
        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("lpf"));
        // Should have connection to lpf.cutoff
        let to_cutoff = conn.iter().find(|(_, dst)| dst == "lpf.cutoff");
        assert!(to_cutoff.is_some(), "expected connection to lpf.cutoff");
        let (src, _) = to_cutoff.unwrap();
        assert!(src.ends_with(".out"), "src should use default output port");
    }

    #[test]
    fn test_chain_port_spec_src_and_dst() {
        // Explicit src:dst port annotation
        let (reg, conn) = parse("White() => noise ->out:in LowPass() => lpf");
        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("lpf"));
        assert!(conn.contains(&("noise.out".to_string(), "lpf.in".to_string())));
    }

    // ---------------------------------------------------------------------------
    // Numeric literals → implicit Const
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_numeric_literal_creates_const() {
        let (reg, conn) =
            parse("White() => noise -> LowPass() => lpf | 4000 ->:cutoff lpf");
        // An implicit Const node is created for the numeric literal
        let const_key = reg.keys().find(|k| k.starts_with("_const_"));
        assert!(
            const_key.is_some(),
            "expected an auto-named Const node, got keys: {:?}",
            reg.keys().collect::<Vec<_>>()
        );
        let to_cutoff = conn.iter().find(|(_, dst)| dst == "lpf.cutoff");
        assert!(to_cutoff.is_some());
    }

    // ---------------------------------------------------------------------------
    // Binary operators (+ and *)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_sum_operator() {
        let (reg, conn) = parse(
            "White() => noise -> ParametricConst(gain=2, bw=.3, freq=400) => t1 | \
             noise -> ParametricConst(gain=2, bw=.3, freq=2000) => t2 | \
             (t1 + t2) => mix",
        );
        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("t1"));
        assert!(reg.contains_key("t2"));
        assert!(reg.contains_key("mix"), "register should contain 'mix'");
        // mix should be a Sum UGen
        // Connect: noise->t1, noise->t2, t1->mix.in1, t2->mix.in2
        assert!(conn.contains(&("noise.out".to_string(), "t1.in".to_string())));
        assert!(conn.contains(&("noise.out".to_string(), "t2.in".to_string())));
        assert!(conn.contains(&("t1.out".to_string(), "mix.in1".to_string())));
        assert!(conn.contains(&("t2.out".to_string(), "mix.in2".to_string())));
    }

    #[test]
    fn test_chain_mult_operator() {
        let (reg, conn) =
            parse("Const(value=2.0) => a | Const(value=3.0) => b | (a * b) => product");
        assert!(reg.contains_key("a"));
        assert!(reg.contains_key("b"));
        assert!(reg.contains_key("product"));
        assert!(conn.contains(&("a.out".to_string(), "product.in1".to_string())));
        assert!(conn.contains(&("b.out".to_string(), "product.in2".to_string())));
    }

    // ---------------------------------------------------------------------------
    // Complex example from the issue
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_issue_example_with_named_refs() {
        let chain = "White() => noise -> LowPass() => lpf -> HighPass() => hpf | \
                     Const(value=4000) ->:cutoff lpf | \
                     Const(value=800) ->:cutoff hpf";
        let (reg, conn) = parse(chain);

        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("lpf"));
        assert!(reg.contains_key("hpf"));
        assert_eq!(conn.len(), 4); // noise->lpf, lpf->hpf, const1->lpf.cutoff, const2->hpf.cutoff

        assert!(conn.contains(&("noise.out".to_string(), "lpf.in".to_string())));
        assert!(conn.contains(&("lpf.out".to_string(), "hpf.in".to_string())));
        let cutoff_conns: Vec<_> = conn
            .iter()
            .filter(|(_, dst)| dst.ends_with(".cutoff"))
            .collect();
        assert_eq!(cutoff_conns.len(), 2);
    }

    #[test]
    fn test_chain_issue_example_numeric_literals() {
        // Same as above but with numeric-literal Const nodes
        let chain = "White() => noise -> LowPass() => lpf -> HighPass() => hpf | \
                     4000 ->:cutoff lpf | \
                     800 ->:cutoff hpf";
        let (reg, conn) = parse(chain);
        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("lpf"));
        assert!(reg.contains_key("hpf"));
        assert_eq!(conn.len(), 4);
    }

    // ---------------------------------------------------------------------------
    // Multiline / whitespace tolerance (newlines, tabs, spaces)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_multiline_whitespace() {
        // Newlines and leading spaces should be treated as whitespace
        let chain = "White() => noise\n  -> ParametricConst(gain=1, bw=.6, freq=2000)\n  => f1\n  |\n  Const(value=4000) ->:in f1";
        let (reg, conn) = parse(chain);
        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("f1"));
        assert_eq!(reg.len(), 3); // noise, f1, one const
        // The const should be connected to f1's first input ("in")
        let to_f1_in = conn.iter().find(|(_, dst)| dst == "f1.in");
        assert!(
            to_f1_in.is_some(),
            "expected connection to f1.in, got: {conn:?}"
        );
    }

    #[test]
    fn test_chain_tab_whitespace() {
        // Tab characters should be treated as whitespace
        let chain = "Const(value=1.0)\t=>\tnoise\t|\tConst(value=2.0)\t=>\tb";
        let (reg, conn) = parse(chain);
        assert!(reg.contains_key("noise"));
        assert!(reg.contains_key("b"));
        assert!(conn.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Integration: from_chain builds a working GenGraph
    // ---------------------------------------------------------------------------

    #[test]
    fn test_chain_from_chain_integration() {
        use crate::graph_facade::GraphFacade;
        let chain = "Const(value=4.0) => freq | Sine() => osc | freq ->:freq osc";
        let gf = GraphFacade::from_chain(chain).expect("from_chain failed");
        let mut g = GenGraph::new(8.0, 8);
        gf.register_and_connect(&mut g)
            .expect("register_and_connect failed");
        // The graph should have exactly 2 nodes and 1 connection
        assert_eq!(g.len(), 2);
        g.process();
    }

    #[test]
    fn test_chain_clock_select_integration() {
        // Build a clock+step chain – forward-references to undefined names are
        // not yet supported, so we verify just the register here.
        let chain2 = "Clock(value=2.0, mode=Samples) => clk | Const(value=1.0) => step";
        let (reg, _) = parse(chain2);
        assert!(reg.contains_key("clk"));
        assert!(reg.contains_key("step"));
    }

    #[test]
    fn test_chain_sum_integration() {
        use crate::graph_facade::GraphFacade;
        // Two constant sources summed together
        let chain = "Const(value=3.0) => a | Const(value=4.0) => b | (a + b) => total";
        let gf = GraphFacade::from_chain(chain).expect("from_chain failed");
        let mut g = GenGraph::new(8.0, 8);
        gf.register_and_connect(&mut g)
            .expect("register_and_connect failed");
        let r = Recorder::from_samples(g, None, 8);
        let out = r.get_output_by_label("total.out");
        assert_eq!(out, vec![7.0; 8]);
    }
}
