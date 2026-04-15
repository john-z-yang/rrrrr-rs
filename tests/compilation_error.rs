use rrrrr_rs::compile::{compilation_error::CompilationError, span::Span};

#[test]
fn test_pretty_printer() {
    let error = CompilationError {
        span: Span { lo: 1522, hi: 1574 },
        reason: "Hallelujah!".to_owned(),
    };

    let source = r#"
I was taught assembler in my second year of school.
It's kinda like construction work with a toothpick for a tool.
So when I made my senior year, I threw my code away,
And learned the way to program that I still prefer today.

Now, some folks on the Internet put their faith in C++.
They swear that it's so powerful, it's what God used for us.
And maybe it lets mortals dredge their objects from the C.
But I think that explains why only God can make a tree.

For God wrote in Lisp code when he filled the leaves with green.
The fractal flowers and recursive roots: the most lovely hack I've seen.
And when I ponder snowflakes, never finding two the same,
I know God likes a language with its own four-letter name.

Now, I've used a SUN under Unix, so I've seen what C can hold.
I've surfed for Perls, found what Fortran's for, got that Java stuff down cold.
Though the chance that I'd write COBOL code is a SNOBOL's chance in Hell.
And I basically hate hieroglyphs, so I won't use APL.

Now, God must know all these languages, and a few I haven't named.
But the Lord made sure, when each sparrow falls, that its flesh will be reclaimed.
And the Lord could not count grains of sand with a 32-bit word.
Who knows where we would go to if Lisp weren't what he preferred?

And God wrote in Lisp code every creature great and small.
Don't search the disk drive for man.c, when the listing's on the wall.
And when I watch the lightning burn unbelievers to a crisp,
I know God had six days to work. So he wrote it all in Lisp.

Yes, God had a deadline. So he wrote it all in Lisp.
"#;
    let output = format!("{}", error.pprint_with_source(source));
    assert_eq!(
        output,
        "Error: Hallelujah!
 --> 32:1:
    |
 32 | Yes, God had a deadline. So he wrote it all in Lisp.
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
"
    )
}
