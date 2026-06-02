/// Errors raised while parsing or evaluating an arithmetic expression.
#[derive(Debug, PartialEq)]
pub enum CalcError {
    ParseError(String),
    DivisionByZero,
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalcError::ParseError(msg) => write!(f, "parse error: {msg}"),
            CalcError::DivisionByZero => write!(f, "division by zero"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, CalcError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
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
            _ if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let value = text
                    .parse::<f64>()
                    .map_err(|_| CalcError::ParseError(format!("invalid number '{text}'")))?;
                tokens.push(Token::Num(value));
            }
            _ => return Err(CalcError::ParseError(format!("unexpected character '{c}'"))),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos:    usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expr(&mut self) -> Result<f64, CalcError> {
        let mut value = self.term()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Plus => {
                    self.pos += 1;
                    value += self.term()?;
                }
                Token::Minus => {
                    self.pos += 1;
                    value -= self.term()?;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn term(&mut self) -> Result<f64, CalcError> {
        let mut value = self.factor()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Star => {
                    self.pos += 1;
                    value *= self.factor()?;
                }
                Token::Slash => {
                    self.pos += 1;
                    let rhs = self.factor()?;
                    if rhs == 0.0 {
                        return Err(CalcError::DivisionByZero);
                    }
                    value /= rhs;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn factor(&mut self) -> Result<f64, CalcError> {
        match self.advance() {
            Some(Token::Num(n)) => Ok(n),
            Some(Token::Minus) => Ok(-self.factor()?),
            Some(Token::LParen) => {
                let value = self.expr()?;
                match self.advance() {
                    Some(Token::RParen) => Ok(value),
                    _ => Err(CalcError::ParseError("expected ')'".to_string())),
                }
            }
            Some(other) => Err(CalcError::ParseError(format!("unexpected token {other:?}"))),
            None => Err(CalcError::ParseError("unexpected end of input".to_string())),
        }
    }
}

/// Evaluate an arithmetic expression with standard precedence and parentheses.
pub fn evaluate(input: &str) -> Result<f64, CalcError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(CalcError::ParseError("empty expression".to_string()));
    }
    let mut parser = Parser { tokens, pos: 0 };
    let value = parser.expr()?;
    if parser.pos != parser.tokens.len() {
        return Err(CalcError::ParseError("unexpected trailing input".to_string()));
    }
    Ok(value)
}

/// Heuristic for whether input looks like an arithmetic expression, used by the overlay.
///
/// The overlay performs the same check client-side; this mirror is exposed for callers
/// that decide server-side whether to evaluate input as a calculation.
#[allow(dead_code)]
pub fn looks_like_expression(input: &str) -> bool {
    let has_digit = input.chars().any(|c| c.is_ascii_digit());
    let trimmed = input.trim();
    let multiple_minus = input.matches('-').count() > 1;
    let minus_not_at_start = input.contains('-') && !trimmed.starts_with('-');
    let has_op = input.contains('+')
        || input.contains('*')
        || input.contains('/')
        || input.contains('(')
        || multiple_minus
        || minus_not_at_start;
    has_digit && has_op
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn addition() {
        assert!(approx(evaluate("1 + 2").unwrap(), 3.0));
    }

    #[test]
    fn precedence() {
        assert!(approx(evaluate("2 + 3 * 4").unwrap(), 14.0));
    }

    #[test]
    fn parentheses() {
        assert!(approx(evaluate("(2 + 3) * 4").unwrap(), 20.0));
    }

    #[test]
    fn float_input() {
        assert!(approx(evaluate("1.5 + 2.5").unwrap(), 4.0));
    }

    #[test]
    fn unary_minus() {
        assert!(approx(evaluate("-3 + 5").unwrap(), 2.0));
    }

    #[test]
    fn division_by_zero() {
        assert_eq!(evaluate("1 / 0"), Err(CalcError::DivisionByZero));
    }

    #[test]
    fn invalid_input() {
        assert!(matches!(evaluate("abc"), Err(CalcError::ParseError(_))));
    }

    #[test]
    fn looks_like_detects_arithmetic_and_rejects_filenames() {
        assert!(looks_like_expression("2 + 2"));
        assert!(looks_like_expression("(3*4)/2"));
        assert!(!looks_like_expression("report.pdf"));
        assert!(!looks_like_expression("hello"));
    }
}
