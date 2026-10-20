import CoreText
import UIKit

/// Paints display items with Core Graphics, in the fonts the measurer
/// resolved, so every glyph lands where layout measured it.
@MainActor
final class DisplayPainter {
    private let fonts: FontTable
    /// Typeset lines by what they draw, so scrolling repaints without
    /// shaping text again.
    private var lines: [LineKey: CTLine] = [:]

    /// Colours for the parts of the page the theme does not colour.
    var checkboxBorder = UIColor.secondaryLabel
    var checkboxFill = UIColor.tintColor
    var slotFill = UIColor.tertiarySystemFill

    init(fonts: FontTable) {
        self.fonts = fonts
    }

    /// Fill `clip` with `page`, then draw every item that touches it, with
    /// document coordinates shifted by `origin`.
    func paint(_ items: [DisplayItem], in context: CGContext, origin: CGPoint, clip: CGRect, page: UIColor) {
        context.setFillColor(page.cgColor)
        context.fill(clip)

        for item in items where Self.bounds(of: item).offsetBy(dx: origin.x, dy: origin.y).intersects(clip) {
            draw(item, in: context, origin: origin)
        }

        if lines.count > Self.cachedLines {
            lines.removeAll(keepingCapacity: true)
        }
    }

    /// Forget typeset lines, as when fonts change.
    func forgetLines() {
        lines.removeAll()
    }

    private func draw(_ item: DisplayItem, in context: CGContext, origin: CGPoint) {
        switch item {
        case .fill(let bounds, let color, let radius, _):
            fill(Self.rect(bounds).offsetBy(dx: origin.x, dy: origin.y), color: UIColor(color), radius: radius)
        case .text(let text, let font, let x, let baseline, _, let color, let letterSpacing, _):
            let key = LineKey(text: text, font: font, color: color, letterSpacing: letterSpacing)
            drawText(key, at: CGPoint(x: CGFloat(x) + origin.x, y: CGFloat(baseline) + origin.y), in: context)
        case .line(let kind, let x, let y, let width, let thickness, let color):
            let start = CGPoint(x: CGFloat(x) + origin.x, y: CGFloat(y) + origin.y)
            drawLine(kind, from: start, width: CGFloat(width), thickness: CGFloat(thickness), color: UIColor(color))
        case .checkbox(let bounds, let checked):
            drawCheckbox(Self.rect(bounds).offsetBy(dx: origin.x, dy: origin.y), checked: checked)
        case .slot(let bounds, _):
            fill(Self.rect(bounds).offsetBy(dx: origin.x, dy: origin.y), color: slotFill, radius: Self.slotRadius)
        }
    }

    private func fill(_ rect: CGRect, color: UIColor, radius: Float) {
        color.setFill()

        if radius > 0 {
            UIBezierPath(roundedRect: rect, cornerRadius: CGFloat(radius)).fill()
        } else {
            UIRectFill(rect)
        }
    }

    private func drawText(_ key: LineKey, at point: CGPoint, in context: CGContext) {
        guard let line = line(for: key) else { return }

        context.saveGState()
        context.textMatrix = .identity
        context.translateBy(x: point.x, y: point.y)
        context.scaleBy(x: 1, y: -1)
        CTLineDraw(line, context)
        context.restoreGState()
    }

    private func line(for key: LineKey) -> CTLine? {
        if let line = lines[key] {
            return line
        }

        guard let font = fonts.font(key.font) else { return nil }

        var attributes: [NSAttributedString.Key: Any] = [
            kCTFontAttributeName as NSAttributedString.Key: font,
            kCTForegroundColorAttributeName as NSAttributedString.Key: UIColor(key.color).cgColor,
        ]

        if key.letterSpacing != 0 {
            attributes[kCTKernAttributeName as NSAttributedString.Key] = CGFloat(key.letterSpacing)
        }

        let line = CTLineCreateWithAttributedString(NSAttributedString(string: key.text, attributes: attributes))
        lines[key] = line

        return line
    }

    private func drawLine(_ kind: LineKind, from start: CGPoint, width: CGFloat, thickness: CGFloat, color: UIColor) {
        guard kind == .squiggle else {
            color.setFill()
            UIRectFill(CGRect(x: start.x, y: start.y - thickness / 2, width: width, height: thickness))

            return
        }

        let amplitude = thickness * Self.squiggleAmplitude
        let wavelength = amplitude * Self.squiggleWavelength
        let path = UIBezierPath()
        path.move(to: start)
        var x = start.x
        var up = true

        while x < start.x + width {
            let next = min(x + wavelength / 2, start.x + width)
            let control = CGPoint(x: (x + next) / 2, y: start.y + (up ? -amplitude : amplitude) * 2)
            path.addQuadCurve(to: CGPoint(x: next, y: start.y), controlPoint: control)
            x = next
            up.toggle()
        }

        color.setStroke()
        path.lineWidth = thickness
        path.stroke()
    }

    private func drawCheckbox(_ rect: CGRect, checked: Bool) {
        let box = UIBezierPath(roundedRect: rect.insetBy(dx: 0.75, dy: 0.75), cornerRadius: Self.checkboxRadius)

        guard checked else {
            checkboxBorder.setStroke()
            box.lineWidth = Self.checkboxBorderWidth
            box.stroke()

            return
        }

        checkboxFill.setFill()
        box.fill()

        let tick = UIBezierPath()
        tick.move(to: CGPoint(x: rect.minX + rect.width * 0.25, y: rect.midY))
        tick.addLine(to: CGPoint(x: rect.minX + rect.width * 0.43, y: rect.maxY - rect.height * 0.28))
        tick.addLine(to: CGPoint(x: rect.maxX - rect.width * 0.22, y: rect.minY + rect.height * 0.28))
        UIColor.white.setStroke()
        tick.lineWidth = Self.checkboxBorderWidth
        tick.lineCapStyle = .round
        tick.lineJoinStyle = .round
        tick.stroke()
    }

    /// The area `item` paints, in document coordinates, grown for glyphs that
    /// reach outside their line box.
    static func bounds(of item: DisplayItem) -> CGRect {
        switch item {
        case .fill(let bounds, _, _, _), .checkbox(let bounds, _), .slot(let bounds, _):
            return rect(bounds)
        case .text(_, _, let x, let baseline, let width, _, _, _):
            // Text runs carry their baseline, not their line box; a generous
            // band around it covers ascenders, descenders, and accents.
            let reach = CGFloat(width) + Self.glyphOverhang * 2
            return CGRect(x: CGFloat(x) - Self.glyphOverhang, y: CGFloat(baseline) - 48, width: reach, height: 64)
        case .line(_, let x, let y, let width, let thickness, _):
            return CGRect(x: CGFloat(x), y: CGFloat(y - thickness * 2), width: CGFloat(width), height: CGFloat(thickness * 4))
        }
    }

    static func rect(_ bounds: Bounds) -> CGRect {
        CGRect(x: CGFloat(bounds.x), y: CGFloat(bounds.y), width: CGFloat(bounds.width), height: CGFloat(bounds.height))
    }

    private static let cachedLines = 2_048
    private static let slotRadius: Float = 6
    private static let checkboxRadius: CGFloat = 3
    private static let checkboxBorderWidth: CGFloat = 1.5
    private static let squiggleAmplitude: CGFloat = 1.5
    private static let squiggleWavelength: CGFloat = 4
    private static let glyphOverhang: CGFloat = 8
}

/// What a typeset line draws.
private struct LineKey: Hashable {
    let text: String
    let font: UInt32
    let color: Rgba
    let letterSpacing: Float
}

extension UIColor {
    /// A colour from the display list.
    convenience init(_ color: Rgba) {
        self.init(
            red: CGFloat(color.r) / 255,
            green: CGFloat(color.g) / 255,
            blue: CGFloat(color.b) / 255,
            alpha: CGFloat(color.a) / 255
        )
    }
}
