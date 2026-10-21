import CoreText
import Foundation

/// Measures text for the editor with CoreText, the engine UIKit draws text
/// with, so lines wrap where UIKit would wrap them and glyphs land where
/// they were measured.
///
/// ``CanvistView`` makes one per editor. Hosts that drive an ``Editor``
/// without the view (tests, React Native, Lynx) can give it one too.
public final class CoreTextMeasurer: PlatformMeasurer, @unchecked Sendable {
    let fonts = FontTable()

    public init() {}

    public func measure(request: MeasureRequest) -> MeasureResponse {
        for face in request.fonts {
            fonts.register(face)
        }

        let metrics = request.metrics.map { id in Self.metrics(of: fonts.font(id)) }

        var advances: [Float] = []
        advances.reserveCapacity(request.segments.reduce(0) { $0 + $1.text.unicodeScalars.count })

        for segment in request.segments {
            Self.advances(of: segment.text, in: fonts.font(segment.font), into: &advances)
        }

        return MeasureResponse(metrics: metrics, advances: advances)
    }

    /// The extent of `font` around the baseline.
    static func metrics(of font: CTFont?) -> VerticalMetrics {
        guard let font else { return VerticalMetrics(ascent: 0, descent: 0) }

        return VerticalMetrics(ascent: Float(CTFontGetAscent(font)), descent: Float(CTFontGetDescent(font)))
    }

    /// Append the advance of each Unicode scalar of `text`: the distance
    /// between the carets CoreText places before and after it. Inside a
    /// ligature CoreText splits the ligature's width between its
    /// characters, so the advances always add up to the line's width.
    static func advances(of text: String, in font: CTFont?, into out: inout [Float]) {
        guard let font else {
            out.append(contentsOf: repeatElement(0, count: text.unicodeScalars.count))

            return
        }

        let line = CTLineCreateWithAttributedString(
            NSAttributedString(string: text, attributes: [kCTFontAttributeName as NSAttributedString.Key: font])
        )
        var index = 0
        var previous = CTLineGetOffsetForStringIndex(line, 0, nil)

        for scalar in text.unicodeScalars {
            index += scalar.utf16.count
            let offset = CTLineGetOffsetForStringIndex(line, index, nil)
            // A right-to-left segment's carets move leftwards.
            out.append(Float(abs(offset - previous)))
            previous = offset
        }
    }

    /// The font with `id`, for painting.
    func font(_ id: UInt32) -> CTFont? {
        fonts.font(id)
    }
}
