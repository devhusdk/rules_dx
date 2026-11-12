use super::{join, quote, word};

#[test]
fn quote_survives_spaces_and_quotes() {
    assert_eq!(quote("/a b/c"), "'/a b/c'");
    assert_eq!(quote("it's"), "'it'\\''s'");
    assert_eq!(quote("plain"), "'plain'");
}

#[test]
fn word_matches_the_allowlist() {
    assert_eq!(word("50%,core"), "50%,core");
    assert_eq!(word("tag:v1,x=y@z+w"), "tag:v1,x=y@z+w");
    assert_eq!(word("my repo"), "'my repo'");
    assert_eq!(word("a$b"), "'a$b'");
    assert_eq!(word(""), "''");
}

#[test]
fn join_quotes_each_word() {
    let words = ["a".to_owned(), "b c".to_owned()];
    assert_eq!(join(&words), "a 'b c'");
}
