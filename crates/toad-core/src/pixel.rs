//! Pixel widths of titles and descriptions as Google renders them.
//! Widths are Arial advance widths in font units (2048 per em). Desktop
//! results use Arial 20px for titles and 14px for descriptions.

const UNITS_PER_EM: f64 = 2048.0;
pub const TITLE_FONT_PX: f64 = 20.0;
pub const DESCRIPTION_FONT_PX: f64 = 14.0;

/// Advance widths for ASCII 32 (space) to 126 (~).
const ARIAL: [u16; 95] = [
    569, 569, 727, 1139, 1139, 1821, 1366, 391, 682, 682, 797, 1196, 569, 682, 569, 569, // ' ' to '/'
    1139, 1139, 1139, 1139, 1139, 1139, 1139, 1139, 1139, 1139, // 0-9
    569, 569, 1196, 1196, 1196, 1139, 2079, // : ; < = > ? @
    1366, 1366, 1479, 1479, 1366, 1251, 1593, 1479, 569, 1024, 1366, 1139, 1706, // A-M
    1479, 1593, 1366, 1593, 1479, 1366, 1251, 1479, 1366, 1933, 1366, 1366, 1251, // N-Z
    569, 569, 569, 961, 1139, 682, // [ \ ] ^ _ `
    1139, 1139, 1024, 1139, 1139, 569, 1139, 1139, 455, 455, 1024, 455, 1706, // a-m
    1139, 1139, 1139, 1139, 682, 1024, 569, 1139, 1024, 1479, 1024, 1024, 1024, // n-z
    684, 532, 684, 1196, // { | } ~
];

/// Fallback for characters outside ASCII: roughly a lowercase letter.
const DEFAULT_UNITS: u16 = 1139;

pub fn text_width_px(text: &str, font_px: f64) -> u32 {
    let units: u64 = text
        .chars()
        .map(|c| {
            let code = c as u32;
            if (32..=126).contains(&code) {
                ARIAL[(code - 32) as usize] as u64
            } else if c.is_whitespace() {
                ARIAL[0] as u64
            } else if is_wide(c) {
                2048
            } else {
                DEFAULT_UNITS as u64
            }
        })
        .sum();
    (units as f64 * font_px / UNITS_PER_EM).round() as u32
}

pub fn title_px(text: &str) -> u32 {
    text_width_px(text, TITLE_FONT_PX)
}

pub fn description_px(text: &str) -> u32 {
    text_width_px(text, DESCRIPTION_FONT_PX)
}

/// CJK and similar full-width characters take a full em.
fn is_wide(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFF00..=0xFF60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_are_sensible() {
        assert_eq!(title_px(""), 0);
        // "Home" in Arial 20px is about 55px.
        let w = title_px("Home");
        assert!((50..=60).contains(&w), "{w}");
        assert!(title_px("WWWW") > title_px("iiii"));
    }
}
