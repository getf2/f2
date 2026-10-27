//! The search's filters, picked instead of typed, as Google Drive's chips
//! under its search: what kind of file, when it last changed, how big, and
//! where (Downloads, Desktop…, whatever the folder you're in). Each
//! is a chip in the bar under the toolbar ("Kind ⌄"); picked, it says what
//! ("PDFs ✕"). They're fsearch's filters underneath (type:, ext:, kind:,
//! mtime:, size:, in:), added to what's typed; for Contents (Spotlight) and
//! a saved search, Spotlight's own (and its scope). With two or more, Clear
//! drops them all.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::Finder;
use crate::macos;
use crate::style::{SEMIBOLD, symbol};

/// What kind of file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Folders,
    Images,
    Videos,
    Music,
    Documents,
    Pdfs,
    Presentations,
    Spreadsheets,
    Archives,
    Apps,
    Code,
    Fonts,
}

impl Kind {
    pub const ALL: [Kind; 12] = [
        Kind::Folders,
        Kind::Images,
        Kind::Videos,
        Kind::Music,
        Kind::Documents,
        Kind::Pdfs,
        Kind::Presentations,
        Kind::Spreadsheets,
        Kind::Archives,
        Kind::Apps,
        Kind::Code,
        Kind::Fonts,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Folders => "Folders",
            Kind::Images => "Images",
            Kind::Videos => "Videos",
            Kind::Music => "Music",
            Kind::Documents => "Documents",
            Kind::Pdfs => "PDFs",
            Kind::Presentations => "Presentations",
            Kind::Spreadsheets => "Spreadsheets",
            Kind::Archives => "Archives",
            Kind::Apps => "Apps",
            Kind::Code => "Code",
            Kind::Fonts => "Fonts",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Kind::Folders => "folder",
            Kind::Images => "photo",
            Kind::Videos => "film",
            Kind::Music => "music.note",
            Kind::Documents => "doc.text",
            Kind::Pdfs => "doc.richtext",
            Kind::Presentations => "rectangle.on.rectangle",
            Kind::Spreadsheets => "tablecells",
            Kind::Archives => "archivebox",
            Kind::Apps => "app",
            Kind::Code => "chevron.left.forwardslash.chevron.right",
            Kind::Fonts => "textformat",
        }
    }

    /// As fsearch has it.
    fn query(self) -> &'static str {
        match self {
            Kind::Folders => "kind:dir",
            Kind::Images => "type:image",
            Kind::Videos => "type:video",
            Kind::Music => "type:audio",
            Kind::Documents => "type:doc",
            Kind::Pdfs => "ext:pdf",
            Kind::Presentations => "ext:key,ppt,pptx",
            Kind::Spreadsheets => "ext:xls,xlsx,numbers,csv",
            Kind::Archives => "type:archive",
            Kind::Apps => "type:app",
            Kind::Code => "type:code",
            Kind::Fonts => "type:font",
        }
    }

    /// As Spotlight has it: the kind's content types.
    fn spotlight(self) -> String {
        let types: &[&str] = match self {
            Kind::Folders => &["public.folder"],
            Kind::Images => &["public.image"],
            Kind::Videos => &["public.movie"],
            Kind::Music => &["public.audio"],
            Kind::Documents => &["public.text", "public.composite-content", "com.adobe.pdf"],
            Kind::Pdfs => &["com.adobe.pdf"],
            Kind::Presentations => &["public.presentation"],
            Kind::Spreadsheets => &["public.spreadsheet"],
            Kind::Archives => &["public.archive", "com.apple.disk-image"],
            Kind::Apps => &["com.apple.application"],
            Kind::Code => &["public.source-code"],
            Kind::Fonts => &["public.font"],
        };
        let each: Vec<String> = types.iter().map(|t| format!("kMDItemContentTypeTree == \"{t}\"")).collect();
        format!("({})", each.join(" || "))
    }
}

/// When it last changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Changed {
    Today,
    Week,
    Month,
    Year,
    Older,
}

impl Changed {
    pub const ALL: [Changed; 5] = [Changed::Today, Changed::Week, Changed::Month, Changed::Year, Changed::Older];

    pub fn name(self) -> &'static str {
        match self {
            Changed::Today => "Today",
            Changed::Week => "Past 7 Days",
            Changed::Month => "Past 30 Days",
            Changed::Year => "Past Year",
            Changed::Older => "Over a Year Ago",
        }
    }

    /// As fsearch has it ("today": since midnight, here).
    fn query(self) -> String {
        match self {
            Changed::Today => {
                let now = chrono::Local::now();
                let since = now.time().signed_duration_since(chrono::NaiveTime::MIN).num_seconds().max(1);
                format!("mtime:<{since}s")
            }
            Changed::Week => "mtime:<7d".into(),
            Changed::Month => "mtime:<30d".into(),
            Changed::Year => "mtime:<1y".into(),
            Changed::Older => "mtime:>1y".into(),
        }
    }

    fn spotlight(self) -> &'static str {
        match self {
            Changed::Today => "kMDItemFSContentChangeDate >= $time.today",
            Changed::Week => "kMDItemFSContentChangeDate >= $time.today(-7)",
            Changed::Month => "kMDItemFSContentChangeDate >= $time.today(-30)",
            Changed::Year => "kMDItemFSContentChangeDate >= $time.today(-365)",
            Changed::Older => "kMDItemFSContentChangeDate < $time.today(-365)",
        }
    }
}

/// How big.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Small,
    Medium,
    Large,
    Huge,
}

impl Size {
    pub const ALL: [Size; 4] = [Size::Small, Size::Medium, Size::Large, Size::Huge];

    pub fn name(self) -> &'static str {
        match self {
            Size::Small => "Under 1 MB",
            Size::Medium => "1 to 100 MB",
            Size::Large => "Over 100 MB",
            Size::Huge => "Over 1 GB",
        }
    }

    fn query(self) -> &'static str {
        match self {
            Size::Small => "size:<1mb",
            Size::Medium => "size:1mb..100mb",
            Size::Large => "size:>100mb",
            Size::Huge => "size:>1gb",
        }
    }

    fn spotlight(self) -> &'static str {
        match self {
            Size::Small => "kMDItemFSSize < 1000000",
            Size::Medium => "(kMDItemFSSize >= 1000000 && kMDItemFSSize <= 100000000)",
            Size::Large => "kMDItemFSSize > 100000000",
            Size::Huge => "kMDItemFSSize > 1000000000",
        }
    }
}

/// Where to look, whatever the folder you're in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Desktop,
    Documents,
    Downloads,
    ICloud,
    Pictures,
    Movies,
    Music,
    Applications,
    Home,
}

impl Place {
    pub const ALL: [Place; 9] = [Place::Desktop, Place::Documents, Place::Downloads, Place::ICloud, Place::Pictures, Place::Movies, Place::Music, Place::Applications, Place::Home];

    pub fn name(self) -> &'static str {
        match self {
            Place::Desktop => "Desktop",
            Place::Documents => "Documents",
            Place::Downloads => "Downloads",
            Place::ICloud => "iCloud Drive",
            Place::Pictures => "Pictures",
            Place::Movies => "Movies",
            Place::Music => "Music",
            Place::Applications => "Applications",
            Place::Home => "Home Folder",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Place::Desktop => "menubar.dock.rectangle",
            Place::Documents => "doc",
            Place::Downloads => "arrow.down.circle",
            Place::ICloud => "icloud",
            Place::Pictures => "photo",
            Place::Movies => "film",
            Place::Music => "music.note",
            Place::Applications => "app",
            Place::Home => "house",
        }
    }

    pub fn path(self) -> PathBuf {
        let home = super::home();
        match self {
            Place::Desktop => home.join("Desktop"),
            Place::Documents => home.join("Documents"),
            Place::Downloads => home.join("Downloads"),
            Place::ICloud => home.join("Library/Mobile Documents/com~apple~CloudDocs"),
            Place::Pictures => home.join("Pictures"),
            Place::Movies => home.join("Movies"),
            Place::Music => home.join("Music"),
            Place::Applications => PathBuf::from("/Applications"),
            Place::Home => home,
        }
    }

    /// As fsearch has it (quoted: iCloud Drive's has a space). Applications
    /// are macOS's too, as Finder's Applications shows them (Calculator,
    /// Notes are in /System/Applications).
    fn query(self) -> String {
        match self {
            Place::Applications => "path:^/(System/)?Applications/".into(),
            _ => format!("in:\"{}\"", self.path().display()),
        }
    }
}

/// The filters picked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Filters {
    pub kind: Option<Kind>,
    pub changed: Option<Changed>,
    pub size: Option<Size>,
    pub place: Option<Place>,
}

impl Filters {
    pub fn any(&self) -> bool {
        self.count() > 0
    }

    /// How many are picked.
    pub fn count(&self) -> usize {
        [self.kind.is_some(), self.changed.is_some(), self.size.is_some(), self.place.is_some()].into_iter().filter(|on| *on).count()
    }

    /// As fsearch has them, to follow what's typed.
    pub fn query(&self) -> String {
        [self.kind.map(|k| k.query().to_string()), self.changed.map(Changed::query), self.size.map(|s| s.query().to_string()), self.place.map(Place::query)]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Where Spotlight's to look instead, if it's picked.
    pub fn scope(&self) -> Option<PathBuf> {
        self.place.map(Place::path)
    }

    /// As Spotlight has them, each to be met.
    pub fn spotlight(&self) -> Vec<String> {
        [self.kind.map(Kind::spotlight), self.changed.map(|c| c.spotlight().to_string()), self.size.map(|s| s.spotlight().to_string())].into_iter().flatten().collect()
    }

    /// What they are, in words: "PDFs, Past 7 Days, Over 100 MB, in
    /// Downloads".
    pub fn said(&self) -> String {
        let place = self.place.map(|p| format!("in {}", p.name()));
        [self.kind.map(Kind::name), self.changed.map(Changed::name), self.size.map(Size::name)].into_iter().flatten().map(str::to_string).chain(place).collect::<Vec<_>>().join(", ")
    }
}

/// What's typed, in fsearch's language (plain words as `natural` takes
/// them: "photos from last week"), and the filters after it.
pub fn full_query(typed: &str, filters: &Filters) -> String {
    let typed = crate::natural::translate(typed).map_or_else(|| typed.to_string(), |plain| plain.query);
    [typed.trim().to_string(), filters.query()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Which chip.
#[derive(Clone, Copy, PartialEq)]
enum Chip {
    Kind,
    Changed,
    Size,
    Place,
}

impl Finder {
    /// The filters changed: searched again (or not searching, with none
    /// and nothing typed).
    fn set_filters(&mut self, filters: Filters, cx: &mut Context<Self>) {
        self.filters = filters;
        self.title_dirty = true;
        self.run_search(cx);
        cx.notify();
    }

    /// A chip's menu: "Any …", then the choices, the one picked checked.
    fn chip_menu(&mut self, chip: Chip, cx: &mut Context<Self>) {
        let now = self.filters;
        let (any, names, picked): (&str, Vec<&'static str>, Option<usize>) = match chip {
            Chip::Kind => ("Any Kind", Kind::ALL.iter().map(|k| k.name()).collect(), now.kind.and_then(|k| Kind::ALL.iter().position(|x| *x == k))),
            Chip::Changed => ("Any Time", Changed::ALL.iter().map(|c| c.name()).collect(), now.changed.and_then(|c| Changed::ALL.iter().position(|x| *x == c))),
            Chip::Size => ("Any Size", Size::ALL.iter().map(|s| s.name()).collect(), now.size.and_then(|s| Size::ALL.iter().position(|x| *x == s))),
            Chip::Place => ("Anywhere", Place::ALL.iter().map(|p| p.name()).collect(), now.place.and_then(|p| Place::ALL.iter().position(|x| *x == p))),
        };
        let mut items = vec![
            macos::MenuItem { title: Some(any.into()), tag: 0, enabled: true, checked: picked.is_none(), ..Default::default() },
            macos::MenuItem { title: None, ..Default::default() },
        ];
        for (i, name) in names.iter().enumerate() {
            // (Over a year ago: the other way, after a line; and the home
            // folder, the whole of it, after one.)
            if (chip == Chip::Changed && Changed::ALL[i] == Changed::Older) || (chip == Chip::Place && Place::ALL[i] == Place::Applications) {
                items.push(macos::MenuItem { title: None, ..Default::default() });
            }
            items.push(macos::MenuItem { title: Some(name.to_string()), tag: i as i32 + 1, enabled: true, checked: picked == Some(i), ..Default::default() });
        }
        // (Not inside this event's handling: the menu runs its own loop.)
        cx.spawn(async move |this, cx| {
            let Some(tag) = macos::popup_menu(&items) else { return };
            let at = (tag as usize).checked_sub(1);
            this.update(cx, |this, cx| {
                let mut filters = this.filters;
                match chip {
                    Chip::Kind => filters.kind = at.and_then(|i| Kind::ALL.get(i).copied()),
                    Chip::Changed => filters.changed = at.and_then(|i| Changed::ALL.get(i).copied()),
                    Chip::Size => filters.size = at.and_then(|i| Size::ALL.get(i).copied()),
                    Chip::Place => filters.place = at.and_then(|i| Place::ALL.get(i).copied()),
                }
                this.set_filters(filters, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The chips: "Kind ⌄", "Date ⌄", "Size ⌄", "Where ⌄"; one picked shows
    /// what, in the accent, with ✕ to drop it; with two or more, Clear.
    pub(super) fn render_filter_chips(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let palette = self.palette;
        let chips = [
            (Chip::Kind, "Kind", self.filters.kind.map(|k| (k.name(), k.symbol()))),
            (Chip::Changed, "Date", self.filters.changed.map(|c| (c.name(), "calendar"))),
            (Chip::Size, "Size", self.filters.size.map(|s| (s.name(), "internaldrive"))),
            (Chip::Place, "Where", self.filters.place.map(|p| (p.name(), p.symbol()))),
        ];
        let clear = (self.filters.count() >= 2).then(|| {
            div()
                .id("filters-clear")
                .h(px(22.))
                .px(px(6.))
                .flex()
                .flex_shrink_0()
                .items_center()
                .rounded_full()
                .text_color(palette.secondary)
                .cursor_pointer()
                .hover(|s| s.bg(palette.glass_selected).text_color(palette.label))
                .role(Role::Button)
                .aria_label("Clear Filters")
                .child("Clear")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.set_filters(Filters::default(), cx);
                    }),
                )
                .into_any_element()
        });
        let mut shown: Vec<AnyElement> = chips
            .into_iter()
            .map(|(chip, label, picked)| {
                let on = picked.is_some();
                let color = if on { palette.accent } else { palette.secondary };
                let id = match chip {
                    Chip::Kind => "filter-kind",
                    Chip::Changed => "filter-date",
                    Chip::Size => "filter-size",
                    Chip::Place => "filter-where",
                };
                let clear = on.then(|| {
                    div()
                        .id((id, 1usize))
                        .size(px(16.))
                        .ml(px(2.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .hover(|s| s.bg(palette.accent.opacity(0.2)))
                        .role(Role::Button)
                        .aria_label(format!("Remove {label} Filter"))
                        .child(symbol("xmark", 8., SEMIBOLD, color, window))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                // (The keyboard stays in the search field.)
                                window.prevent_default();
                                cx.stop_propagation();
                                let mut filters = this.filters;
                                match chip {
                                    Chip::Kind => filters.kind = None,
                                    Chip::Changed => filters.changed = None,
                                    Chip::Size => filters.size = None,
                                    Chip::Place => filters.place = None,
                                }
                                this.set_filters(filters, cx);
                            }),
                        )
                });
                div()
                    .id(id)
                    .h(px(22.))
                    .pl(px(8.))
                    .pr(px(if on { 3. } else { 6. }))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(4.))
                    .rounded_full()
                    .border_1()
                    .when(on, |d| d.bg(palette.accent.opacity(0.14)).border_color(palette.accent.opacity(0.35)).text_color(palette.accent))
                    .when(!on, |d| d.border_color(palette.separator).text_color(palette.secondary).hover(|s| s.bg(palette.glass_selected)))
                    .cursor_pointer()
                    .role(Role::Button)
                    .aria_label(match picked {
                        Some((name, _)) => format!("{label}: {name}"),
                        None => format!("{label} Filter"),
                    })
                    .children(picked.map(|(_, icon)| symbol(icon, 11., SEMIBOLD, color, window)))
                    .child(picked.map_or(label, |(name, _)| name))
                    .when(!on, |d| d.child(symbol("chevron.down", 8., SEMIBOLD, color, window)))
                    .children(clear)
                    // On press (the keyboard staying in the search field).
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.chip_menu(chip, cx);
                        }),
                    )
                    .into_any_element()
            })
            .collect();
        shown.extend(clear);
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::{Changed, Filters, Kind, Place, Size, full_query};

    #[test]
    fn queries() {
        let none = Filters::default();
        assert_eq!(full_query("invoice", &none), "invoice");
        let pdfs = Filters { kind: Some(Kind::Pdfs), changed: Some(Changed::Week), size: None, place: None };
        assert_eq!(full_query("invoice", &pdfs), "invoice ext:pdf mtime:<7d");
        assert_eq!(full_query("", &pdfs), "ext:pdf mtime:<7d");
        assert_eq!(pdfs.said(), "PDFs, Past 7 Days");
        let big = Filters { kind: None, changed: None, size: Some(Size::Medium), place: None };
        assert_eq!(big.query(), "size:1mb..100mb");
        assert_eq!(full_query("screenshots", &big).split_whitespace().last(), Some("size:1mb..100mb"), "plain words first, then the filters");
        assert!(Filters { kind: Some(Kind::Images), ..none }.spotlight()[0].contains("public.image"));
        // (Each parses, as fsearch takes it.)
        for kind in Kind::ALL {
            for changed in Changed::ALL {
                for size in Size::ALL {
                    let q = full_query("x", &Filters { kind: Some(kind), changed: Some(changed), size: Some(size), place: None });
                    assert!(fsearch::query::Query::parse(&q, "/Users/me").is_ok(), "{q}");
                }
            }
        }
        // (Where: in that folder, iCloud Drive's space and all.)
        let icloud = full_query("x", &Filters { place: Some(Place::ICloud), ..none });
        let q = fsearch::query::Query::parse(&icloud, &super::super::home().to_string_lossy()).unwrap();
        assert!(q.scope.as_deref().is_some_and(|s| String::from_utf8_lossy(s).ends_with("com~apple~CloudDocs")), "{icloud}");
        assert_eq!(Filters { kind: Some(Kind::Pdfs), place: Some(Place::Downloads), ..none }.said(), "PDFs, in Downloads");
        // (Applications: macOS's too, not just /Applications.)
        let apps = full_query("calculator", &Filters { place: Some(Place::Applications), ..none });
        let q = fsearch::query::Query::parse(&apps, "/Users/me").unwrap();
        assert!(q.match_path(b"/System/Applications/Calculator.app", 1, 0, 0).is_some(), "{apps}");
        assert!(q.match_path(b"/Applications/Calculator.app", 1, 0, 0).is_some(), "{apps}");
        assert!(q.match_path(b"/Users/me/Calculator.app", 1, 0, 0).is_none(), "{apps}");
    }
}
