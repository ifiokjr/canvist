---
"canvist_editor": minor
---

# Add structural editing commands to `canvist_editor`

- Enter splits text blocks (a heading continues as a paragraph at its end), and Backspace and Delete join blocks across containers, adopting the rest of a joined list item or quote.
- Block types, block attributes, wrapping and lifting (such as blockquotes), and atomic blocks such as horizontal rules.
- Lists generic over list and item types: split, indent, outdent, join with the previous item, wrap, and toggle, converting between list types in place while keeping block ids.
- `Slice` for copy, cut, and paste: copying keeps partly selected containers around their children, and pasting merges open text at both ends and converts or flattens blocks the destination does not accept.
- Property tests that random command sequences keep every invariant and that every update's inverse restores the document exactly.
