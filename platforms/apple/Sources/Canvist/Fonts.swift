import CoreText
import UIKit

/// The fonts layout uses, by the id it gave each, resolved once from their
/// CSS family lists. The measurer fills it and the painter reads it, so text
/// is drawn in exactly the font it was measured in.
final class FontTable: @unchecked Sendable {
  private let lock = NSLock()
  private var fonts: [UInt32: CTFont] = [:]

  /// Resolve and keep `face`, replacing an earlier font with its id.
  func register(_ face: FontFace) {
    let font = Self.resolve(face)
    lock.withLock { fonts[face.id] = font }
  }

  /// The font with `id`, once registered.
  func font(_ id: UInt32) -> CTFont? {
    lock.withLock { fonts[id] }
  }

  /// The first family in `face.families` the system has, at its size and
  /// weight, in italic when asked; the system font when none match.
  static func resolve(_ face: FontFace) -> CTFont {
    let size = CGFloat(face.size)
    let weight = UIFont.Weight(css: face.weight)
    let font =
      face.families.lazy
      .compactMap { family(named: $0, size: size, weight: weight) }
      .first ?? UIFont.systemFont(ofSize: size, weight: weight)

    return face.italic ? italic(font) : font as CTFont
  }

  /// `name` as a font, mapping CSS generic families to the system's.
  private static func family(named name: String, size: CGFloat, weight: UIFont.Weight) -> UIFont? {
    switch name.lowercased() {
    case "system-ui", "ui-sans-serif", "sans-serif", "-apple-system", "blinkmacsystemfont":
      return .systemFont(ofSize: size, weight: weight)
    case "ui-monospace", "monospace":
      return .monospacedSystemFont(ofSize: size, weight: weight)
    case "ui-serif", "serif":
      return designed(.serif, size: size, weight: weight)
    case "ui-rounded":
      return designed(.rounded, size: size, weight: weight)
    default:
      let descriptor = UIFontDescriptor(fontAttributes: [
        .family: name,
        .traits: [UIFontDescriptor.TraitKey.weight: weight],
      ])
      let installed = !descriptor.matchingFontDescriptors(withMandatoryKeys: [.family]).isEmpty
      return installed ? UIFont(descriptor: descriptor, size: size) : nil
    }
  }

  private static func designed(
    _ design: UIFontDescriptor.SystemDesign,
    size: CGFloat,
    weight: UIFont.Weight
  ) -> UIFont? {
    let system = UIFont.systemFont(ofSize: size, weight: weight)

    return system.fontDescriptor.withDesign(design).map { UIFont(descriptor: $0, size: size) }
  }

  /// The italic face of `font`, or an oblique one slanted as browsers
  /// synthesize it when the family has no italic.
  private static func italic(_ font: UIFont) -> CTFont {
    if let descriptor = font.fontDescriptor.withSymbolicTraits(
      font.fontDescriptor.symbolicTraits.union(.traitItalic)
    ) {
      return UIFont(descriptor: descriptor, size: font.pointSize) as CTFont
    }

    var slant = CGAffineTransform(a: 1, b: 0, c: tan(obliqueAngle), d: 1, tx: 0, ty: 0)

    return CTFontCreateCopyWithAttributes(font as CTFont, font.pointSize, &slant, nil)
  }

  /// The slant of synthesized oblique text, as CSS specifies it.
  private static let obliqueAngle = 14 * CGFloat.pi / 180
}

extension UIFont.Weight {
  /// The weight CSS writes as `css`, from 100 (thin) to 900 (black),
  /// rounded to the nearest hundred.
  init(css: UInt16) {
    let hundreds = (Int(css) + Self.cssStep / 2) / Self.cssStep
    let index = min(max(hundreds, 1), Self.byHundreds.count) - 1
    self = Self.byHundreds[index]
  }

  private static let cssStep = 100

  /// UIKit's weights for CSS 100 through 900.
  private static let byHundreds: [UIFont.Weight] = [
    .ultraLight, .thin, .light, .regular, .medium, .semibold, .bold, .heavy, .black,
  ]
}
