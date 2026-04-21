//! Expression evaluator — recursive descent, no external dependencies.

use std::collections::HashMap;

/// Evaluation error types.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    UnknownVariable(String),
    DivisionByZero,
    InvalidNumber(String),
    EmptyExpression,
    ExtraTokens(String),
}

/// Expression evaluator — recursive descent parser and evaluator.
pub struct ExprEvaluator {
    locals: HashMap<String, String>,
}

impl ExprEvaluator {
    /// Create a new evaluator with the given local variables.
    pub fn new(locals: HashMap<String, String>) -> Self {
        Self { locals }
    }

    /// Evaluate an arithmetic expression and return the result.
    pub fn evaluate(&self, expr: &str) -> Result<f64, EvalError> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err(EvalError::EmptyExpression);
        }
        let tokens = self.tokenize(expr)?;
        let mut parser = Parser::new(tokens, &self.locals);
        let result = parser.parse_expr()?;
        if parser.pos < parser.tokens.len() {
            let remaining = parser.tokens[parser.pos..]
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            return Err(EvalError::ExtraTokens(remaining));
        }
        Ok(result)
    }

    fn tokenize(&self, expr: &str) -> Result<Vec<Token>, EvalError> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '0'..='9' => {
                    let mut num = String::from(c);
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() || c == '.' {
                            num.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push(Token::Number(num));
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    let mut name = String::from(c);
                    while let Some(&c) = chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            name.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push(Token::Variable(name));
                }
                '+' => tokens.push(Token::Plus),
                '-' => tokens.push(Token::Minus),
                '*' => tokens.push(Token::Mul),
                '/' => tokens.push(Token::Div),
                '(' => tokens.push(Token::LParen),
                ')' => tokens.push(Token::RParen),
                ' ' | '\t' | '\n' => {} // skip whitespace
                _ => {
                    return Err(EvalError::InvalidNumber(format!(
                        "Unexpected character: {}",
                        c
                    )))
                }
            }
        }
        Ok(tokens)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(String),
    Variable(String),
    Plus,
    Minus,
    Mul,
    Div,
    LParen,
    RParen,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Number(n) => write!(f, "{}", n),
            Token::Variable(v) => write!(f, "{}", v),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Mul => write!(f, "*"),
            Token::Div => write!(f, "/"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
        }
    }
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    locals: std::cell::RefCell<&'a HashMap<String, String>>,
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token>, locals: &'a HashMap<String, String>) -> Self {
        Self {
            tokens,
            pos: 0,
            locals: std::cell::RefCell::new(locals),
        }
    }

    fn parse_expr(&mut self) -> Result<f64, EvalError> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<f64, EvalError> {
        let mut left = self.parse_mul_div()?;
        while let Some(token) = self.current() {
            match token {
                Token::Plus => {
                    self.advance();
                    left = left + self.parse_mul_div()?;
                }
                Token::Minus => {
                    self.advance();
                    left = left - self.parse_mul_div()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> Result<f64, EvalError> {
        let mut left = self.parse_unary()?;
        while let Some(token) = self.current() {
            match token {
                Token::Mul => {
                    self.advance();
                    left = left * self.parse_unary()?;
                }
                Token::Div => {
                    self.advance();
                    let rhs = self.parse_unary()?;
                    if rhs == 0.0 {
                        return Err(EvalError::DivisionByZero);
                    }
                    left = left / rhs;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<f64, EvalError> {
        if let Some(Token::Minus) = self.current() {
            self.advance();
            let val = self.parse_unary()?; // Recursive call to handle chained unary minus
            return Ok(-val);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<f64, EvalError> {
        let token = self.current().ok_or(EvalError::EmptyExpression)?;
        match token {
            Token::Number(ref s) => {
                self.advance();
                s.parse::<f64>()
                    .map_err(|_| EvalError::InvalidNumber(s.clone()))
            }
            Token::Variable(ref name) => {
                self.advance();
                let locals = self.locals.borrow();
                let value_str = locals
                    .get(name)
                    .ok_or_else(|| EvalError::UnknownVariable(name.clone()))?;
                value_str
                    .parse::<f64>()
                    .map_err(|_| EvalError::InvalidNumber(name.clone()))
            }
            Token::LParen => {
                self.advance();
                let result = self.parse_expr()?;
                if self.current() == Some(Token::RParen) {
                    self.advance();
                    Ok(result)
                } else {
                    Err(EvalError::InvalidNumber(
                        "Missing closing parenthesis".into(),
                    ))
                }
            }
            _ => Err(EvalError::InvalidNumber(format!("Unexpected token: {}", token))),
        }
    }

    fn current(&self) -> Option<Token> {
        self.tokens.get(self.pos).cloned()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_evaluator(locals: &[(&str, &str)]) -> ExprEvaluator {
        let locals: HashMap<String, String> = locals
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        ExprEvaluator::new(locals)
    }

    // Tokenization tests

    #[test]
    fn test_tokenize_numbers() {
        let eval = make_evaluator(&[]);
        let tokens = eval.tokenize("42").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number("42".to_string()));

        let tokens = eval.tokenize("3.14159").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number("3.14159".to_string()));

        let tokens = eval.tokenize("123.456").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Number("123.456".to_string()));
    }

    #[test]
    fn test_tokenize_variables() {
        let eval = make_evaluator(&[]);
        let tokens = eval.tokenize("x").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Variable("x".to_string()));

        let tokens = eval.tokenize("count").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Variable("count".to_string()));

        let tokens = eval.tokenize("_private").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Variable("_private".to_string()));

        let tokens = eval.tokenize("x1").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], Token::Variable("x1".to_string()));
    }

    #[test]
    fn test_tokenize_operators() {
        let eval = make_evaluator(&[]);
        let tokens = eval.tokenize("a + b").unwrap();
        assert_eq!(tokens, vec![
            Token::Variable("a".to_string()),
            Token::Plus,
            Token::Variable("b".to_string()),
        ]);

        let tokens = eval.tokenize("x - y * z").unwrap();
        assert_eq!(tokens, vec![
            Token::Variable("x".to_string()),
            Token::Minus,
            Token::Variable("y".to_string()),
            Token::Mul,
            Token::Variable("z".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_whitespace() {
        let eval = make_evaluator(&[]);
        let tokens = eval.tokenize("  x   +   y  ").unwrap();
        assert_eq!(tokens, vec![
            Token::Variable("x".to_string()),
            Token::Plus,
            Token::Variable("y".to_string()),
        ]);
    }

    #[test]
    fn test_tokenize_unexpected_char() {
        let eval = make_evaluator(&[]);
        let result = eval.tokenize("x @ y");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EvalError::InvalidNumber(_)));
    }

    // Evaluation tests

    #[test]
    fn test_eval_simple_addition() {
        let eval = make_evaluator(&[("x", "10"), ("y", "3")]);
        assert_eq!(eval.evaluate("x + y").unwrap(), 13.0);
    }

    #[test]
    fn test_eval_order_of_operations() {
        let eval = make_evaluator(&[("x", "10"), ("y", "3")]);
        // x + y * 2 = 10 + 6 = 16 (multiplication has higher precedence)
        assert_eq!(eval.evaluate("x + y * 2").unwrap(), 16.0);
        // (x + y) * 2 = 26
        assert_eq!(eval.evaluate("(x + y) * 2").unwrap(), 26.0);
    }

    #[test]
    fn test_eval_with_variables() {
        let eval = make_evaluator(&[("a", "5"), ("b", "2"), ("c", "4")]);
        // a + b * c = 5 + 8 = 13
        assert_eq!(eval.evaluate("a + b * c").unwrap(), 13.0);
        // (a + b) * c = 28
        assert_eq!(eval.evaluate("(a + b) * c").unwrap(), 28.0);
    }

    #[test]
    fn test_eval_unknown_variable() {
        let eval = make_evaluator(&[("x", "10")]);
        let result = eval.evaluate("x + z");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EvalError::UnknownVariable(v) if v == "z"));
    }

    #[test]
    fn test_eval_division_by_zero() {
        let eval = make_evaluator(&[("n", "0"), ("x", "10")]);
        let result = eval.evaluate("x / n");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EvalError::DivisionByZero));
    }

    #[test]
    fn test_eval_empty_expression() {
        let eval = make_evaluator(&[]);
        let result = eval.evaluate("");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EvalError::EmptyExpression));

        let result = eval.evaluate("   ");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EvalError::EmptyExpression));
    }

    #[test]
    fn test_eval_extra_tokens() {
        let eval = make_evaluator(&[("x", "10")]);
        let result = eval.evaluate("x +");
        eprintln!("x + result: {:?}", result);
        assert!(result.is_err());
        let err = result.unwrap_err();
        eprintln!("x + error: {:?}", err);
        // "x +" should produce an error - either ExtraTokens or EmptyExpression
        // depending on how the incomplete expression is handled
        assert!(matches!(err, EvalError::ExtraTokens(_) | EvalError::EmptyExpression));
    }

    #[test]
    fn test_eval_unary_minus() {
        let eval = make_evaluator(&[("x", "5")]);
        // Test single unary minus
        let r1 = eval.evaluate("-x");
        eprintln!("-x result: {:?}", r1);
        assert_eq!(r1.unwrap(), -5.0);
        // Test double unary minus
        let r2 = eval.evaluate("--x");
        eprintln!("--x result: {:?}", r2);
        assert_eq!(r2.unwrap(), 5.0);
        // Test unary minus with addition
        let r3 = eval.evaluate("-x + 3");
        eprintln!("-x + 3 result: {:?}", r3);
        assert_eq!(r3.unwrap(), -2.0);
    }

    #[test]
    fn test_eval_parentheses() {
        let eval = make_evaluator(&[("a", "2"), ("b", "3"), ("c", "4")]);
        assert_eq!(eval.evaluate("(a + b) * c").unwrap(), 20.0);
        assert_eq!(eval.evaluate("a * (b + c)").unwrap(), 14.0);
    }

    #[test]
    fn test_eval_nested_parentheses() {
        let eval = make_evaluator(&[("x", "2")]);
        assert_eq!(eval.evaluate("((x + 3) * 2)").unwrap(), 10.0);
        assert_eq!(eval.evaluate("(((x)))").unwrap(), 2.0);
    }

    #[test]
    fn test_eval_literal_numbers() {
        let eval = make_evaluator(&[]);
        assert_eq!(eval.evaluate("42").unwrap(), 42.0);
        assert_eq!(eval.evaluate("3.14").unwrap(), 3.14);
        assert_eq!(eval.evaluate("2 + 3").unwrap(), 5.0);
    }

    #[test]
    fn test_eval_all_operators() {
        let eval = make_evaluator(&[("a", "10"), ("b", "3")]);
        assert_eq!(eval.evaluate("a + b").unwrap(), 13.0);
        assert_eq!(eval.evaluate("a - b").unwrap(), 7.0);
        assert_eq!(eval.evaluate("a * b").unwrap(), 30.0);
        assert_eq!(eval.evaluate("a / b").unwrap(), 10.0 / 3.0);
    }

    #[test]
    fn test_eval_invalid_number_in_variable() {
        // Variable value that can't be parsed as f64
        let eval = make_evaluator(&[("name", "'hello'")]);
        let result = eval.evaluate("name + 1");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EvalError::InvalidNumber(_)));
    }

    #[test]
    fn test_eval_missing_closing_paren() {
        let eval = make_evaluator(&[("x", "5")]);
        let result = eval.evaluate("(x + 3");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EvalError::InvalidNumber(ref s) if s.contains("parenthesis")));
    }
}