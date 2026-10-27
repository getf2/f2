//! The search field's cheat sheet (its ?, or Help > Search Tips): what
//! fsearch's query language can do, each example a click from trying.

use gpui::*;

use super::Finder;
use super::chrome::TOOLBAR_HEIGHT;
use crate::style::{MEDIUM, SEMIBOLD, symbol};

/// Sections of examples, each with what it finds.
const TIPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Names",
        &[
            ("invoice", "Names like it, typos forgiven (invoce works too)"),
            ("tax 2024", "Names with both words"),
            ("\"google chrome\"", "Words together, space and all"),
            ("'main.rs", "These letters exactly, no typos"),
            ("^Screenshot", "Names starting with it"),
            (".pdf$", "Names ending with it"),
            ("readme !draft", "Leave out names with a word"),
        ],
    ),
    (
        "Patterns",
        &[
            ("*.pdf", "Any name, then .pdf"),
            ("*.{jpg,png}", "Either ending"),
            ("Developer/readme", "In a folder named like Developer"),
            ("Developer/**/*.md", "Anywhere below that folder"),
            ("readme !node_modules/", "Not inside folders named so"),
        ],
    ),
    (
        "Filters",
        &[
            ("ext:pdf,docx", "These extensions"),
            ("type:image", "Or video, audio, doc, code, archive, font, app"),
            ("kind:folder", "Or file, link"),
            ("size:>1gb", "Or <, or 1gb..5gb (b kb mb gb tb)"),
            ("mtime:<7d", "Changed in the last 7 days (min h d w mo y)"),
            ("mtime:>1y", "Not changed for a year"),
            ("in:~/Documents", "Inside a folder, whatever the scope"),
            ("re:^\\d+\\.txt$", "Names matching a regex"),
            ("path:screenshots", "Paths matching a regex"),
        ],
    ),
    (
        "Inside files",
        &[
            ("grep:TODO", "Text in code and text files"),
            ("regex:fn\\s+main", "A regex, line by line"),
            ("sym:main", "Where a function or type is defined"),
            ("ext:rs grep:unwrap", "Only in some files"),
        ],
    ),
    (
        "Together",
        &[
            ("invoice ext:pdf mtime:<1y", "Invoice PDFs from the past year"),
            ("type:image size:>5mb mtime:<30d", "Big images from the past month"),
        ],
    ),
];

/// Examples at most this long sit beside what they find; longer ones above.
const BESIDE: usize = 22;

impl Finder {
    pub(super) fn toggle_search_tips(&mut self, cx: &mut Context<Self>) {
        self.search_tips = !self.search_tips;
        cx.notify();
    }

    /// Help > Search Tips: the tips, with the search field ready.
    pub(super) fn show_search_tips(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_tips = true;
        self.focus_search(window, cx);
    }

    /// Search for an example (by name: Spotlight doesn't speak this).
    fn try_tip(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search_contents = false;
        self.query.update(cx, |q, cx| {
            q.set_value(text, window, cx);
            q.focus(window, cx);
        });
        self.run_search(cx);
    }

    /// The tips under the search field, over the files.
    pub(super) fn render_search_tips(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.search_tips {
            return None;
        }
        let palette = self.palette;
        let mut sections: Vec<AnyElement> = Vec::new();
        for (n, (title, tips)) in TIPS.iter().enumerate() {
            let mut rows: Vec<AnyElement> = Vec::new();
            for (i, (example, finds)) in tips.iter().enumerate() {
                let example = *example;
                let chip = div()
                    .id(("tip", n * 100 + i))
                    .flex_shrink_0()
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(px(5.))
                    .bg(palette.glass_selected)
                    .font_family("Menlo")
                    .text_size(px(11.))
                    .text_color(palette.label)
                    .cursor_pointer()
                    .hover(|s| s.bg(palette.accent.opacity(0.3)))
                    .role(Role::Button)
                    .aria_label(format!("Search for {example}"))
                    .child(example)
                    .on_click(cx.listener(move |this, _, window, cx| this.try_tip(example, window, cx)));
                let finds = div().min_w_0().text_size(px(12.)).text_color(palette.secondary).child(*finds);
                let row = if example.chars().count() <= BESIDE {
                    div().flex().items_center().child(div().w(px(170.)).flex_shrink_0().flex().child(chip)).child(finds)
                } else {
                    div().flex().flex_col().items_start().gap(px(2.)).child(chip).child(finds.pl(px(6.)))
                };
                rows.push(row.py(px(2.)).into_any_element());
            }
            sections.push(
                div()
                    .flex()
                    .flex_col()
                    .child(div().mt_3().mb_1().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.tertiary).child(*title))
                    .children(rows)
                    .into_any_element(),
            );
        }
        let close = div()
            .id("tips-close")
            .size(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover(|s| s.bg(palette.glass_selected))
            .role(Role::Button)
            .aria_label("Close Search Tips")
            .child(symbol("xmark", 10., SEMIBOLD, palette.secondary, window))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_search_tips(cx)));
        let tall = window.viewport_size().height - px(TOOLBAR_HEIGHT + 24.);
        Some(
            div()
                .id("search-tips")
                .absolute()
                .top(px(TOOLBAR_HEIGHT - 4.))
                .right(px(12.))
                .w(px(470.))
                .max_h(tall)
                .overflow_y_scroll()
                .p_4()
                .pt_3()
                .rounded(px(14.))
                .bg(palette.sidebar)
                .border_1()
                .border_color(palette.glass_border)
                .shadow_2xl()
                .text_size(px(12.))
                .occlude()
                .role(Role::Dialog)
                .aria_label("Search Tips")
                // A click here leaves the keyboard in the search field.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).text_size(px(13.)).child("Search Tips"))
                        .child(close),
                )
                .child(div().mt_1().text_color(palette.secondary).child(
                    "Click one to try it. Searches go by name, in this folder or the whole Mac (pick in the bar under the toolbar). No need to type filters: Kind, Date and Size in that bar pick them.",
                ))
                .children(sections)
                .child(
                    div()
                        .mt_3()
                        .pt_3()
                        .border_t_1()
                        .border_color(palette.separator)
                        .flex()
                        .gap_2()
                        .text_color(palette.secondary)
                        .child(symbol("doc.text.magnifyingglass", 13., MEDIUM, palette.secondary, window))
                        .child(div().flex_1().child(
                            "Contents, in that bar, looks inside PDFs and documents with Spotlight instead: plain words there, not these.",
                        )),
                )
                .into_any_element(),
        )
    }
}
