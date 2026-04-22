fn char_to_morse(c: char) -> Option<&'static str> {
    match c.to_ascii_uppercase() {
        'A' => Some(".-"),
        'B' => Some("-..."),
        'C' => Some("-.-."),
        'D' => Some("-.."),
        'E' => Some("."),
        'F' => Some("..-."),
        'G' => Some("--."),
        'H' => Some("...."),
        'I' => Some(".."),
        'J' => Some(".---"),
        'K' => Some("-.-"),
        'L' => Some(".-.."),
        'M' => Some("--"),
        'N' => Some("-."),
        'O' => Some("---"),
        'P' => Some(".--."),
        'Q' => Some("--.-"),
        'R' => Some(".-."),
        'S' => Some("..."),
        'T' => Some("-"),
        'U' => Some("..-"),
        'V' => Some("...-"),
        'W' => Some(".--"),
        'X' => Some("-..-"),
        'Y' => Some("-.--"),
        'Z' => Some("--..")  ,
        '0' => Some("-----"),
        '1' => Some(".----"),
        '2' => Some("..---"),
        '3' => Some("...--"),
        '4' => Some("....-"),
        '5' => Some("....."),
        '6' => Some("-...."),
        '7' => Some("--..."),
        '8' => Some("---.." ),
        '9' => Some("----."),
        '.' => Some(".-.-.-"),
        ',' => Some("--..--"),
        '?' => Some("..--.."),
        '!' => Some("-.-.--"),
        '\'' => Some(".----."),
        '/' => Some("-..-."),
        '-' => Some("-....-"),
        _ => None,
    }
}

fn morse_to_char(morse: &str) -> Option<char> {
    match morse {
        ".-"     => Some('a'),
        "-..."   => Some('b'),
        "-.-."   => Some('c'),
        "-.."    => Some('d'),
        "."      => Some('e'),
        "..-."   => Some('f'),
        "--."    => Some('g'),
        "...."   => Some('h'),
        ".."     => Some('i'),
        ".---"   => Some('j'),
        "-.-"    => Some('k'),
        ".-.."   => Some('l'),
        "--"     => Some('m'),
        "-."     => Some('n'),
        "---"    => Some('o'),
        ".--."   => Some('p'),
        "--.-"   => Some('q'),
        ".-."    => Some('r'),
        "..."    => Some('s'),
        "-"      => Some('t'),
        "..-"    => Some('u'),
        "...-"   => Some('v'),
        ".--"    => Some('w'),
        "-..-"   => Some('x'),
        "-.--"   => Some('y'),
        "--.."   => Some('z'),
        "-----"  => Some('0'),
        ".----"  => Some('1'),
        "..---"  => Some('2'),
        "...--"  => Some('3'),
        "....-"  => Some('4'),
        "....."  => Some('5'),
        "-...."  => Some('6'),
        "--..."  => Some('7'),
        "---.."  => Some('8'),
        "----."  => Some('9'),
        ".-.-.-" => Some('.'),
        "--..--" => Some(','),
        "..--.." => Some('?'),
        "-.-.--" => Some('!'),
        ".----." => Some('\''),
        "-..-."  => Some('/'),
        "-....-" => Some('-'),
        _ => None,
    }
}

/// Encode a text string to Morse code.
/// Letters within a word are space-separated; words are separated by " / ".
/// Unrecognised characters are silently dropped.
pub fn encode(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter_map(char_to_morse)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Decode a Morse code string back to text.
/// Expects letters separated by single spaces and words by " / ".
pub fn decode(morse: &str) -> String {
    morse
        .split(" / ")
        .map(|word| {
            word.split_whitespace()
                .filter_map(morse_to_char)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let original = "hello world";
        assert_eq!(decode(&encode(original)), original);
    }

    #[test]
    fn numbers() {
        assert_eq!(decode(&encode("sos 911")), "sos 911");
    }

    #[test]
    fn known_encoding() {
        assert_eq!(encode("sos"), "... --- ...");
        assert_eq!(encode("hi there"), ".... .. / - .... . .-. .");
    }
}
