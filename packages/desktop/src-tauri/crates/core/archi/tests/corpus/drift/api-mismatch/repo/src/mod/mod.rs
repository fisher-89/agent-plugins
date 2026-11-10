pub fn file_stem(timestamp: &str) -> String {
    timestamp
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}
