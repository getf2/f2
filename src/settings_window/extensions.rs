//! Settings › Extensions (see `crate::extensions`): Quick Open's
//! extensions, one picked from the list, with its settings (its manifest's
//! preferences: passwords kept in the keychain, the rest with F2's
//! settings), its commands, and what it may do; and Developer Mode, for
//! those that are programs.

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use serde_json::Value;

use super::{SettingsWindow, Tab, tile};
use crate::commands::Icon;
use crate::extensions::{self, Extension, PrefKind, Preference};
use crate::settings::Settings;
use crate::style::{Palette, SEMIBOLD, glass, symbol};

/// Settings › Extensions, at extension `id` (one that needs something set
/// before it runs: what it needs, marked).
pub fn open_extension(id: &str, cx: &mut App) {
    super::open_on(Some(Tab::Extensions), cx);
    let Some((handle, view)) = cx.try_global::<super::Open>().and_then(|o| o.0.clone()) else { return };
    let id = id.to_string();
    handle
        .update(cx, |_, _, cx| {
            view.update(cx, |this, cx| {
                this.ext_selected = Some(id);
                cx.notify();
            })
        })
        .ok();
}

/// Settings › Extensions, New Extension…'s name field open.
pub fn new_extension(cx: &mut App) {
    super::open_on(Some(Tab::Extensions), cx);
    let Some((handle, view)) = cx.try_global::<super::Open>().and_then(|o| o.0.clone()) else { return };
    handle.update(cx, |_, window, cx| view.update(cx, |this, cx| this.start_new_extension(window, cx))).ok();
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

/// An extension's icon, as its commands' tiles.
fn icon(ext: &Extension, size: f32, window: &Window) -> AnyElement {
    match ext.icon(None, None) {
        Icon::Image(file) => img(file).size(px(size)).rounded(px(5.)).into_any_element(),
        Icon::Tile(name, color) => tile(name, color, window).size(px(size)).into_any_element(),
        Icon::Of(_) => div().size(px(size)).into_any_element(),
    }
}

/// What a program extension runs ("python3 main.py").
fn program_line(ext: &Extension) -> String {
    match &ext.manifest.run {
        Some(extensions::Program::Line(line)) => line.clone(),
        Some(extensions::Program::Args(args)) => args.join(" "),
        None => String::new(),
    }
}

/// Whether `ext` (a program) was allowed to run, where it is.
fn allowed(ext: &Extension, settings: &Settings) -> bool {
    settings.allowed_extension_programs.get(&ext.id).is_some_and(|dir| std::path::Path::new(dir) == ext.dir)
}

/// Developer Mode's switch: programs run (each once it's allowed).
fn developer_mode(on: bool, palette: &Palette, window: &Window) -> Div {
    div()
        .mt_4()
        .pt_3()
        .border_t_1()
        .border_color(palette.separator)
        .child(
            div()
                .id("ext-developer")
                .flex()
                .items_center()
                .gap_2()
                .child(check_box(on, palette, window))
                .child("Developer Mode")
                .on_click(|_, _, cx| Settings::update(cx, |s| s.extension_developer_mode = !s.extension_developer_mode)),
        )
        .child(div().mt_1().text_size(px(11.)).text_color(palette.secondary).child(format!(
            "Runs extensions that are programs, in any language, not just ones in JavaScript. A program runs as you, not in {}'s sandbox, so each must be allowed before it runs.",
            crate::APP_NAME
        )))
}

/// A value as typed in a field.
fn shown(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

impl SettingsWindow {
    /// `pref`'s field for `ext` (made the first time): what's typed kept
    /// as it's typed; a password's when Return's pressed or Save clicked.
    fn ext_field(&mut self, ext: &Extension, pref: &Preference, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        let key = format!("{}/{}", ext.id, pref.id);
        if let Some(field) = self.ext_fields.get(&key) {
            return field.clone();
        }
        let password = pref.kind == PrefKind::Password;
        let placeholder = if !pref.placeholder.is_empty() {
            pref.placeholder.clone()
        } else if password {
            "Paste it here".into()
        } else {
            String::new()
        };
        let value = if password { String::new() } else { shown(ext.value(pref).as_ref()) };
        let field = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value).masked(password));
        let (id, pref_id, kind) = (ext.id.clone(), pref.id.clone(), pref.kind);
        self.ext_watch.push(cx.subscribe_in(&field, window, move |this, field, e: &InputEvent, window, cx| match e {
            InputEvent::Change if kind != PrefKind::Password => {
                let text = field.read(cx).value().to_string();
                let value = match kind {
                    PrefKind::Number if text.trim().is_empty() => Value::Null,
                    PrefKind::Number => match text.trim().parse::<f64>() {
                        Ok(n) => serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number),
                        // (Not a number yet: kept as it was.)
                        Err(_) => return,
                    },
                    _ => Value::String(text),
                };
                set_value(&id, &pref_id, value, cx);
            }
            InputEvent::PressEnter { .. } if kind == PrefKind::Password => this.save_password(&id, &pref_id, window, cx),
            _ => {}
        }));
        self.ext_fields.insert(key, field.clone());
        field
    }

    /// New Extension…'s form: its name's field, focused.
    pub(super) fn start_new_extension(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| InputState::new(window, cx).placeholder("Its name: “Linear Issues”"));
        self.ext_watch.push(cx.subscribe_in(&field, window, |this, _, e: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = e {
                this.create_extension(window, cx);
            }
        }));
        field.update(cx, |f, cx| f.focus(window, cx));
        (self.ext_new, self.ext_new_error) = (Some(field), None);
        cx.notify();
    }

    /// The extension named in New Extension…'s field made, and picked.
    fn create_extension(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.ext_new.clone() else { return };
        let name = field.read(cx).value().to_string();
        match extensions::create(&name) {
            Ok((id, _)) => {
                (self.ext_selected, self.ext_made) = (Some(id.clone()), Some(id));
                (self.ext_new, self.ext_new_error) = (None, None);
            }
            Err(why) => self.ext_new_error = Some(why),
        }
        cx.notify();
    }

    /// New Extension…'s button, or its form.
    fn new_extension_form(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let Some(field) = self.ext_new.clone() else {
            // (The store's, in Quick Open; and one of your own.)
            let browse = glass(26., &palette)
                .id("ext-store")
                .px_3()
                .flex()
                .items_center()
                .gap_1()
                .child(symbol("puzzlepiece.extension", 11., SEMIBOLD, palette.label, window))
                .child("Browse the Store…")
                .on_click(|_, _, cx| crate::quick_panel::show_native(crate::store::extension(), "browse", serde_json::Value::Null, cx));
            let new = glass(26., &palette)
                .id("ext-new")
                .px_3()
                .flex()
                .items_center()
                .gap_1()
                .child(symbol("plus", 11., SEMIBOLD, palette.label, window))
                .child("New Extension…")
                .on_click(cx.listener(|this, _, window, cx| this.start_new_extension(window, cx)));
            return div().flex().items_center().gap_2().child(browse).child(new).into_any_element();
        };
        div()
            .p_3()
            .rounded_md()
            .bg(palette.label.opacity(0.05))
            .child(div().font_weight(FontWeight::SEMIBOLD).child("New Extension"))
            .child(
                div()
                    .mt_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(260.)).child(Input::new(&field)))
                    .child(glass(26., &palette).id("ext-create").px_3().child("Create").on_click(cx.listener(|this, _, window, cx| this.create_extension(window, cx))))
                    .child(div().id("ext-new-cancel").text_size(px(12.)).text_color(palette.accent).child("Cancel").on_click(cx.listener(|this, _, _, cx| {
                        (this.ext_new, this.ext_new_error) = (None, None);
                        cx.notify();
                    }))),
            )
            .child(div().mt_2().text_size(px(11.)).text_color(palette.secondary).child(format!(
                "In JavaScript, in your Extensions folder: a command showing a few rows, to change into yours, with what your editor needs to know {}'s API. It runs in {}'s sandbox, so it needs no Developer Mode.",
                crate::APP_NAME,
                crate::APP_NAME
            )))
            .when_some(self.ext_new_error.clone(), |d, why| d.child(div().mt_1().text_size(px(11.)).text_color(rgb(0xff9f0a)).child(why)))
            .into_any_element()
    }

    /// The password typed in its field, kept in the keychain (the field
    /// emptied, saying it's saved).
    fn save_password(&mut self, id: &str, pref: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ext) = extensions::any_by_id(id) else { return };
        let Some(field) = self.ext_fields.get(&format!("{id}/{pref}")).cloned() else { return };
        let typed = field.read(cx).value().trim().to_string();
        if typed.is_empty() {
            return;
        }
        if ext.keep_password(pref, &typed) {
            field.update(cx, |f, cx| f.set_value("", window, cx));
            extensions::forget_shown(id);
        }
        cx.notify();
    }

    pub(super) fn render_extensions(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let heading = |text: &'static str| div().mt_4().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let note = |text: String| div().text_size(px(11.)).text_color(palette.secondary).child(text);
        let (exts, broken) = extensions::every();
        let folder = extensions::folder();
        let open_folder = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            let folder = extensions::folder();
            let _ = std::fs::create_dir_all(&folder);
            crate::browser::open(folder, None, cx);
            cx.activate(true);
        };
        let developer = settings.extension_developer_mode;
        // (New Extension…: a button among the others, or its form, on a
        // line of its own.)
        let new_extension = self.new_extension_form(window, cx);
        let (button, form) = if self.ext_new.is_some() { (None, Some(div().mt_3().child(new_extension))) } else { (Some(new_extension), None) };
        if exts.is_empty() && broken.is_empty() {
            return div()
                .child(note(format!("No extensions yet. Each is a folder in {}, with an f2.json: EXTENSIONS.md says what goes in it.", folder.display())))
                .children(form)
                .child(div().mt_3().flex().gap_2().children(button).child(glass(26., &palette).id("ext-folder").px_3().child("Open Extensions Folder").on_click(open_folder)))
                .child(developer_mode(developer, &palette, window))
                .into_any_element();
        }
        // The one picked (else the first).
        let picked = self.ext_selected.clone().filter(|id| exts.iter().any(|e| &e.id == id)).or_else(|| exts.first().map(|e| e.id.clone()));
        let list = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(exts.iter().map(|ext| {
                let on = picked.as_deref() == Some(ext.id.as_str());
                // (Off, or a program that can't run without Developer Mode.)
                let off = settings.disabled_extensions.contains(&ext.id) || (ext.program() && !developer);
                let waiting = ext.program() && !allowed(ext, settings);
                let id = ext.id.clone();
                div()
                    .id(SharedString::from(format!("ext-pick-{}", ext.id)))
                    .px_2()
                    .py(px(4.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(on, |d| d.bg(palette.glass_selected))
                    .when(off, |d| d.opacity(0.55))
                    .child(icon(ext, 18., window))
                    .child(ext.manifest.name.clone())
                    // (What it still needs, marked.)
                    .when(!off && (waiting || !ext.missing().is_empty()), |d| d.child(div().size(px(6.)).rounded_full().bg(rgb(0xff9f0a))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        (this.ext_selected, this.ext_made) = (Some(id.clone()), None);
                        cx.notify();
                    }))
            }))
            .children(broken.iter().map(|b| {
                let name = b.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                div().px_2().py(px(4.)).flex().items_center().gap_2().text_color(palette.secondary).child(symbol("exclamationmark.triangle.fill", 12., SEMIBOLD, rgb(0xff9f0a).into(), window)).child(name)
            }));
        let mut body = div().child(list.children(button)).children(form);
        for b in &broken {
            let name = b.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            body = body.child(div().mt_1().text_size(px(11.)).text_color(rgb(0xff9f0a)).child(format!("{name} isn't working: {}", b.why)));
        }
        let Some(ext) = picked.and_then(|id| exts.into_iter().find(|e| e.id == id)) else { return body.child(developer_mode(developer, &palette, window)).into_any_element() };
        let off = settings.disabled_extensions.contains(&ext.id);
        let missing = ext.missing();
        let id = ext.id.clone();

        // What it is, and whether it's on.
        body = body.child(
            div()
                .mt_4()
                .pt_3()
                .border_t_1()
                .border_color(palette.separator)
                .flex()
                .items_start()
                .gap_3()
                .child(icon(&ext, 32., window))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(ext.manifest.name.clone()))
                        .when(!ext.manifest.description.is_empty(), |d| d.child(div().mt(px(2.)).text_size(px(12.)).text_color(palette.secondary).child(ext.manifest.description.clone()))),
                )
                .child(
                    div()
                        .id("ext-on")
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(check_box(!off, &palette, window))
                        .child("On")
                        .on_click(move |_, _, cx| {
                            let id = id.clone();
                            Settings::update(cx, move |s| {
                                if let Some(at) = s.disabled_extensions.iter().position(|d| *d == id) {
                                    s.disabled_extensions.remove(at);
                                } else {
                                    s.disabled_extensions.push(id);
                                }
                            })
                        }),
                ),
        );
        let banner = || div().mt_2().px_2().py(px(6.)).rounded_md().bg(rgb(0xff9f0a).opacity(0.14)).text_size(px(12.));
        let editor = crate::tools::chosen(crate::tools::Kind::Editor, settings);
        // (Just made: where it is, and how to open it.)
        if self.ext_made.as_ref() == Some(&ext.id) {
            let folder = ext.dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            body = body.child(banner().bg(rgb(0x30d158).opacity(0.14)).child(format!(
                "Made, in Extensions › {folder}. {}type “{}” in Quick Open to run it; save a change and it runs again.",
                editor.as_ref().map_or(String::new(), |t| format!("Open it in {} (below), and ", t.name)),
                ext.manifest.name
            )));
        }
        // A program: only with Developer Mode on, and once it's allowed.
        if ext.program() && !off {
            let name = ext.manifest.name.clone();
            if !developer {
                body = body.child(banner().child(format!("{name} is a program, not a script in {}'s sandbox: it runs only with Developer Mode on (below).", crate::APP_NAME)));
            } else if !allowed(&ext, settings) {
                let (id, dir) = (ext.id.clone(), ext.dir.display().to_string());
                body = body.child(
                    banner()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().min_w_0().child(format!(
                            "{name} runs “{}” on your Mac, as you: it can read and change your files, and reach anything you can. Allow it only if you trust where it came from.",
                            program_line(&ext)
                        )))
                        .child(glass(26., &palette).id("ext-allow").flex_shrink_0().px_3().child("Allow").on_click(move |_, _, cx| {
                            let (id, dir) = (id.clone(), dir.clone());
                            Settings::update(cx, move |s| {
                                s.allowed_extension_programs.insert(id, dir);
                            })
                        })),
                );
            }
        }
        if !missing.is_empty() && !off {
            body = body.child(banner().child(format!(
                "{} needs {} before it can run.",
                ext.manifest.name,
                missing.iter().map(|p| format!("its {}", p.title)).collect::<Vec<_>>().join(" and ")
            )));
        }

        // Its settings.
        let prefs = ext.preferences().to_vec();
        if !prefs.is_empty() {
            body = body.child(heading("Settings"));
        }
        for pref in prefs {
            let needed = missing.iter().any(|m| m.id == pref.id);
            let title = div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().font_weight(FontWeight::MEDIUM).when(needed, |d| d.text_color(rgb(0xff9f0a))).child(pref.title.clone()))
                .when(pref.required, |d| d.child(div().text_size(px(10.)).text_color(palette.secondary).child("Required")));
            let about = (!pref.description.is_empty()).then(|| note(pref.description.clone()));
            let control: AnyElement = match pref.kind {
                PrefKind::Checkbox => {
                    let on = ext.value(&pref).and_then(|v| v.as_bool()).unwrap_or(false);
                    let (id, pid) = (ext.id.clone(), pref.id.clone());
                    div()
                        .id(SharedString::from(format!("ext-check-{}-{}", ext.id, pref.id)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .py(px(3.))
                        .child(check_box(on, &palette, window))
                        .child(pref.title.clone())
                        .on_click(move |_, _, cx| set_value(&id, &pid, Value::Bool(!on), cx))
                        .into_any_element()
                }
                PrefKind::Dropdown => {
                    let now = ext.value(&pref).map(|v| shown(Some(&v))).unwrap_or_default();
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .children(pref.options.iter().enumerate().map(|(i, choice)| {
                            let (id, pid, value) = (ext.id.clone(), pref.id.clone(), choice.value().to_string());
                            div()
                                .id(SharedString::from(format!("ext-choice-{}-{}-{i}", ext.id, pref.id)))
                                .px_3()
                                .py(px(3.))
                                .rounded_md()
                                .text_size(px(12.))
                                .when(now == value, |d| d.bg(palette.glass_selected))
                                .child(choice.title().to_string())
                                .on_click(move |_, _, cx| set_value(&id, &pid, Value::String(value.clone()), cx))
                        }))
                        .into_any_element()
                }
                PrefKind::Password => {
                    let field = self.ext_field(&ext, &pref, window, cx);
                    let saved = ext.has_password(&pref.id);
                    let (id, pid) = (ext.id.clone(), pref.id.clone());
                    let (id2, pid2) = (id.clone(), pid.clone());
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().w(px(240.)).child(Input::new(&field)))
                        .child(glass(26., &palette).id(SharedString::from(format!("ext-save-{}-{}", ext.id, pref.id))).px_3().child("Save").on_click(cx.listener(move |this, _, window, cx| this.save_password(&id, &pid, window, cx))))
                        .when(saved, |d| {
                            d.child(div().flex().items_center().gap_1().text_size(px(12.)).text_color(palette.secondary).child(symbol("checkmark.circle.fill", 12., SEMIBOLD, rgb(0x30d158).into(), window)).child("Saved in your keychain"))
                                .child(
                                    div()
                                        .id(SharedString::from(format!("ext-forget-{}-{}", ext.id, pref.id)))
                                        .text_size(px(12.))
                                        .text_color(palette.accent)
                                        .child("Forget")
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            if let Some(ext) = extensions::any_by_id(&id2) {
                                                ext.keep_password(&pid2, "");
                                                extensions::forget_shown(&id2);
                                            }
                                            cx.notify();
                                        })),
                                )
                        })
                        .into_any_element()
                }
                PrefKind::Text | PrefKind::Number => {
                    let field = self.ext_field(&ext, &pref, window, cx);
                    div().w(px(if pref.kind == PrefKind::Number { 100. } else { 300. })).child(Input::new(&field)).into_any_element()
                }
            };
            // (Where to get one: not once there's one.)
            let saved = pref.kind == PrefKind::Password && ext.has_password(&pref.id);
            let link = pref.link.clone().filter(|_| !saved).map(|link| {
                div()
                    .id(SharedString::from(format!("ext-link-{}-{}", ext.id, pref.id)))
                    .text_size(px(12.))
                    .text_color(palette.accent)
                    .child("Get One…")
                    .on_click(move |_, _, cx| cx.open_url(&link))
            });
            body = body.child(
                div()
                    .mt_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(pref.kind != PrefKind::Checkbox, |d| d.child(title))
                    .child(div().flex().flex_wrap().items_center().gap_3().child(control).children(link))
                    .children(about),
            );
        }

        // Its commands, to open from here.
        let specs: Vec<_> = ext.manifest.commands.iter().filter(|s| !s.hidden).cloned().collect();
        if !specs.is_empty() {
            body = body.child(heading("Commands"));
        }
        for spec in specs {
            let (id, command) = (ext.id.clone(), spec.id.clone());
            let tile_icon = match ext.icon(spec.icon.as_deref(), None) {
                Icon::Image(file) => img(file).size(px(18.)).rounded(px(4.)).into_any_element(),
                Icon::Tile(name, color) => tile(name, color, window).size(px(18.)).into_any_element(),
                Icon::Of(_) => div().size(px(18.)).into_any_element(),
            };
            body = body.child(
                div()
                    .py(px(3.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(tile_icon)
                    .child(div().flex_1().child(spec.title.clone()))
                    .when_some(spec.keyword.clone().filter(|k| !k.is_empty()), |d, k| {
                        d.child(div().px(px(5.)).h(px(18.)).rounded(px(4.)).bg(palette.label.opacity(0.07)).flex().items_center().text_size(px(11.)).text_color(palette.secondary).child(k))
                    })
                    .child(glass(24., &palette).id(SharedString::from(format!("ext-open-{}-{}", ext.id, spec.id))).px_2().text_size(px(12.)).child("Open").on_click(move |_, _, cx| crate::quick_panel::show_extension(&id, &command, cx))),
            );
        }

        // What it may do.
        body = body.child(heading("What it can do"));
        if ext.manifest.javascript() {
            let hosts = &ext.manifest.permissions.network;
            body = body.child(note(format!("Runs in {}'s sandbox: it can't read your files or start other programs.", crate::APP_NAME))).child(note(if hosts.is_empty() {
                "Reaches no websites.".to_string()
            } else {
                format!("Reaches {} (over https).", hosts.join(", "))
            }));
        } else {
            body = body.child(note(format!("Runs “{}” on your Mac, as you: it can do anything you can.", program_line(&ext))));
            if allowed(&ext, settings) {
                let id = ext.id.clone();
                body = body.child(
                    div().flex().items_center().gap_2().child(note("Allowed to run.".into())).child(
                        div().id("ext-disallow").text_size(px(11.)).text_color(palette.accent).child("Don't Allow").on_click(move |_, _, cx| {
                            let id = id.clone();
                            Settings::update(cx, move |s| {
                                s.allowed_extension_programs.remove(&id);
                            })
                        }),
                    ),
                );
            }
        }
        let passwords = ext.preferences().iter().filter(|p| p.kind == PrefKind::Password).count();
        if passwords > 0 {
            body = body.child(note(format!("Keeps {} in your keychain.", if passwords == 1 { "a password".to_string() } else { format!("{passwords} passwords") })));
        }
        let dir = ext.dir.clone();
        let edit = editor.map(|tool| {
            let dir = ext.dir.clone();
            glass(26., &palette).id("ext-edit").px_3().child(format!("Open in {}", tool.name)).on_click(move |_, _, _| crate::tools::open(&tool, crate::tools::Kind::Editor, &[dir.clone()]))
        });
        body.child(
            div().mt_3().flex().gap_2().children(edit).child(glass(26., &palette).id("ext-show").px_3().child("Show in Folder").on_click(move |_, _, cx| {
                crate::browser::open(dir.clone(), None, cx);
                cx.activate(true);
            })),
        )
        .child(div().mt_2().text_size(px(11.)).text_color(palette.tertiary).child(format!("{} · {}", ext.id, ext.dir.display())))
        .child(developer_mode(developer, &palette, window))
        .into_any_element()
    }
}

/// An extension's setting set (and what it showed forgotten: with it
/// changed, it may show something else).
fn set_value(id: &str, pref: &str, value: Value, cx: &mut App) {
    let (id, pref) = (id.to_string(), pref.to_string());
    extensions::forget_shown(&id);
    Settings::update(cx, move |s| {
        let values = s.extension_preferences.entry(id).or_default();
        if value.is_null() {
            values.remove(&pref);
        } else {
            values.insert(pref, value);
        }
    });
}
