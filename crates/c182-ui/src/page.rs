//! Display snapshot of one management page. The aircraft adapter fills it; the
//! view draws it and reports which control was used. Nothing here names an
//! aircraft part: labels, states and tones all arrive as data.

/// One clickable source action: the identifier the adapter resolves, the text
/// shown on it, whether it can be used now and the click events it accepts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Control {
    pub id: u32,
    pub label: String,
    pub enabled: bool,
    pub mask: u32,
}

/// One option of a [`Item::Choice`]; `selected` marks the current one.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub control: Control,
    pub selected: bool,
}

/// One measured level in a [`Item::Bars`] chart. `level` is 0 to 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub label: String,
    pub level: f32,
    pub text: String,
    pub tone: u32,
    pub action: Option<Control>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A named thing, its present state and, when work is available, the
    /// action that changes it. Tone 0 draws no condition dot.
    Status {
        label: String,
        state: String,
        tone: u32,
        note: String,
        action: Option<Control>,
    },
    /// An on/off setting. The control flips it; `state` words the position.
    Toggle {
        label: String,
        on: bool,
        state: String,
        control: Control,
    },
    /// One of several. Each option is its own action. Without a label the
    /// options fill the width and wrap onto further lines.
    Choice { label: String, options: Vec<Choice> },
    /// A value with a decrease and an increase action.
    Stepper {
        label: String,
        value: String,
        minus: Control,
        plus: Control,
    },
    /// Plain actions side by side.
    Actions(Vec<Control>),
    /// Measured levels as vertical bars.
    Bars(Vec<Bar>),
    /// A caption and its value.
    Field {
        label: String,
        value: String,
        tone: u32,
    },
    /// Free text.
    Text { text: String, tone: u32 },
}

/// A titled group of items in the left (0) or right (1) column.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panel {
    pub title: String,
    pub column: usize,
    pub items: Vec<Item>,
}

/// A banner above the page content, such as the reason a page is locked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notice {
    pub text: String,
    pub tone: u32,
    pub action: Option<Control>,
}

/// A decision that must be confirmed or cancelled before the page is used
/// again. The rest of the page is drawn disabled beneath it.
#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub title: String,
    pub items: Vec<Item>,
    pub confirm: Control,
    pub cancel: Control,
}

/// One action of the plain two-column list: a button, a −/+ pair when `plus`
/// is set, or a slider when `value` is set.
#[derive(Clone, Debug)]
pub struct Row {
    pub id: u32,
    pub plus: Option<u32>,
    pub label: String,
    pub section: String,
    pub reason: String,
    pub column: usize,
    pub enabled: bool,
    pub plus_enabled: bool,
    pub visible: bool,
    pub mask: u32,
    pub plus_mask: u32,
    pub tone: u32,
    pub value: Option<(f64, f64, f64)>,
}

#[derive(Clone, Debug)]
pub struct Readout {
    pub text: String,
    pub tone: u32,
}

/// A page is drawn from `panels`, `notice` and `dialog` when any is present,
/// and otherwise as the plain list of `rows` beside `readouts` and `report`.
#[derive(Clone, Debug)]
pub struct Page {
    pub title: String,
    pub tab: Option<usize>,
    pub ready: bool,
    pub opacity: f32,
    pub rows: Vec<Row>,
    pub readouts: Vec<Readout>,
    pub report: String,
    pub notice: Option<Notice>,
    pub panels: Vec<Panel>,
    pub dialog: Option<Dialog>,
    /// Whether the window is popped out to its own operating-system window,
    /// for the header's pop-out control; `None` draws no control.
    pub popout: Option<bool>,
}
impl Default for Page {
    fn default() -> Self {
        Self {
            title: String::new(),
            tab: None,
            ready: true,
            opacity: 1.,
            rows: vec![],
            readouts: vec![],
            report: String::new(),
            notice: None,
            panels: vec![],
            dialog: None,
            popout: None,
        }
    }
}
impl Page {
    /// Whether the page uses panels rather than the plain list.
    pub fn structured(&self) -> bool {
        !self.panels.is_empty() || self.dialog.is_some() || self.notice.is_some()
    }
}

/// Removes the layout whitespace of fixed-position source text: tabs, runs of
/// spaces, trailing spaces and repeated blank lines.
pub fn tidy(text: &str) -> String {
    let mut lines: Vec<String> = vec![];
    for line in text.split('\n') {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() && lines.last().is_none_or(String::is_empty) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::tidy;
    #[test]
    fn tidy_drops_layout_whitespace_and_keeps_paragraphs() {
        assert_eq!(
            tidy("\n\t\t\n  Outside Temp:   15 C \n\n\n\n\t Cabin is Cool\r\n\n"),
            "Outside Temp: 15 C\n\nCabin is Cool"
        );
        assert_eq!(tidy(" \n\t\n"), "");
    }
}
