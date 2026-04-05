use rrrrr_rs::compile::{
    compilation_error::CompilationError,
    ident::Symbol,
    pass::read::{lex::tokenize, token::Token},
    sexpr::{Bool, Char, Num, Str},
    span::Span,
};

#[test]
fn test_tokenize_empty() {
    assert_eq!(
        tokenize("").unwrap(),
        vec![Token::EoF(Span { lo: 0, hi: 0 })]
    );
}

#[test]
fn test_tokenize_multiline() {
    let src = "`(#())
; #
\"ab\" ; #
; #
\"\" 9.0001 0 -3 -42.00 -100 some-symbol <=? list->vector ;
2 #t #\\ (...) \"

  \"  \" 123
    456
\"";
    assert_eq!(
        tokenize(src).unwrap(),
        vec![
            Token::QuasiQuote(Span { lo: 0, hi: 1 }),
            Token::LParen(Span { lo: 1, hi: 2 }),
            Token::HashLParen(Span { lo: 2, hi: 4 }),
            Token::RParen(Span { lo: 4, hi: 5 }),
            Token::RParen(Span { lo: 5, hi: 6 }),
            Token::Str(Str("ab".to_string()), Span { lo: 11, hi: 15 }),
            Token::Str(Str("".to_string()), Span { lo: 24, hi: 26 }),
            Token::Num(Num(9.0001), Span { lo: 27, hi: 33 }),
            Token::Num(Num(0.0), Span { lo: 34, hi: 35 }),
            Token::Num(Num(-3.0), Span { lo: 36, hi: 38 }),
            Token::Num(Num(-42.0), Span { lo: 39, hi: 45 }),
            Token::Num(Num(-100.0), Span { lo: 46, hi: 50 }),
            Token::Id(Symbol::new("some-symbol"), Span { lo: 51, hi: 62 }),
            Token::Id(Symbol::new("<=?"), Span { lo: 63, hi: 66 }),
            Token::Id(Symbol::new("list->vector"), Span { lo: 67, hi: 79 }),
            Token::Num(Num(2.0), Span { lo: 82, hi: 83 }),
            Token::Bool(Bool(true), Span { lo: 84, hi: 86 }),
            Token::Char(Char(' '), Span { lo: 87, hi: 90 }),
            Token::LParen(Span { lo: 90, hi: 91 }),
            Token::Id(Symbol::new("..."), Span { lo: 91, hi: 94 }),
            Token::RParen(Span { lo: 94, hi: 95 }),
            Token::Str(Str("\n\n  ".to_string()), Span { lo: 96, hi: 102 }),
            Token::Str(
                Str(" 123\n    456\n".to_string()),
                Span { lo: 104, hi: 119 }
            ),
            Token::EoF(Span { lo: 119, hi: 119 })
        ]
    );
}

#[test]
fn test_tokenize_minus_as_identifier() {
    assert_eq!(
        tokenize("-").unwrap(),
        vec![
            Token::Id(Symbol::new("-"), Span { lo: 0, hi: 1 }),
            Token::EoF(Span { lo: 1, hi: 1 }),
        ]
    );
}

#[test]
fn test_tokenize_minus_identifier_in_list_head() {
    assert_eq!(
        tokenize("(- 1 2)").unwrap(),
        vec![
            Token::LParen(Span { lo: 0, hi: 1 }),
            Token::Id(Symbol::new("-"), Span { lo: 1, hi: 2 }),
            Token::Num(Num(1.0), Span { lo: 3, hi: 4 }),
            Token::Num(Num(2.0), Span { lo: 5, hi: 6 }),
            Token::RParen(Span { lo: 6, hi: 7 }),
            Token::EoF(Span { lo: 7, hi: 7 }),
        ]
    );
}

#[test]
fn test_tokenize_hyphen_prefixed_identifier() {
    assert_eq!(
        tokenize("-foo").unwrap(),
        vec![
            Token::Id(Symbol::new("-foo"), Span { lo: 0, hi: 4 }),
            Token::EoF(Span { lo: 4, hi: 4 }),
        ]
    );
}

#[test]
fn test_tokenize_escape_double_quote() {
    let result = tokenize(
        r#"

        "\""

        "#,
    )
    .unwrap();
    assert_eq!(
        result[0],
        Token::Str(Str("\"".to_string()), Span { lo: 10, hi: 14 })
    );
}

#[test]
fn test_tokenize_escape_slashes() {
    let result = tokenize(
        r#"

        "\\"

        "#,
    )
    .unwrap();
    assert_eq!(
        result[0],
        Token::Str(Str("\\".to_string()), Span { lo: 10, hi: 14 })
    );
}

#[test]
fn test_tokenize_escape_multiple_slashes() {
    let result = tokenize(
        r#"

        "\\\""

        "#,
    )
    .unwrap();
    assert_eq!(
        result[0],
        Token::Str(Str("\\\"".to_string()), Span { lo: 10, hi: 16 })
    );
}

#[test]
fn test_tokenize_escape_multiple_slashes_and_double_quote() {
    let result = tokenize(
        r#"

        "\\\"\\\"\\"

        "#,
    )
    .unwrap();
    assert_eq!(
        result[0],
        Token::Str(Str("\\\"\\\"\\".to_string()), Span { lo: 10, hi: 22 })
    );
}

#[test]
fn test_tokenize_unterminated_single_line_string() {
    let res = tokenize("\"");
    assert!(
        matches!(
            res,
            Err(CompilationError {
                span: Span { lo: 0, hi: 1 },
                reason: _
            })
        ),
        "{:?}",
        res
    );

    let res = tokenize("1   \"");
    assert!(
        matches!(
            res,
            Err(CompilationError {
                span: Span { lo: 4, hi: 5 },
                reason: _
            })
        ),
        "{:?}",
        res
    );
}

#[test]
fn test_tokenize_multibyte_utf8_in_string() {
    let src = "\"λ\" 42";
    let tokens = tokenize(src).unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Str(Str("λ".to_string()), Span { lo: 0, hi: 4 }),
            Token::Num(Num(42.0), Span { lo: 5, hi: 7 }),
            Token::EoF(Span { lo: 7, hi: 7 }),
        ]
    );
}

#[test]
fn test_tokenize_multibyte_utf8_char_literal() {
    let src = "#\\λ 1";
    let tokens = tokenize(src).unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Char(Char('λ'), Span { lo: 0, hi: 4 }),
            Token::Num(Num(1.0), Span { lo: 5, hi: 6 }),
            Token::EoF(Span { lo: 6, hi: 6 }),
        ]
    );
}

#[test]
fn test_tokenize_named_char_literals() {
    let src = "#\\space #\\newline";
    let tokens = tokenize(src).unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Char(Char(' '), Span { lo: 0, hi: 7 }),
            Token::Char(Char('\n'), Span { lo: 8, hi: 17 }),
            Token::EoF(Span { lo: 17, hi: 17 }),
        ]
    );
}

#[test]
fn test_tokenize_named_char_literals_at_eof() {
    assert_eq!(
        tokenize("#\\space").unwrap(),
        vec![
            Token::Char(Char(' '), Span { lo: 0, hi: 7 }),
            Token::EoF(Span { lo: 7, hi: 7 }),
        ]
    );
    assert_eq!(
        tokenize("#\\newline").unwrap(),
        vec![
            Token::Char(Char('\n'), Span { lo: 0, hi: 9 }),
            Token::EoF(Span { lo: 9, hi: 9 }),
        ]
    );
}

#[test]
fn test_tokenize_unterminated_multiline_string() {
    let res = tokenize("\"\n123\n456\n\" \"\n123\n456\n");
    assert!(
        matches!(
            res,
            Err(CompilationError {
                span: Span { lo: 12, hi: 22 },
                reason: _
            })
        ),
        "{:?}",
        res
    );
}
