// Draws the app icon, F2 on Finder's two-tone face, into an .iconset folder
// for iconutil. (assets/icon.png, for the README, is its icon_128x128@2x.png.)
import AppKit

guard CommandLine.arguments.count == 2 else {
    fatalError("usage: render-icon.swift <out.iconset>")
}
let out = URL(fileURLWithPath: CommandLine.arguments[1])

func rgb(_ hex: Int, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat(hex >> 16 & 255) / 255, green: CGFloat(hex >> 8 & 255) / 255,
            blue: CGFloat(hex & 255) / 255, alpha: alpha)
}

/// Where a string's ink is, relative to its baseline.
func ink(_ text: NSAttributedString) -> CGRect {
    CTLineGetImageBounds(CTLineCreateWithAttributedString(text), nil)
}

/// Draws a string with its ink's left edge at `x`, centered on `y`.
func draw(_ text: NSAttributedString, x: CGFloat, centerY y: CGFloat) {
    let cg = NSGraphicsContext.current!.cgContext
    cg.textMatrix = .identity
    cg.textPosition = CGPoint(x: x - ink(text).minX, y: y - ink(text).midY)
    CTLineDraw(CTLineCreateWithAttributedString(text), cg)
}

// Laid out on Apple's 1024pt icon grid (the shape covers 824 of it, centered).
func drawIcon() {
    let plate = NSRect(x: 100, y: 100, width: 824, height: 824)
    let face = NSBezierPath(roundedRect: plate, xRadius: 185, yRadius: 185)

    let heavy = NSFont.systemFont(ofSize: 440, weight: .heavy)
    let font = heavy.fontDescriptor.withDesign(.rounded).flatMap { NSFont(descriptor: $0, size: 440) } ?? heavy
    let white: [NSAttributedString.Key: Any] = [.font: font, NSAttributedString.Key(kCTForegroundColorAttributeName as String): NSColor.white.cgColor]
    let f = NSAttributedString(string: "F", attributes: white)
    let two = NSAttributedString(string: "2", attributes: white)
    let gap: CGFloat = 70
    let left = 512 - (ink(f).width + gap + ink(two).width) / 2
    let split = left + ink(f).width + gap / 2

    // The light half, then the dark one, its edge wavy like Finder's profile.
    NSGraphicsContext.saveGraphicsState()
    face.addClip()
    NSGradient(starting: rgb(0x8EDBFF), ending: rgb(0x2E9BF0))!.draw(in: plate, angle: -90)
    let dark = NSBezierPath()
    dark.move(to: NSPoint(x: split + 10, y: 0))
    dark.curve(to: NSPoint(x: split - 40, y: 560),
               controlPoint1: NSPoint(x: split + 30, y: 300), controlPoint2: NSPoint(x: split - 60, y: 420))
    dark.curve(to: NSPoint(x: split + 10, y: 1024),
               controlPoint1: NSPoint(x: split - 20, y: 700), controlPoint2: NSPoint(x: split + 40, y: 820))
    dark.line(to: NSPoint(x: 1024, y: 1024))
    dark.line(to: NSPoint(x: 1024, y: 0))
    dark.close()
    NSGradient(starting: rgb(0x3C7BF0), ending: rgb(0x123FB8))!.draw(in: dark, angle: -90)
    NSGraphicsContext.restoreGraphicsState()

    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowColor = rgb(0x002266, 0.3)
    shadow.shadowBlurRadius = 16
    shadow.shadowOffset = NSSize(width: 0, height: -8)
    shadow.set()
    draw(f, x: left, centerY: 512)
    draw(two, x: split + gap / 2, centerY: 512)
    NSGraphicsContext.restoreGraphicsState()
}

for base in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let px = base * scale
        let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
            samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
            bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        NSAffineTransform(transform: AffineTransform(scale: CGFloat(px) / 1024)).concat()
        drawIcon()
        NSGraphicsContext.restoreGraphicsState()
        let name = scale == 1 ? "icon_\(base)x\(base).png" : "icon_\(base)x\(base)@2x.png"
        try! rep.representation(using: .png, properties: [:])!.write(to: out.appendingPathComponent(name))
    }
}
