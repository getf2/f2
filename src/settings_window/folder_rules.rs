//! Settings › Folder Rules (src/rules.rs): the rules, each to turn on or
//! off, change or remove; a form for one, with what it would do right now;
//! starters; and what the rules did lately, to undo.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use gpui_kit::component::input::{Input, InputState};

use super::{SettingsWindow, Tab, home, tile};
use crate::rules::{self, Rule, Then};
use crate::settings::Settings;
use crate::style::{Palette, SEMIBOLD, glass, symbol};

/// Its tile's color.
pub(super) const COLOR: u32 = 0xbf5af2;

/// A rule's form, open: a new one (`index` None) or which.
pub(super) struct RuleForm {
    index: Option<usize>,
    /// Whether it's on (a new one is, once it's saved).
    on: bool,
    folder: PathBuf,
    kinds: Vec<String>,
    named: Entity<InputState>,
    /// Each: whether it's ticked, and its number.
    larger: (bool, Entity<InputState>),
    added: (bool, Entity<InputState>),
    unopened: (bool, Entity<InputState>),
    then: Then,
    error: Option<&'static str>,
    /// What it'd do now, for the rule as the form had it: how many, the
    /// first few names (None: still being looked at).
    preview: Option<(Rule, Option<(usize, Vec<String>)>)>,
    _watch: Vec<Subscription>,
}

impl RuleForm {
    /// The rule as the form has it; Err if a ticked number isn't one.
    fn rule(&self, cx: &App) -> Result<Rule, &'static str> {
        let number = |(on, input): &(bool, Entity<InputState>)| -> Result<Option<u32>, &'static str> {
            if !on {
                return Ok(None);
            }
            input.read(cx).value().trim().parse::<u32>().map(Some).map_err(|_| "Type a number of days (or megabytes) for what's ticked.")
        };
        Ok(Rule {
            on: self.on,
            folder: self.folder.clone(),
            kinds: self.kinds.clone(),
            named: self.named.read(cx).value().trim().to_string(),
            larger_mb: number(&self.larger)?,
            added_days: number(&self.added)?,
            unopened_days: number(&self.unopened)?,
            then: self.then.clone(),
        })
    }
}

/// Settings › Folder Rules, with a new rule's form open for `folder`
/// (a window's "Folder Rules…").
pub fn open_for(folder: PathBuf, cx: &mut App) {
    super::open_at("rules", cx);
    let Some((handle, view)) = cx.try_global::<super::Open>().and_then(|o| o.0.clone()) else { return };
    handle
        .update(cx, |_, window, cx| {
            view.update(cx, |this, cx| this.start_rule(None, Rule { folder, ..Rule::default() }, window, cx)).ok();
        })
        .ok();
}

/// A ticked box, as Settings' others.
fn check_box(on: bool, palette: &Palette, window: &Window) -> Div {
    div()
        .size(px(14.))
        .flex_shrink_0()
        .rounded_sm()
        .border_1()
        .border_color(palette.glass_border)
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window)))
}

/// One of a row of choices.
fn chip(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool, palette: &Palette) -> Stateful<Div> {
    div().id(id.into()).px_3().py(px(3.)).rounded_md().text_size(px(12.)).when(on, |d| d.bg(palette.glass_selected)).child(label.into())
}

impl SettingsWindow {
    /// Open the form for a new rule (`index` None) or one to change, as
    /// `rule` has it.
    pub(super) fn start_rule(&mut self, index: Option<usize>, rule: Rule, window: &mut Window, cx: &mut Context<Self>) {
        let field = |value: String, placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
        };
        let number = |n: Option<u32>, fallback: u32, window: &mut Window, cx: &mut Context<Self>| {
            (n.is_some(), field(n.unwrap_or(fallback).to_string(), "", window, cx))
        };
        let named = field(rule.named.clone(), "Any name", window, cx);
        let larger = number(rule.larger_mb, 100, window, cx);
        let added = number(rule.added_days, 7, window, cx);
        let unopened = number(rule.unopened_days, 30, window, cx);
        // (What it'd do follows what's typed.)
        let watch = [&named, &larger.1, &added.1, &unopened.1].into_iter().map(|i| cx.observe(i, |_, _, cx| cx.notify())).collect();
        self.tab = Tab::Rules;
        self.rule_form = Some(RuleForm {
            index,
            on: index.is_none() || rule.on,
            folder: rule.folder,
            kinds: rule.kinds,
            named,
            larger,
            added,
            unopened,
            then: rule.then,
            error: None,
            preview: None,
            _watch: watch,
        });
        cx.notify();
    }

    /// Keep the form's rule (if it can be kept), and close it.
    fn save_rule(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.rule_form else { return };
        let rule = match form.rule(cx) {
            Ok(rule) => rule,
            Err(e) => {
                form.error = Some(e);
                return cx.notify();
            }
        };
        if let Some(problem) = rule.problem() {
            form.error = Some(problem);
            return cx.notify();
        }
        let index = form.index;
        Settings::update(cx, |s| match index.filter(|&i| i < s.rules.len()) {
            Some(i) => s.rules[i] = rule,
            None => s.rules.push(rule),
        });
        self.rule_form = None;
        cx.notify();
    }

    /// A folder picked for the form: the one it's for, or (`to`) where it
    /// moves things.
    fn choose_folder(&mut self, to: bool, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = chosen.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| {
                    if let Some(form) = &mut this.rule_form {
                        if to { form.then = Then::Move(path) } else { form.folder = path }
                        form.error = None;
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// What the form's rule would do right now, looked at again (off the
    /// main thread) when the rule's changed.
    fn refresh_preview(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.rule_form else { return };
        let Ok(rule) = form.rule(cx) else {
            form.preview = None;
            return;
        };
        if form.preview.as_ref().is_some_and(|(r, _)| *r == rule) {
            return;
        }
        form.preview = Some((rule.clone(), None));
        let spared = cx.try_global::<rules::Log>().map(|l| l.spared.iter().cloned().collect()).unwrap_or_default();
        cx.spawn(async move |this, cx| {
            let looked = rule.clone();
            let found = cx
                .background_executor()
                .spawn(async move {
                    let (plan, _) = looked.plan(rules::now(), &spared);
                    let names = plan.iter().take(3).map(|(p, _)| p.file_name().unwrap_or_default().to_string_lossy().into_owned()).collect();
                    (plan.len(), names)
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(form) = &mut this.rule_form
                    && let Some((r, preview)) = &mut form.preview
                    && *r == rule
                {
                    *preview = Some(found);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_rules(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.refresh_preview(cx);
        let palette = self.palette;
        let heading = |text: &'static str| div().mt_4().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let editing = self.rule_form.as_ref().and_then(|f| f.index);
        let rows: Vec<Div> = settings
            .rules
            .iter()
            .enumerate()
            .map(|(i, rule)| {
                let trouble = rules::trouble(rule, cx);
                let on = rule.on;
                div()
                    .child(
                        div()
                            .px_2()
                            .py(px(6.))
                            .rounded_md()
                            .flex()
                            .items_center()
                            .gap_3()
                            .when(editing == Some(i), |d| d.bg(palette.glass_selected))
                            .child(div().id(("rule-on", i)).child(check_box(on, &palette, window)).on_click(move |_, _, cx| {
                                Settings::update(cx, |s| {
                                    if let Some(r) = s.rules.get_mut(i) {
                                        r.on = !r.on;
                                    }
                                })
                            }))
                            .child(tile("folder.fill", 0x0a84ff, window))
                            .child(div().w(px(110.)).flex_shrink_0().truncate().font_weight(FontWeight::MEDIUM).child(rules::folder_name(&rule.folder)))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(if on { palette.secondary } else { palette.tertiary }).child(rule.words()))
                            .child(
                                div()
                                    .id(("rule-edit", i))
                                    .size(px(22.))
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|d| d.bg(palette.glass_selected))
                                    .child(symbol("pencil", 11., 0., palette.secondary, window))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let Some(rule) = Settings::get(cx).rules.get(i).cloned() else { return };
                                        this.start_rule(Some(i), rule, window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .id(("rule-delete", i))
                                    .size(px(22.))
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|d| d.bg(palette.glass_selected))
                                    .child(symbol("trash", 11., 0., palette.secondary, window))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.rule_form = None;
                                        Settings::update(cx, |s| {
                                            if i < s.rules.len() {
                                                s.rules.remove(i);
                                            }
                                        });
                                    })),
                            ),
                    )
                    .when_some(trouble, |d, t| d.child(div().pl(px(64.)).text_size(px(11.)).text_color(rgb(0xff453a)).child(t)))
            })
            .collect();
        let lately: Vec<rules::Entry> = cx.try_global::<rules::Log>().map(|l| l.entries.iter().rev().take(8).cloned().collect()).unwrap_or_default();
        let starters = rules::starters();
        div()
            .child(div().mt_1().text_size(px(12.)).text_color(palette.secondary).child(
                "Keep a folder tidy on its own: now and then F2 looks in it, and moves what fits a rule. Only files loose in the folder, once they've finished arriving. Nothing is replaced or deleted (the Trash at most, after some days), and each move can be undone below.",
            ))
            .child(div().mt_3().flex().flex_col().gap_1().children(rows))
            .children(self.render_rule_form(window, cx))
            .when(self.rule_form.is_none(), |d| {
                d.child(div().mt_3().flex().child(glass(26., &palette).id("add-rule").px_3().child("Add Rule").on_click(cx.listener(|this, _, window, cx| {
                    this.start_rule(None, Rule::default(), window, cx)
                }))))
                .child(heading("Or start from one of these:"))
                .child(div().flex().flex_col().gap_1().children(starters.into_iter().enumerate().map(|(i, (name, rule))| {
                    div()
                        .id(("starter", i))
                        .px_2()
                        .py(px(4.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap_2()
                        .hover(|d| d.bg(palette.glass_selected))
                        .child(symbol("plus.circle", 12., 0., palette.accent, window))
                        .child(name)
                        .on_click(cx.listener(move |this, _, window, cx| this.start_rule(None, rule.clone(), window, cx)))
                })))
            })
            .when(!lately.is_empty(), |d| {
                d.child(heading("Lately:")).children(lately.into_iter().map(|entry| {
                    let when = chrono::DateTime::from_timestamp(entry.at, 0).map(|t| t.with_timezone(&chrono::Local));
                    let today = chrono::Local::now().date_naive();
                    let when = when.map_or_else(String::new, |t| {
                        if t.date_naive() == today { t.format("%H:%M").to_string() } else { t.format("%b %-d, %H:%M").to_string() }
                    });
                    let at = entry.at;
                    div()
                        .px_2()
                        .py(px(4.))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().w(px(84.)).flex_shrink_0().text_size(px(12.)).text_color(palette.secondary).child(when))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().truncate().child(entry.words()))
                                .child(div().truncate().text_size(px(11.)).text_color(palette.secondary).child(entry.rule.clone())),
                        )
                        .child(if entry.undone {
                            div().text_size(px(12.)).text_color(palette.secondary).child("Undone").into_any_element()
                        } else {
                            glass(24., &palette)
                                .id(("rule-undo", at as u64))
                                .px_3()
                                .text_size(px(12.))
                                .child("Undo")
                                .on_click(move |_, _, cx| rules::undo(at, cx))
                                .into_any_element()
                        })
                }))
            })
            .into_any_element()
    }

    /// The open rule's form.
    fn render_rule_form(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.rule_form.as_ref()?;
        let palette = self.palette;
        let label = |text: &'static str| div().w(px(80.)).flex_shrink_0().text_size(px(12.)).text_color(palette.secondary).child(text);
        let row = |l: &'static str| div().flex().items_center().gap_2().child(label(l));
        let home = home();
        let places = [home.join("Desktop"), home.join("Downloads"), home.join("Documents")];
        let mut folders: Vec<PathBuf> = places.to_vec();
        if !folders.contains(&form.folder) {
            folders.push(form.folder.clone());
        }
        let kinds: Vec<&'static str> = crate::tidy::kinds().collect();
        // A ticked number: "Larger than [100] MB".
        let ticked = |id: &'static str, (on, input): &(bool, Entity<InputState>), before: &'static str, after: &'static str, window: &Window, cx: &mut Context<Self>| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .pl(px(88.))
                .child(
                    div()
                        .id(id)
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(check_box(*on, &palette, window))
                        .child(before)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = &mut this.rule_form {
                                let tick = match id {
                                    "rule-larger" => &mut f.larger.0,
                                    "rule-added" => &mut f.added.0,
                                    _ => &mut f.unopened.0,
                                };
                                *tick = !*tick;
                                f.error = None;
                            }
                            cx.notify();
                        })),
                )
                .child(div().w(px(56.)).child(Input::new(input)))
                .child(after)
        };
        let preview = match &form.preview {
            Some((rule, Some((0, _)))) => format!("Nothing in {} fits it right now.", rules::folder_name(&rule.folder)),
            Some((rule, Some((n, names)))) => {
                let mut list = names.iter().map(|n| format!("“{n}”")).collect::<Vec<_>>().join(", ");
                if *n > names.len() {
                    list = format!("{list} and {} more", n - names.len());
                }
                let to = match &rule.then {
                    Then::Trash => format!("to the {}", crate::macos::bin()),
                    Then::Move(to) => format!("into {}", rules::folder_name(to)),
                    Then::Sort => "into folders by kind".into(),
                };
                format!("Right now it would move {list} {to}.")
            }
            Some((_, None)) => "Looking…".into(),
            None => String::new(),
        };
        // (What would keep it from being saved, said as soon as it's so.)
        let problem = form.error.or_else(|| form.rule(cx).ok().and_then(|r| r.problem()));
        let then_is = |t: &Then| std::mem::discriminant(t) == std::mem::discriminant(&form.then);
        Some(
            div()
                .mt_2()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(palette.glass_border)
                .bg(palette.sidebar)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    row("In").child(div().flex().flex_wrap().items_center().gap_1().children(folders.into_iter().map(|path| {
                        let on = path == form.folder;
                        chip(SharedString::from(format!("rule-in-{}", path.display())), rules::folder_name(&path), on, &palette).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = &mut this.rule_form {
                                f.folder = path.clone();
                                f.error = None;
                            }
                            cx.notify();
                        }))
                    }))
                    .child(chip("rule-in-choose", "Choose…", false, &palette).on_click(cx.listener(|this, _, window, cx| this.choose_folder(false, window, cx))))),
                )
                .child(
                    row("Files").child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .child(chip("rule-kind-any", "Any Kind", form.kinds.is_empty(), &palette).on_click(cx.listener(|this, _, _, cx| {
                                if let Some(f) = &mut this.rule_form {
                                    f.kinds.clear();
                                }
                                cx.notify();
                            })))
                            .children(kinds.into_iter().map(|kind| {
                                let on = form.kinds.iter().any(|k| k == kind);
                                chip(SharedString::from(format!("rule-kind-{kind}")), kind, on, &palette).on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(f) = &mut this.rule_form {
                                        if let Some(at) = f.kinds.iter().position(|k| k == kind) {
                                            f.kinds.remove(at);
                                        } else {
                                            f.kinds.push(kind.to_string());
                                        }
                                    }
                                    cx.notify();
                                }))
                            })),
                    ),
                )
                .child(row("Named").child(div().w(px(200.)).child(Input::new(&form.named))).child(div().text_size(px(11.)).text_color(palette.secondary).child("a word in the name (or leave it empty)")))
                .child(ticked("rule-larger", &form.larger, "Larger than", "MB", window, cx))
                .child(ticked("rule-added", &form.added, "Added more than", "days ago", window, cx))
                .child(ticked("rule-unopened", &form.unopened, "Not opened in", "days", window, cx))
                .child(
                    row("Then").child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .child(chip("rule-sort", "Sort into Folders by Kind", then_is(&Then::Sort), &palette).on_click(cx.listener(|this, _, _, cx| {
                                if let Some(f) = &mut this.rule_form {
                                    f.then = Then::Sort;
                                    f.error = None;
                                }
                                cx.notify();
                            })))
                            .child(chip("rule-move", "Move to a Folder", then_is(&Then::Move(PathBuf::new())), &palette).on_click(cx.listener(|this, _, _, cx| {
                                if let Some(f) = &mut this.rule_form
                                    && !matches!(f.then, Then::Move(_))
                                {
                                    f.then = Then::Move(PathBuf::new());
                                    f.error = None;
                                }
                                cx.notify();
                            })))
                            .child(chip("rule-trash", format!("Move to the {}", crate::macos::bin()), then_is(&Then::Trash), &palette).on_click(cx.listener(|this, _, _, cx| {
                                if let Some(f) = &mut this.rule_form {
                                    f.then = Then::Trash;
                                    f.error = None;
                                }
                                cx.notify();
                            }))),
                    ),
                )
                .when_some(if let Then::Move(to) = &form.then { Some(to.clone()) } else { None }, |d, to| {
                    let chosen = !to.as_os_str().is_empty();
                    d.child(
                        div()
                            .pl(px(88.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(chosen, |d| d.child(symbol("folder.fill", 12., 0., rgb(0x0a84ff).into(), window)).child(div().truncate().child(to.display().to_string().replacen(&home.display().to_string(), "~", 1))))
                            .child(glass(24., &palette).id("rule-move-choose").px_3().text_size(px(12.)).child(if chosen { "Change…" } else { "Choose a Folder…" }).on_click(cx.listener(|this, _, window, cx| this.choose_folder(true, window, cx)))),
                    )
                })
                .when(!preview.is_empty() && problem.is_none(), |d| d.child(div().pl(px(88.)).text_size(px(12.)).text_color(palette.secondary).child(preview)))
                .when_some(problem, |d, problem| d.child(div().pl(px(88.)).text_size(px(12.)).text_color(rgb(0xff453a)).child(problem)))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(glass(26., &palette).id("rule-cancel").px_3().child("Cancel").on_click(cx.listener(|this, _, _, cx| {
                            this.rule_form = None;
                            cx.notify();
                        })))
                        .child(
                            div()
                                .id("rule-save")
                                .h(px(26.))
                                .px_3()
                                .rounded_md()
                                .bg(palette.accent)
                                .text_color(palette.selected_text)
                                .flex()
                                .items_center()
                                .child(if form.index.is_some() { "Save" } else { "Save and Turn On" })
                                .on_click(cx.listener(|this, _, _, cx| this.save_rule(cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}
