//! What the editor draws on a screenshot (arrows, shapes, text, numbered
//! steps, blurred areas), in the image's own pixels, y down. Each mark
//! comes down to a few `Prim`itives that both the editor (GPUI) and the
//! export (Core Graphics, `nf_render_shot`) draw, so what's seen is what's
//! saved.

use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

pub fn p(x: f32, y: f32) -> P {
    P { x, y }
}

impl P {
    fn plus(self, o: P) -> P {
        p(self.x + o.x, self.y + o.y)
    }
    fn minus(self, o: P) -> P {
        p(self.x - o.x, self.y - o.y)
    }
    fn times(self, k: f32) -> P {
        p(self.x * k, self.y * k)
    }
    fn len(self) -> f32 {
        self.x.hypot(self.y)
    }
}

/// A rectangle from two corners, either way round: x, y, width, height.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct R {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub fn rect(a: P, b: P) -> R {
    R { x: a.x.min(b.x), y: a.y.min(b.y), w: (a.x - b.x).abs(), h: (a.y - b.y).abs() }
}

impl R {
    pub fn contains(&self, q: P, slack: f32) -> bool {
        q.x >= self.x - slack && q.x <= self.x + self.w + slack && q.y >= self.y - slack && q.y <= self.y + self.h + slack
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Select,
    Arrow,
    Line,
    Rect,
    Ellipse,
    Pen,
    Highlight,
    Text,
    Counter,
    Blur,
    Crop,
}

impl Tool {
    pub const ALL: [Tool; 11] =
        [Tool::Select, Tool::Arrow, Tool::Line, Tool::Rect, Tool::Ellipse, Tool::Pen, Tool::Highlight, Tool::Text, Tool::Counter, Tool::Blur, Tool::Crop];

    /// Its name, symbol, and key (BetterShot's where it has one).
    pub fn about(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Tool::Select => ("Select", "cursorarrow", "v"),
            Tool::Arrow => ("Arrow", "arrow.up.right", "a"),
            Tool::Line => ("Line", "line.diagonal", "l"),
            Tool::Rect => ("Rectangle", "rectangle", "r"),
            Tool::Ellipse => ("Ellipse", "circle", "o"),
            Tool::Pen => ("Pen", "scribble", "d"),
            Tool::Highlight => ("Highlight", "highlighter", "h"),
            Tool::Text => ("Text", "textformat", "t"),
            Tool::Counter => ("Step Number", "1.circle", "1"),
            Tool::Blur => ("Blur", "square.grid.3x3.fill", "b"),
            Tool::Crop => ("Crop", "crop", "c"),
        }
    }

    pub fn for_key(key: &str) -> Option<Tool> {
        match key {
            "p" => Some(Tool::Blur),
            "n" => Some(Tool::Counter),
            "escape" => Some(Tool::Select),
            key => Tool::ALL.into_iter().find(|t| t.about().2 == key),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Kind {
    Arrow(P, P),
    Line(P, P),
    Rect(P, P),
    Ellipse(P, P),
    Pen(Vec<P>),
    Highlight(P, P),
    /// Its top left, and what it says.
    Text(P, String),
    /// Its center, and its number.
    Counter(P, u32),
    Blur(P, P),
}

/// A mark: what, in which color, how thick (pixels: its lines' width, or
/// a text's or a step number's size follows it).
#[derive(Clone, PartialEq, Debug)]
pub struct Mark {
    pub kind: Kind,
    pub color: u32,
    pub width: f32,
}

/// What a mark is drawn with.
#[derive(Clone, PartialEq, Debug)]
pub enum Prim {
    /// Round-capped, round-joined.
    Stroke { points: Vec<P>, width: f32, color: u32, alpha: f32, closed: bool },
    Fill { points: Vec<P>, color: u32, alpha: f32 },
    Disc { center: P, r: f32, color: u32 },
    /// At its top left, or centered on `at`.
    Text { at: P, size: f32, color: u32, text: String, center: bool },
    Pixelate { rect: R },
}

/// A text's size for a line width (the S, M, L widths: 3, 5, 8 points).
pub fn text_size(width: f32) -> f32 {
    width * 3.2 + 10.
}

/// A step number's radius.
pub fn counter_radius(width: f32) -> f32 {
    width * 1.6 + 10.
}

impl Mark {
    pub fn prims(&self) -> Vec<Prim> {
        let (color, width) = (self.color, self.width);
        let stroke = |points: Vec<P>, closed| Prim::Stroke { points, width, color, alpha: 1., closed };
        match &self.kind {
            Kind::Arrow(from, to) => arrow(*from, *to, width, color),
            Kind::Line(a, b) => vec![stroke(vec![*a, *b], false)],
            Kind::Rect(a, b) => {
                let r = rect(*a, *b);
                vec![stroke(vec![p(r.x, r.y), p(r.x + r.w, r.y), p(r.x + r.w, r.y + r.h), p(r.x, r.y + r.h)], true)]
            }
            Kind::Ellipse(a, b) => {
                let r = rect(*a, *b);
                let (c, rx, ry) = (p(r.x + r.w / 2., r.y + r.h / 2.), r.w / 2., r.h / 2.);
                let points = (0..72).map(|i| {
                    let t = i as f32 / 72. * std::f32::consts::TAU;
                    p(c.x + rx * t.cos(), c.y + ry * t.sin())
                });
                vec![stroke(points.collect(), true)]
            }
            Kind::Pen(points) => vec![stroke(points.clone(), false)],
            Kind::Highlight(a, b) => {
                let r = rect(*a, *b);
                vec![Prim::Fill { points: vec![p(r.x, r.y), p(r.x + r.w, r.y), p(r.x + r.w, r.y + r.h), p(r.x, r.y + r.h)], color, alpha: 0.35 }]
            }
            Kind::Text(at, text) => vec![Prim::Text { at: *at, size: text_size(width), color, text: text.clone(), center: false }],
            Kind::Counter(center, n) => {
                let r = counter_radius(width);
                // (White on it, but black on a white or yellow one.)
                let ink = if matches!(color, 0xffffff | 0xffcc00) { 0x000000 } else { 0xffffff };
                vec![Prim::Disc { center: *center, r, color }, Prim::Text { at: *center, size: r * 1.15, color: ink, text: n.to_string(), center: true }]
            }
            Kind::Blur(a, b) => vec![Prim::Pixelate { rect: rect(*a, *b) }],
        }
    }

    /// Where it is, roughly (a text's by its size: for picking it).
    pub fn bounds(&self) -> R {
        let points: Vec<P> = match &self.kind {
            Kind::Arrow(a, b) | Kind::Line(a, b) | Kind::Rect(a, b) | Kind::Ellipse(a, b) | Kind::Highlight(a, b) | Kind::Blur(a, b) => vec![*a, *b],
            Kind::Pen(points) => points.clone(),
            Kind::Text(at, text) => {
                let size = text_size(self.width);
                let lines = text.lines().count().max(1) as f32;
                let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
                vec![*at, at.plus(p(longest * size * 0.58, lines * size * 1.25))]
            }
            Kind::Counter(c, _) => {
                let r = counter_radius(self.width);
                vec![c.minus(p(r, r)), c.plus(p(r, r))]
            }
        };
        let (mut lo, mut hi) = (points[0], points[0]);
        for q in &points {
            (lo.x, lo.y, hi.x, hi.y) = (lo.x.min(q.x), lo.y.min(q.y), hi.x.max(q.x), hi.y.max(q.y));
        }
        rect(lo, hi)
    }

    /// Whether `q` is on it (within `slack`): on a line itself, not just
    /// in its bounds, for lines and arrows.
    pub fn hit(&self, q: P, slack: f32) -> bool {
        let slack = slack + self.width / 2.;
        match &self.kind {
            Kind::Arrow(a, b) | Kind::Line(a, b) => distance_to_segment(q, *a, *b) <= slack,
            Kind::Pen(points) => points.windows(2).any(|w| distance_to_segment(q, w[0], w[1]) <= slack),
            Kind::Rect(a, b) | Kind::Ellipse(a, b) => {
                // (On its outline, or inside a small one.)
                let r = rect(*a, *b);
                r.contains(q, slack) && (r.w < slack * 4. || r.h < slack * 4. || !rect(p(r.x + slack, r.y + slack), p(r.x + r.w - slack, r.y + r.h - slack)).contains(q, 0.))
            }
            _ => self.bounds().contains(q, slack),
        }
    }

    pub fn moved(&self, by: P) -> Mark {
        let m = |q: &P| q.plus(by);
        let kind = match &self.kind {
            Kind::Arrow(a, b) => Kind::Arrow(m(a), m(b)),
            Kind::Line(a, b) => Kind::Line(m(a), m(b)),
            Kind::Rect(a, b) => Kind::Rect(m(a), m(b)),
            Kind::Ellipse(a, b) => Kind::Ellipse(m(a), m(b)),
            Kind::Pen(points) => Kind::Pen(points.iter().map(m).collect()),
            Kind::Highlight(a, b) => Kind::Highlight(m(a), m(b)),
            Kind::Text(at, text) => Kind::Text(m(at), text.clone()),
            Kind::Counter(c, n) => Kind::Counter(m(c), *n),
            Kind::Blur(a, b) => Kind::Blur(m(a), m(b)),
        };
        Mark { kind, ..self.clone() }
    }

    /// Too small to keep (a click, not a drag).
    pub fn is_empty(&self) -> bool {
        match &self.kind {
            Kind::Arrow(a, b) | Kind::Line(a, b) => a.minus(*b).len() < 4.,
            Kind::Rect(a, b) | Kind::Ellipse(a, b) | Kind::Highlight(a, b) | Kind::Blur(a, b) => {
                let r = rect(*a, *b);
                r.w < 4. || r.h < 4.
            }
            Kind::Pen(points) => points.len() < 2,
            Kind::Text(_, text) => text.trim().is_empty(),
            Kind::Counter(..) => false,
        }
    }
}

fn distance_to_segment(q: P, a: P, b: P) -> f32 {
    let ab = b.minus(a);
    let t = if ab.len() == 0. { 0. } else { ((q.x - a.x) * ab.x + (q.y - a.y) * ab.y) / (ab.x * ab.x + ab.y * ab.y) };
    q.minus(a.plus(ab.times(t.clamp(0., 1.)))).len()
}

/// An arrow: its shaft, and a filled head (smaller on a short one).
fn arrow(from: P, to: P, width: f32, color: u32) -> Vec<Prim> {
    let d = to.minus(from);
    let len = d.len();
    if len < 1. {
        return Vec::new();
    }
    let dir = d.times(1. / len);
    let across = p(-dir.y, dir.x);
    let head = (width * 3.6 + 8.).min(len * 0.6);
    let base = to.minus(dir.times(head));
    let half = head * 0.55;
    vec![
        // (Into the head a little: no gap at its base.)
        Prim::Stroke { points: vec![from, base.plus(dir.times(head * 0.25))], width, color, alpha: 1., closed: false },
        Prim::Fill { points: vec![to, base.plus(across.times(half)), base.minus(across.times(half))], color, alpha: 1. },
    ]
}

/// The soft gradients a screenshot can sit on (BetterShot's kind): from
/// the top left to the bottom right.
pub const BACKGROUNDS: [(u32, u32); 10] = [
    (0xa1c4fd, 0xc2e9fb),
    (0xfbc2eb, 0xa6c1ee),
    (0x84fab0, 0x8fd3f4),
    (0xf6d365, 0xfda085),
    (0xa18cd1, 0xfbc2eb),
    (0xff9a9e, 0xfecfef),
    (0xfddb92, 0xd1fdff),
    (0x30cfd0, 0x330867),
    (0x0f2027, 0x2c5364),
    (0x434343, 0x000000),
];

/// A screenshot set on a background: which, how much around it, how round
/// its corners, whether it casts a shadow (in points).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Background {
    pub gradient: usize,
    pub padding: f32,
    pub radius: f32,
    pub shadow: bool,
}

impl Default for Background {
    fn default() -> Self {
        Background { gradient: 0, padding: 64., radius: 12., shadow: true }
    }
}

fn hex(color: u32) -> String {
    format!("#{color:06X}")
}

fn flat(points: &[P]) -> Vec<f32> {
    points.iter().flat_map(|q| [q.x, q.y]).collect()
}

/// What `nf_render_shot` draws: `image` with `marks`, cropped and set on a
/// background, written to `out`. `scale`: its pixels per point.
pub fn spec(image: &str, out: &str, scale: f32, marks: &[Mark], crop: Option<R>, background: Option<Background>) -> Value {
    let prims: Vec<Value> = marks
        .iter()
        .flat_map(Mark::prims)
        .map(|prim| match prim {
            Prim::Stroke { points, width, color, alpha, closed } => {
                json!({"kind": "stroke", "points": flat(&points), "width": width, "color": hex(color), "alpha": alpha, "closed": closed})
            }
            Prim::Fill { points, color, alpha } => json!({"kind": "fill", "points": flat(&points), "color": hex(color), "alpha": alpha}),
            Prim::Disc { center, r, color } => json!({"kind": "disc", "x": center.x, "y": center.y, "r": r, "color": hex(color)}),
            Prim::Text { at, size, color, text, center } => {
                json!({"kind": "text", "x": at.x, "y": at.y, "size": size, "color": hex(color), "text": text, "center": center, "bold": center})
            }
            Prim::Pixelate { rect } => json!({"kind": "pixelate", "rect": [rect.x, rect.y, rect.w, rect.h]}),
        })
        .collect();
    json!({
        "image": image,
        "out": out,
        "scale": scale,
        "pixel": pixel(scale),
        "marks": prims,
        "crop": crop.map(|r| json!([r.x.round(), r.y.round(), r.w.round(), r.h.round()])),
        "background": background.map(|b| {
            let (from, to) = BACKGROUNDS[b.gradient.min(BACKGROUNDS.len() - 1)];
            json!({"from": hex(from), "to": hex(to), "padding": (b.padding * scale).round(), "radius": b.radius * scale, "shadow": b.shadow})
        }),
    })
}

/// The blur's squares' size, in pixels.
pub fn pixel(scale: f32) -> f32 {
    (10. * scale).round()
}

#[cfg(test)]
mod tests {
    use super::{Kind, Mark, Prim, Tool, p, rect, spec};

    fn mark(kind: Kind) -> Mark {
        Mark { kind, color: 0xff3b30, width: 10. }
    }

    #[test]
    fn arrows_have_heads() {
        let prims = mark(Kind::Arrow(p(0., 0.), p(200., 0.))).prims();
        let Prim::Fill { points, .. } = &prims[1] else { panic!() };
        assert_eq!(points[0], p(200., 0.), "the head's tip is where it points");
        assert!(points[1].x < 200. && points[1].y != 0.);
        assert!(mark(Kind::Arrow(p(5., 5.), p(5., 5.))).prims().is_empty());
    }

    #[test]
    fn picking_and_moving() {
        let line = mark(Kind::Line(p(0., 0.), p(100., 100.)));
        assert!(line.hit(p(50., 52.), 4.));
        assert!(!line.hit(p(80., 20.), 4.));
        let box_ = mark(Kind::Rect(p(0., 0.), p(300., 200.)));
        assert!(box_.hit(p(1., 100.), 4.), "on its edge");
        assert!(!box_.hit(p(150., 100.), 4.), "not in its middle");
        let moved = line.moved(p(10., -5.));
        assert_eq!(moved.kind, Kind::Line(p(10., -5.), p(110., 95.)));
        assert!(mark(Kind::Rect(p(0., 0.), p(2., 50.))).is_empty());
        assert_eq!(rect(p(10., 10.), p(0., 5.)).w, 10.);
    }

    #[test]
    fn keys_and_spec() {
        assert_eq!(Tool::for_key("a"), Some(Tool::Arrow));
        assert_eq!(Tool::for_key("p"), Some(Tool::Blur));
        assert_eq!(Tool::for_key("z"), None);
        let s = spec("/in.png", "/out.png", 2., &[mark(Kind::Counter(p(10., 10.), 3))], None, None);
        assert_eq!(s["marks"].as_array().unwrap().len(), 2);
        assert_eq!(s["marks"][1]["text"], "3");
        assert!(s["background"].is_null());
    }
}
