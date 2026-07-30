use ratatui::style::Color;

pub fn tc_success(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(98, 209, 117)
    } else {
        Color::Green
    }
}

pub fn tc_error(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(220, 80, 80)
    } else {
        Color::Red
    }
}

pub fn tc_warn(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(220, 180, 50)
    } else {
        Color::Yellow
    }
}

pub fn tc_muted(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(90, 90, 90)
    } else {
        Color::DarkGray
    }
}

pub fn tc_accent(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(32, 178, 170)
    } else {
        Color::Cyan
    }
}

pub fn tc_anchor(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(170, 80, 200)
    } else {
        Color::Magenta
    }
}
