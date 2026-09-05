//! Splits an FFmpeg command written as one multi-line string into argv entries.
//!
//! No shell is involved, so this deliberately avoids shell backslash escaping:
//! on Windows every argument is full of `\`, and treating those as escapes
//! silently corrupts paths. `shell-words` and `shlex` follow POSIX and do
//! exactly that; `winsplit` keeps backslashes literal but ignores single quotes
//! and has no line continuation. Hence this.
//!
//! The rules, in full:
//!
//! - space, tab, carriage return and newline separate arguments;
//! - `'` and `"` both group, and behave identically: the quotes are removed and
//!   everything between them is kept verbatim, backslashes included;
//! - a backslash is always a literal character, never an escape;
//! - except that a backslash immediately followed by a line ending is a Bash
//!   style continuation and counts as whitespace.

/// Splits `input` into arguments suitable for `std::process::Command::args`.
///
/// # Errors
///
/// Returns an error if a quote is never closed, since the intent of the rest of
/// the line cannot be guessed.
pub fn parse(input: &str) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut args = Vec::new();
    let mut current = String::new();
    // Tracks `""` and `''`, which are arguments even though they hold nothing.
    let mut quoted = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // Line continuation. Handled here rather than in a pre-pass so that
            // a trailing backslash keeps its literal meaning everywhere else.
            '\\' if matches!(chars.peek(), Some('\n' | '\r')) => {
                if chars.peek() == Some(&'\r') {
                    chars.next();
                }

                // A lone carriage return is still a separator, so only consume a
                // newline when one actually follows.
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }

                push(&mut args, &mut current, &mut quoted);
            }

            ' ' | '\t' | '\r' | '\n' => push(&mut args, &mut current, &mut quoted),

            '\'' | '"' => {
                let closing = c;
                quoted = true;

                loop {
                    match chars.next() {
                        // Everything inside is verbatim, backslashes included,
                        // which is what keeps Windows paths intact.
                        Some(c) if c != closing => current.push(c),
                        Some(_) => break,
                        None => {
                            return Err(format!("Unterminated {closing} quote in: {input}").into());
                        }
                    }
                }
            }

            _ => current.push(c),
        }
    }

    push(&mut args, &mut current, &mut quoted);

    Ok(args)
}

/// Ends the argument being built, if there is one.
fn push(args: &mut Vec<String>, current: &mut String, quoted: &mut bool) {
    if !current.is_empty() || *quoted {
        args.push(std::mem::take(current));
        *quoted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(input: &str) -> Vec<String> {
        parse(input).expect("should parse")
    }

    #[test]
    fn keeps_backslashes_in_windows_paths() {
        assert_eq!(
            parsed(r"-i C:\Users\basse\clip.mp4"),
            ["-i", r"C:\Users\basse\clip.mp4"]
        );
    }

    #[test]
    fn handles_linux_paths() {
        assert_eq!(
            parsed("-i /home/basse/clips/clip.mp4"),
            ["-i", "/home/basse/clips/clip.mp4"]
        );
    }

    #[test]
    fn strips_single_quotes() {
        assert_eq!(parsed("-i 'output.mp4'"), ["-i", "output.mp4"]);
    }

    #[test]
    fn strips_double_quotes() {
        assert_eq!(parsed(r#"-i "output.mp4""#), ["-i", "output.mp4"]);
    }

    #[test]
    fn keeps_spaces_inside_quotes() {
        assert_eq!(
            parsed(r#"-metadata "title=Pasu Voltaic 91.4""#),
            ["-metadata", "title=Pasu Voltaic 91.4"]
        );
        assert_eq!(
            parsed("-vf 'drawtext=text=1w4ts reload 847'"),
            ["-vf", "drawtext=text=1w4ts reload 847"]
        );
    }

    #[test]
    fn treats_unix_continuation_as_whitespace() {
        assert_eq!(
            parsed("-c copy \\\n-preset slow"),
            ["-c", "copy", "-preset", "slow"]
        );
    }

    #[test]
    fn treats_windows_continuation_as_whitespace() {
        assert_eq!(
            parsed("-c copy \\\r\n-preset slow"),
            ["-c", "copy", "-preset", "slow"]
        );
    }

    #[test]
    fn mixes_quote_styles() {
        assert_eq!(
            parsed(r#"-i 'in file.mp4' -metadata "title=A B" -y"#),
            ["-i", "in file.mp4", "-metadata", "title=A B", "-y"]
        );
    }

    #[test]
    fn keeps_literal_backslashes_everywhere() {
        // Bare, single quoted and double quoted must all agree.
        assert_eq!(parsed(r"C:\file.mp4"), [r"C:\file.mp4"]);
        assert_eq!(parsed(r"'C:\file.mp4'"), [r"C:\file.mp4"]);
        assert_eq!(parsed(r#""C:\file.mp4""#), [r"C:\file.mp4"]);

        // A backslash not followed by a line ending stays put.
        assert_eq!(
            parsed(r"-vf movie=C:\Overlays\logo.png"),
            ["-vf", r"movie=C:\Overlays\logo.png"]
        );
    }

    #[test]
    fn parses_a_realistic_multiline_command() {
        let command = "ffmpeg -i \"C:\\file.mp4\" \\\n  -c copy \\\n  'output.mp4'\n";

        assert_eq!(
            parsed(command),
            ["ffmpeg", "-i", r"C:\file.mp4", "-c", "copy", "output.mp4"]
        );
    }

    #[test]
    fn reports_an_unterminated_quote() {
        assert!(parse(r#"-i "unclosed.mp4"#).is_err());
        assert!(parse("-i 'unclosed.mp4").is_err());
    }

    #[test]
    fn keeps_empty_quoted_arguments() {
        assert_eq!(parsed(r#"-metadata title="""#), ["-metadata", "title="]);
        assert_eq!(parsed(r#"a "" b"#), ["a", "", "b"]);
    }

    #[test]
    fn ignores_surrounding_and_repeated_whitespace() {
        assert_eq!(parsed("  \t-y\n\n-c  copy \r\n"), ["-y", "-c", "copy"]);
        assert_eq!(parsed(""), Vec::<String>::new());
    }

    #[test]
    fn accepts_quotes_partway_through_an_argument() {
        // FFmpeg options are commonly written `key="value with spaces"`.
        assert_eq!(
            parsed(r#"-metadata title="Hello World""#),
            ["-metadata", "title=Hello World"]
        );
        assert_eq!(
            parsed(r#"-x264-params "keyint=60:min-keyint=60""#),
            ["-x264-params", "keyint=60:min-keyint=60"]
        );
    }

    #[test]
    fn preserves_the_other_quote_style_when_nested() {
        // Filter graphs quote their own values, so the inner quotes have to
        // survive for FFmpeg's filter parser to see them.
        assert_eq!(
            parsed(r#"-vf "drawtext=text='Hello World':fontsize=20""#),
            ["-vf", "drawtext=text='Hello World':fontsize=20"]
        );
        assert_eq!(
            parsed(r#"-vf 'drawtext=text="Hello World"'"#),
            ["-vf", r#"drawtext=text="Hello World""#]
        );
    }

    #[test]
    fn keeps_apostrophes_inside_double_quotes() {
        // Which matters here: the game is called KovaaK's.
        assert_eq!(
            parsed(r#"-metadata "title=KovaaK's PB""#),
            ["-metadata", "title=KovaaK's PB"]
        );
    }

    #[test]
    fn leaves_unquoted_punctuation_alone() {
        assert_eq!(
            parsed("-filter_complex [0:v]scale=1280:-1[v];[v]fps=60[out]"),
            ["-filter_complex", "[0:v]scale=1280:-1[v];[v]fps=60[out]"]
        );
        assert_eq!(
            parsed("-c:v libx264 -b:v 6M"),
            ["-c:v", "libx264", "-b:v", "6M"]
        );
    }

    #[test]
    fn rejects_a_bare_apostrophe() {
        // A lone `'` opens a quote that never closes, exactly as a shell would
        // treat it. Quoting the value is the fix: "title=KovaaK's".
        assert!(parse("-metadata title=KovaaK's").is_err());
    }
}
