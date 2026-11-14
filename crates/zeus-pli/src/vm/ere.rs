//! POSIX Extended Regular Expressions for the REGEX built-in function
//! (LRM, REGEX: "The pattern p must conform to the POSIX standard for
//! Extended Regular Expressions").
//!
//! POSIX chooses the leftmost match and, among those, the longest; each
//! subexpression in turn is then leftmost and longest. Engines that take
//! the first alternative that matches (Perl, the Rust regex crate) give
//! other matches for patterns such as `(a|ab)c`, so this is a small
//! backtracking matcher that enumerates the matches at the leftmost start
//! and ranks them by the POSIX rules. Not compared with the z/OS C library
//! regcomp/regexec that the LRM refers to.

#[derive(Debug, Clone)]
enum Node {
    Empty,
    Char(char),
    Any,
    Class(bool, Vec<ClassItem>),
    Bol,
    Eol,
    Group(usize, Box<Node>),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat(Box<Node>, u32, Option<u32>),
}

#[derive(Debug, Clone)]
enum ClassItem {
    Char(char),
    Range(char, char),
    Named(String),
}

pub struct Regex {
    node: Node,
    groups: usize,
}

/// A pattern that is not a valid ERE.
#[derive(Debug)]
pub struct Invalid;

/// The matching took too many steps.
#[derive(Debug)]
pub struct TooComplex;

/// Index and length of the whole match and of each subexpression (None:
/// the subexpression took no part in the match).
pub type Match = Vec<Option<(usize, usize)>>;

struct Parser<'a> {
    s: &'a [char],
    i: usize,
    groups: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn alt(&mut self) -> Result<Node, Invalid> {
        let mut v = vec![self.concat()?];
        while self.peek() == Some('|') {
            self.i += 1;
            v.push(self.concat()?);
        }
        Ok(if v.len() == 1 { v.pop().unwrap() } else { Node::Alt(v) })
    }

    fn concat(&mut self) -> Result<Node, Invalid> {
        let mut v = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            v.push(self.quantified(atom)?);
        }
        Ok(match v.len() {
            0 => Node::Empty,
            1 => v.pop().unwrap(),
            _ => Node::Concat(v),
        })
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect::<String>().parse().ok()
    }

    fn quantified(&mut self, mut atom: Node) -> Result<Node, Invalid> {
        loop {
            let (min, max) = match self.peek() {
                Some('*') => (0, None),
                Some('+') => (1, None),
                Some('?') => (0, Some(1)),
                Some('{') => {
                    self.i += 1;
                    let min = self.number().ok_or(Invalid)?;
                    let max = if self.peek() == Some(',') {
                        self.i += 1;
                        if self.peek() == Some('}') { None } else { Some(self.number().ok_or(Invalid)?) }
                    } else {
                        Some(min)
                    };
                    if self.peek() != Some('}') || max.is_some_and(|m| m < min) {
                        return Err(Invalid);
                    }
                    self.i += 1;
                    atom = Node::Repeat(Box::new(atom), min, max);
                    continue;
                }
                _ => return Ok(atom),
            };
            self.i += 1;
            atom = Node::Repeat(Box::new(atom), min, max);
        }
    }

    fn atom(&mut self) -> Result<Node, Invalid> {
        let c = self.peek().ok_or(Invalid)?;
        self.i += 1;
        Ok(match c {
            '(' => {
                self.groups += 1;
                let n = self.groups;
                let inner = self.alt()?;
                if self.peek() != Some(')') {
                    return Err(Invalid);
                }
                self.i += 1;
                Node::Group(n, Box::new(inner))
            }
            '.' => Node::Any,
            '^' => Node::Bol,
            '$' => Node::Eol,
            '[' => self.class()?,
            '\\' => {
                let c = self.peek().ok_or(Invalid)?;
                self.i += 1;
                Node::Char(c)
            }
            '*' | '+' | '?' | '{' => return Err(Invalid),
            c => Node::Char(c),
        })
    }

    fn class(&mut self) -> Result<Node, Invalid> {
        let neg = self.peek() == Some('^');
        if neg {
            self.i += 1;
        }
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().ok_or(Invalid)?;
            if c == ']' && !first {
                self.i += 1;
                break;
            }
            first = false;
            self.i += 1;
            if c == '[' && self.peek() == Some(':') {
                let rest: String = self.s[self.i + 1..].iter().collect();
                let end = rest.find(":]").ok_or(Invalid)?;
                items.push(ClassItem::Named(rest[..end].to_string()));
                self.i += 1 + rest[..end].chars().count() + 2;
                continue;
            }
            if self.peek() == Some('-') && self.s.get(self.i + 1).is_some_and(|&d| d != ']') {
                let hi = self.s[self.i + 1];
                if hi < c {
                    return Err(Invalid);
                }
                self.i += 2;
                items.push(ClassItem::Range(c, hi));
            } else {
                items.push(ClassItem::Char(c));
            }
        }
        Ok(Node::Class(neg, items))
    }
}

fn named(n: &str, c: char) -> bool {
    match n {
        "alpha" => c.is_alphabetic(),
        "digit" => c.is_ascii_digit(),
        "alnum" => c.is_alphanumeric(),
        "upper" => c.is_uppercase(),
        "lower" => c.is_lowercase(),
        "space" => c.is_whitespace(),
        "blank" => c == ' ' || c == '\t',
        "punct" => c.is_ascii_punctuation(),
        "xdigit" => c.is_ascii_hexdigit(),
        "cntrl" => c.is_control(),
        "print" => !c.is_control(),
        "graph" => !c.is_control() && c != ' ',
        _ => false,
    }
}

impl Regex {
    pub fn new(p: &str) -> Result<Regex, Invalid> {
        let s: Vec<char> = p.chars().collect();
        let mut ps = Parser { s: &s, i: 0, groups: 0 };
        let node = ps.alt()?;
        if ps.i != s.len() {
            return Err(Invalid);
        }
        Ok(Regex { node, groups: ps.groups })
    }

    /// The POSIX match in `x` starting the search at `from` (0-based).
    pub fn find(&self, x: &[char], from: usize) -> Result<Option<Match>, TooComplex> {
        for start in from..=x.len() {
            let mut m = Matcher {
                x,
                steps: 0,
                best: None,
            };
            let mut caps = vec![None; self.groups + 1];
            m.run(&self.node, start, &mut caps)?;
            if let Some((end, mut caps)) = m.best {
                caps[0] = Some((start, end - start));
                return Ok(Some(caps));
            }
        }
        Ok(None)
    }
}

const STEP_LIMIT: usize = 2_000_000;

struct Matcher<'a> {
    x: &'a [char],
    steps: usize,
    best: Option<(usize, Match)>,
}

type Caps = Vec<Option<(usize, usize)>>;

impl Matcher<'_> {
    /// Whether a match (end, caps) ranks above the best so far: longer;
    /// then, subexpression by subexpression, leftmost and then longest.
    fn offer(&mut self, end: usize, caps: &Caps) {
        let better = match &self.best {
            None => true,
            Some((be, bc)) => {
                if end != *be {
                    end > *be
                } else {
                    let mut r = false;
                    for (a, b) in caps.iter().zip(bc).skip(1) {
                        match (a, b) {
                            (Some(a), Some(b)) if a == b => continue,
                            (Some(a), Some(b)) => {
                                r = a.0 < b.0 || (a.0 == b.0 && a.1 > b.1);
                                break;
                            }
                            (Some(_), None) => {
                                r = true;
                                break;
                            }
                            (None, Some(_)) => break,
                            (None, None) => continue,
                        }
                    }
                    r
                }
            }
        };
        if better {
            self.best = Some((end, caps.clone()));
        }
    }

    /// Matches the whole pattern at `pos`; every way it can end is offered.
    fn run(&mut self, n: &Node, pos: usize, caps: &mut Caps) -> Result<(), TooComplex> {
        self.seq(&[n], pos, caps, &mut |m: &mut Matcher, e, c| {
            m.offer(e, c);
            Ok(())
        })
    }

    /// Matches the nodes one after another, then `k`.
    fn seq(
        &mut self,
        ns: &[&Node],
        pos: usize,
        caps: &mut Caps,
        k: &mut dyn FnMut(&mut Matcher, usize, &mut Caps) -> Result<(), TooComplex>,
    ) -> Result<(), TooComplex> {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return Err(TooComplex);
        }
        let Some((first, rest)) = ns.split_first() else {
            return k(self, pos, caps);
        };
        let x = self.x;
        let one = |ok: bool| ok.then_some(pos + 1);
        let simple = match first {
            Node::Empty => Some(Some(pos)),
            Node::Char(c) => Some(one(x.get(pos) == Some(c))),
            Node::Any => Some(one(pos < x.len())),
            Node::Class(neg, items) => Some(one(x.get(pos).is_some_and(|&c| {
                let hit = items.iter().any(|it| match it {
                    ClassItem::Char(d) => c == *d,
                    ClassItem::Range(a, b) => (*a..=*b).contains(&c),
                    ClassItem::Named(n) => named(n, c),
                });
                hit != *neg
            }))),
            Node::Bol => Some((pos == 0).then_some(pos)),
            Node::Eol => Some((pos == x.len()).then_some(pos)),
            _ => None,
        };
        if let Some(next) = simple {
            return match next {
                Some(p) => self.seq(rest, p, caps, k),
                None => Ok(()),
            };
        }
        match first {
            Node::Concat(v) => {
                let mut all: Vec<&Node> = v.iter().collect();
                all.extend_from_slice(rest);
                self.seq(&all, pos, caps, k)
            }
            Node::Alt(v) => {
                for a in v {
                    let mut all = vec![a];
                    all.extend_from_slice(rest);
                    self.seq(&all, pos, caps, k)?;
                }
                Ok(())
            }
            Node::Group(g, inner) => {
                let g = *g;
                let saved = caps[g];
                let rest: Vec<&Node> = rest.to_vec();
                self.seq(&[inner.as_ref()], pos, caps, &mut |m, e, c| {
                    let inner_saved = c[g];
                    c[g] = Some((pos, e - pos));
                    let r = m.seq(&rest, e, c, k);
                    c[g] = inner_saved;
                    r
                })?;
                caps[g] = saved;
                Ok(())
            }
            Node::Repeat(inner, min, max) => {
                self.repeat(inner, *min, *max, 0, pos, rest, caps, k)
            }
            _ => unreachable!(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &mut self,
        inner: &Node,
        min: u32,
        max: Option<u32>,
        done: u32,
        pos: usize,
        rest: &[&Node],
        caps: &mut Caps,
        k: &mut dyn FnMut(&mut Matcher, usize, &mut Caps) -> Result<(), TooComplex>,
    ) -> Result<(), TooComplex> {
        if done >= min {
            self.seq(rest, pos, caps, k)?;
        }
        if max.is_some_and(|m| done >= m) {
            return Ok(());
        }
        let rest: Vec<&Node> = rest.to_vec();
        self.seq(&[inner], pos, caps, &mut |m, e, c| {
            // an iteration that matches nothing ends the repetition
            if e == pos && done >= min {
                return Ok(());
            }
            m.repeat(inner, min, max, done + 1, e, &rest, c, k)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(p: &str, x: &str, from: usize) -> Option<Match> {
        let x: Vec<char> = x.chars().collect();
        Regex::new(p).unwrap().find(&x, from).unwrap()
    }

    #[test]
    fn lrm_examples() {
        let x = "12Allan3Allen4Alan5Allan678";
        assert_eq!(find("All(a|e)n", x, 0).unwrap()[0], Some((2, 5)));
        assert_eq!(find("All(a|e)n", x, 3).unwrap()[0], Some((8, 5)));
        assert_eq!(find("All(a|e)n", x, 9).unwrap()[0], Some((19, 5)));
        assert!(find("All(a|e)n", x, 20).is_none());
        assert_eq!(find("[hc]+at", "the cat in the hat", 0).unwrap()[0], Some((4, 3)));
    }

    #[test]
    fn leftmost_longest() {
        assert_eq!(find("a|ab|abc", "xabcd", 0).unwrap()[0], Some((1, 3)));
        // the POSIX rule: the first subexpression is longest first
        let m = find("(a|ab)(c|bcd)(d*)", "abcd", 0).unwrap();
        assert_eq!(m[0], Some((0, 4)));
        assert_eq!(m[1], Some((0, 2)));
        assert_eq!(m[2], Some((2, 1)));
        assert_eq!(m[3], Some((3, 1)));
        assert_eq!(find("x*", "aaa", 0).unwrap()[0], Some((0, 0)));
        assert_eq!(find("a{2,3}", "aaaa", 0).unwrap()[0], Some((0, 3)));
    }

    #[test]
    fn lrm_subexpressions() {
        let p = "([a-zA-Z]+) * ([a-zA-Z]+) * ((([a-zA-Z1-9]+)\\.){0,1}([a-zA-Z1-9]+))";
        let m = find(p, " CREATE DATABASE TESTDB;", 0).unwrap();
        assert_eq!(m[1], Some((1, 6)), "{m:?}");
        assert_eq!(m[2], Some((8, 8)));
        assert_eq!(m[3], Some((17, 6)));
    }

    #[test]
    fn invalid() {
        assert!(Regex::new("a(b").is_err());
        assert!(Regex::new("*a").is_err());
        assert!(Regex::new("[a").is_err());
        assert!(Regex::new("a{3,1}").is_err());
    }
}
