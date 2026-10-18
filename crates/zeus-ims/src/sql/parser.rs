//! The COBOL grammar, rather than the broader Java/JDBC grammar.
use super::SqlError;

type Result<T> = std::result::Result<T, SqlError>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Name(String),
    Delimited(String),
    Text(String),
    Number(String),
    Symbol(char),
    Op(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Name {
    pub qualifier: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Text(String),
    Number(String),
    Parameter(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Projection {
    Column(Name),
    Aggregate(String, Option<Name>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Predicate {
    pub column: Name,
    pub op: String,
    pub value: Value,
}

/// Disjunction of conjunctions; AND binds more tightly than OR.
pub(crate) type Condition = Vec<Vec<Predicate>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Statement {
    Select {
        columns: Vec<Projection>, // empty means *
        tables: Vec<Name>,
        condition: Condition,
        group: Vec<Name>,
        order: Vec<(Name, bool)>, // true means DESC
    },
    Insert {
        table: Name,
        columns: Vec<Name>,
        values: Vec<Value>,
    },
    Update {
        table: Name,
        assignments: Vec<(Name, Value)>,
        condition: Condition,
    },
    Delete {
        table: Name,
        condition: Condition,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Prepared {
    pub statement: Statement,
    pub parameters: usize,
    pub semicolon: bool,
}

fn syntax(message: impl Into<String>) -> SqlError {
    SqlError::new("42601", message)
}

fn tokens(source: &str) -> Result<Vec<Token>> {
    let chars: Vec<_> = source.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '\'' || c == '"' {
            let quote = c;
            let mut value = String::new();
            i += 1;
            loop {
                let c = *chars
                    .get(i)
                    .ok_or_else(|| SqlError::new("42603", "unterminated quoted token"))?;
                i += 1;
                if c == quote {
                    if chars.get(i) == Some(&quote) {
                        i += 1;
                    } else {
                        break;
                    }
                }
                value.push(c);
            }
            if quote == '\'' {
                out.push(Token::Text(value));
            } else if value.is_empty() {
                return Err(syntax("empty delimited identifier"));
            } else {
                out.push(Token::Delimited(value));
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while chars
                .get(i)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                i += 1;
            }
            out.push(Token::Name(
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase(),
            ));
            continue;
        }
        if c.is_ascii_digit()
            || (matches!(c, '+' | '-' | '.') && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            if c == '+' || c == '-' {
                i += 1;
            }
            let mut digits = 0;
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                digits += 1;
                i += 1;
            }
            if chars.get(i) == Some(&'.') {
                i += 1;
                while chars.get(i).is_some_and(char::is_ascii_digit) {
                    digits += 1;
                    i += 1;
                }
            }
            if digits == 0 {
                return Err(syntax("numeric constant requires digits"));
            }
            out.push(Token::Number(chars[start..i].iter().collect()));
            continue;
        }
        if matches!(c, '=' | '<' | '>' | '!') {
            let mut op = c.to_string();
            i += 1;
            if let Some(&next) = chars.get(i)
                && (next == '=' || (c == '<' && next == '>'))
            {
                op.push(next);
                i += 1;
            }
            if !matches!(op.as_str(), "=" | "<>" | "!=" | "<" | ">" | "<=" | ">=") {
                return Err(syntax("invalid comparison operator"));
            }
            out.push(Token::Op(op));
            continue;
        }
        if matches!(c, ',' | '(' | ')' | '*' | '.' | '?' | ';') {
            out.push(Token::Symbol(c));
            i += 1;
            if c == ';' {
                break;
            }
        } else {
            return Err(syntax(format!("unexpected SQL character {c:?}")));
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    parameters: usize,
}

impl Parser {
    fn word(&mut self, word: &str) -> bool {
        if self.tokens.get(self.pos) == Some(&Token::Name(word.into())) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, word: &str) -> Result<()> {
        if self.word(word) {
            Ok(())
        } else {
            Err(syntax(format!("{word} expected")))
        }
    }
    fn symbol(&mut self, c: char) -> bool {
        if self.tokens.get(self.pos) == Some(&Token::Symbol(c)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_symbol(&mut self, c: char) -> Result<()> {
        if self.symbol(c) {
            Ok(())
        } else {
            Err(syntax(format!("{c} expected")))
        }
    }
    fn identifier(&mut self) -> Result<String> {
        match self.tokens.get(self.pos).cloned() {
            Some(Token::Delimited(name)) => {
                if name.chars().count() > 128 {
                    return Err(SqlError::new(
                        "42622",
                        "SQL identifier exceeds 128 characters",
                    ));
                }
                self.pos += 1;
                Ok(name)
            }
            Some(Token::Name(name)) => {
                if name.len() > 128 {
                    return Err(SqlError::new(
                        "42622",
                        "SQL identifier exceeds 128 characters",
                    ));
                }
                if matches!(
                    name.as_str(),
                    "SELECT"
                        | "FROM"
                        | "WHERE"
                        | "GROUP"
                        | "ORDER"
                        | "BY"
                        | "AND"
                        | "OR"
                        | "AS"
                        | "DISTINCT"
                        | "ALL"
                        | "JOIN"
                        | "INNER"
                        | "OUTER"
                        | "NULL"
                        | "DEFAULT"
                        | "INSERT"
                        | "UPDATE"
                        | "DELETE"
                        | "SET"
                        | "INTO"
                        | "VALUES"
                        | "ASC"
                        | "DESC"
                ) {
                    return Err(SqlError::new(
                        "42602",
                        "reserved word requires a delimited identifier",
                    ));
                }
                self.pos += 1;
                Ok(name)
            }
            _ => Err(syntax("identifier expected")),
        }
    }
    fn name(&mut self) -> Result<Name> {
        let first = self.identifier()?;
        if self.symbol('.') {
            Ok(Name {
                qualifier: Some(first),
                name: self.identifier()?,
            })
        } else {
            Ok(Name {
                qualifier: None,
                name: first,
            })
        }
    }
    fn names(&mut self) -> Result<Vec<Name>> {
        let mut names = vec![self.name()?];
        while self.symbol(',') {
            names.push(self.name()?);
        }
        Ok(names)
    }
    fn value(&mut self) -> Result<Value> {
        let value = match self.tokens.get(self.pos).cloned() {
            Some(Token::Text(s)) => Value::Text(s),
            Some(Token::Number(s)) => Value::Number(s),
            Some(Token::Symbol('?')) => {
                let i = self.parameters;
                self.parameters += 1;
                Value::Parameter(i)
            }
            _ => return Err(syntax("literal or parameter marker expected")),
        };
        self.pos += 1;
        Ok(value)
    }
    fn condition(&mut self) -> Result<Condition> {
        if !self.word("WHERE") {
            return Ok(Vec::new());
        }
        let mut groups = vec![Vec::new()];
        loop {
            let column = self.name()?;
            let Some(Token::Op(op)) = self.tokens.get(self.pos).cloned() else {
                return Err(syntax("comparison operator expected"));
            };
            self.pos += 1;
            let value = self.value()?;
            let group = groups.last_mut().unwrap();
            group.push(Predicate { column, op, value });
            if self.word("AND") {
                // The integer-specific limit is checked against catalog types.
            } else if self.word("OR") {
                groups.push(Vec::new());
            } else {
                break;
            }
        }
        Ok(groups)
    }
    fn statement(&mut self) -> Result<Statement> {
        if self.word("SELECT") {
            let mut columns = Vec::new();
            if !self.symbol('*') {
                loop {
                    let name = self.name()?;
                    if self.symbol('(') {
                        if name.qualifier.is_some()
                            || !matches!(
                                name.name.as_str(),
                                "SUM" | "AVG" | "MIN" | "MAX" | "COUNT"
                            )
                        {
                            return Err(SqlError::new(
                                "23610",
                                "unsupported COBOL aggregate function",
                            ));
                        }
                        let column = if self.symbol('*') {
                            if name.name != "COUNT" {
                                return Err(syntax("only COUNT accepts *"));
                            }
                            None
                        } else {
                            Some(self.name()?)
                        };
                        self.expect_symbol(')')?;
                        columns.push(Projection::Aggregate(name.name, column));
                    } else {
                        columns.push(Projection::Column(name));
                    }
                    if !self.symbol(',') {
                        break;
                    }
                }
            }
            self.expect("FROM")?;
            let tables = self.names()?;
            let condition = self.condition()?;
            let group = if self.word("GROUP") {
                self.expect("BY")?;
                self.names()?
            } else {
                Vec::new()
            };
            let mut order = Vec::new();
            if self.word("ORDER") {
                self.expect("BY")?;
                loop {
                    let name = self.name()?;
                    if self.symbol('(') {
                        return Err(SqlError::new("23612", "ORDER BY aggregate is unsupported"));
                    }
                    let descending = self.word("DESC");
                    if !descending {
                        self.word("ASC");
                    }
                    order.push((name, descending));
                    if !self.symbol(',') {
                        break;
                    }
                }
            }
            Ok(Statement::Select {
                columns,
                tables,
                condition,
                group,
                order,
            })
        } else if self.word("INSERT") {
            self.expect("INTO")?;
            let table = self.name()?;
            self.expect_symbol('(')?;
            let columns = self.names()?;
            self.expect_symbol(')')?;
            self.expect("VALUES")?;
            self.expect_symbol('(')?;
            let mut values = vec![self.value()?];
            while self.symbol(',') {
                values.push(self.value()?);
            }
            self.expect_symbol(')')?;
            if columns.len() != values.len() {
                return Err(SqlError::new(
                    "42802",
                    "INSERT column and value counts differ",
                ));
            }
            Ok(Statement::Insert {
                table,
                columns,
                values,
            })
        } else if self.word("UPDATE") {
            let table = self.name()?;
            self.expect("SET")?;
            let mut assignments = Vec::new();
            loop {
                let name = self.name()?;
                if self.tokens.get(self.pos) != Some(&Token::Op("=".into())) {
                    return Err(syntax("= expected"));
                }
                self.pos += 1;
                assignments.push((name, self.value()?));
                if !self.symbol(',') {
                    break;
                }
            }
            Ok(Statement::Update {
                table,
                assignments,
                condition: self.condition()?,
            })
        } else if self.word("DELETE") {
            self.expect("FROM")?;
            let table = self.name()?;
            Ok(Statement::Delete {
                table,
                condition: self.condition()?,
            })
        } else {
            Err(syntax(
                "IMS COBOL supports dynamic SELECT, INSERT, UPDATE and DELETE; use DL/I system services for commit and rollback",
            ))
        }
    }
}

pub(crate) fn prepare(source: &str) -> Result<Prepared> {
    if source.trim().is_empty() {
        return Err(SqlError::new("42617", "empty SQL statement source"));
    }
    if source.chars().count() > 32767 {
        return Err(SqlError::new("54001", "SQL statement exceeds 32767 bytes"));
    }
    let mut parser = Parser {
        tokens: tokens(source)?,
        pos: 0,
        parameters: 0,
    };
    let statement = parser.statement()?;
    let semicolon = parser.symbol(';');
    if parser.pos != parser.tokens.len() {
        return Err(syntax("unexpected trailing SQL clause"));
    }
    Ok(Prepared {
        statement,
        parameters: parser.parameters,
        semicolon,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_literals_and_delimited_names() {
        let p = prepare(
            "select \"mixed\" from pcb.seg where name='a  b''c' and amount>=-12.50 or id=?;",
        )
        .unwrap();
        assert_eq!(p.parameters, 1);
        let Statement::Select {
            columns, condition, ..
        } = p.statement
        else {
            panic!()
        };
        assert_eq!(
            columns,
            vec![Projection::Column(Name {
                qualifier: None,
                name: "mixed".into()
            })]
        );
        assert_eq!(condition.len(), 2);
        assert_eq!(condition[0][0].value, Value::Text("a  b'c".into()));
        assert_eq!(condition[0][1].value, Value::Number("-12.50".into()));
        prepare("SELECT \"FROM\" FROM \"WHERE\" WHERE \"OR\"=1").unwrap();
        assert!(prepare("SELECT FROM FROM SEGMENT").is_err());
    }
    #[test]
    fn parameter_positions_follow_statement_order() {
        let p = prepare("UPDATE P.S SET A=?, B=? WHERE ID=?").unwrap();
        assert_eq!(p.parameters, 3);
        let Statement::Update {
            assignments,
            condition,
            ..
        } = p.statement
        else {
            panic!()
        };
        assert_eq!(assignments[1].1, Value::Parameter(1));
        assert_eq!(condition[0][0].value, Value::Parameter(2));
    }
    #[test]
    fn accepts_cobol_aggregates_grouping_and_ordering() {
        prepare("SELECT KIND, SUM(AMOUNT), AVG(AMOUNT), MIN(ID), MAX(ID), COUNT(*) FROM P.S GROUP BY KIND ORDER BY KIND DESC").unwrap();
        prepare("INSERT INTO P.S (ID, NAME) VALUES (12, ?)").unwrap();
        prepare("DELETE FROM P.S WHERE ID<>12").unwrap();
    }
    #[test]
    fn rejects_java_only_and_unsupported_syntax() {
        for source in [
            "SELECT DISTINCT ID FROM P.S",
            "SELECT ID AS X FROM P.S",
            "SELECT * FROM P.S WHERE ID IN (1,2)",
            "SELECT * FROM P.S WHERE ID BETWEEN 1 AND 2",
            "SELECT * FROM P.S FETCH FIRST 1 ROWS ONLY",
            "SELECT SUM(*) FROM P.S",
            "COMMIT",
            "ROLLBACK",
            "INSERT INTO P.S (ID) VALUES (1,2)",
            "DELETE FROM P.S WHERE ID ! 1",
            "SELECT * FROM P.S WHERE ID='x",
        ] {
            assert!(prepare(source).is_err(), "accepted {source}");
        }
    }
    #[test]
    fn limits_consecutive_and_without_limiting_disjunctions() {
        let predicates = ["ID=1"; 11].join(" AND ");
        prepare(&format!("SELECT * FROM S WHERE {predicates}")).unwrap();
        prepare(&format!("SELECT * FROM S WHERE {predicates} AND ID=2")).unwrap();
        prepare(&format!(
            "SELECT * FROM S WHERE {predicates} OR {predicates}"
        ))
        .unwrap();
    }
}
