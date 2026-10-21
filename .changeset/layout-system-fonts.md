---
"canvist_layout": feat
---

# Measure with system fonts, and resync a layout without its commits

- `ParleyMeasurer::with_system_fonts` reads the fonts installed on the machine, for desktop hosts. Registered fonts add families without standing in for the generic ones.
- `DocumentLayout::resync` brings a layout up to date with a document it missed commits for, keeping measurements and the layouts of unchanged blocks.
