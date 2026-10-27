// Draws the DMG window's background (an arrow from F2 to Applications,
// and what to do) as <out>.png and <out>@2x.png, which dmgbuild combines.
// Laid out for scripts/dmg-settings.py: 640×400 under the title bar, the
// icons' centers at (160, 175) and (480, 175) from its top left; the
// bottom 30 or so under Finder's path bar, if it's shown.
import AppKit

guard CommandLine.arguments.count == 2 else {
    fatalError("usage: render-dmg-background.swift <out, without .png>")
}
let out = CommandLine.arguments[1]
let size = NSSize(width: 640, height: 400)

func rgb(_ hex: Int, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat(hex >> 16 & 255) / 255, green: CGFloat(hex >> 8 & 255) / 255,
            blue: CGFloat(hex & 255) / 255, alpha: alpha)
}

// (AppKit counts y from the bottom; Finder, from the top.)
func draw() {
    let all = NSRect(origin: .zero, size: size)
    NSGradient(starting: rgb(0xFBFCFE), ending: rgb(0xEAF0F8))!.draw(in: all, angle: -90)
    // A soft glow of the icon's blue, behind where the icons sit.
    NSGradient(colors: [rgb(0x8EDBFF, 0.22), rgb(0x8EDBFF, 0)])!
        .draw(fromCenter: NSPoint(x: 320, y: 225), radius: 0, toCenter: NSPoint(x: 320, y: 225), radius: 300, options: [])

    // The arrow: a gentle arc between the icons, in the icon's blue.
    let y: CGFloat = 400 - 175
    let end = NSPoint(x: 386, y: y + 2)
    let control = NSPoint(x: 344, y: y + 14)
    let arrow = NSBezierPath()
    arrow.move(to: NSPoint(x: 254, y: y - 4))
    arrow.curve(to: end, controlPoint1: NSPoint(x: 296, y: y + 26), controlPoint2: control)
    // The head, along the curve's last direction: two strokes 35° off it.
    // (One path, one stroke: no darker overlap where they meet.)
    let along = atan2(end.y - control.y, end.x - control.x)
    func arm(_ turn: CGFloat) -> NSPoint {
        NSPoint(x: end.x - 18 * cos(along + turn), y: end.y - 18 * sin(along + turn))
    }
    arrow.move(to: arm(0.61))
    arrow.line(to: end)
    arrow.line(to: arm(-0.61))
    arrow.lineWidth = 5
    arrow.lineCapStyle = .round
    arrow.lineJoinStyle = .round
    rgb(0x3C7BF0, 0.85).setStroke()
    arrow.stroke()

    let caption = NSAttributedString(
        string: "Drag F2 into Applications to install it",
        attributes: [.font: NSFont.systemFont(ofSize: 13, weight: .medium), .foregroundColor: rgb(0x6E6E73)])
    let w = caption.size().width
    caption.draw(at: NSPoint(x: (size.width - w) / 2, y: 400 - 308))
}

for scale in [1, 2] {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(size.width) * scale, pixelsHigh: Int(size.height) * scale,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = size
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    draw()
    NSGraphicsContext.restoreGraphicsState()
    let name = scale == 1 ? "\(out).png" : "\(out)@2x.png"
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: name))
}
