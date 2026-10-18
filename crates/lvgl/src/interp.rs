//! A small interpreter for the C that LVGLab generates. The preview runs the
//! very code of the export against the LVGL that is compiled into the
//! program, so it can never show anything the export does not produce.
//!
//! It understands what the generator writes: global and local variables,
//! arrays, calls of LVGL functions, user functions (components) with
//! `return`, and simple expressions. A statement it cannot run (event
//! callbacks, helpers of the generated code) is skipped and listed.

use std::collections::HashMap;
use std::ffi::{CString, c_char, c_void};

/// One entry of the generated tables in `vendor/names.c`.
#[repr(C)]
struct FunctionEntry {
    name: *const c_char,
    address: *const c_void,
}

#[repr(C)]
struct ConstantEntry {
    name: *const c_char,
    value: i64,
}

unsafe extern "C" {
    static lvglab_functions: [FunctionEntry; 0];
    static lvglab_symbols: [FunctionEntry; 0];
    static lvglab_constants: [ConstantEntry; 0];
}

fn read_functions(table: *const FunctionEntry) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut entry = table;
    // SAFETY: the tables end with an entry whose name is NULL.
    unsafe {
        while !(*entry).name.is_null() {
            let name = std::ffi::CStr::from_ptr((*entry).name).to_string_lossy().into_owned();
            map.insert(name, (*entry).address as usize);
            entry = entry.add(1);
        }
    }
    map
}

fn read_constants() -> HashMap<String, i64> {
    let mut map = HashMap::new();
    let mut entry = std::ptr::addr_of!(lvglab_constants).cast::<ConstantEntry>();
    // SAFETY: as above.
    unsafe {
        while !(*entry).name.is_null() {
            let name = std::ffi::CStr::from_ptr((*entry).name).to_string_lossy().into_owned();
            map.insert(name, (*entry).value);
            entry = entry.add(1);
        }
    }
    map
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Num(i64),
    Str(Vec<u8>),
    Punct(char),
}

fn tokenize(source: &str) -> Vec<Tok> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line_start = true;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '#' && line_start {
            while i < chars.len() && chars[i] != '\n' {
                // Continued macro lines.
                if chars[i] == '\\' && chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c.is_ascii_alphabetic() || c == '_' {
            line_start = false;
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if c.is_ascii_digit() {
            line_start = false;
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let digits = text.trim_end_matches(['u', 'U', 'l', 'L']);
            let value = if let Some(hex) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
                i64::from_str_radix(hex, 16).ok()
            } else if digits.contains('.') {
                digits.trim_end_matches(['f', 'F']).parse::<f64>().ok().map(|f| f as i64)
            } else {
                digits.parse().ok()
            };
            tokens.push(Tok::Num(value.unwrap_or(0)));
        } else if c == '"' {
            line_start = false;
            i += 1;
            let mut text: Vec<u8> = Vec::new();
            let push = |text: &mut Vec<u8>, c: char| text.extend_from_slice(c.to_string().as_bytes());
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                    match chars.get(i) {
                        Some('n') => text.push(b'\n'),
                        Some('t') => text.push(b'\t'),
                        Some('r') => text.push(b'\r'),
                        Some('0') => text.push(0),
                        Some('x') => {
                            // `\xEF\x80\x8C` style bytes: collect them as raw bytes of UTF-8.
                            let hex: String = chars[i + 1..].iter().take(2).collect();
                            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                text.push(byte);
                            }
                            i += 2;
                        }
                        Some(&other) => push(&mut text, other),
                        None => {}
                    }
                } else {
                    push(&mut text, chars[i]);
                }
                i += 1;
            }
            i += 1;
            tokens.push(Tok::Str(text));
        } else if c == '\'' {
            line_start = false;
            let value = chars.get(i + 1).map_or(0, |&c| c as i64);
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            i += 1;
            tokens.push(Tok::Num(value));
        } else {
            line_start = false;
            tokens.push(Tok::Punct(c));
            i += 1;
        }
    }
    tokens
}

struct Function {
    params: Vec<String>,
    /// Token range of the body, without the braces.
    body: std::ops::Range<usize>,
}

/// Values of the interpreter are machine words: integers, pointers, small
/// structs such as `lv_color_t`.
type Word = u64;

pub struct Interpreter {
    tokens: Vec<Tok>,
    functions: HashMap<String, Function>,
    lvgl_functions: HashMap<String, usize>,
    symbols: HashMap<String, usize>,
    constants: HashMap<String, i64>,
    globals: HashMap<String, Word>,
    scopes: Vec<HashMap<String, Word>>,
    /// Memory handed to LVGL (strings, arrays); lives as long as the interpreter.
    strings: Vec<CString>,
    arrays: Vec<Vec<u8>>,
    /// Objects created without a parent: the screens.
    pub screens: Vec<Word>,
    /// Statements that were skipped, for diagnostics.
    pub skipped: Vec<String>,
    returned: Option<Word>,
    depth: usize,
    /// Only this `_screen_init` function runs (the others are skipped).
    pub only_screen: Option<String>,
}

/// Names that are skipped without a note: they only wire up behavior the
/// preview does not run.
const IGNORED_CALLS: &[&str] =
    &["lv_obj_add_event_cb", "lv_obj_set_user_data", "lv_scr_load_anim", "lv_obj_remove_event_cb"];

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            functions: HashMap::new(),
            lvgl_functions: read_functions(std::ptr::addr_of!(lvglab_functions).cast()),
            symbols: read_functions(std::ptr::addr_of!(lvglab_symbols).cast()),
            constants: read_constants(),
            globals: HashMap::new(),
            scopes: Vec::new(),
            strings: Vec::new(),
            arrays: Vec::new(),
            screens: Vec::new(),
            skipped: Vec::new(),
            returned: None,
            depth: 0,
            only_screen: None,
        }
    }

    /// Reads a C file: function definitions are remembered and global
    /// variables are initialized.
    pub fn load(&mut self, source: &str) {
        let base = self.tokens.len();
        self.tokens.extend(tokenize(source));
        let end = self.tokens.len();
        let mut i = base;
        while i < end {
            // A statement or definition at the top level.
            let start = i;
            let mut depth = 0;
            let mut is_function = false;
            while i < end {
                match &self.tokens[i] {
                    Tok::Punct('(') | Tok::Punct('[') => depth += 1,
                    Tok::Punct(')') | Tok::Punct(']') => depth -= 1,
                    Tok::Punct('{') if depth == 0 => {
                        // `= {` starts an initializer, otherwise a function body.
                        is_function = !matches!(self.tokens.get(i.wrapping_sub(1)), Some(Tok::Punct('=')))
                            && !matches!(self.tokens.get(i.wrapping_sub(1)), Some(Tok::Punct(']')));
                        break;
                    }
                    Tok::Punct(';') if depth == 0 => break,
                    _ => {}
                }
                i += 1;
            }
            if i >= end {
                break;
            }
            if is_function {
                let body_start = i + 1;
                let mut braces = 1;
                i += 1;
                while i < end && braces > 0 {
                    match &self.tokens[i] {
                        Tok::Punct('{') => braces += 1,
                        Tok::Punct('}') => braces -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                let body = body_start..i - 1;
                if let Some((name, params)) = self.signature(start, body_start - 1) {
                    self.functions.insert(name, Function { params, body });
                }
            } else {
                // A global declaration, possibly with an initializer in braces.
                let mut j = i;
                if self.tokens[i] == Tok::Punct('{') {
                    let mut braces = 0;
                    while j < end {
                        match &self.tokens[j] {
                            Tok::Punct('{') => braces += 1,
                            Tok::Punct('}') => {
                                braces -= 1;
                                if braces == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    while j < end && self.tokens[j] != Tok::Punct(';') {
                        j += 1;
                    }
                }
                self.scopes.clear();
                let range = start..j;
                self.statement(range);
                i = j + 1;
            }
        }
    }

    /// Name and parameter names of the function whose head is `tokens[from..to]`
    /// (up to the closing parenthesis).
    fn signature(&self, from: usize, to: usize) -> Option<(String, Vec<String>)> {
        let head = &self.tokens[from..to];
        let open = head.iter().position(|t| *t == Tok::Punct('('))?;
        let Tok::Ident(name) = head.get(open.checked_sub(1)?)? else { return None };
        let mut params = Vec::new();
        let mut last_ident: Option<&String> = None;
        let mut depth = 0;
        for token in &head[open + 1..] {
            match token {
                Tok::Punct('(') => depth += 1,
                Tok::Punct(')') if depth == 0 => {
                    params.extend(last_ident.cloned());
                    break;
                }
                Tok::Punct(')') => depth -= 1,
                Tok::Punct(',') if depth == 0 => params.extend(last_ident.take().cloned()),
                Tok::Ident(id) => last_ident = Some(id),
                _ => {}
            }
        }
        // `(void)` has no parameters.
        if params == ["void"] {
            params.clear();
        }
        Some((name.clone(), params))
    }

    /// Calls a function of LVGL directly.
    pub fn call_lvgl(&mut self, name: &str, args: &[Word]) -> Option<Word> {
        let address = *self.lvgl_functions.get(name)?;
        let mut a = [0_u64; 8];
        for (slot, value) in a.iter_mut().zip(args) {
            *slot = *value;
        }
        // SAFETY: see `invoke`.
        let function: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(address) };
        Some(function(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]))
    }

    /// Makes `&name` resolve to a static image descriptor.
    pub fn add_symbol(&mut self, name: &str, address: usize) {
        self.symbols.insert(name.to_owned(), address);
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub fn global(&self, name: &str) -> Option<Word> {
        self.globals.get(name).copied()
    }

    /// Calls a function of the loaded code.
    pub fn call(&mut self, name: &str, args: &[Word]) -> Option<Word> {
        if self.depth > 24 {
            return None;
        }
        let (params, body) = {
            let function = self.functions.get(name)?;
            (function.params.clone(), function.body.clone())
        };
        let mut scope = HashMap::new();
        for (param, value) in params.iter().zip(args) {
            scope.insert(param.clone(), *value);
        }
        self.scopes.push(scope);
        self.depth += 1;
        let saved = self.returned.take();
        self.block(body);
        let result = self.returned.take();
        self.returned = saved;
        self.depth -= 1;
        self.scopes.pop();
        Some(result.unwrap_or(0))
    }

    /// Runs the statements of `range` one by one.
    fn block(&mut self, range: std::ops::Range<usize>) {
        let mut i = range.start;
        while i < range.end && self.returned.is_none() {
            let start = i;
            let mut depth = 0;
            while i < range.end {
                match &self.tokens[i] {
                    Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => depth += 1,
                    Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => depth -= 1,
                    Tok::Punct(';') if depth == 0 => break,
                    _ => {}
                }
                i += 1;
            }
            self.statement(start..i.min(range.end));
            i += 1;
        }
    }

    fn lookup(&self, name: &str) -> Option<Word> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).or_else(|| self.globals.get(name)).copied()
    }

    fn assign(&mut self, name: &str, value: Word) {
        if let Some(scope) = self.scopes.iter_mut().rev().find(|s| s.contains_key(name)) {
            scope.insert(name.to_owned(), value);
        } else {
            self.globals.insert(name.to_owned(), value);
        }
    }

    fn declare(&mut self, name: &str, value: Word) {
        match self.scopes.last_mut() {
            Some(scope) => scope.insert(name.to_owned(), value),
            None => self.globals.insert(name.to_owned(), value),
        };
    }

    fn skip(&mut self, range: std::ops::Range<usize>, why: &str) {
        let text: Vec<String> = self.tokens[range]
            .iter()
            .take(14)
            .map(|t| match t {
                Tok::Ident(s) => s.clone(),
                Tok::Num(n) => n.to_string(),
                Tok::Str(s) => format!("{:?}", String::from_utf8_lossy(s)),
                Tok::Punct(c) => c.to_string(),
            })
            .collect();
        if self.skipped.len() < 200 {
            self.skipped.push(format!("{} ({why})", text.join(" ")));
        }
    }

    fn statement(&mut self, range: std::ops::Range<usize>) {
        if range.is_empty() {
            return;
        }
        let tokens = self.tokens[range.clone()].to_vec();
        let first = match &tokens[0] {
            Tok::Ident(s) => s.as_str(),
            _ => return,
        };
        match first {
            "return" => {
                let value = if tokens.len() > 1 { self.eval_tokens(&tokens[1..]) } else { Some(0) };
                self.returned = Some(value.unwrap_or(0));
                return;
            }
            // Control flow is not interpreted.
            "for" | "if" | "while" | "else" | "do" | "switch" | "typedef" | "extern" => return,
            _ => {}
        }
        // Where does the first `=` (not `==`) at depth 0 sit, and how many words come before it?
        let mut depth = 0;
        let mut equals = None;
        for (k, token) in tokens.iter().enumerate() {
            match token {
                Tok::Punct('(') | Tok::Punct('[') | Tok::Punct('{') => depth += 1,
                Tok::Punct(')') | Tok::Punct(']') | Tok::Punct('}') => depth -= 1,
                Tok::Punct('=') if depth == 0 && tokens.get(k + 1) != Some(&Tok::Punct('=')) => {
                    equals = Some(k);
                    break;
                }
                _ => {}
            }
        }
        // `fn(...)` or `fn(...);` without an assignment.
        let is_call = matches!(tokens.get(1), Some(Tok::Punct('('))) && equals.is_none();
        if is_call {
            let Tok::Ident(name) = &tokens[0] else { return };
            if IGNORED_CALLS.contains(&name.as_str()) {
                return;
            }
            if self.eval_tokens(&tokens).is_none() {
                self.skip(range, "call");
            }
            return;
        }
        let Some(eq) = equals else {
            // `type name;` or `type name[N];`
            if let Some(Tok::Ident(name)) = tokens.iter().rfind(|t| matches!(t, Tok::Ident(_))) {
                if tokens.len() >= 2 && !matches!(tokens[0], Tok::Punct(_)) && !is_call {
                    let name = name.clone();
                    self.declare(&name, 0);
                }
            }
            return;
        };
        let left = &tokens[..eq];
        let right = &tokens[eq + 1..];
        let words = left.iter().filter(|t| matches!(t, Tok::Ident(_))).count();
        let Some(Tok::Ident(name)) = left.iter().rfind(|t| matches!(t, Tok::Ident(_))) else { return };
        let name = name.clone();
        let is_array = left.contains(&Tok::Punct('['));
        if is_array {
            let (element, _) = self.declaration_shape(left);
            let value = self.make_array(element, left, right);
            self.declare(&name, value);
            return;
        }
        let Some(value) = self.eval_tokens(right) else {
            // The variable exists, but holds NULL.
            if words > 1 {
                self.declare(&name, 0);
            }
            self.skip(range, "value");
            return;
        };
        if words > 1 {
            self.declare(&name, value);
        } else {
            self.assign(&name, value);
        }
        if matches!(right.first(), Some(Tok::Ident(f)) if f == "lv_obj_create")
            && matches!(right.get(2), Some(Tok::Ident(p)) if p == "NULL")
            && value != 0
        {
            self.screens.push(value);
        }
    }

    /// Element description of a declaration like `static lv_point_t name[]`.
    fn declaration_shape(&self, left: &[Tok]) -> (Element, usize) {
        let words: Vec<&str> =
            left.iter().filter_map(|t| if let Tok::Ident(s) = t { Some(s.as_str()) } else { None }).collect();
        let pointer = left.contains(&Tok::Punct('*'));
        let element = if pointer {
            Element::Pointer
        } else if words.contains(&"lv_point_t") {
            Element::Point
        } else if words.contains(&"lv_color_t") || words.contains(&"uint32_t") || words.contains(&"int32_t") {
            Element::Word32
        } else if words.contains(&"lv_coord_t") || words.contains(&"int16_t") || words.contains(&"uint16_t") {
            Element::Word16
        } else if words.contains(&"uint8_t") || words.contains(&"char") {
            Element::Byte
        } else {
            Element::Pointer
        };
        (element, words.len())
    }

    /// Allocates the array of a declaration and fills it with its initializer.
    fn make_array(&mut self, element: Element, left: &[Tok], right: &[Tok]) -> Word {
        // Size from the brackets, or from the initializer.
        let open = left.iter().position(|t| *t == Tok::Punct('['));
        let declared = open.and_then(|o| {
            let inner = &left[o + 1..];
            let close = inner.iter().rposition(|t| *t == Tok::Punct(']'))?;
            if close == 0 { None } else { self.eval_tokens(&inner[..close]) }
        });
        let mut values: Vec<Word> = Vec::new();
        if right.first() == Some(&Tok::Punct('{')) {
            // Split the elements at depth 1 commas.
            let inner = &right[1..right.len().saturating_sub(1)];
            let mut depth = 0;
            let mut start = 0;
            let mut parts: Vec<&[Tok]> = Vec::new();
            for (k, token) in inner.iter().enumerate() {
                match token {
                    Tok::Punct('{') | Tok::Punct('(') => depth += 1,
                    Tok::Punct('}') | Tok::Punct(')') => depth -= 1,
                    Tok::Punct(',') if depth == 0 => {
                        parts.push(&inner[start..k]);
                        start = k + 1;
                    }
                    _ => {}
                }
            }
            if start < inner.len() {
                parts.push(&inner[start..]);
            }
            for part in parts {
                if part.first() == Some(&Tok::Punct('{')) {
                    // A point: two coordinates.
                    let coords: Vec<Word> = part[1..part.len().saturating_sub(1)]
                        .split(|t| *t == Tok::Punct(','))
                        .map(|c| self.eval_tokens(c).unwrap_or(0))
                        .collect();
                    values.extend(coords);
                } else {
                    values.push(self.eval_tokens(part).unwrap_or(0));
                }
            }
        }
        let count = declared.map_or(values.len(), |n| n as usize).max(values.len());
        let size = match element {
            Element::Pointer => 8,
            Element::Word32 => 4,
            Element::Word16 | Element::Point => 2,
            Element::Byte => 1,
        };
        // Declared sizes of canvas buffers are in pixels of 4 bytes.
        let mut memory = vec![0_u8; count.max(1) * size.max(1)];
        for (index, value) in values.iter().enumerate() {
            let at = index * size;
            if at + size <= memory.len() {
                memory[at..at + size].copy_from_slice(&value.to_le_bytes()[..size]);
            }
        }
        let address = memory.as_ptr() as Word;
        self.arrays.push(memory);
        address
    }

    fn eval_tokens(&mut self, tokens: &[Tok]) -> Option<Word> {
        let mut position = 0;
        let value = self.expression(tokens, &mut position, 0)?;
        (position == tokens.len()).then_some(value)
    }

    fn precedence(op: char) -> Option<u8> {
        match op {
            '|' => Some(1),
            '&' => Some(2),
            '+' | '-' => Some(3),
            '*' | '/' | '%' => Some(4),
            _ => None,
        }
    }

    fn expression(&mut self, tokens: &[Tok], at: &mut usize, min: u8) -> Option<Word> {
        let mut left = self.unary(tokens, at)?;
        while let Some(Tok::Punct(op)) = tokens.get(*at) {
            let Some(level) = Self::precedence(*op) else { break };
            if level < min {
                break;
            }
            // `||` and `&&` are not supported.
            if matches!(tokens.get(*at + 1), Some(Tok::Punct(next)) if next == op) {
                return None;
            }
            *at += 1;
            let right = self.expression(tokens, at, level + 1)?;
            left = match op {
                '|' => left | right,
                '&' => left & right,
                '+' => left.wrapping_add(right),
                '-' => left.wrapping_sub(right),
                '*' => left.wrapping_mul(right),
                '/' => left.checked_div(right).unwrap_or(0),
                _ => left.checked_rem(right).unwrap_or(0),
            };
        }
        Some(left)
    }

    fn unary(&mut self, tokens: &[Tok], at: &mut usize) -> Option<Word> {
        match tokens.get(*at)? {
            Tok::Punct('-') => {
                *at += 1;
                Some(self.unary(tokens, at)?.wrapping_neg())
            }
            Tok::Punct('!') => {
                *at += 1;
                Some(u64::from(self.unary(tokens, at)? == 0))
            }
            Tok::Punct('&') => {
                *at += 1;
                let Tok::Ident(name) = tokens.get(*at)? else { return None };
                *at += 1;
                // The address of a font of LVGL; generated assets are not available.
                self.symbols.get(name).map(|&a| a as Word)
            }
            Tok::Punct('(') => {
                // A cast `(type)value`, otherwise a parenthesized expression.
                let is_cast = matches!(tokens.get(*at + 1), Some(Tok::Ident(t)) if t.ends_with("_t") || t == "void" || t == "char" || t == "int" || t == "bool")
                    && tokens[*at..].iter().position(|t| *t == Tok::Punct(')')).is_some_and(|close| {
                        tokens[*at + 1..*at + close].iter().all(|t| matches!(t, Tok::Ident(_) | Tok::Punct('*')))
                    });
                if is_cast {
                    let close = tokens[*at..].iter().position(|t| *t == Tok::Punct(')'))?;
                    *at += close + 1;
                    return self.unary(tokens, at);
                }
                *at += 1;
                let value = self.expression(tokens, at, 0)?;
                if tokens.get(*at) != Some(&Tok::Punct(')')) {
                    return None;
                }
                *at += 1;
                Some(value)
            }
            Tok::Num(n) => {
                *at += 1;
                Some(*n as Word)
            }
            Tok::Str(s) => {
                *at += 1;
                let s = s.clone();
                Some(self.string(&s))
            }
            Tok::Ident(name) => {
                let name = name.clone();
                *at += 1;
                if tokens.get(*at) == Some(&Tok::Punct('(')) {
                    return self.invoke(&name, tokens, at);
                }
                match name.as_str() {
                    "NULL" | "false" => Some(0),
                    "true" => Some(1),
                    "LV_FONT_DEFAULT" => self.symbols.get("lv_font_montserrat_14").map(|&a| a as Word),
                    _ => self
                        .lookup(&name)
                        .or_else(|| self.constants.get(&name).map(|&v| v as Word))
                        .or_else(|| self.indexed(&name, tokens, at)),
                }
            }
            Tok::Punct(_) => None,
        }
    }

    /// `array[index]`: only reads of declared arrays.
    fn indexed(&mut self, _name: &str, _tokens: &[Tok], _at: &mut usize) -> Option<Word> {
        None
    }

    fn string(&mut self, bytes: &[u8]) -> Word {
        let c = CString::new(bytes.iter().copied().filter(|b| *b != 0).collect::<Vec<u8>>()).unwrap_or_default();
        let pointer = c.as_ptr() as Word;
        self.strings.push(c);
        pointer
    }

    /// A call of a function: arguments are evaluated first.
    fn invoke(&mut self, name: &str, tokens: &[Tok], at: &mut usize) -> Option<Word> {
        // Find the matching parenthesis.
        let open = *at;
        let mut depth = 0;
        let mut close = open;
        for (k, token) in tokens.iter().enumerate().skip(open) {
            match token {
                Tok::Punct('(') => depth += 1,
                Tok::Punct(')') => {
                    depth -= 1;
                    if depth == 0 {
                        close = k;
                        break;
                    }
                }
                _ => {}
            }
        }
        if close == open {
            return None;
        }
        let inside = &tokens[open + 1..close];
        *at = close + 1;
        if name == "sizeof" {
            return Some(8);
        }
        let mut args = Vec::new();
        if !inside.is_empty() {
            let mut start = 0;
            let mut depth = 0;
            let mut parts = Vec::new();
            for (k, token) in inside.iter().enumerate() {
                match token {
                    Tok::Punct('(') | Tok::Punct('{') | Tok::Punct('[') => depth += 1,
                    Tok::Punct(')') | Tok::Punct('}') | Tok::Punct(']') => depth -= 1,
                    Tok::Punct(',') if depth == 0 => {
                        parts.push(&inside[start..k]);
                        start = k + 1;
                    }
                    _ => {}
                }
            }
            parts.push(&inside[start..]);
            for part in parts {
                args.push(self.eval_tokens(part)?);
            }
        }
        if name == "lv_pct" {
            // LV_PCT: a percentage is stored with the "special" bit.
            let value = args.first().copied().unwrap_or(0) as i64;
            let spec = 1_i64 << 13;
            return Some(if value < 0 { (1000 - value) | spec } else { value | spec } as Word);
        }
        if let Some(only) = &self.only_screen {
            if (name.ends_with("_screen_init") && name != only)
                || matches!(name, "lv_disp_load_scr" | "lv_scr_load" | "_ui_screen_change")
            {
                return Some(0);
            }
        }
        if self.functions.contains_key(name) {
            return self.call(name, &args);
        }
        let address = *self.lvgl_functions.get(name)?;
        // Missing arguments read as zero; extra registers are ignored by the callee.
        let mut a = [0_u64; 8];
        for (slot, value) in a.iter_mut().zip(&args) {
            *slot = *value;
        }
        if args.len() > 8 {
            return None;
        }
        // SAFETY: `address` is a function of the compiled LVGL. All arguments of
        // the generated code are integers, pointers or 32-bit structs and fit
        // the integer registers of the C calling convention.
        let function: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(address) };
        Some(function(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]))
    }
}

#[derive(Clone, Copy)]
enum Element {
    Pointer,
    Word32,
    Word16,
    Point,
    Byte,
}
